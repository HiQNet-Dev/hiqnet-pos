//! DTO de catalogo: productos, categorias, variantes y modificadores.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct ProductDraft {
    pub sku: String,
    pub name: String,
    /// Precio como texto decimal ("12.50") para no perder exactitud en JSON.
    pub price: String,
    pub tax_rate_bp: u16,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProductView {
    pub id: ProductId,
    pub sku: String,
    pub name: String,
    /// Precio neto, sin impuesto. Es el importe que se guarda y cobra.
    pub unit_price: String,
    /// Precio final al público de una unidad, con impuesto incluido.
    pub public_unit_total: String,
    pub tax_rate_bp: u16,
    pub active: bool,
    pub category_id: Option<String>,
    /// Variantes activas publicadas con precio (solo en el listado de catálogo).
    pub active_variants: u32,
    /// Precio final al público más bajo entre las variantes activas.
    pub variant_public_from: Option<String>,
    /// Si el producto tiene al menos un grupo de modificadores activo.
    pub has_modifiers: bool,
}

/// Vista previa del precio de un producto o variante todavía no guardado.
///
/// La calcula el dominio con la misma política que la venta: la interfaz nunca
/// multiplica por la tasa. No abre transacción ni escribe nada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProductPricePreviewView {
    pub unit_price: String,
    pub tax: String,
    pub public_unit_total: String,
    pub tax_rate_bp: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryDraft {
    pub name: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryView {
    pub id: String,
    pub name: String,
    pub sort_order: i32,
    pub active: bool,
}

impl From<&Category> for CategoryView {
    fn from(category: &Category) -> Self {
        Self {
            id: category.id.to_string(),
            name: category.name.clone(),
            sort_order: category.sort_order,
            active: category.active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductVariantDraft {
    pub product_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub price: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductVariantView {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub unit_price: String,
    pub active: bool,
    pub sort_order: i32,
}

impl From<&ProductVariant> for ProductVariantView {
    fn from(variant: &ProductVariant) -> Self {
        Self {
            id: variant.id.to_string(),
            product_id: variant.product_id.to_string(),
            name: variant.name.clone(),
            sku: variant.sku.clone(),
            unit_price: variant.unit_price.to_string(),
            active: variant.active,
            sort_order: variant.sort_order,
        }
    }
}

impl ProductView {
    /// Vista de un producto suelto: sin variantes ni modificadores conocidos.
    pub(in crate::service) fn of(product: &Product) -> AppResult<Self> {
        Self::with_configuration(product, &[], false)
    }

    /// Vista con la configuración comercial del producto. `variants` contiene
    /// solo las variantes activas y `has_modifiers` indica si algún grupo activo
    /// obliga a configurar la venta.
    pub(in crate::service) fn with_configuration(
        product: &Product,
        variants: &[&ProductVariant],
        has_modifiers: bool,
    ) -> AppResult<Self> {
        let public_unit_total = product.public_unit_total().map_err(AppError::Domain)?;
        let variant_public_from = variants
            .iter()
            .map(|variant| product.variant_public_unit_total(variant))
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::Domain)?
            .into_iter()
            .min()
            .map(|total| total.to_string());

        Ok(Self {
            id: product.id,
            sku: product.sku.clone(),
            name: product.name.clone(),
            unit_price: product.unit_price.to_string(),
            public_unit_total: public_unit_total.to_string(),
            tax_rate_bp: product.tax_rate_bp,
            active: product.active,
            category_id: product.category_id.map(|id| id.to_string()),
            active_variants: u32::try_from(variants.len()).unwrap_or(u32::MAX),
            variant_public_from,
            has_modifiers,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierGroupDraft {
    pub product_id: String,
    pub name: String,
    pub min: u32,
    pub max: u32,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierGroupView {
    pub id: String,
    pub name: String,
    pub min: u32,
    pub max: u32,
    pub sort_order: i32,
    pub active: bool,
}

impl From<&ModifierGroup> for ModifierGroupView {
    fn from(group: &ModifierGroup) -> Self {
        Self {
            id: group.id.to_string(),
            name: group.name.clone(),
            min: group.min,
            max: group.max,
            sort_order: group.sort_order,
            active: group.active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierOptionDraft {
    pub group_id: String,
    pub name: String,
    pub kind: String,
    pub price_delta: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierOptionView {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub kind: String,
    pub price_delta: String,
    pub sort_order: i32,
    pub active: bool,
}

impl From<&ModifierOption> for ModifierOptionView {
    fn from(option: &ModifierOption) -> Self {
        Self {
            id: option.id.to_string(),
            group_id: option.group_id.to_string(),
            name: option.name.clone(),
            kind: option.kind.as_str().to_owned(),
            price_delta: option.price_delta.to_string(),
            sort_order: option.sort_order,
            active: option.active,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierSelectionView {
    pub selected_option_id: String,
    pub name: String,
    pub kind: String,
    pub price_delta: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierConfigurationView {
    pub product_id: String,
    pub product_name: String,
    pub variant_id: Option<String>,
    pub variant_name: Option<String>,
    pub base_price: String,
    pub modifiers_total: String,
    pub tax: String,
    pub unit_price: String,
    pub tax_rate_bp: u16,
    pub selected_options: Vec<ModifierSelectionView>,
}

impl From<ModifierConfiguration> for ModifierConfigurationView {
    fn from(configuration: ModifierConfiguration) -> Self {
        Self {
            product_id: configuration.product_id.to_string(),
            product_name: configuration.product_name,
            variant_id: configuration.variant_id.map(|id| id.to_string()),
            variant_name: configuration.variant_name,
            base_price: Money::from_minor_units(configuration.base_price_minor)
                .unwrap()
                .to_string(),
            modifiers_total: Money::from_minor_units(configuration.modifiers_total_minor)
                .unwrap()
                .to_string(),
            tax: Money::from_minor_units(configuration.tax_minor)
                .unwrap()
                .to_string(),
            unit_price: Money::from_minor_units(configuration.total_minor)
                .unwrap()
                .to_string(),
            tax_rate_bp: configuration.tax_rate_bp,
            selected_options: configuration
                .selected_options
                .into_iter()
                .map(|option| ModifierSelectionView {
                    selected_option_id: option.id.to_string(),
                    name: option.name,
                    kind: option.kind.as_str().to_owned(),
                    price_delta: option.price_delta.to_string(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierConfigurationDraft {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub selected_option_ids: Vec<String>,
}
