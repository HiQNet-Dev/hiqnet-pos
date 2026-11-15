//! Enrolamiento del dispositivo con HiQNet Platform.
//!
//! POS conserva la autoridad local: el enrolamiento solo vincula la identidad
//! pública del dispositivo con una organización aprobada por el owner en el
//! Portal. No concede derechos comerciales; la licencia firmada sigue siendo el
//! único documento que autoriza escrituras.

use super::*;

impl PosService {
    /// Inicia un challenge de enrolamiento. Requiere que el owner local esté
    /// autenticado; la aprobación ocurre después en el Portal de Platform.
    pub async fn begin_platform_enrollment(
        &self,
        actor: &Actor,
    ) -> AppResult<PlatformEnrollmentView> {
        actor.require(pos_core::identity::Permission::ManageOrganization)?;
        let signer = self
            .device_signer
            .as_ref()
            .ok_or(AppError::EnrollmentUnavailable)?;
        let client = self
            .enrollment_client
            .as_ref()
            .ok_or(AppError::EnrollmentUnavailable)?;
        let platform_url = self
            .enrollment_url
            .clone()
            .ok_or(AppError::EnrollmentUnavailable)?;

        let mut tx = self.store.begin().await?;
        let state = tx
            .installation_state()
            .await?
            .ok_or(pos_core::DomainError::AlreadyInitialized)?;
        if actor.organization_id != state.organization_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: pos_core::identity::Permission::ManageOrganization.as_str(),
            }
            .into());
        }
        let previous = tx.enrollment_session().await?;
        if let Some(existing) = previous.as_ref()
            && existing.status == EnrollmentSessionStatus::Completed
            && existing.installation_id == state.installation_id
        {
            tx.rollback().await?;
            return Ok(enrollment_view(existing));
        }
        let installation_id = state.installation_id;
        let previous_device_code = previous
            .as_ref()
            .and_then(|session| session.device_code.clone());
        tx.rollback().await?;

        let challenge = client
            .begin(BeginPlatformEnrollment {
                installation_id,
                product_key: "pos".into(),
                public_key: signer.public_key(),
                // La etiqueta debe ser única por organización en Platform.
                device_label: format!("HiQNet POS {}", &installation_id.to_string()[..8]),
                app_version: env!("CARGO_PKG_VERSION").into(),
                platform: std::env::consts::OS.into(),
            })
            .await?;

        let session = EnrollmentSession {
            platform_url,
            device_code: Some(challenge.device_code),
            user_code: challenge.user_code,
            verification_uri: challenge.verification_uri,
            nonce: challenge.nonce,
            expires_at: self.clock.now()
                + time::Duration::seconds(
                    i64::try_from(challenge.expires_in_seconds).unwrap_or(600),
                ),
            installation_id,
            status: EnrollmentSessionStatus::Pending,
            organization_id: None,
            bound_at: None,
        };

        let mut tx = self.store.begin().await?;
        let current = tx.enrollment_session().await?;
        if let Some(current) = current.as_ref() {
            if current.status == EnrollmentSessionStatus::Completed {
                tx.rollback().await?;
                return Ok(enrollment_view(current));
            }
            if current.device_code.as_ref() != previous_device_code.as_ref() {
                // Otra invocación de begin ganó la carrera; no la sobrescribimos.
                return Err(AppError::Conflict);
            }
        }
        tx.save_enrollment_session(&session).await?;
        tx.append_audit(&AuditEvent::new(
            state.organization_id,
            None,
            Some(actor.user_id),
            AuditAction::PlatformEnrollmentStarted,
            "installation",
            installation_id.to_string(),
            serde_json::json!({"platform_host": platform_host(&session.platform_url)}),
        ))
        .await?;
        tx.commit().await?;
        Ok(enrollment_view(&session))
    }

    /// Completa el enrolamiento después de que el owner aprobó el código en el
    /// Portal. Firma el mensaje canónico con la clave privada del dispositivo.
    pub async fn complete_platform_enrollment(
        &self,
        actor: &Actor,
    ) -> AppResult<PlatformEnrollmentView> {
        actor.require(pos_core::identity::Permission::ManageOrganization)?;
        let signer = self
            .device_signer
            .as_ref()
            .ok_or(AppError::EnrollmentUnavailable)?;
        let client = self
            .enrollment_client
            .as_ref()
            .ok_or(AppError::EnrollmentUnavailable)?;

        let mut tx = self.store.begin().await?;
        let session = tx.enrollment_session().await?.ok_or(AppError::NotFound)?;
        let state = tx
            .installation_state()
            .await?
            .ok_or(pos_core::DomainError::AlreadyInitialized)?;
        if actor.organization_id != state.organization_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: pos_core::identity::Permission::ManageOrganization.as_str(),
            }
            .into());
        }
        if session.installation_id != state.installation_id {
            return Err(AppError::Conflict);
        }
        if session.status == EnrollmentSessionStatus::Completed {
            tx.rollback().await?;
            return Ok(enrollment_view(&session));
        }
        if self.clock.now() > session.expires_at {
            tx.clear_enrollment_session().await?;
            tx.commit().await?;
            return Err(AppError::EnrollmentExpired);
        }
        let device_code = session.device_code.as_deref().ok_or(AppError::Conflict)?;
        let signature_b64 =
            signer.sign_proof(device_code, session.installation_id, &session.nonce)?;
        let public_key = signer.public_key();
        tx.rollback().await?;

        let binding = client
            .prove(PlatformProof {
                device_code: device_code.to_owned(),
                installation_id: session.installation_id,
                public_key,
                nonce: session.nonce.clone(),
                signature_b64,
            })
            .await?;
        if binding.installation_id != session.installation_id {
            return Err(AppError::Conflict);
        }

        let completed = EnrollmentSession {
            status: EnrollmentSessionStatus::Completed,
            organization_id: Some(binding.organization_id),
            bound_at: Some(binding.bound_at),
            device_code: None,
            ..session
        };
        let mut tx = self.store.begin().await?;
        // El challenge pudo reemplazarse mientras se esperaba a Platform: no
        // guardar un resultado de un device_code que ya no es el vigente.
        let actual = tx.enrollment_session().await?.ok_or(AppError::NotFound)?;
        if actual.device_code.as_deref() != Some(device_code)
            || actual.installation_id != completed.installation_id
        {
            return Err(AppError::Conflict);
        }
        tx.save_enrollment_session(&completed).await?;
        tx.append_audit(&AuditEvent::new(
            state.organization_id,
            None,
            Some(actor.user_id),
            AuditAction::PlatformEnrollmentCompleted,
            "installation",
            completed.installation_id.to_string(),
            serde_json::json!({
                "platform_organization_id": binding.organization_id.to_string(),
                "public_key": signer.public_key(),
            }),
        ))
        .await?;
        tx.commit().await?;
        Ok(enrollment_view(&completed))
    }

    /// Estado visible del enrolamiento, sin secretos del challenge.
    pub async fn platform_enrollment_status(
        &self,
        actor: &Actor,
    ) -> AppResult<PlatformEnrollmentView> {
        actor.require(pos_core::identity::Permission::ManageOrganization)?;
        let mut tx = self.store.begin().await?;
        let state = tx
            .installation_state()
            .await?
            .ok_or(pos_core::DomainError::AlreadyInitialized)?;
        if actor.organization_id != state.organization_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: pos_core::identity::Permission::ManageOrganization.as_str(),
            }
            .into());
        }
        let session = tx.enrollment_session().await?;
        if matches!(
            session.as_ref(),
            Some(session)
                if session.status == EnrollmentSessionStatus::Pending
                    && self.clock.now() > session.expires_at
        ) {
            tx.clear_enrollment_session().await?;
            tx.commit().await?;
            return Ok(PlatformEnrollmentView {
                status: "none",
                user_code: None,
                verification_uri: None,
                expires_at: None,
                organization_id: None,
                installation_id: state.installation_id,
                completed: false,
            });
        }
        tx.rollback().await?;
        Ok(match session {
            Some(session) => enrollment_view(&session),
            None => PlatformEnrollmentView {
                status: "none",
                user_code: None,
                verification_uri: None,
                expires_at: None,
                organization_id: None,
                installation_id: state.installation_id,
                completed: false,
            },
        })
    }
}

