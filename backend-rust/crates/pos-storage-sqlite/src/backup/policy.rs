//! Politica, previsualizacion y pertenencia del respaldo.

use super::*;

/// Información segura para confirmar una restauración. No expone la licencia ni
/// rutas internas; el backend conserva esos valores en el marcador privado.
#[derive(Debug, Clone, Serialize)]
pub struct RestorePreview {
    pub backup_valid_until: String,
    pub preserves_commercial_state: bool,
}

/// Estado completo de `local_installation_state` que se conserva al restaurar.
/// Es privado del adaptador: nunca atraviesa IPC ni se escribe fuera del
/// almacenamiento privado de la aplicación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InstallationRecord {
    pub(super) installation_id: String,
    pub(super) organization_id: String,
    pub(super) plan: String,
    pub(super) license_status: String,
    pub(super) features_json: String,
    pub(super) trial_started_at: String,
    pub(super) valid_until: String,
    pub(super) license_grant_json: Option<String>,
    pub(super) license_signature_b64: Option<String>,
}

/// Valida un respaldo seleccionado para restaurarlo sobre la instalación local
/// actual. La restauración entre instalaciones u organizaciones se rechaza. En
/// 0.1.0 se rechazan snapshots que contengan material de licencia firmado:
/// todavía no hay un verificador con clave pública configurado durante este
/// arranque temprano. El estado comercial vigente se preserva exactamente, por
/// lo que un trial antiguo no puede extenderse mediante recuperación.
pub async fn preview_restore_candidate(
    database_path: &Path,
    backup_path: &Path,
) -> Result<RestorePreview, StorageError> {
    let current = read_current_installation(database_path).await?;
    let backup = read_backup_installation(backup_path).await?;
    validate_restore_policy(&current, &backup)?;
    Ok(RestorePreview {
        backup_valid_until: backup.valid_until,
        preserves_commercial_state: true,
    })
}

pub(super) fn validate_restore_policy(
    current: &InstallationRecord,
    backup: &InstallationRecord,
) -> Result<(), StorageError> {
    if current.installation_id != backup.installation_id
        || current.organization_id != backup.organization_id
    {
        return Err(StorageError::RestoreIdentityMismatch);
    }
    // En Local 0.1.0 no hay una clave pública de licencias configurada para el
    // arranque temprano. Rechazar, en vez de aceptar o copiar material firmado
    // sin verificación Ed25519, evita escalar o revivir derechos.
    if current.license_grant_json.is_some()
        || current.license_signature_b64.is_some()
        || backup.license_grant_json.is_some()
        || backup.license_signature_b64.is_some()
    {
        return Err(StorageError::RestoreLicensePolicy);
    }
    Ok(())
}

pub(super) async fn read_backup_installation(
    path: &Path,
) -> Result<InstallationRecord, StorageError> {
    verify_backup(path).await?;
    read_installation(path, StorageError::InvalidBackup).await
}

pub(super) async fn read_current_installation(
    path: &Path,
) -> Result<InstallationRecord, StorageError> {
    read_installation(path, StorageError::Restore).await
}

pub(super) async fn read_installation(
    path: &Path,
    error: StorageError,
) -> Result<InstallationRecord, StorageError> {
    let mut connection = open_read_only_connection(path)
        .await
        .map_err(|_| error.clone())?;
    let row = sqlx::query(
        "SELECT installation_id, organization_id, plan, license_status, features_json, \
         trial_started_at, valid_until, license_grant_json, license_signature_b64 \
         FROM local_installation_state WHERE id = 1",
    )
    .fetch_optional(&mut connection)
    .await
    .map_err(|_| error.clone())?
    .ok_or(error.clone())?;
    let record = InstallationRecord {
        installation_id: row.get("installation_id"),
        organization_id: row.get("organization_id"),
        plan: row.get("plan"),
        license_status: row.get("license_status"),
        features_json: row.get("features_json"),
        trial_started_at: row.get("trial_started_at"),
        valid_until: row.get("valid_until"),
        license_grant_json: row.get("license_grant_json"),
        license_signature_b64: row.get("license_signature_b64"),
    };
    connection
        .close()
        .await
        .map_err(|_| StorageError::Restore)?;
    Ok(record)
}

pub(super) async fn restore_preserved_commercial(
    database_path: &Path,
    state: &InstallationRecord,
) -> Result<(), StorageError> {
    let _process_lock = try_lock(database_path)?;
    let mut connection = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(false)
        .foreign_keys(true)
        .connect()
        .await
        .map_err(|_| StorageError::Restore)?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut connection)
        .await
        .map_err(|_| StorageError::Restore)?;
    let update = sqlx::query(
        "UPDATE local_installation_state SET \
         installation_id = ?, organization_id = ?, plan = ?, license_status = ?, features_json = ?, \
         trial_started_at = ?, valid_until = ?, license_grant_json = ?, license_signature_b64 = ?, \
         updated_at = ? WHERE id = 1",
    )
    .bind(&state.installation_id)
    .bind(&state.organization_id)
    .bind(&state.plan)
    .bind(&state.license_status)
    .bind(&state.features_json)
    .bind(&state.trial_started_at)
    .bind(&state.valid_until)
    .bind(&state.license_grant_json)
    .bind(&state.license_signature_b64)
    .bind(time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).map_err(|_| StorageError::Restore)?)
    .execute(&mut connection)
    .await;
    if update.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut connection).await;
        let _ = connection.close().await;
        return Err(StorageError::Restore);
    }
    sqlx::query("COMMIT")
        .execute(&mut connection)
        .await
        .map_err(|_| StorageError::Restore)?;
    connection.close().await.map_err(|_| StorageError::Restore)
}
