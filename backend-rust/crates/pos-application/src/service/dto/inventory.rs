//! DTO de inventario y diagnostico de integridad.

use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct StockLevelView {
    pub product_id: String,
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InventoryMovementDraft {
    pub product_id: String,
    pub kind: String,
    pub quantity: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryMovementView {
    pub id: String,
    pub product_id: String,
    pub kind: String,
    pub quantity: String,
    pub reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

impl From<&InventoryMovement> for InventoryMovementView {
    fn from(movement: &InventoryMovement) -> Self {
        Self {
            id: movement.id.to_string(),
            product_id: movement.product_id.to_string(),
            kind: movement.kind.as_str().to_owned(),
            quantity: movement.quantity.to_string(),
            reason: movement.reason.clone(),
            occurred_at: movement.occurred_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IntegrityStatus {
    Consistent,
    Mismatch,
    Unverifiable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntegrityFinding {
    pub scope: String,
    pub entity_id: Option<String>,
    pub status: IntegrityStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntegrityDiagnosticReport {
    pub organization_id: String,
    pub branch_id: String,
    /// Semantica agregada: el peor estado de todos los hallazgos.
    pub status: IntegrityStatus,
    /// Numero de movimientos efectivamente leidos, maximo 100000.
    pub movement_count: usize,
    /// `true` si el limite no interrumpio el recorrido.
    pub complete: bool,
    pub findings: Vec<IntegrityFinding>,
}
