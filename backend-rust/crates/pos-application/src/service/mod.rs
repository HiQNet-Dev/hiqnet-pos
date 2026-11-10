//! Casos de uso del POS, organizados por dominio.
//!
//! Cada archivo agrupa los metodos de un dominio y sus DTO. `PosService` se
//! reparte en implementaciones inherentes dentro del mismo crate.

use crate::error::{AppError, AppResult};
use crate::ports::{
    InstallationState, LicenseVerifier, SharedClock, SharedHasher, SharedLicenseVerifier,
    SharedStore, StoreTx, VerifiedLicense,
};
use pos_core::audit::{AuditAction, AuditEvent};
use pos_core::cash::{CashMovement, CashMovementKind, CashRegister, CashShift, CashShiftStatus};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::entitlements::{
    CommercialState, FeatureKey, LicenseStatus, PlanName, local_features,
};
use pos_core::expenses::{Expense, ExpenseCategory, WasteRecord};
use pos_core::identity::{Actor, Role, User, normalize_username};
use pos_core::ids::{
    BranchId, CashRegisterId, CashShiftId, ExpenseCategoryId, ExpenseId, InstallationId,
    OrganizationId, ProductId, PurchaseOrderId, PurchaseOrderLineId, SaleItemId, SupplierId,
    UserId, WasteId,
};
use pos_core::inventory::{InventoryMovement, MovementKind};
use pos_core::modifiers::{ModifierConfiguration, ModifierGroup, ModifierOption, OptionKind};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::purchasing::{
    PurchaseOrder, PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, Supplier,
};
use pos_core::quantity::Quantity;
use pos_core::returns::{
    RefundBasis, ReturnDisposition, SaleReturn, SaleReturnLine, refund_increment,
};
use pos_core::sales::{Sale, SaleItem, SalePayment, SaleStatus, validate_payment};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use time::OffsetDateTime;

pub const MIN_PASSWORD_LEN: usize = 10;
const MAX_AUDIT_PAGE: i64 = 200;

mod audit;
mod cash;
mod catalog;
mod core;
mod dto;
mod expenses;
mod helpers;
mod inventory;
mod licensing;
mod purchasing;
mod reporting;
mod returns;
mod sales;
#[cfg(test)]
mod tests;
mod users;

pub use dto::*;
use helpers::*;

/// Fachada de casos de uso del POS. No conoce SQL, Tauri ni React.
pub struct PosService {
    store: SharedStore,
    hasher: SharedHasher,
    clock: SharedClock,
    /// Ausente por defecto: sin verificador, una licencia firmada se rechaza.
    license: Option<SharedLicenseVerifier>,
}
