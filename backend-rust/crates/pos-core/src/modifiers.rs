use crate::catalog::{Product, ProductVariant};
use crate::error::{DomainError, DomainResult};
use crate::ids::{ModifierGroupId, ModifierOptionId, OrganizationId, ProductId, ProductVariantId};
use crate::money::Money;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Tipo de opción de modificador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    Choice,
    Extra,
    Omission,
}

impl OptionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::Extra => "extra",
            Self::Omission => "omission",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "choice" => Some(Self::Choice),
            "extra" => Some(Self::Extra),
            "omission" => Some(Self::Omission),
            _ => None,
        }
    }
}

/// Grupo de modificadores asociado a un producto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifierGroup {
    pub id: ModifierGroupId,
    pub organization_id: OrganizationId,
    pub product_id: ProductId,
    pub name: String,
    pub min: u32,
    pub max: u32,
    pub sort_order: i32,
    pub active: bool,
    pub created_at: time::OffsetDateTime,
}

impl ModifierGroup {
    pub fn new(
        organization_id: OrganizationId,
        product_id: ProductId,
        name: impl Into<String>,
        min: u32,
        max: u32,
        sort_order: i32,
    ) -> DomainResult<Self> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        if min > max {
            return Err(DomainError::InvalidModifierLimits);
        }
        if max == 0 && min != 0 {
            return Err(DomainError::InvalidModifierLimits);
        }
        Ok(Self {
            id: ModifierGroupId::new(),
            organization_id,
            product_id,
            name: name.to_owned(),
            min,
            max,
            sort_order,
            active: true,
            created_at: time::OffsetDateTime::now_utc(),
        })
    }
}

/// Opción de un grupo de modificadores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifierOption {
    pub id: ModifierOptionId,
    pub organization_id: OrganizationId,
    pub group_id: ModifierGroupId,
    pub name: String,
    pub kind: OptionKind,
    pub price_delta: Money,
    pub sort_order: i32,
    pub active: bool,
    pub created_at: time::OffsetDateTime,
}

impl ModifierOption {
    pub fn new(
        organization_id: OrganizationId,
        group_id: ModifierGroupId,
        name: impl Into<String>,
        kind: OptionKind,
        price_delta: Money,
        sort_order: i32,
    ) -> DomainResult<Self> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        if matches!(kind, OptionKind::Omission) && price_delta != Money::ZERO {
            return Err(DomainError::InvalidPriceDelta);
        }
        Ok(Self {
            id: ModifierOptionId::new(),
            organization_id,
            group_id,
            name: name.to_owned(),
            kind,
            price_delta,
            sort_order,
            active: true,
            created_at: time::OffsetDateTime::now_utc(),
        })
    }
}

/// Selección individual de una opción.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedOption {
    pub id: ModifierOptionId,
    pub group_id: ModifierGroupId,
    pub name: String,
    pub kind: OptionKind,
    pub price_delta: Money,
}

/// Resultado validado de una configuración.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifierConfiguration {
    pub product_id: ProductId,
    pub product_name: String,
    pub variant_id: Option<ProductVariantId>,
    pub variant_name: Option<String>,
    pub selected_options: Vec<SelectedOption>,
    pub base_price_minor: i64,
    pub modifiers_total_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub tax_rate_bp: u16,
}

impl ModifierConfiguration {
    pub fn unit_price(&self) -> Money {
        Money::from_minor_units(self.total_minor).unwrap_or(Money::ZERO)
    }
}

