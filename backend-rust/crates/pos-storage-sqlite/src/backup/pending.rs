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
    /// La UI importa una copia privada y puede eliminarla; la CLI recibe una
    /// ruta del operador y nunca debe borrar su archivo fuente.
    #[serde(default = "remove_imported_backup")]
    pub(super) remove_backup_after_restore: bool,
}

fn remove_imported_backup() -> bool {
    true
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
        backup_path: absolute_backup_path(backup_path)?,
        backup_sha256: sha256_file(backup_path)?,
        installation_id: current.installation_id.clone(),
        organization_id: current.organization_id.clone(),
        preserved_commercial: current,
        remove_backup_after_restore: true,
    };
    write_pending_restore(database_path, &pending)
}

/// Resuelve la ruta del respaldo a absoluta antes de guardarla. La reanudación
/// tras una caída ocurre en otro arranque, posiblemente con otro directorio de
/// trabajo; una ruta relativa dejaría el marcador irrecuperable.
fn absolute_backup_path(path: &Path) -> Result<PathBuf, StorageError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| StorageError::Restore)?
            .join(path)
    };
    Ok(absolute)
}

/// Restauración offline ordinaria desde la CLI: exige una instalación vigente
/// verificable y no consume el respaldo del operador. El bloqueo del proceso se
/// mantiene desde la validación hasta la reposición del estado comercial.
pub async fn restore_cli_backup(
    database_path: &Path,
    backup_path: &Path,
) -> Result<(), StorageError> {
    let process_lock = try_lock(database_path)?;
    if pending_restore_path(database_path).exists() {
        return Err(StorageError::PendingRestore);
    }
    // Una base ausente o corrupta no permite verificar identidad ni derechos:
    // se rechaza en lugar de improvisar una recuperación sin política.
    verify_backup(database_path)
        .await
        .map_err(|_| StorageError::Restore)?;
    let current = read_current_installation(database_path).await?;
    let backup = read_backup_installation(backup_path).await?;
    validate_restore_policy(&current, &backup)?;
    let pending = PendingRestore {
        version: 1,
        backup_path: absolute_backup_path(backup_path)?,
        backup_sha256: sha256_file(backup_path)?,
        installation_id: current.installation_id.clone(),
        organization_id: current.organization_id.clone(),
        preserved_commercial: current,
        remove_backup_after_restore: false,
    };
    write_pending_restore(database_path, &pending)?;
    consume_pending_restore_locked(database_path, &process_lock)
        .await
        .map(|_| ())
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
    let process_lock = try_lock(database_path)?;
    consume_pending_restore_locked(database_path, &process_lock).await
}

async fn consume_pending_restore_locked(
    database_path: &Path,
    process_lock: &DatabaseLock,
) -> Result<PendingRestoreOutcome, StorageError> {
    let path = pending_restore_path(database_path);
    if !path.is_file() {
        return Ok(PendingRestoreOutcome::None);
    }
    let bytes = std::fs::read(&path).map_err(|_| StorageError::PendingRestore)?;
    let pending: PendingRestore =
        serde_json::from_slice(&bytes).map_err(|_| StorageError::PendingRestore)?;
    if pending.version != 1
        || !pending.backup_path.is_absolute()
        || sha256_file(&pending.backup_path)? != pending.backup_sha256
        || !pending.backup_path.is_file()
    {
        return Err(StorageError::PendingRestore);
    }

    // Una sola exclusión durante validación, promoción y reposición comercial.
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

    // Ambas etapas son idempotentes: si el proceso cayó entre la promoción y la
    // reposición, repetir el mismo respaldo y el mismo estado comercial es
    // seguro. No se infiere la etapa comparando registros.
    let report =
        restore_from_backup_locked(database_path, &pending.backup_path, process_lock).await?;
    if let Err(error) = restore_preserved_commercial_locked(
        database_path,
        &pending.preserved_commercial,
        process_lock,
    )
    .await
    {
        // La restauración ya promovida no es válida si no se puede restablecer el
        // estado comercial anterior. Se intenta volver a la copia automática.
        if let Some(recovery) = report.recovery_copy
            && restore_from_backup_locked(database_path, &recovery, process_lock)
                .await
                .is_err()
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
    if pending.remove_backup_after_restore {
        let _ = std::fs::remove_file(&pending.backup_path);
    }
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