fn enrollment_view(session: &EnrollmentSession) -> PlatformEnrollmentView {
    PlatformEnrollmentView {
        status: session.status.as_str(),
        user_code: Some(session.user_code.clone()),
        verification_uri: Some(session.verification_uri.clone()),
        expires_at: Some(
            session
                .expires_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        ),
        organization_id: session.organization_id.map(|value| value.to_string()),
        installation_id: session.installation_id,
        completed: session.status == EnrollmentSessionStatus::Completed,
    }
}

/// Valida la URL de Platform. Solo HTTPS, salvo loopback en desarrollo local.
pub fn normalize_platform_url(value: &str) -> AppResult<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return Err(AppError::EnrollmentUnavailable);
    }
    let (insecure, rest) = if let Some(rest) = trimmed.strip_prefix("https://") {
        (false, rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        (true, rest)
    } else {
        return Err(AppError::EnrollmentUnavailable);
    };
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() || host.contains('@') || host.contains('?') || host.contains('#') {
        return Err(AppError::EnrollmentUnavailable);
    }
    if insecure {
        let hostname = host
            .strip_prefix('[')
            .and_then(|value| value.split(']').next())
            .unwrap_or_else(|| host.split(':').next().unwrap_or_default());
        let local = matches!(hostname, "localhost" | "127.0.0.1" | "::1");
        if !local {
            return Err(AppError::EnrollmentUnavailable);
        }
    }
    Ok(trimmed.to_owned())
}

fn platform_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::normalize_platform_url;

    #[test]
    fn acepta_https_y_loopback_http() {
        assert_eq!(
            normalize_platform_url(" https://platform.example.com/ ").unwrap(),
            "https://platform.example.com"
        );
        assert_eq!(
            normalize_platform_url("http://127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            normalize_platform_url("http://[::1]:8080").unwrap(),
            "http://[::1]:8080"
        );
    }

    #[test]
    fn rechaza_http_remoto_esquemas_y_espacios() {
        for value in [
            "http://platform.example.com",
            "ftp://platform.example.com",
            "https://",
            "   ",
            "https://platform.example.com/ruta con espacio",
        ] {
            assert!(normalize_platform_url(value).is_err(), "{value}");
        }
    }
}
