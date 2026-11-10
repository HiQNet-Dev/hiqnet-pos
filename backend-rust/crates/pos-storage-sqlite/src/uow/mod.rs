use async_trait::async_trait;
use pos_application::{
    AppError, AppResult, CashReportTotals, CatalogConfiguration, ExpenseCategoryReportRow,
    ExpensesReportTotals, InstallationState, InventoryReportTotals, ProductSalesReportRow,
    PurchasesReportTotals, ReturnedTotals, ReturnsReportTotals, SalesReportTotals, Store, StoreTx,
};
use pos_core::audit::{AuditAction, AuditEvent};
use pos_core::cash::{CashMovement, CashRegister, CashShift, CashShiftStatus};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::expenses::{Expense, ExpenseCategory, WasteRecord};
use pos_core::identity::{Role, User};
use pos_core::ids::{
    AuditEventId, BranchId, CategoryId, ExpenseCategoryId, ExpenseId, OrganizationId, ProductId,
    PurchaseOrderId, PurchaseOrderLineId, PurchaseReceiptId, SaleReturnId, SaleReturnLineId,
    SupplierId, UserId, WasteId,
};
use pos_core::inventory::InventoryMovement;
use pos_core::inventory::MovementKind;
use pos_core::modifiers::{ModifierGroup, ModifierOption};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::purchasing::{
    PurchaseOrder, PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, Supplier,
};
use pos_core::quantity::Quantity;
use pos_core::returns::{ReturnDisposition, SaleReturn, SaleReturnLine};
use pos_core::sales::{Sale, SaleItem, SalePayment, SaleStatus};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::str::FromStr;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

mod audit;
mod cash;
mod catalog;
mod expenses;
mod inventory;
mod modifiers;
mod organization;
mod purchasing;
mod reporting;
mod returns;
mod rows;
mod sales;
mod store_tx;
mod users;

use rows::*;

/// Implementación SQLite del puerto `Store`.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn begin(&self) -> AppResult<Box<dyn StoreTx>> {
        let tx = self.pool.begin().await.map_err(map_sqlx)?;
        Ok(Box::new(SqliteTx { tx: Some(tx) }))
    }
}

struct SqliteTx {
    tx: Option<Transaction<'static, Sqlite>>,
}

impl SqliteTx {
    fn tx(&mut self) -> AppResult<&mut Transaction<'static, Sqlite>> {
        self.tx.as_mut().ok_or(AppError::Storage)
    }
}
