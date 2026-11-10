//! Restauracion atomica: preparacion, promocion y reversion.

use super::*;

/// Resultado de una restauración completada.
#[derive(Debug, Clone)]
pub struct RestoreReport {
    /// Copia del estado previo, creada antes de reemplazar la base.
    pub recovery_copy: Option<PathBuf>,
}

/// Restaura un respaldo sobre `database_path`.
///
/// Procedimiento para uso **offline**, con la aplicación cerrada:
///
/// 1. verifica el respaldo (integridad, tablas, claves foráneas, historial de
///    migraciones y versión de esquema);
/// 2. toma el bloqueo cooperativo del proceso, que impide que otra instancia de
///    HiQNet POS abra la base durante el procedimiento;
/// 3. abre la base destino en modo de bloqueo exclusivo y ejecuta
///    `PRAGMA wal_checkpoint(TRUNCATE)`. En WAL, un lector no bloquea a un
///    escritor, así que `BEGIN EXCLUSIVE` no sirve para detectarlo: el
///    checkpoint TRUNCATE sí falla con `busy` mientras exista cualquier otra
///    conexión y además vuelca el WAL en el archivo principal, de modo que
///    ningún dato pendiente se pierde;
/// 4. crea la copia de recuperación con `VACUUM INTO`, ya con el WAL volcado, y
///    la valida por integridad;
/// 5. comprueba que el WAL quedó vacío y **aparta** `-wal`/`-shm` en lugar de
///    borrarlos: un lector de solo lectura puede dejar residuos sin mantener la
///    base abierta, y un WAL con páginas pendientes obliga a abortar;
/// 6. prepara `<base>.restore-tmp` con el respaldo, lo sincroniza a disco y lo
///    verifica antes de promoverlo;
/// 7. promueve el temporal sobre la base con un reemplazo de entrada de
///    directorio y verifica el resultado;
/// 8. si la verificación final o la promoción fallan, vuelve a la copia de
///    recuperación y verifica esa reversión. Si también falla, devuelve
///    [`StorageError::RestoreCritical`] conservando base, copia y temporal para
///    recuperación manual.
///
/// Garantías reales: la restauración es segura frente a otra instancia de
/// HiQNet POS, frente a conexiones SQLite externas y frente a fallos de E/S
/// detectables. No es una transacción del sistema de archivos: un corte de
/// energía entre la promoción y la verificación puede dejar la base en el
/// estado del respaldo sin confirmación, y por eso siempre queda una copia de
/// recuperación junto a la base.
pub async fn restore_from_backup(
    database_path: &Path,
    backup_path: &Path,
) -> Result<RestoreReport, StorageError> {
    verify_backup(backup_path).await?;

    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Restore)?;
    }

    let _process_lock = try_lock(database_path)?;

    // 1. Estado previo: checkpoint seguro, copia de recuperación y bloqueo
    //    exclusivo que se mantiene hasta después del reemplazo.
    let mut prepared = prepare_destination(database_path).await?;
    let recovery = prepared.recovery.clone();

    // 2. Promoción desde el respaldo verificado.
    let mut staged = StagedFile::new(staged_path(database_path));
    let outcome = promote_and_verify(
        database_path,
        backup_path,
        &staged,
        true,
        &mut prepared.guard,
    )
    .await;
    if let Some(connection) = prepared.guard.take() {
        let _ = connection.close().await;
    }

    match outcome {
        Ok(()) => Ok(RestoreReport {
            recovery_copy: recovery,
        }),
        Err(failure) => {
            // Si la base original nunca se tocó, no hay nada que revertir.
            let Some(recovery_path) = recovery else {
                return Err(failure.error);
            };
            if !failure.destination_touched {
                // El destino sigue intacto: la copia de recuperación es
                // redundante y se descarta para no acumular artefactos.
                let _ = std::fs::remove_file(&recovery_path);
                return Err(failure.error);
            }

            match rollback_to(database_path, &recovery_path).await {
                Ok(()) => Err(failure.error),
                Err(_) => {
                    // Recuperación manual: la copia de recuperación es el
                    // artefacto esencial. Si es válida se duplica en el
                    // temporal para que soporte pueda promoverla con la misma
                    // utilidad; si no, se informa sin borrar nada.
                    let staged_path = staged.path().to_path_buf();
                    staged.keep();
                    let staged_artifact = if std::fs::copy(&recovery_path, &staged_path).is_ok()
                        && verify_snapshot(&staged_path).await.is_ok()
                    {
                        staged_path.display().to_string()
                    } else {
                        let _ = std::fs::remove_file(&staged_path);
                        "no disponible".to_string()
                    };
                    Err(StorageError::RestoreCritical {
                        database: database_path.display().to_string(),
                        recovery: recovery_path.display().to_string(),
                        staged: staged_artifact,
                    })
                }
            }
        }
    }
}

