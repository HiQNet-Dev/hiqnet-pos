//! DTO de devoluciones.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct SaleReturnLineDraft {
    pub sale_item_id: String,
    pub quantity: String,
    pub disposition: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaleReturnDraft {
    pub sale_id: String,
    /// Turno de reembolso. Puede faltar cuando la devolución no mueve efectivo.
    pub shift_id: Option<String>,
    pub reason: String,
    pub idempotency_key: String,
    pub lines: Vec<SaleReturnLineDraft>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaleReturnPreviewDraft {
    pub sale_id: String,
    pub lines: Vec<SaleReturnLineDraft>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnLineView {
    pub sale_item_id: String,
    pub product_name: String,
    pub quantity: String,
    pub disposition: String,
    pub refund_net: String,
    pub refund_tax: String,
    pub refund_total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnView {
    pub id: String,
    pub sale_id: String,
    pub sale_folio: String,
    pub reason: String,
    pub refund_total: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub lines: Vec<SaleReturnLineView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReturnableSaleItemView {
    pub id: String,
    pub product_name: String,
    pub variant_name: Option<String>,
    pub modifiers_summary: String,
    pub quantity: String,
    pub returned_quantity: String,
    pub returnable_quantity: String,
    pub unit_refund: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReturnableSaleView {
    pub id: String,
    pub folio: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub total: String,
    pub items: Vec<ReturnableSaleItemView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnPreviewLineView {
    pub sale_item_id: String,
    pub quantity: String,
    pub refund_total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnPreviewView {
    pub refund_total: String,
    pub lines: Vec<SaleReturnPreviewLineView>,
}
