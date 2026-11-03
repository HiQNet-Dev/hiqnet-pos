use crate::error::{DomainError, DomainResult};
use crate::ids::{OrganizationId, ProductId};
use crate::money::Money;
use serde::{Deserialize, Serialize};

/// Producto de catálogo. El precio es un importe exacto con impuesto expresado
/// en puntos base (16 % = 1600).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: ProductId,
    pub organization_id: OrganizationId,
    pub sku: String,
    pub name: String,
    pub unit_price: Money,
    pub tax_rate_bp: u16,
    pub active: bool,
    pub created_at: time::OffsetDateTime,
}

impl Product {
    pub fn new(
        id: ProductId,
        organization_id: OrganizationId,
        sku: impl Into<String>,
        name: impl Into<String>,
        unit_price: Money,
        tax_rate_bp: u16,
    ) -> DomainResult<Self> {
        let sku = normalize_sku(&sku.into())?;
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        if tax_rate_bp > 10_000 {
            return Err(DomainError::InvalidTaxRate);
        }
        Ok(Self {
            id,
            organization_id,
            sku,
            name: name.to_owned(),
            unit_price,
            tax_rate_bp,
            active: true,
            created_at: time::OffsetDateTime::now_utc(),
        })
    }

    /// Impuesto contenido en el precio de una unidad.
    pub fn unit_tax(&self) -> DomainResult<Money> {
        let base = i128::from(self.unit_price.minor_units()) * i128::from(self.tax_rate_bp);
        let tax = div_round_half_up(base, 10_000)?;
        Money::from_minor_units(i64::try_from(tax).map_err(|_| DomainError::AmountOverflow)?)
    }
}

fn div_round_half_up(numerator: i128, denominator: i128) -> DomainResult<i128> {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder.abs() * 2 >= denominator {
        Ok(quotient + numerator.signum())
    } else {
        Ok(quotient)
    }
}

pub fn normalize_sku(value: &str) -> DomainResult<String> {
    let sku = value.trim().to_ascii_uppercase();
    if sku.is_empty() || sku.len() > 32 {
        return Err(DomainError::InvalidSku(sku));
    }
    let valid = sku
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !valid {
        return Err(DomainError::InvalidSku(sku));
    }
    Ok(sku)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(price: i64, tax: u16) -> Product {
        Product::new(
            ProductId::new(),
            OrganizationId::new(),
            " sku-1 ",
            "Café americano",
            Money::from_minor_units(price).unwrap(),
            tax,
        )
        .unwrap()
    }

    #[test]
    fn normaliza_sku() {
        assert_eq!(product(1000, 1600).sku, "SKU-1");
        assert!(
            Product::new(
                ProductId::new(),
                OrganizationId::new(),
                "sku con espacio",
                "Producto",
                Money::ZERO,
                0
            )
            .is_err()
        );
    }

    #[test]
    fn calcula_impuesto_redondeado() {
        assert_eq!(product(10000, 1600).unit_tax().unwrap().minor_units(), 1600);
        assert_eq!(product(1050, 1600).unit_tax().unwrap().minor_units(), 168);
    }

    #[test]
    fn rechaza_impuesto_invalido() {
        assert_eq!(
            Product::new(
                ProductId::new(),
                OrganizationId::new(),
                "SKU-2",
                "Producto",
                Money::ZERO,
                10_001
            )
            .unwrap_err(),
            DomainError::InvalidTaxRate
        );
    }
}