pub(super) struct PromotionFailure {
    pub(super) error: StorageError,
    /// Indica si la base destino pudo quedar modificada y requiere reversión.
    pub(super) destination_touched: bool,
}

/// Estado previo de la base destino.
pub(super) struct PreparedDestination {
    /// Conexión que mantiene el bloqueo exclusivo hasta la promoción.
    pub(super) guard: Option<sqlx::SqliteConnection>,
    pub(super) recovery: Option<PathBuf>,
}

/// Prepara el destino: bloqueo exclusivo real, volcado del WAL y copia de
/// recuperación verificada. Si algo falla, el destino no se modifica.
///
/// La conexión de guardia **no se cierra aquí**: se conserva para que ninguna
/// conexión externa pueda escribir entre el checkpoint y el reemplazo.
pub(super) async fn prepare_destination(
    database_path: &Path,
) -> Result<PreparedDestination, StorageError> {
    if !database_path.is_file() {
        return Ok(PreparedDestination {
            guard: None,
            recovery: None,
        });
    }

    let mut guard = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(3))
        // Mantiene el bloqueo mientras dure la conexión: impide que otra
        // conexión se una a mitad del procedimiento.
        .locking_mode(SqliteLockingMode::Exclusive)
        .connect()
        .await
        .map_err(|_| StorageError::DatabaseInUse)?;

    // En WAL los lectores no bloquean a los escritores, así que `BEGIN
    // EXCLUSIVE` no detecta un lector externo. `wal_checkpoint(TRUNCATE)` sí:
    // devuelve `busy = 1` mientras exista cualquier otra conexión con el WAL
    // abierto, y además deja el archivo principal al día.
    let checkpoint = match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut guard)
        .await
    {
        Ok(row) => row,
        Err(_) => {
            let _ = guard.close().await;
            return Err(destination_state(database_path).await);
        }
    };
    let busy: i64 = checkpoint.try_get(0).unwrap_or(1);
    if busy != 0 {
        let _ = guard.close().await;
        return Err(StorageError::DatabaseInUse);
    }

    let path = recovery_path(database_path);
    let target = path.to_string_lossy().to_string();
    let snapshot = sqlx::query("VACUUM INTO ?")
        .bind(target)
        .execute(&mut guard)
        .await;
    if snapshot.is_err() {
        let _ = guard.close().await;
        return Err(StorageError::Restore);
    }
    if fault(RestoreStep::RecoveryCopy) {
        let _ = std::fs::remove_file(&path);
    }

    if wal_has_data(database_path) {
        let _ = guard.close().await;
        let _ = std::fs::remove_file(&path);
        return Err(StorageError::DatabaseInUse);
    }

    if let Err(error) = verify_snapshot(&path).await {
        let _ = guard.close().await;
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }

    // El bloqueo exclusivo sigue tomado: el WAL está vacío y nadie más puede
    // escribir hasta que se cierre esta conexión.
    Ok(PreparedDestination {
        guard: Some(guard),
        recovery: Some(path),
    })
}

