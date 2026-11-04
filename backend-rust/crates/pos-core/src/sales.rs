use crate::error::{DomainError, DomainResult};
use crate::ids::{
    BranchId, CashRegisterId, CashShiftId, OrganizationId, SaleId, SaleItemId, SalePaymentId,
    UserId,
};
use crate::money::Money;
use crate::quantity::Quantity;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaleStatus {
    Completed,
}

impl SaleStatus {
    pub fn as_str(self) -> &'static str {
        "completed"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sale {
    pub id: SaleId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub register_id: CashRegisterId,
    pub shift_id: CashShiftId,
    pub operator_id: UserId,
    pub folio: String,
    pub occurred_at: OffsetDateTime,
    pub status: SaleStatus,
    pub subtotal: Money,
    pub tax: Money,
    pub total: Money,
    pub idempotency_key: String,
    pub request_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleItem {
    pub id: SaleItemId,
    pub sale_id: SaleId,
    pub product_id: crate::ids::ProductId,
    pub variant_id: Option<crate::ids::ProductVariantId>,
    pub product_name: String,
    pub variant_name: Option<String>,
    pub quantity: Quantity,
    pub unit_price: Money,
    pub unit_tax: Money,
    pub line_total: Money,
    pub modifiers_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalePayment {
    pub id: SalePaymentId,
    pub sale_id: SaleId,
    pub method: String,
    pub amount: Money,
    pub change_given: Money,
    pub occurred_at: OffsetDateTime,
}

impl SaleItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sale_id: SaleId,
        product_id: crate::ids::ProductId,
        variant_id: Option<crate::ids::ProductVariantId>,
        product_name: String,
        variant_name: Option<String>,
        quantity: Quantity,
        unit_price: Money,
        unit_tax: Money,
        line_total: Money,
        modifiers_summary: String,
    ) -> Self {
        Self {
            id: SaleItemId::new(),
            sale_id,
            product_id,
            variant_id,
            product_name,
            variant_name,
            quantity,
            unit_price,
            unit_tax,
            line_total,
            modifiers_summary,
        }
    }
}

pub fn validate_payment(total: Money, received: Money) -> DomainResult<Money> {
    if received < total {
        return Err(DomainError::InsufficientPayment);
    }
    received.checked_sub(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calcula_cambio() {
        let total = Money::from_minor_units(1000).unwrap();
        let received = Money::from_minor_units(1500).unwrap();
        assert_eq!(
            validate_payment(total, received).unwrap().minor_units(),
            500
        );
    }
}
