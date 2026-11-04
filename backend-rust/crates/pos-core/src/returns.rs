use crate::error::{DomainError, DomainResult};
use crate::ids::{
    BranchId, CashMovementId, CashShiftId, InventoryMovementId, OrganizationId, SaleId, SaleItemId,
    SaleReturnId, SaleReturnLineId, UserId,
};
use crate::money::Money;
use crate::quantity::Quantity;
use crate::sales::SaleItem;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Destino físico de la mercancía devuelta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnDisposition {
    /// Vuelve al inventario mediante un `inbound`.
    Restock,
    /// No regresa a existencias; se conserva el motivo y la decisión.
    Discard,
}

impl ReturnDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Restock => "restock",
            Self::Discard => "discard",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "restock" => Some(Self::Restock),
            "discard" => Some(Self::Discard),
            _ => None,
        }
    }
}

/// Devolución de una venta. La venta original nunca se modifica: el estado
/// efectivo se deriva de las devoluciones asociadas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleReturn {
    pub id: SaleReturnId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub sale_id: SaleId,
    /// Turno abierto desde el que se entrega el efectivo del reembolso.
    pub shift_id: CashShiftId,
    pub cash_movement_id: CashMovementId,
    pub operator_id: UserId,
    pub reason: String,
    pub refund_total: Money,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleReturnLine {
    pub id: SaleReturnLineId,
    pub return_id: SaleReturnId,
    pub sale_item_id: SaleItemId,
    pub quantity: Quantity,
    pub disposition: ReturnDisposition,
    pub refund_net: Money,
    pub refund_tax: Money,
    pub refund_total: Money,
    pub inbound_movement_id: Option<InventoryMovementId>,
}

/// Importe reembolsable de `quantity` unidades según los snapshots de la venta.
///
/// Los precios unitarios ya están redondeados en la venta original, de modo que
/// multiplicar por unidades enteras es exacto: la suma de devoluciones parciales
/// nunca excede la línea y una devolución completa cuadra al centavo.
pub fn refund_for_quantity(
    item: &SaleItem,
    quantity: Quantity,
) -> DomainResult<(Money, Money, Money)> {
    if quantity.is_zero() {
        return Err(DomainError::NegativeQuantity);
    }
    if quantity.milli_units() > item.quantity.milli_units() {
        return Err(DomainError::NegativeQuantity);
    }
    let net = item.unit_price.multiply_by(quantity)?;
    let tax = item.unit_tax.multiply_by(quantity)?;
    Ok((net, tax, net.checked_add(tax)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ProductId, SaleItemId};

    fn item(quantity: &str, unit_price: i64, unit_tax: i64) -> SaleItem {
        SaleItem {
            id: SaleItemId::new(),
            sale_id: SaleId::new(),
            product_id: ProductId::new(),
            variant_id: None,
            product_name: "Café".into(),
            variant_name: None,
            quantity: quantity.parse().unwrap(),
            unit_price: Money::from_minor_units(unit_price).unwrap(),
            unit_tax: Money::from_minor_units(unit_tax).unwrap(),
            line_total: Money::from_minor_units(0).unwrap(),
            modifiers_summary: String::new(),
        }
    }

    #[test]
    fn devoluciones_parciales_suman_exactamente_la_linea() {
        let item = item("3", 1400, 224);
        let (net_a, tax_a, total_a) = refund_for_quantity(&item, "1".parse().unwrap()).unwrap();
        let (_net_b, _tax_b, total_b) = refund_for_quantity(&item, "2".parse().unwrap()).unwrap();
        assert_eq!((net_a.minor_units(), tax_a.minor_units()), (1400, 224));
        assert_eq!(total_a.minor_units(), 1624);
        assert_eq!(total_b.minor_units(), 3248);
        assert_eq!(
            total_a.minor_units() + total_b.minor_units(),
            item.unit_price.minor_units() * 3 + item.unit_tax.minor_units() * 3
        );
    }

    #[test]
    fn no_permite_devolver_mas_de_lo_vendido_en_la_linea() {
        let item = item("2", 1000, 0);
        assert!(refund_for_quantity(&item, "3".parse().unwrap()).is_err());
    }
}
