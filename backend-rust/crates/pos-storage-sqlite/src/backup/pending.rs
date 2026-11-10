//! Restauracion diferida y espacio de trabajo SAF.

use super::*;

/// Resultado que el arranque puede registrar sin abrir todavía el pool de la
/// aplicación. Una restauración se consume antes de `open_locked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingRestoreOutcome {
    None,
    Restored,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PendingRestore {
    pub(super) version: u8,
    pub(super) backup_path: PathBuf,
    pub(super) backup_sha256: String,
    pub(super) installation_id: String,
    pub(super) organization_id: String,
    pub(super) preserved_commercial: InstallationRecord,
}

/// Crea el marcador solamente después de que la UI confirma explícitamente la
/// restauración. Volvemos a validar para que un preview anterior nunca autorice
/// un archivo modificado ni un cambio de identidad posterior.
pub async fn schedule_pending_restore(
    database_path: &Path,
    backup_path: &Path,
) -> Result<(), StorageError> {
    let current = read_current_installation(database_path).await?;
    let backup = read_backup_installation(backup_path).await?;
    validate_restore_policy(&current, &backup)?;
    let pending = PendingRestore {
        version: 1,
        backup_path: backup_path.to_path_buf(),
        backup_sha256: sha256_file(backup_path)?,
        installation_id: current.installation_id.clone(),
        organization_id: current.organization_id.clone(),
        preserved_commercial: current,
    };
    write_pending_restore(database_path, &pending)
}

/// Ejecuta una restauración que fue confirmada previamente y quedó marcada en
/// almacenamiento privado. Debe invocarse antes de abrir el pool de la app.
///
/// El marcador permanece hasta que todo el procedimiento termina. Si Android
/// mata el proceso entre etapas, el siguiente arranque vuelve a validar la
/// huella, identidad y snapshot. Repetir una promoción del mismo respaldo y
/// reponer el mismo estado comercial es idempotente.
pub async fn consume_pending_restore(
    database_path: &Path,
) -> Result<PendingRestoreOutcome, StorageError> {
    let path = pending_restore_path(database_path);
    if !path.is_file() {
        return Ok(PendingRestoreOutcome::None);
    }
    let bytes = std::fs::read(&path).map_err(|_| StorageError::PendingRestore)?;
    let pending: PendingRestore =
        serde_json::from_slice(&bytes).map_err(|_| StorageError::PendingRestore)?;
    if pending.version != 1
        || sha256_file(&pending.backup_path)? != pending.backup_sha256
        || !pending.backup_path.is_file()
    {
        return Err(StorageError::PendingRestore);
    }

    let current = read_current_installation(database_path).await?;
    // Solo la identidad debe permanecer: tras una interrupción entre la
    // promoción y la reposición del estado comercial, la base ya contiene el
    // snapshot y volver a comparar todo el estado impediría reanudar. La
    // operación completa (restaurar + reponer estado) es idempotente.
    if current.installation_id != pending.installation_id
        || current.organization_id != pending.organization_id
    {
        return Err(StorageError::PendingRestore);
    }
    let backup = read_backup_installation(&pending.backup_path).await?;
    validate_restore_policy(&current, &backup)?;

    let report = restore_from_backup(database_path, &pending.backup_path).await?;
    if let Err(error) =
        restore_preserved_commercial(database_path, &pending.preserved_commercial).await
    {
        // La restauración ya promovida no es válida si no se puede restablecer el
        // estado comercial anterior. Se intenta volver a la copia automática.
        if let Some(recovery) = report.recovery_copy
            && restore_from_backup(database_path, &recovery).await.is_err()
        {
            return Err(StorageError::RestoreCritical {
                database: database_path.display().to_string(),
                recovery: recovery.display().to_string(),
                staged: staged_path(database_path).display().to_string(),
            });
        }
        return Err(error);
    }

    // Borrar el marcador solo después de verificar que el estado comercial
    // vigente volvió a quedar intacto. El archivo importado también se elimina:
    // era una copia privada del respaldo, no un artefacto permanente.
    std::fs::remove_file(&path).map_err(|_| StorageError::Restore)?;
    let _ = std::fs::remove_file(&pending.backup_path);
    Ok(PendingRestoreOutcome::Restored)
}

pub(super) fn pending_restore_path(database_path: &Path) -> PathBuf {
    database_path.with_file_name("hiqnet-pos.restore-pending.json")
}

/// Directorio privado del adaptador SAF, derivado de la ubicación de la base.
pub fn saf_workspace(database_path: &Path, purpose: &str) -> PathBuf {
    database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("saf")
        .join(purpose)
}

/// Elimina temporales de exportación/importación que no pertenezcan a una
/// restauración pendiente. Se invoca al arrancar, después de consumir el
/// marcador: si el marcador existiera, no se llega aquí.
pub fn cleanup_saf_staging(database_path: &Path) {
    for purpose in ["exports", "imports"] {
        let Ok(entries) = std::fs::read_dir(saf_workspace(database_path, purpose)) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

pub(super) fn write_pending_restore(
    database_path: &Path,
    pending: &PendingRestore,
) -> Result<(), StorageError> {
    let path = pending_restore_path(database_path);
    let temporary = path.with_extension("json.tmp");
    let payload = serde_json::to_vec(pending).map_err(|_| StorageError::Restore)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Restore)?;
    }
    std::fs::write(&temporary, payload).map_err(|_| StorageError::Restore)?;
    sync_file(&temporary).map_err(|_| StorageError::Restore)?;
    std::fs::rename(&temporary, &path).map_err(|_| StorageError::Restore)?;
    sync_parent(&path);
    Ok(())
}
