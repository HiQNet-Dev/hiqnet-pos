//! DTO de ventas.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleItemDraft {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub quantity: String,
    #[serde(default)]
    pub selected_option_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutDraft {
    pub shift_id: String,
    pub idempotency_key: String,
    pub items: Vec<SaleItemDraft>,
    pub cash_received: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleItemView {
    pub id: String,
    pub product_id: String,
    pub product_name: String,
    pub variant_name: Option<String>,
    pub quantity: String,
    pub unit_price: String,
    pub unit_tax: String,
    pub line_total: String,
    pub modifiers_summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SalePaymentView {
    pub method: String,
    pub amount: String,
    pub change: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleView {
    pub id: String,
    pub folio: String,
    pub status: String,
    pub subtotal: String,
    pub tax: String,
    pub total: String,
    pub items: Vec<SaleItemView>,
    pub payment: Option<SalePaymentView>,
}