/// Valida una selección y calcula el precio configurado.
pub fn configure(
    product: &Product,
    variant: Option<&ProductVariant>,
    groups: &[ModifierGroup],
    options: &[ModifierOption],
    selected: &[ModifierOptionId],
) -> DomainResult<ModifierConfiguration> {
    if !product.active {
        return Err(DomainError::InactiveProduct);
    }
    let base_price = match variant {
        Some(variant) => {
            if !variant.active {
                return Err(DomainError::InactiveProduct);
            }
            if variant.product_id != product.id
                || variant.organization_id != product.organization_id
            {
                return Err(DomainError::AccessDenied {
                    required: "catalog.manage",
                });
            }
            variant.unit_price
        }
        None => product.unit_price,
    };

    let mut selected_options = Vec::new();
    let mut unique: BTreeSet<ModifierOptionId> = BTreeSet::new();
    for option_id in selected {
        if !unique.insert(*option_id) {
            return Err(DomainError::DuplicateOption);
        }
        let option = options
            .iter()
            .find(|option| option.id == *option_id)
            .ok_or(DomainError::NotFound)?;
        if !option.active {
            return Err(DomainError::InactiveOption);
        }
        let group = groups
            .iter()
            .find(|group| group.id == option.group_id)
            .ok_or(DomainError::NotFound)?;
        if !group.active {
            return Err(DomainError::InactiveProduct);
        }
        if group.product_id != product.id || option.organization_id != product.organization_id {
            return Err(DomainError::AccessDenied {
                required: "catalog.manage",
            });
        }
        selected_options.push(SelectedOption {
            id: option.id,
            group_id: option.group_id,
            name: option.name.clone(),
            kind: option.kind,
            price_delta: option.price_delta,
        });
    }

    for group in groups {
        if !group.active {
            continue;
        }
        let count = selected_options
            .iter()
            .filter(|option| option.group_id == group.id)
            .count() as u32;
        if count < group.min || count > group.max {
            return Err(DomainError::ModifierRuleViolation);
        }
    }

    let mut modifiers_total = Money::ZERO;
    for option in &selected_options {
        modifiers_total = modifiers_total.checked_add(option.price_delta)?;
    }

    let tax_rate_bp = product.tax_rate_bp;
    // Política única de impuestos (`crate::pricing`): precios netos, impuesto
    // sumado por separado con redondeo half-up a centavos por unidad. Variantes
    // y modificadores usan la tasa del producto padre (ADR-003 y ADR-006).
    let tax = crate::pricing::unit_tax(base_price, tax_rate_bp)?
        .checked_add(crate::pricing::unit_tax(modifiers_total, tax_rate_bp)?)?;
    let total = base_price.checked_add(modifiers_total)?.checked_add(tax)?;

    Ok(ModifierConfiguration {
        product_id: product.id,
        product_name: product.name.clone(),
        variant_id: variant.map(|value| value.id),
        variant_name: variant.map(|value| value.name.clone()),
        selected_options,
        base_price_minor: base_price.minor_units(),
        modifiers_total_minor: modifiers_total.minor_units(),
        tax_minor: tax.minor_units(),
        total_minor: total.minor_units(),
        tax_rate_bp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OrganizationId, ProductId};
    use crate::money::Money;

    #[test]
    fn rechaza_min_mayor_que_max() {
        let organization_id = OrganizationId::new();
        let product_id = ProductId::new();
        let result = ModifierGroup::new(organization_id, product_id, "X", 2, 1, 0);
        assert!(result.is_err());
    }

    #[test]
    fn calcula_precio_base_mas_extra() {
        let organization_id = OrganizationId::new();
        let product = Product::new(
            ProductId::new(),
            organization_id,
            "LIC-001",
            "Licuado de fresa",
            Money::from_minor_units(3000).unwrap(),
            1600,
        )
        .unwrap();
        let group = ModifierGroup::new(organization_id, product.id, "Extras", 0, 2, 0).unwrap();
        let option = ModifierOption::new(
            organization_id,
            group.id,
            "Proteína",
            OptionKind::Extra,
            Money::from_minor_units(500).unwrap(),
            0,
        )
        .unwrap();
        let options = [option];
        let result = configure(&product, None, &[group], &options, &[options[0].id]).unwrap();
        assert_eq!(result.base_price_minor, 3000);
        assert_eq!(result.modifiers_total_minor, 500);
    }
}
