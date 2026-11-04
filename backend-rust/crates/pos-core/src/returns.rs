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
    /// Turno abierto desde el que se entrega el efectivo. Es `None` cuando la
    /// devolución no tiene importe monetario (solo restitución física).
    pub shift_id: Option<CashShiftId>,
    /// Salida de caja única. Es `None` cuando el reembolso es de cero centavos.
    pub cash_movement_id: Option<CashMovementId>,
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

/// Acumulados previos de una partida, necesarios para distribuir el importe
/// original sin sumar redondeos independientes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefundBasis {
    pub returned_quantity: Quantity,
    pub refunded_net: Money,
    pub refunded_tax: Money,
}

impl RefundBasis {
    pub const EMPTY: RefundBasis = RefundBasis {
        returned_quantity: Quantity::ZERO,
        refunded_net: Money::ZERO,
        refunded_tax: Money::ZERO,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRefund {
    pub net: Money,
    pub tax: Money,
    pub total: Money,
}

/// Incremento de reembolso de `quantity` unidades adicionales.
///
/// Política acumulativa: el importe se calcula redondeando (half-up, la misma
/// regla que la venta) el importe correspondiente a la cantidad **acumulada**
/// devuelta y se resta lo ya reembolsado. Así:
///
/// - la suma de incrementos nunca excede el importe original de la línea;
/// - devolver toda la cantidad cuadra exactamente con el importe vendido;
/// - varias devoluciones parciales dan el mismo total que una sola;
/// - el cálculo de vista previa y el transaccional son idénticos.
///
/// Un incremento puede ser de cero centavos (por ejemplo cantidades muy
/// pequeñas de productos baratos); en ese caso se permite la restitución física
/// sin movimiento de caja ficticio.
pub fn refund_increment(
    item: &SaleItem,
    basis: &RefundBasis,
    quantity: Quantity,
) -> DomainResult<LineRefund> {
    if quantity.is_zero() {
        return Err(DomainError::NegativeQuantity);
    }
    let total_quantity = basis.returned_quantity.checked_add(quantity)?;
    if total_quantity.milli_units() > item.quantity.milli_units() {
        return Err(DomainError::ReturnExceedsSale);
    }
    let cumulative_net = item.unit_price.multiply_by(total_quantity)?;
    let cumulative_tax = item.unit_tax.multiply_by(total_quantity)?;
    if basis.refunded_net.minor_units() > cumulative_net.minor_units()
        || basis.refunded_tax.minor_units() > cumulative_tax.minor_units()
    {
        // Los datos previos ya superan el importe vendido: nunca reembolsar más.
        return Err(DomainError::ReturnExceedsSale);
    }
    let net = cumulative_net.checked_sub(basis.refunded_net)?;
    let tax = cumulative_tax.checked_sub(basis.refunded_tax)?;
    Ok(LineRefund {
        net,
        tax,
        total: net.checked_add(tax)?,
    })
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

    fn quantity(value: &str) -> Quantity {
        value.parse().unwrap()
    }

    #[test]
    fn devoluciones_enteras_suman_exactamente_la_linea() {
        let item = item("3", 1400, 224);
        let first = refund_increment(&item, &RefundBasis::EMPTY, quantity("1")).unwrap();
        assert_eq!(first.total.minor_units(), 1624);
        let basis = RefundBasis {
            returned_quantity: quantity("1"),
            refunded_net: first.net,
            refunded_tax: first.tax,
        };
        let second = refund_increment(&item, &basis, quantity("2")).unwrap();
        assert_eq!(second.total.minor_units(), 3248);
        assert_eq!(first.total.minor_units() + second.total.minor_units(), 4872);
    }

    #[test]
    fn no_permite_devolver_mas_de_lo_vendido_en_la_linea() {
        let item = item("2", 1000, 0);
        assert_eq!(
            refund_increment(&item, &RefundBasis::EMPTY, quantity("3")),
            Err(DomainError::ReturnExceedsSale)
        );
    }

    #[test]
    fn importe_acumulado_no_supera_la_linea_con_fracciones() {
        // 3 unidades de $0.01: la línea vale $0.03.
        let item = item("3", 1, 0);
        let mut basis = RefundBasis::EMPTY;
        let mut total = 0_i64;
        for _ in 0..6 {
            let step = refund_increment(&item, &basis, quantity("0.5")).unwrap();
            total += step.total.minor_units();
            basis = RefundBasis {
                returned_quantity: basis
                    .returned_quantity
                    .checked_add(quantity("0.5"))
                    .unwrap(),
                refunded_net: basis.refunded_net.checked_add(step.net).unwrap(),
                refunded_tax: basis.refunded_tax.checked_add(step.tax).unwrap(),
            };
        }
        assert_eq!(total, 3, "el acumulado debe cuadrar con la línea");
    }

    #[test]
    fn incremento_cero_es_valido_y_no_altera_el_acumulado() {
        let item = item("1", 1, 0);
        let first = refund_increment(&item, &RefundBasis::EMPTY, quantity("0.001")).unwrap();
        assert_eq!(first.total.minor_units(), 0);
        let basis = RefundBasis {
            returned_quantity: quantity("0.001"),
            refunded_net: first.net,
            refunded_tax: first.tax,
        };
        let rest = refund_increment(&item, &basis, quantity("0.999")).unwrap();
        assert_eq!(rest.total.minor_units(), 1);
        assert_eq!(first.total.minor_units() + rest.total.minor_units(), 1);
    }

    #[test]
    fn devoluciones_parciales_no_cambian_el_total_final() {
        let item = item("0.5", 3, 0); // línea = round(0.015) = 0.02
        let first = refund_increment(&item, &RefundBasis::EMPTY, quantity("0.25")).unwrap();
        assert_eq!(first.total.minor_units(), 1);
        let basis = RefundBasis {
            returned_quantity: quantity("0.25"),
            refunded_net: first.net,
            refunded_tax: first.tax,
        };
        let second = refund_increment(&item, &basis, quantity("0.25")).unwrap();
        assert_eq!(second.total.minor_units(), 1);
        assert_eq!(first.total.minor_units() + second.total.minor_units(), 2);
    }
}
