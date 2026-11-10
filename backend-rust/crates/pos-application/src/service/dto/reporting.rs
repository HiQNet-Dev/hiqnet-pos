//! DTO de reportes y estado comercial.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct ReportsQuery {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsProductView {
    pub product_id: String,
    pub product_name: String,
    pub quantity: String,
    pub net: String,
    pub tax: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsSalesView {
    pub count: i64,
    pub subtotal: String,
    pub tax: String,
    pub total: String,
    pub returned_count: i64,
    pub returned_total: String,
    pub net_total: String,
    pub products: Vec<ReportsProductView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsCashView {
    pub shifts_opened: i64,
    pub shifts_closed: i64,
    pub entries: String,
    pub exits: String,
    pub expenses_from_cash: String,
    pub refunds: String,
    pub difference: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsPurchasesView {
    pub orders_created: i64,
    pub pending_orders: i64,
    pub received_quantity: String,
    pub received_cost: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsExpenseCategoryView {
    pub name: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsExpensesView {
    pub total: String,
    pub from_cash: String,
    pub external: String,
    pub categories: Vec<ReportsExpenseCategoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsInventoryView {
    pub stock_quantity: String,
    pub inbound: String,
    pub outbound: String,
    pub waste: String,
    pub adjustment_in: String,
    pub adjustment_out: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsView {
    #[serde(with = "time::serde::rfc3339")]
    pub from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub to: OffsetDateTime,
    pub sales: ReportsSalesView,
    pub cash: ReportsCashView,
    pub purchases: ReportsPurchasesView,
    pub expenses: ReportsExpensesView,
    pub inventory: ReportsInventoryView,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommercialStateView {
    pub plan: &'static str,
    pub status: &'static str,
    #[serde(with = "time::serde::rfc3339")]
    pub valid_until: OffsetDateTime,
    pub features: Vec<&'static str>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub initialized: bool,
    pub product_version: &'static str,
    pub commercial: Option<CommercialStateView>,
    pub user_count: i64,
}