/// Distingue una base dañada de una base en uso cuando el checkpoint falla.
pub(super) async fn destination_state(path: &Path) -> StorageError {
    match SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .connect()
        .await
    {
        Ok(mut connection) => {
            let verdict: Result<String, _> = sqlx::query_scalar("PRAGMA quick_check")
                .fetch_one(&mut connection)
                .await;
            let _ = connection.close().await;
            match verdict {
                Ok(value) if value != "ok" => StorageError::Corrupt,
                _ => StorageError::DatabaseInUse,
            }
        }
        Err(_) => StorageError::DatabaseInUse,
    }
}

/// Un WAL con tamaño distinto de cero puede contener páginas no volcadas: no
/// se reemplaza la base hasta haberlo vaciado con el checkpoint.
pub(super) fn wal_has_data(database_path: &Path) -> bool {
    let mut candidate = database_path.as_os_str().to_owned();
    candidate.push("-wal");
    let path = PathBuf::from(candidate);
    std::fs::metadata(&path)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false)
}

/// Aparta `-wal` y `-shm` en lugar de borrarlos: conserva cualquier residuo
/// para análisis y evita que el archivo recién promovido herede páginas del
/// anterior.
///
/// `allow_data` distingue los dos momentos: antes de reemplazar, un WAL con
/// páginas pendientes obliga a abortar (`false`); después de reemplazar, ese
/// WAL pertenece a la base anterior y debe apartarse para que nunca quede junto
/// al archivo nuevo (`true`).
pub(super) fn quarantine_sidecars(
    database_path: &Path,
    allow_data: bool,
) -> Result<(), StorageError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let name = database_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "hiqnet-pos.db".into());

    for (suffix, label) in [("-wal", "wal"), ("-shm", "shm")] {
        let mut candidate = database_path.as_os_str().to_owned();
        candidate.push(suffix);
        let path = PathBuf::from(candidate);
        if !path.exists() {
            continue;
        }
        if suffix == "-wal" && !allow_data {
            let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            if size > 0 {
                return Err(StorageError::DatabaseInUse);
            }
        }
        let target = database_path.with_file_name(format!("{name}.{label}.pre-restore-{stamp}"));
        std::fs::rename(&path, &target).map_err(|_| StorageError::Restore)?;
    }
    Ok(())
}

/// Copia el origen al temporal, lo sincroniza, lo valida y lo promueve.
///
/// `guard` mantiene el bloqueo exclusivo de SQLite: se conserva hasta después
/// del reemplazo para que ninguna conexión externa pueda escribir entre el
/// checkpoint y la promoción.
pub(super) async fn promote_and_verify(
    database_path: &Path,
    source: &Path,
    staged: &StagedFile,
    strict: bool,
    guard: &mut Option<sqlx::SqliteConnection>,
) -> Result<(), PromotionFailure> {
    let fail = |error: StorageError, destination_touched: bool| PromotionFailure {
        error,
        destination_touched,
    };

    std::fs::copy(source, staged.path()).map_err(|_| fail(StorageError::Restore, false))?;
    if fault(RestoreStep::StagedCopy) {
        let _ = std::fs::remove_file(staged.path());
    }
    sync_file(staged.path()).map_err(|_| fail(StorageError::Restore, false))?;
    if fault(RestoreStep::StagedValidation) {
        let _ = std::fs::write(staged.path(), b"temporal corrupto");
    }

    let validation = if strict {
        verify_backup(staged.path()).await
    } else {
        verify_snapshot(staged.path()).await
    };
    validation.map_err(|error| fail(error, false))?;

    if fault(RestoreStep::Promotion) {
        let _ = std::fs::remove_file(staged.path());
    }

    // Un WAL con páginas pendientes jamás debe separarse del archivo principal.
    if wal_has_data(database_path) {
        return Err(fail(StorageError::DatabaseInUse, false));
    }

    #[cfg(test)]
    race::hit(race::Point::BeforePromotion);

    // Reemplazo de la entrada de directorio. En Unix es atómico frente a otros
    // procesos; en Windows es `MoveFileEx` con `REPLACE_EXISTING`, que tampoco
    // es una transacción del sistema de archivos.
    //
    // Se intenta con el bloqueo exclusivo tomado. Si el sistema de archivos no
    // permite reemplazar un archivo abierto, se cierra la guardia y se apartan
    // los sidecars antes de reintentar; esa ventana queda documentada y es la
    // única en la que otra conexión podría incorporarse.
    let mut promoted = std::fs::rename(staged.path(), database_path).is_ok();
    if !promoted {
        // Windows deniega reemplazar un archivo con un handle abierto, incluso
        // con `FILE_SHARE_DELETE`. Hay que soltar el bloqueo y aceptar una
        // ventana mínima, protegida por las comprobaciones inmediatas.
        if let Some(connection) = guard.take() {
            let _ = connection.close().await;
        }
        #[cfg(test)]
        race::hit(race::Point::BeforePromotionUnlocked);
        // Si en la ventana apareció un WAL con datos, pertenece a una escritura
        // externa: la base principal está intacta y se aborta sin tocar nada.
        quarantine_sidecars(database_path, false).map_err(|error| fail(error, false))?;
        promoted = std::fs::rename(staged.path(), database_path).is_ok();
        if !promoted {
            return Err(fail(StorageError::Restore, false));
        }
    }

    if let Some(connection) = guard.take() {
        let _ = connection.close().await;
    }
    // Cualquier sidecar que haya aparecido durante la promoción pertenece a la
    // base anterior: se aparta siempre, aunque contenga datos, para que nunca
    // quede un WAL separado del archivo principal al que corresponde.
    quarantine_sidecars(database_path, true)
        .map_err(|error| fail(error, database_path.is_file()))?;
    sync_parent(database_path);

    if fault(RestoreStep::FinalVerification) {
        let _ = std::fs::write(database_path, b"promocion corrupta");
    }

    let validation = if strict {
        verify_backup(database_path).await
    } else {
        verify_snapshot(database_path).await
    };
    validation.map_err(|error| fail(error, true))
}

