//! Licencias firmadas: clave publica, verificador y comando IPC.

use super::*;

/// Clave publica de verificacion de licencias.
///
/// Se embebe en tiempo de compilacion con `POS_LICENSE_PUBLIC_KEY`. En
/// compilaciones de depuracion se admite ademas la variable de entorno
/// `HIQNET_POS_LICENSE_PUBLIC_KEY` para pruebas locales. El binario nunca
/// contiene clave privada ni semilla de firma.
fn license_public_key() -> Option<String> {
    if let Some(value) = option_env!("POS_LICENSE_PUBLIC_KEY") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    #[cfg(debug_assertions)]
    {
        std::env::var("HIQNET_POS_LICENSE_PUBLIC_KEY")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

pub fn license_verifier() -> Option<pos_application::SharedLicenseVerifier> {
    let key = license_public_key()?;
    match pos_licensing::Ed25519LicenseVerifier::from_public_key_base64(&key) {
        Ok(verifier) => Some(Arc::new(verifier)),
        Err(error) => {
            tracing::error!(
                error = %error,
                "la clave publica de licencias configurada no es valida"
            );
            None
        }
    }
}

/// Añade el verificador de licencias al servicio. Sin clave pública avisa y
/// deja el servicio sin verificador: toda licencia firmada se rechazará.
pub fn with_license_verifier(service: PosService) -> PosService {
    match license_verifier() {
        Some(verifier) => service.with_license_verifier(verifier),
        None => {
            tracing::warn!(
                "sin clave publica de licencias: cualquier licencia firmada se rechazara"
            );
            service
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn install_license(
    state: State<'_, AppState>,
    grant_json: String,
    signature_b64: String,
) -> IpcResult<CommercialStateView> {
    let actor = authenticated(&state).await?;
    Ok(state
        .service
        .install_license(&actor, grant_json, signature_b64)
        .await?)
}
