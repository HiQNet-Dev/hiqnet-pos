//! Copia de respaldo y utilidades de archivo.

use super::*;

/// Tope de tamaño para un respaldo. El adaptador SAF aplica además un límite
/// equivalente durante la copia; aquí protege también a la utilidad offline.
pub(super) const MAX_BACKUP_BYTES: u64 = 512 * 1024 * 1024;

/// Crea un respaldo consistente usando `VACUUM INTO`, que no requiere detener
/// la aplicación y produce un archivo SQLite íntegro. El archivo recién creado
/// se verifica; si no es válido se elimina y nunca se ofrece como respaldo.
pub async fn backup_to(pool: &SqlitePool, target: &Path) -> Result<(), StorageError> {
    if target.exists() {
        return Err(StorageError::Backup);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Backup)?;
    }
    let staged = backup_staged_path(target);
    let result = async {
        sqlx::query("VACUUM INTO ?")
            .bind(staged.to_string_lossy().to_string())
            .execute(pool)
            .await
            .map_err(|_| StorageError::Backup)?;
        restrict_sensitive_permissions(&staged)?;
        redact_enrollment_device_codes(&staged).await?;
        if fault(RestoreStep::BackupValidation) {
            return Err(StorageError::Backup);
        }
        verify_backup(&staged).await?;
        sync_file(&staged).map_err(|_| StorageError::Backup)?;
        std::fs::rename(&staged, target).map_err(|_| StorageError::Backup)?;
        sync_parent(target);
        Ok(())
    }
    .await;

    if let Err(error) = result {
        remove_sensitive_temporary(&staged)?;
        return Err(error);
    }
    Ok(())
}

/// El destino seleccionado no se publica hasta que este archivo intermedio se
/// haya saneado y validado.
pub(super) fn backup_staged_path(target: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "hiqnet-pos.db".into());
    target.with_file_name(format!(".{name}.backup-tmp-{stamp}"))
}

/// Los challenges pendientes son secretos de un solo uso. Un respaldo no debe
/// transportarlos; los vínculos completados se conservan sin su device_code.
pub(super) async fn redact_enrollment_device_codes(path: &Path) -> Result<(), StorageError> {
    let mut connection = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .connect()
        .await
        .map_err(|_| StorageError::Backup)?;
    let result: Result<(), StorageError> = async {
        let enrollment_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'platform_enrollment'",
        )
        .fetch_one(&mut connection)
        .await
        .map_err(|_| StorageError::Backup)?;
        if enrollment_exists == 0 {
            return Ok(());
        }
        sqlx::query("DELETE FROM platform_enrollment WHERE status = 'pending'")
            .execute(&mut connection)
            .await
            .map_err(|_| StorageError::Backup)?;
        sqlx::query("UPDATE platform_enrollment SET device_code = NULL WHERE status = 'completed'")
            .execute(&mut connection)
            .await
            .map_err(|_| StorageError::Backup)?;
        Ok(())
    }
    .await;
    let _ = connection.close().await;
    result.map_err(|_| StorageError::Backup)?;
    Ok(())
}

/// Un temporal de respaldo puede contener secretos antes del saneamiento. En
/// Unix se restringe al propietario; Windows conserva sus ACL predeterminadas.
fn restrict_sensitive_permissions(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| StorageError::Backup)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn remove_sensitive_temporary(path: &Path) -> Result<(), StorageError> {
    if fault(RestoreStep::BackupTemporaryCleanup) {
        return Err(StorageError::BackupTemporaryCleanup);
    }
    std::fs::remove_file(path).map_err(|_| StorageError::BackupTemporaryCleanup)
}

pub(super) fn sha256_file(path: &Path) -> Result<String, StorageError> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|_| StorageError::PendingRestore)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| StorageError::PendingRestore)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
