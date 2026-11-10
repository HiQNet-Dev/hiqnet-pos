//! Auditoria y respaldos.

use super::*;

impl PosService {
    pub async fn list_audit(&self, actor: &Actor, limit: u32) -> AppResult<Vec<AuditEventView>> {
        actor.require(pos_core::identity::Permission::ViewAudit)?;
        let limit = i64::from(limit).clamp(1, MAX_AUDIT_PAGE);
        let mut tx = self.store.begin().await?;
        let events = tx.list_audit(actor.organization_id, limit).await?;
        tx.rollback().await?;
        Ok(events.iter().map(AuditEventView::from).collect())
    }

    pub async fn record_backup(&self, actor: &Actor, backup_path: &str) -> AppResult<()> {
        actor.require(pos_core::identity::Permission::ManageBackups)?;
        let mut tx = self.store.begin().await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::BackupCreated,
            "backup",
            backup_path.to_owned(),
            serde_json::json!({}),
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
