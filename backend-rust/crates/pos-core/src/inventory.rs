use crate::error::{DomainError, DomainResult};
use crate::ids::{BranchId, InventoryMovementId, OrganizationId, ProductId, UserId};
use crate::quantity::Quantity;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Tipo de movimiento de inventario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementKind {
    Inbound,
    Outbound,
    Adjustment,
}

impl MovementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::Adjustment => "adjustment",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "inbound" => Some(Self::Inbound),
            "outbound" => Some(Self::Outbound),
            "adjustment" => Some(Self::Adjustment),
            _ => None,
        }
    }
}

/// Movimiento de inventario append-only.
///
/// La cantidad siempre es positiva; la dirección la determina `kind`.
/// Un movimiento nunca se edita ni elimina: las correcciones son nuevos
/// movimientos de ajuste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryMovement {
    pub id: InventoryMovementId,
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub product_id: ProductId,
    pub kind: MovementKind,
    pub quantity: Quantity,
    pub reason: Option<String>,
    pub actor_user_id: Option<UserId>,
    pub occurred_at: OffsetDateTime,
}

impl InventoryMovement {
    pub fn new(
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: ProductId,
        kind: MovementKind,
        quantity: Quantity,
        reason: Option<String>,
        actor_user_id: Option<UserId>,
    ) -> DomainResult<Self> {
        if quantity.is_zero() {
            return Err(DomainError::NegativeQuantity);
        }
        let reason = reason
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if matches!(kind, MovementKind::Adjustment) && reason.is_none() {
            return Err(DomainError::EmptyName);
        }
        Ok(Self {
            id: InventoryMovementId::new(),
            organization_id,
            branch_id,
            product_id,
            kind,
            quantity,
            reason,
            actor_user_id,
            occurred_at: OffsetDateTime::now_utc(),
        })
    }

    /// Signo del movimiento sobre el stock disponible (+1 o -1).
    pub fn signed_milli_units(&self) -> i64 {
        match self.kind {
            MovementKind::Inbound | MovementKind::Adjustment => self.quantity.milli_units(),
            MovementKind::Outbound => -self.quantity.milli_units(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rechaza_cantidad_cero() {
        let result = InventoryMovement::new(
            OrganizationId::new(),
            BranchId::new(),
            ProductId::new(),
            MovementKind::Inbound,
            Quantity::ZERO,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn ajuste_requiere_motivo() {
        let result = InventoryMovement::new(
            OrganizationId::new(),
            BranchId::new(),
            ProductId::new(),
            MovementKind::Adjustment,
            Quantity::from_milli_units(1000).unwrap(),
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn salida_es_negativa() {
        let movement = InventoryMovement::new(
            OrganizationId::new(),
            BranchId::new(),
            ProductId::new(),
            MovementKind::Outbound,
            Quantity::from_milli_units(2000).unwrap(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(movement.signed_milli_units(), -2000);
    }
}
