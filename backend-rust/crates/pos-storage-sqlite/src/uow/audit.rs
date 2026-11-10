//! Auditoria.

use super::*;

pub(super) async fn append_audit(tx: &mut SqliteTx, event: &AuditEvent) -> AppResult<()> {
    let payload = serde_json::to_string(&event.payload).map_err(|_| AppError::Storage)?;
    sqlx::query(
        "INSERT INTO audit_event \
             (id, organization_id, branch_id, actor_user_id, action, entity_type, entity_id, \
              payload_json, occurred_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id.to_string())
    .bind(event.organization_id.to_string())
    .bind(event.branch_id.map(|id| id.to_string()))
    .bind(event.actor_user_id.map(|id| id.to_string()))
    .bind(event.action.as_str())
    .bind(&event.entity_type)
    .bind(&event.entity_id)
    .bind(payload)
    .bind(format_ts(event.occurred_at))
    .execute(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_audit(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    limit: i64,
) -> AppResult<Vec<AuditEvent>> {
    let rows = sqlx::query(
        "SELECT id, organization_id, branch_id, actor_user_id, action, entity_type, entity_id, \
             payload_json, occurred_at FROM audit_event \
             WHERE organization_id = ? ORDER BY occurred_at DESC LIMIT ?",
    )
    .bind(organization_id.to_string())
    .bind(limit)
    .fetch_all(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;

    rows.into_iter()
        .map(|row| -> AppResult<AuditEvent> {
            let action =
                AuditAction::from_key(&row.get::<String, _>("action")).ok_or(AppError::Storage)?;
            let payload = serde_json::from_str(&row.get::<String, _>("payload_json"))
                .map_err(|_| AppError::Storage)?;
            Ok(AuditEvent {
                id: row
                    .get::<String, _>("id")
                    .parse::<AuditEventId>()
                    .map_err(|_| AppError::Storage)?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_optional_id(row.get("branch_id"))?,
                actor_user_id: parse_optional_id(row.get("actor_user_id"))?,
                action,
                entity_type: row.get("entity_type"),
                entity_id: row.get("entity_id"),
                payload,
                occurred_at: parse_ts(row.get("occurred_at"))?,
            })
        })
        .collect()
}
