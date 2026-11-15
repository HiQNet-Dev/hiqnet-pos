//! Enrolamiento con HiQNet Platform: cliente HTTP y clave del dispositivo.
//!
//! La clave privada se guarda junto a la base con permisos restrictivos y no
//! entra en respaldos ni en la base SQLite. En Windows el archivo hereda las
//! ACL del perfil del usuario; el endurecimiento con DPAPI queda pendiente.

use super::*;
use pos_application::{
    AppError, AppResult, BeginPlatformEnrollment, PlatformChallenge, PlatformEnrollmentBinding,
    PlatformEnrollmentClient, PlatformEnrollmentView, PlatformProof,
};
use pos_licensing::DeviceIdentity;
use serde::Deserialize;
use std::path::Path;

/// Carga la identidad del dispositivo o la crea si aún no existe.
pub fn load_or_create_identity(database_path: &Path) -> Result<DeviceIdentity, String> {
    let path = pos_storage_sqlite::device_identity_path(database_path);
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err("la clave del dispositivo debe ser un archivo regular".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(
                    "la clave del dispositivo tiene permisos demasiado abiertos; corrige el archivo"
                        .into(),
                );
            }
        }
    }
    match std::fs::read_to_string(&path) {
        Ok(value) => {
            let seed = pos_licensing::device_identity::seed_from_base64(&value)
                .map_err(|error| error.to_string())?;
            Ok(DeviceIdentity::from_seed_bytes(seed))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let identity = DeviceIdentity::generate();
            let encoded = pos_licensing::device_identity::seed_to_base64(&identity.seed_bytes());
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            use std::io::Write;
            let mut file = options
                .open(&path)
                .map_err(|_| "no fue posible guardar la clave del dispositivo".to_owned())?;
            file.write_all(encoded.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|_| "no fue posible guardar la clave del dispositivo".to_owned())?;
            Ok(identity)
        }
        Err(_) => Err("no fue posible leer la clave del dispositivo".into()),
    }
}

/// Cliente HTTP mínimo del protocolo de enrolamiento.
pub struct HttpPlatformEnrollment {
    base_url: String,
    client: reqwest::Client,
}

impl HttpPlatformEnrollment {
    pub fn new(base_url: String) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| "no fue posible configurar el cliente HTTP".to_owned())?;
        Ok(Self { base_url, client })
    }
}

fn valid_device_code(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
}

#[derive(Deserialize)]
struct BeginResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    nonce: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct ProofResponse {
    installation_id: String,
    organization_id: String,
    #[serde(with = "time::serde::rfc3339")]
    bound_at: time::OffsetDateTime,
}

fn status_error(status: reqwest::StatusCode) -> AppError {
    match status.as_u16() {
        404 | 410 => AppError::EnrollmentExpired,
        409 => AppError::Conflict,
        400..=499 => AppError::InvalidRequest,
        _ => AppError::EnrollmentUnavailable,
    }
}

#[async_trait::async_trait]
impl PlatformEnrollmentClient for HttpPlatformEnrollment {
    async fn begin(&self, request: BeginPlatformEnrollment) -> AppResult<PlatformChallenge> {
        let response = self
            .client
            .post(format!("{}/api/v1/enrollment-requests", self.base_url))
            .json(&serde_json::json!({
                "installation_id": request.installation_id,
                "product_key": request.product_key,
                "public_key": request.public_key,
                "device_label": request.device_label,
                "app_version": request.app_version,
                "platform": request.platform,
            }))
            .send()
            .await
            .map_err(|_| AppError::EnrollmentUnavailable)?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let body: BeginResponse = response
            .json()
            .await
            .map_err(|_| AppError::EnrollmentUnavailable)?;
        Ok(PlatformChallenge {
            device_code: body.device_code,
            user_code: body.user_code,
            verification_uri: body.verification_uri,
            nonce: body.nonce,
            expires_in_seconds: body.expires_in,
        })
    }

    async fn prove(&self, request: PlatformProof) -> AppResult<PlatformEnrollmentBinding> {
        if !valid_device_code(&request.device_code) {
            return Err(AppError::EnrollmentUnavailable);
        }
        let response = self
            .client
            .post(format!(
                "{}/api/v1/enrollment-requests/{}/proof",
                self.base_url, request.device_code
            ))
            .json(&serde_json::json!({
                "installation_id": request.installation_id,
                "public_key": request.public_key,
                "nonce": request.nonce,
                "signature": request.signature_b64,
            }))
            .send()
            .await
            .map_err(|_| AppError::EnrollmentUnavailable)?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let body: ProofResponse = response
            .json()
            .await
            .map_err(|_| AppError::EnrollmentUnavailable)?;
        Ok(PlatformEnrollmentBinding {
            installation_id: body
                .installation_id
                .parse()
                .map_err(|_| AppError::EnrollmentUnavailable)?,
            organization_id: body
                .organization_id
                .parse()
                .map_err(|_| AppError::EnrollmentUnavailable)?,
            bound_at: body.bound_at,
        })
    }
}

/// Configura el cliente de enrolamiento si `HIQNET_PLATFORM_URL` está definida.
///
/// Sin URL la app sigue operando 100 % offline y los comandos de enrolamiento
/// responden con un error claro. Una configuración inválida degrada a offline
/// en lugar de impedir el arranque.
pub fn with_platform_enrollment(service: PosService, database_path: &Path) -> PosService {
    let Ok(value) = std::env::var("HIQNET_PLATFORM_URL") else {
        return service;
    };
    if value.trim().is_empty() {
        return service;
    }
    let (url, identity) = match (
        pos_application::normalize_platform_url(&value),
        load_or_create_identity(database_path),
    ) {
        (Ok(url), Ok(identity)) => (url, identity),
        (Err(_), _) => {
            tracing::warn!("HIQNET_PLATFORM_URL invalida: enrolamiento deshabilitado");
            return service;
        }
        (_, Err(error)) => {
            tracing::warn!(%error, "identidad de dispositivo no disponible: enrolamiento deshabilitado");
            return service;
        }
    };
    match HttpPlatformEnrollment::new(url.clone()) {
        Ok(client) => service.with_platform_enrollment(Arc::new(identity), Arc::new(client), url),
        Err(error) => {
            tracing::warn!(%error, "cliente de enrolamiento no disponible");
            service
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn platform_enrollment_status(
    state: State<'_, AppState>,
) -> IpcResult<PlatformEnrollmentView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.platform_enrollment_status(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn begin_platform_enrollment(
    state: State<'_, AppState>,
) -> IpcResult<PlatformEnrollmentView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.begin_platform_enrollment(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn complete_platform_enrollment(
    state: State<'_, AppState>,
) -> IpcResult<PlatformEnrollmentView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.complete_platform_enrollment(&actor).await?)
}