/// Devuelve la base al estado previo y comprueba que quedó íntegra.
pub(super) async fn rollback_to(database_path: &Path, recovery: &Path) -> Result<(), StorageError> {
    if fault(RestoreStep::Rollback) {
        return Err(StorageError::Restore);
    }
    let staged = StagedFile::new(staged_path(database_path));
    let mut guard = None;
    promote_and_verify(database_path, recovery, &staged, false, &mut guard)
        .await
        .map_err(|failure| failure.error)?;
    verify_snapshot(database_path).await
}

pub(super) fn sync_file(path: &Path) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.sync_all()
}

#[cfg(unix)]
pub(super) fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent()
        && let Ok(directory) = std::fs::File::open(parent)
    {
        let _ = directory.sync_all();
    }
}

/// En Windows no existe un `fsync` de directorio equivalente accesible desde
/// `std`; la durabilidad del renombrado depende del sistema de archivos.
#[cfg(not(unix))]
pub(super) fn sync_parent(_path: &Path) {}

/// Archivo temporal que se elimina al salir salvo que se conserve a propósito.
pub(super) struct StagedFile {
    pub(super) path: PathBuf,
    pub(super) keep: bool,
}

impl StagedFile {
    fn new(path: PathBuf) -> Self {
        if path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
        Self { path, keep: false }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(super) fn staged_path(database_path: &Path) -> PathBuf {
    let name = database_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "hiqnet-pos.db".into());
    database_path.with_file_name(format!("{name}.restore-tmp"))
}

/// Ruta libre para la copia de recuperación. Usa milisegundos y, si aún
/// colisiona, añade un sufijo: nunca sobrescribe una copia anterior.
pub(super) fn recovery_path(database_path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let name = database_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "hiqnet-pos.db".into());

    for attempt in 0..100 {
        let candidate = if attempt == 0 {
            format!("{name}.pre-restore-{stamp}.db")
        } else {
            format!("{name}.pre-restore-{stamp}-{attempt}.db")
        };
        let path = database_path.with_file_name(candidate);
        if !path.exists() {
            return path;
        }
    }
    database_path.with_file_name(format!("{name}.pre-restore-{stamp}-final.db"))
}
