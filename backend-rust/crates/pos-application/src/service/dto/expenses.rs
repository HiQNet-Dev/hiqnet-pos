//! DTO de gastos y merma.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct ExpenseCategoryDraft {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpenseDraft {
    pub category_id: String,
    pub amount: String,
    pub concept: String,
    pub paid_from_cash: bool,
    pub shift_id: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WasteDraft {
    pub product_id: String,
    pub quantity: String,
    pub reason: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpenseCategoryView {
    pub id: String,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpenseView {
    pub id: String,
    pub category_id: String,
    pub amount: String,
    pub concept: String,
    pub paid_from_cash: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct WasteView {
    pub id: String,
    pub product_id: String,
    pub quantity: String,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}
