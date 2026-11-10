//! DTO de auditoria y respaldos.

use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEventView {
    pub id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub actor_user_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

impl From<&AuditEvent> for AuditEventView {
    fn from(event: &AuditEvent) -> Self {
        Self {
            id: event.id.to_string(),
            action: event.action.as_str().to_owned(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            actor_user_id: event.actor_user_id.map(|id| id.to_string()),
            occurred_at: event.occurred_at,
        }
    }
}
