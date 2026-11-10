//! DTO de compras: proveedores y ordenes.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct SupplierDraft {
    pub code: Option<String>,
    pub name: String,
    pub contact: Option<String>,
    pub rfc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupplierView {
    pub id: String,
    pub code: Option<String>,
    pub name: String,
    pub contact: Option<String>,
    pub rfc: Option<String>,
    pub active: bool,
}

impl From<&Supplier> for SupplierView {
    fn from(value: &Supplier) -> Self {
        Self {
            id: value.id.to_string(),
            code: value.code.clone(),
            name: value.name.clone(),
            contact: value.contact.clone(),
            rfc: value.rfc.clone(),
            active: value.active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseOrderLineDraft {
    pub product_id: String,
    pub quantity: String,
    pub unit_cost: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseOrderDraft {
    pub supplier_id: String,
    pub lines: Vec<PurchaseOrderLineDraft>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseReceiptLineDraft {
    pub order_line_id: String,
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseReceiptDraft {
    pub order_id: String,
    pub idempotency_key: String,
    pub lines: Vec<PurchaseReceiptLineDraft>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseOrderLineView {
    pub id: String,
    pub product_id: String,
    pub quantity: String,
    pub received_quantity: String,
    pub unit_cost: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseOrderView {
    pub id: String,
    pub supplier_id: String,
    pub folio: String,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub lines: Vec<PurchaseOrderLineView>,
}
