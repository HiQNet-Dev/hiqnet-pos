use crate::ids::{AuditEventId, BranchId, OrganizationId, UserId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Acciones auditables con clave estable. La auditoría es append-only: se
/// escribe en la misma transacción que la mutación que la origina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    InstallationBootstrapped,
    SessionLoggedIn,
    SessionLoginRejected,
    ProductCreated,
    CategoryCreated,
    ProductVariantCreated,
    ProductCategoryChanged,
    InventoryMovementRegistered,
    BackupCreated,
}

impl AuditAction {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "installation.bootstrapped" => Some(AuditAction::InstallationBootstrapped),
            "session.logged_in" => Some(AuditAction::SessionLoggedIn),
            "session.login_rejected" => Some(AuditAction::SessionLoginRejected),
            "catalog.product_created" => Some(AuditAction::ProductCreated),
            "catalog.category_created" => Some(AuditAction::CategoryCreated),
            "catalog.variant_created" => Some(AuditAction::ProductVariantCreated),
            "catalog.product_category_changed" => Some(AuditAction::ProductCategoryChanged),
            "inventory.movement_registered" => Some(AuditAction::InventoryMovementRegistered),
            "backup.created" => Some(AuditAction::BackupCreated),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AuditAction::InstallationBootstrapped => "installation.bootstrapped",
            AuditAction::SessionLoggedIn => "session.logged_in",
            AuditAction::SessionLoginRejected => "session.login_rejected",
            AuditAction::ProductCreated => "catalog.product_created",
            AuditAction::CategoryCreated => "catalog.category_created",
            AuditAction::ProductVariantCreated => "catalog.variant_created",
            AuditAction::ProductCategoryChanged => "catalog.product_category_changed",
            AuditAction::InventoryMovementRegistered => "inventory.movement_registered",
            AuditAction::BackupCreated => "backup.created",
        }
    }
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Evento de auditoría inmutable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub organization_id: OrganizationId,
    pub branch_id: Option<BranchId>,
    pub actor_user_id: Option<UserId>,
    pub action: AuditAction,
    pub entity_type: String,
    pub entity_id: String,
    /// Detalle no sensible (sin contraseñas, tokens ni datos personales).
    pub payload: serde_json::Value,
    pub occurred_at: time::OffsetDateTime,
}

impl AuditEvent {
    pub fn new(
        organization_id: OrganizationId,
        branch_id: Option<BranchId>,
        actor_user_id: Option<UserId>,
        action: AuditAction,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: AuditEventId::new(),
            organization_id,
            branch_id,
            actor_user_id,
            action,
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            payload,
            occurred_at: time::OffsetDateTime::now_utc(),
        }
    }
}
