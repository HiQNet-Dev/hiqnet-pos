use crate::error::{DomainError, DomainResult};
use crate::ids::ProductId;
use crate::ids::{
    BranchId, CashMovementId, CashShiftId, ExpenseCategoryId, ExpenseId, OrganizationId, UserId,
    WasteId,
};
use crate::money::Money;
use crate::quantity::Quantity;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseCategory {
    pub id: ExpenseCategoryId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub active: bool,
    pub created_at: OffsetDateTime,
}

impl ExpenseCategory {
    pub fn new(organization_id: OrganizationId, name: impl Into<String>) -> DomainResult<Self> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        Ok(Self {
            id: ExpenseCategoryId::new(),
            organization_id,
            name,
            active: true,
            created_at: OffsetDateTime::now_utc(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expense {
    pub id: ExpenseId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub category_id: ExpenseCategoryId,
    pub amount: Money,
    pub concept: String,
    pub cash_shift_id: Option<CashShiftId>,
    pub cash_movement_id: Option<CashMovementId>,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub created_by: UserId,
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasteRecord {
    pub id: WasteId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub product_id: ProductId,
    pub quantity: Quantity,
    pub reason: String,
    pub inventory_movement_id: crate::ids::InventoryMovementId,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub created_by: UserId,
    pub occurred_at: OffsetDateTime,
}
