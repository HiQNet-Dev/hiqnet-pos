use crate::error::{DomainError, DomainResult};
use crate::ids::{
    BranchId, OrganizationId, ProductId, PurchaseOrderId, PurchaseOrderLineId, PurchaseReceiptId,
    SupplierId, UserId,
};
use crate::money::Money;
use crate::quantity::Quantity;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supplier {
    pub id: SupplierId,
    pub organization_id: OrganizationId,
    pub code: Option<String>,
    pub name: String,
    pub contact: Option<String>,
    pub rfc: Option<String>,
    pub active: bool,
    pub created_at: OffsetDateTime,
}

impl Supplier {
    pub fn new(
        organization_id: OrganizationId,
        code: Option<String>,
        name: String,
        contact: Option<String>,
        rfc: Option<String>,
    ) -> DomainResult<Self> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        Ok(Self {
            id: SupplierId::new(),
            organization_id,
            code: clean_optional(code),
            name,
            contact: clean_optional(contact),
            rfc: clean_optional(rfc),
            active: true,
            created_at: OffsetDateTime::now_utc(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseOrderStatus {
    Draft,
    Ordered,
    PartiallyReceived,
    Received,
}

impl PurchaseOrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ordered => "ordered",
            Self::PartiallyReceived => "partially_received",
            Self::Received => "received",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "ordered" => Some(Self::Ordered),
            "partially_received" => Some(Self::PartiallyReceived),
            "received" => Some(Self::Received),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseOrder {
    pub id: PurchaseOrderId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub supplier_id: SupplierId,
    pub folio: String,
    pub status: PurchaseOrderStatus,
    pub created_by: UserId,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseOrderLine {
    pub id: PurchaseOrderLineId,
    pub order_id: PurchaseOrderId,
    pub product_id: ProductId,
    pub requested_quantity: Quantity,
    pub received_quantity: Quantity,
    pub unit_cost: Money,
}

impl PurchaseOrderLine {
    pub fn new(
        order_id: PurchaseOrderId,
        product_id: ProductId,
        requested_quantity: Quantity,
        unit_cost: Money,
    ) -> DomainResult<Self> {
        if requested_quantity.is_zero() {
            return Err(DomainError::NegativeQuantity);
        }
        Ok(Self {
            id: PurchaseOrderLineId::new(),
            order_id,
            product_id,
            requested_quantity,
            received_quantity: Quantity::ZERO,
            unit_cost,
        })
    }

    pub fn remaining(&self) -> DomainResult<Quantity> {
        let value = self
            .requested_quantity
            .milli_units()
            .checked_sub(self.received_quantity.milli_units())
            .ok_or(DomainError::QuantityOverflow)?;
        Quantity::from_milli_units(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseReceipt {
    pub id: PurchaseReceiptId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub order_id: PurchaseOrderId,
    pub received_by: UserId,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub received_at: OffsetDateTime,
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
}
