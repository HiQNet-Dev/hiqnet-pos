use crate::error::{DomainError, DomainResult};
use crate::ids::{CashMovementId, CashRegisterId, CashShiftId, OrganizationId, UserId};
use crate::money::Money;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashShiftStatus {
    Open,
    Closed,
}

impl CashShiftStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashMovementKind {
    In,
    Out,
}

impl CashMovementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "in" => Some(Self::In),
            "out" => Some(Self::Out),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashRegister {
    pub id: CashRegisterId,
    pub organization_id: OrganizationId,
    pub branch_id: crate::ids::BranchId,
    pub name: String,
    pub code: String,
    pub active: bool,
    pub created_at: OffsetDateTime,
}

impl CashRegister {
    pub fn new(
        organization_id: OrganizationId,
        branch_id: crate::ids::BranchId,
        name: impl Into<String>,
        code: impl Into<String>,
    ) -> DomainResult<Self> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        let code = code.into();
        let code = code.trim().to_ascii_uppercase();
        if code.is_empty() || code.len() > 16 {
            return Err(DomainError::InvalidSku(code));
        }
        Ok(Self {
            id: CashRegisterId::new(),
            organization_id,
            branch_id,
            name: name.to_owned(),
            code,
            active: true,
            created_at: OffsetDateTime::now_utc(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashShift {
    pub id: CashShiftId,
    pub organization_id: OrganizationId,
    pub branch_id: crate::ids::BranchId,
    pub register_id: CashRegisterId,
    pub opened_by: UserId,
    pub opened_at: OffsetDateTime,
    pub opening_amount: Money,
    pub status: CashShiftStatus,
    pub closed_by: Option<UserId>,
    pub closed_at: Option<OffsetDateTime>,
    pub expected_amount: Option<Money>,
    pub counted_amount: Option<Money>,
    pub difference_minor: Option<i64>,
}

impl CashShift {
    pub fn new(
        organization_id: OrganizationId,
        branch_id: crate::ids::BranchId,
        register_id: CashRegisterId,
        opened_by: UserId,
        opening_amount: Money,
    ) -> Self {
        Self {
            id: CashShiftId::new(),
            organization_id,
            branch_id,
            register_id,
            opened_by,
            opened_at: OffsetDateTime::now_utc(),
            opening_amount,
            status: CashShiftStatus::Open,
            closed_by: None,
            closed_at: None,
            expected_amount: None,
            counted_amount: None,
            difference_minor: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.status == CashShiftStatus::Open
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashMovement {
    pub id: CashMovementId,
    pub organization_id: OrganizationId,
    pub branch_id: crate::ids::BranchId,
    pub register_id: CashRegisterId,
    pub shift_id: CashShiftId,
    pub kind: CashMovementKind,
    pub amount: Money,
    pub reason: String,
    pub actor_user_id: Option<UserId>,
    pub occurred_at: OffsetDateTime,
}

impl CashMovement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        organization_id: OrganizationId,
        branch_id: crate::ids::BranchId,
        register_id: CashRegisterId,
        shift_id: CashShiftId,
        kind: CashMovementKind,
        amount: Money,
        reason: impl Into<String>,
        actor_user_id: Option<UserId>,
    ) -> DomainResult<Self> {
        let reason = reason.into();
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(DomainError::EmptyName);
        }
        if amount == Money::ZERO {
            return Err(DomainError::NegativeAmount);
        }
        Ok(Self {
            id: CashMovementId::new(),
            organization_id,
            branch_id,
            register_id,
            shift_id,
            kind,
            amount,
            reason: reason.to_owned(),
            actor_user_id,
            occurred_at: OffsetDateTime::now_utc(),
        })
    }

    pub fn signed_minor_units(&self) -> i64 {
        match self.kind {
            CashMovementKind::In => self.amount.minor_units(),
            CashMovementKind::Out => -self.amount.minor_units(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashReconciliation {
    pub opening_minor: i64,
    pub entries_minor: i64,
    pub exits_minor: i64,
    pub expected_minor: i64,
    pub counted_minor: i64,
    pub difference_minor: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movimiento_requiere_motivo() {
        let register = CashRegisterId::new();
        let result = CashMovement::new(
            OrganizationId::new(),
            crate::ids::BranchId::new(),
            register,
            CashShiftId::new(),
            CashMovementKind::Out,
            Money::from_minor_units(100).unwrap(),
            "   ",
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn saldo_por_signo() {
        let movement = CashMovement::new(
            OrganizationId::new(),
            crate::ids::BranchId::new(),
            CashRegisterId::new(),
            CashShiftId::new(),
            CashMovementKind::In,
            Money::from_minor_units(500).unwrap(),
            "apertura",
            None,
        )
        .unwrap();
        assert_eq!(movement.signed_minor_units(), 500);
    }
}
