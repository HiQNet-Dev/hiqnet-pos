//! DTO de caja: registros, turnos y movimientos.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct CashRegisterDraft {
    pub name: String,
    pub code: String,
}

/// Identidad legible de la sucursal del actor autenticado.
#[derive(Debug, Clone, Serialize)]
pub struct BranchView {
    pub id: String,
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CashRegisterView {
    pub id: String,
    pub name: String,
    pub code: String,
    pub active: bool,
}

impl From<&CashRegister> for CashRegisterView {
    fn from(register: &CashRegister) -> Self {
        Self {
            id: register.id.to_string(),
            name: register.name.clone(),
            code: register.code.clone(),
            active: register.active,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CashMovementView {
    pub id: String,
    pub kind: String,
    pub amount: String,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: time::OffsetDateTime,
}

impl From<&CashMovement> for CashMovementView {
    fn from(movement: &CashMovement) -> Self {
        Self {
            id: movement.id.to_string(),
            kind: movement.kind.as_str().to_owned(),
            amount: movement.amount.to_string(),
            reason: movement.reason.clone(),
            occurred_at: movement.occurred_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CashShiftView {
    pub id: String,
    pub register_id: String,
    pub status: String,
    pub opened_by: String,
    #[serde(with = "time::serde::rfc3339")]
    pub opened_at: time::OffsetDateTime,
    pub closed_by: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<time::OffsetDateTime>,
    pub opening_amount: String,
    pub expected_amount: String,
    pub entry_total: String,
    pub exit_total: String,
    pub counted_amount: Option<String>,
    pub difference_minor: Option<i64>,
    pub movements: Vec<CashMovementView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashMovementDraft {
    pub shift_id: String,
    pub kind: String,
    pub amount: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashOpenShiftDraft {
    pub register_id: String,
    pub opening_amount: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashCloseShiftDraft {
    pub shift_id: String,
    pub counted_amount: String,
}
