//! Instalacion y reemplazo de licencias firmadas.

use super::*;

impl PosService {
    /// Instala o reemplaza una licencia firmada.
    ///
    /// El documento se verifica **antes** de persistirse. Si la firma no es
    /// valida, no cambia el estado comercial y se registra el rechazo en la
    /// auditoria (esta operacion abre su propia transaccion, asi que el evento
    /// sobrevive al error).
    pub async fn install_license(
        &self,
        actor: &Actor,
        grant_json: String,
        signature_b64: String,
    ) -> AppResult<CommercialStateView> {
        actor.require(pos_core::identity::Permission::ManageOrganization)?;

        let verifier = self.license.as_deref().ok_or(AppError::Crypto)?;

        let mut tx = self.store.begin().await?;
        let current = tx
            .installation_state()
            .await?
            .ok_or(pos_core::DomainError::AlreadyInitialized)?;

        let license = match verifier.verify(&grant_json, &signature_b64) {
            Ok(license) => license,
            Err(_) => return reject_license(tx, &current, "signature_rejected").await,
        };
        if license.installation_id != current.installation_id
            || license.organization_id != current.organization_id
        {
            return reject_license(tx, &current, "identity_mismatch").await;
        }

        let state = InstallationState {
            installation_id: current.installation_id,
            organization_id: current.organization_id,
            trial_started_at: current.trial_started_at,
            plan: license.plan.as_str().to_owned(),
            license_status: LicenseStatus::Active.as_str().to_owned(),
            features: license
                .features
                .iter()
                .map(|feature| feature.as_str().to_owned())
                .collect(),
            valid_until: license.valid_until,
            license_grant_json: Some(grant_json),
            license_signature_b64: Some(signature_b64),
        };
        tx.save_installation_state(&state).await?;

        tx.append_audit(&AuditEvent::new(
            state.organization_id,
            None,
            Some(actor.user_id),
            AuditAction::LicenseInstalled,
            "installation",
            state.installation_id.to_string(),
            serde_json::json!({
                "plan": state.plan,
                "valid_until": state.valid_until.to_string(),
                "features": state.features,
            }),
        ))
        .await?;
        tx.commit().await?;

        self.commercial_view(&state)
    }
}

/// Registra el rechazo y cierra la transaccion antes de devolver el error.
///
/// El mensaje hacia la interfaz es neutral: no revela si fallo la firma, la
/// version o la identidad.
async fn reject_license(
    mut tx: Box<dyn crate::ports::StoreTx>,
    current: &InstallationState,
    reason: &'static str,
) -> AppResult<CommercialStateView> {
    tx.append_audit(&AuditEvent::new(
        current.organization_id,
        None,
        None,
        AuditAction::LicenseRejected,
        "installation",
        current.installation_id.to_string(),
        serde_json::json!({ "reason": reason }),
    ))
    .await?;
    tx.commit().await?;
    tracing::warn!(reason, "licencia rechazada al intentar instalarla");
    Err(AppError::Crypto)
}
