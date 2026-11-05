use sqlx::sqlite::{SqliteConnectOptions, SqliteLockingMode, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::lock::try_lock;
use crate::{StorageError, embedded_migrations, latest_schema_version};

/// Tablas que existen desde la primera migración: cualquier base ajena a
/// HiQNet POS carece de ellas, pero un respaldo antiguo conserva su utilidad.
const CORE_TABLES: [&str; 6] = [
    "organization",
    "branch",
    "app_user",
    "product",
    "audit_event",
    "local_installation_state",
];

/// Resultado de una restauración completada.
#[derive(Debug, Clone)]
pub struct RestoreReport {
    /// Copia del estado previo, creada antes de reemplazar la base.
    pub recovery_copy: Option<PathBuf>,
}

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
    let target_text = target.to_string_lossy().to_string();
    sqlx::query("VACUUM INTO ?")
        .bind(target_text)
        .execute(pool)
        .await
        .map_err(|_| StorageError::Backup)?;

    if let Err(error) = verify_backup(target).await {
        let _ = std::fs::remove_file(target);
        return Err(error);
    }
    Ok(())
}

/// Comprueba que un archivo sea una base SQLite íntegra, del esquema esperado,
/// con historial de migraciones coherente y no más nueva que este ejecutable.
/// No modifica el archivo.
pub async fn verify_backup(path: &Path) -> Result<(), StorageError> {
    verify_structure(path).await?;
    let mut connection = open_read_only_connection(path).await?;
    verify_migration_history(&mut connection).await?;
    connection
        .close()
        .await
        .map_err(|_| StorageError::InvalidBackup)
}

/// Verificación estructural: integridad, tablas base y claves foráneas.
async fn verify_structure(path: &Path) -> Result<(), StorageError> {
    if !path.is_file() {
        return Err(StorageError::InvalidBackup);
    }

    let mut connection = open_read_only_connection(path).await?;

    let integrity = sqlx::query("PRAGMA integrity_check")
        .fetch_one(&mut connection)
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    let verdict: String = integrity.get(0);
    if verdict != "ok" {
        return Err(StorageError::InvalidBackup);
    }

    let tables = sqlx::query(
        "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'table' AND name IN \
         ('organization','branch','app_user','product','audit_event','local_installation_state')",
    )
    .fetch_one(&mut connection)
    .await
    .map_err(|_| StorageError::InvalidBackup)?;
    let total: i64 = tables.get("total");
    if total != CORE_TABLES.len() as i64 {
        return Err(StorageError::InvalidBackup);
    }

    // Un respaldo con referencias rotas no debe aceptarse como recuperación.
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut connection)
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    if !violations.is_empty() {
        return Err(StorageError::InvalidBackup);
    }

    connection
        .close()
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    Ok(())
}

async fn open_read_only_connection(path: &Path) -> Result<sqlx::SqliteConnection, StorageError> {
    SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .foreign_keys(true)
        .connect()
        .await
        .map_err(|_| StorageError::InvalidBackup)
}

/// Un historial con huecos, migraciones fallidas o checksums distintos indica
/// una base manipulada, incompleta o incompatible con este ejecutable.
async fn verify_migration_history(
    connection: &mut sqlx::SqliteConnection,
) -> Result<(), StorageError> {
    let applied =
        sqlx::query("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut *connection)
            .await
            .map_err(|_| StorageError::InvalidBackup)?;

    if applied.is_empty() {
        return Err(StorageError::InvalidBackup);
    }

    let supported = embedded_migrations();

    let mut previous: i64 = 0;
    for row in &applied {
        let version: i64 = row.get("version");
        let success: i64 = row
            .try_get("success")
            .map_err(|_| StorageError::InvalidBackup)?;
        if success != 1 {
            return Err(StorageError::InvalidBackup);
        }
        if version > latest_schema_version() {
            return Err(StorageError::IncompatibleSchema);
        }
        // Un salto en la numeración significa historial incompleto o alterado.
        if version != previous + 1 {
            return Err(StorageError::InvalidBackup);
        }
        // Las migraciones publicadas nunca cambian, así que el checksum debe
        // coincidir byte a byte con la migración embebida.
        let stored: Vec<u8> = row.get("checksum");
        let known = supported
            .iter()
            .find(|migration| migration.version == version)
            .ok_or(StorageError::InvalidBackup)?;
        if known.checksum.as_ref() != stored.as_slice() {
            return Err(StorageError::InvalidBackup);
        }
        previous = version;
    }
    Ok(())
}

/// Verifica que un archivo sea un SQLite legible e íntegro, sin exigir el
/// esquema de HiQNet POS. Se usa para la copia de recuperación, que debe servir
/// incluso si la base previa tenía una migración fallida.
async fn verify_snapshot(path: &Path) -> Result<(), StorageError> {
    if !path.is_file() {
        return Err(StorageError::Restore);
    }
    let mut connection = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .connect()
        .await
        .map_err(|_| StorageError::Restore)?;
    let verdict: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut connection)
        .await
        .map_err(|_| StorageError::Restore)?;
    connection
        .close()
        .await
        .map_err(|_| StorageError::Restore)?;
    if verdict != "ok" {
        return Err(StorageError::Restore);
    }
    Ok(())
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

struct PromotionFailure {
    error: StorageError,
    /// Indica si la base destino pudo quedar modificada y requiere reversión.
    destination_touched: bool,
}

/// Estado previo de la base destino.
struct PreparedDestination {
    /// Conexión que mantiene el bloqueo exclusivo hasta la promoción.
    guard: Option<sqlx::SqliteConnection>,
    recovery: Option<PathBuf>,
}

/// Prepara el destino: bloqueo exclusivo real, volcado del WAL y copia de
/// recuperación verificada. Si algo falla, el destino no se modifica.
///
/// La conexión de guardia **no se cierra aquí**: se conserva para que ninguna
/// conexión externa pueda escribir entre el checkpoint y el reemplazo.
async fn prepare_destination(database_path: &Path) -> Result<PreparedDestination, StorageError> {
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
async fn destination_state(path: &Path) -> StorageError {
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
fn wal_has_data(database_path: &Path) -> bool {
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
fn quarantine_sidecars(database_path: &Path, allow_data: bool) -> Result<(), StorageError> {
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
async fn promote_and_verify(
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
async fn rollback_to(database_path: &Path, recovery: &Path) -> Result<(), StorageError> {
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

fn sync_file(path: &Path) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.sync_all()
}

#[cfg(unix)]
fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent()
        && let Ok(directory) = std::fs::File::open(parent)
    {
        let _ = directory.sync_all();
    }
}

/// En Windows no existe un `fsync` de directorio equivalente accesible desde
/// `std`; la durabilidad del renombrado depende del sistema de archivos.
#[cfg(not(unix))]
fn sync_parent(_path: &Path) {}

/// Archivo temporal que se elimina al salir salvo que se conserve a propósito.
struct StagedFile {
    path: PathBuf,
    keep: bool,
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

fn staged_path(database_path: &Path) -> PathBuf {
    let name = database_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "hiqnet-pos.db".into());
    database_path.with_file_name(format!("{name}.restore-tmp"))
}

/// Ruta libre para la copia de recuperación. Usa milisegundos y, si aún
/// colisiona, añade un sufijo: nunca sobrescribe una copia anterior.
fn recovery_path(database_path: &Path) -> PathBuf {
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

/// Abre un pool auxiliar de solo lectura para pruebas de restauración.
pub async fn open_read_only(path: &Path) -> Result<SqlitePool, StorageError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| StorageError::Open)
}

// --- Puntos de fallo para pruebas -------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum RestoreStep {
    RecoveryCopy,
    StagedCopy,
    StagedValidation,
    Promotion,
    FinalVerification,
    Rollback,
}

/// En compilaciones normales los puntos de fallo no existen.
#[inline]
fn fault(_step: RestoreStep) -> bool {
    #[cfg(test)]
    {
        faults::take(_step)
    }
    #[cfg(not(test))]
    {
        false
    }
}

#[cfg(test)]
mod faults {
    use super::RestoreStep;
    use std::sync::atomic::{AtomicU32, Ordering};

    static INJECTED: AtomicU32 = AtomicU32::new(0);

    fn mask(step: RestoreStep) -> u32 {
        match step {
            RestoreStep::RecoveryCopy => 1 << 0,
            RestoreStep::StagedCopy => 1 << 1,
            RestoreStep::StagedValidation => 1 << 2,
            RestoreStep::Promotion => 1 << 3,
            RestoreStep::FinalVerification => 1 << 4,
            RestoreStep::Rollback => 1 << 5,
        }
    }

    /// Permite encadenar fallos (por ejemplo, verificación final y reversión).
    pub(super) fn inject(steps: &[RestoreStep]) {
        let mut bits = 0;
        for step in steps {
            bits |= mask(*step);
        }
        INJECTED.store(bits, Ordering::SeqCst);
    }

    pub(super) fn take(step: RestoreStep) -> bool {
        let bit = mask(step);
        let mut current = INJECTED.load(Ordering::SeqCst);
        loop {
            if current & bit == 0 {
                return false;
            }
            match INJECTED.compare_exchange(
                current,
                current & !bit,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(observed) => current = observed,
            }
        }
    }
}

#[cfg(test)]
mod race {
    //! Gancho exclusivo de pruebas para inspeccionar el intervalo entre el
    //! checkpoint y la promoción. No existe en compilaciones normales.
    use std::sync::Mutex;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Point {
        /// Con el bloqueo exclusivo todavía tomado.
        BeforePromotion,
        /// Después de soltar la guardia y apartar sidecars, justo antes de
        /// reemplazar el archivo: la ventana mínima del procedimiento.
        BeforePromotionUnlocked,
    }

    type Hook = Box<dyn FnOnce() + Send>;

    static HOOK: Mutex<Option<(Point, Hook)>> = Mutex::new(None);

    pub fn arm(point: Point, hook: Hook) {
        *HOOK.lock().unwrap_or_else(|poison| poison.into_inner()) = Some((point, hook));
    }

    pub fn hit(point: Point) {
        let hook = {
            let mut slot = HOOK.lock().unwrap_or_else(|poison| poison.into_inner());
            match slot.as_ref() {
                Some((armed, _)) if *armed == point => slot.take().map(|(_, hook)| hook),
                _ => None,
            }
        };
        if let Some(hook) = hook {
            hook();
        }
    }
}

#[cfg(test)]
mod failure_tests {
    // La serialización es intencional: el inyector de fallos es global y las
    // pruebas de este módulo comparten el mismo proceso.
    #![allow(clippy::await_holding_lock)]

    use super::*;
    use crate::open;
    use std::sync::Mutex;

    /// Las pruebas comparten el inyector global: se serializan entre sí.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    async fn pending_count(path: &Path) -> i64 {
        let pool = open(path).await.unwrap();
        let value: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product")
            .fetch_one(&pool)
            .await
            .unwrap();
        pool.close().await;
        value
    }

    async fn scenario() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("hiqnet-pos.db");
        let backup = dir.path().join("respaldo.db");

        let pool = open(&database).await.unwrap();
        sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1', 'Org', '2026-01-01T00:00:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        for id in ["p-1", "p-2"] {
            sqlx::query(
                "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
                 VALUES (?, 'org-1', ?, ?, 100, 0, 1, '2026-01-01T00:00:00Z')",
            )
            .bind(id)
            .bind(id)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        }
        backup_to(&pool, &backup).await.unwrap();

        // Cambio posterior al respaldo: la restauración debe deshacerlo.
        sqlx::query(
            "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
             VALUES ('p-3', 'org-1', 'P-3', 'P-3', 100, 0, 1, '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        (dir, database, backup)
    }

    fn staged(database: &Path) -> PathBuf {
        staged_path(database)
    }

    #[tokio::test]
    async fn fallo_al_crear_la_copia_de_recuperacion_no_toca_la_base() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        // Fallo simulado: el snapshot no queda disponible para verificarse.
        faults::inject(&[RestoreStep::RecoveryCopy]);
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
        assert_eq!(pending_count(&database).await, 3);
    }

    #[tokio::test]
    async fn fallo_al_copiar_el_temporal_deja_la_base_intacta() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        // El temporal existe como directorio: la copia no puede completarse.
        std::fs::create_dir_all(staged(&database)).unwrap();
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
        assert_eq!(pending_count(&database).await, 3);
    }

    #[tokio::test]
    async fn fallo_durante_la_copia_del_temporal_limpia_el_archivo() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        faults::inject(&[RestoreStep::StagedCopy]);
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
        assert_eq!(pending_count(&database).await, 3);
        assert!(!staged(&database).exists(), "sin archivos temporales");
    }

    #[tokio::test]
    async fn fallo_al_validar_el_temporal_deja_la_base_intacta() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        faults::inject(&[RestoreStep::StagedValidation]);
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(
            matches!(error, StorageError::InvalidBackup | StorageError::Restore),
            "obtenido {error:?}"
        );
        assert_eq!(pending_count(&database).await, 3);
        assert!(!staged(&database).exists(), "el temporal se elimina");
    }

    #[tokio::test]
    async fn fallo_al_promover_deja_la_base_intacta() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        faults::inject(&[RestoreStep::Promotion]);
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
        assert_eq!(pending_count(&database).await, 3);
        assert!(!staged(&database).exists(), "el temporal se elimina");
    }

    #[tokio::test]
    async fn fallo_en_la_verificacion_final_revierte_a_la_copia_previa() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        faults::inject(&[RestoreStep::FinalVerification]);
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(
            matches!(error, StorageError::InvalidBackup | StorageError::Restore),
            "obtenido {error:?}"
        );
        assert_eq!(
            pending_count(&database).await,
            3,
            "la reversión devuelve el estado previo completo"
        );
    }

    #[tokio::test]
    async fn fallo_de_reversion_conserva_artefactos_y_avisa() {
        let _serial = serial();
        let (dir, database, backup) = scenario().await;

        faults::inject(&[RestoreStep::FinalVerification, RestoreStep::Rollback]);
        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        let StorageError::RestoreCritical {
            database: base,
            recovery,
            staged,
        } = error
        else {
            panic!("se esperaba RestoreCritical, obtenido {error:?}");
        };

        // Todo lo necesario para recuperar a mano sigue presente.
        assert!(Path::new(&base).is_file(), "la base se conserva");
        assert!(
            Path::new(&recovery).is_file(),
            "la copia de recuperación se conserva"
        );
        assert!(
            Path::new(&staged).is_file(),
            "el temporal validado se conserva"
        );
        assert_eq!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains("pre-restore"))
                .count(),
            1,
            "una sola copia de recuperación"
        );
        // La copia de recuperación sí es utilizable.
        verify_snapshot(Path::new(&recovery)).await.unwrap();
    }

    #[tokio::test]
    async fn restauracion_correcta_deja_el_estado_respaldado() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        let report = restore_from_backup(&database, &backup).await.unwrap();
        assert!(
            report
                .recovery_copy
                .as_ref()
                .is_some_and(|path| path.exists())
        );
        assert_eq!(pending_count(&database).await, 2);
    }

    #[tokio::test]
    async fn restaura_aunque_un_lector_previo_deje_sidecars() {
        let _serial = serial();
        let (dir, database, backup) = scenario().await;

        // Un lector de solo lectura puede dejar `-wal`/`-shm` sin mantener la
        // base en uso: su presencia no debe interpretarse como conexión activa.
        let mut reader = SqliteConnectOptions::new()
            .filename(&database)
            .read_only(true)
            .connect()
            .await
            .unwrap();
        sqlx::query("SELECT COUNT(*) FROM product")
            .fetch_one(&mut reader)
            .await
            .unwrap();
        reader.close().await.unwrap();

        restore_from_backup(&database, &backup).await.unwrap();
        assert_eq!(pending_count(&database).await, 2);

        // Los residuos se conservan apartados, no se destruyen.
        let quarantined: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".wal.pre-restore-") || name.contains(".shm.pre-restore-"))
            .collect();
        for name in quarantined {
            let path = dir.path().join(&name);
            if name.contains(".wal.pre-restore-") {
                assert_eq!(
                    std::fs::metadata(&path).unwrap().len(),
                    0,
                    "solo se aparta un WAL ya vaciado"
                );
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn otra_conexion_no_puede_escribir_entre_checkpoint_y_promocion() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        let (arrived_tx, arrived_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel();
        race::arm(
            race::Point::BeforePromotion,
            Box::new(move || {
                let _ = arrived_tx.send(());
                let _ = go_rx.recv_timeout(Duration::from_secs(10));
            }),
        );

        let writer_database = database.clone();
        let writer = std::thread::spawn(move || {
            // Espera a que la utilidad esté a punto de promover.
            let _ = arrived_rx.recv_timeout(Duration::from_secs(10));
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let outcome = runtime.block_on(async move {
                let mut connection = SqliteConnectOptions::new()
                    .filename(&writer_database)
                    .busy_timeout(Duration::from_millis(500))
                    .connect()
                    .await
                    .map_err(|_| "conexion rechazada")?;
                let result = sqlx::query(
                    "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at)                      VALUES ('intruso', 'org-1', 'INT', 'Intruso', 100, 0, 1, '2026-01-01T00:00:00Z')",
                )
                .execute(&mut connection)
                .await
                .map_err(|_| "escritura rechazada");
                let _ = connection.close().await;
                result.map(|_| ())
            });
            let _ = go_tx.send(());
            outcome
        });

        let report = restore_from_backup(&database, &backup).await;
        let intento = writer.join().expect("hilo del intruso");

        // La conexión externa no puede escribir mientras se sostiene el bloqueo
        // exclusivo durante la copia y la validación.
        assert!(
            intento.is_err(),
            "una conexión externa escribió entre el checkpoint y la promoción"
        );
        match report {
            Ok(_) => {}
            Err(error) => assert!(
                matches!(error, StorageError::DatabaseInUse | StorageError::Restore),
                "error inesperado: {error:?}"
            ),
        }

        verify_backup(&database).await.unwrap();
        assert_eq!(
            pending_count(&database).await,
            2,
            "estado del respaldo íntegro"
        );
        assert!(
            !wal_has_data(&database),
            "quedó un WAL con datos junto a la base promovida"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn escritura_externa_en_la_ventana_aborta_sin_perder_datos() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        // En la ventana la guardia ya se soltó: una conexión externa sí puede
        // entrar. La promoción debe abortar y sus datos confirmados sobrevivir.
        let (written_tx, written_rx) = std::sync::mpsc::channel();
        race::arm(
            race::Point::BeforePromotionUnlocked,
            Box::new(move || {
                let _ = written_rx.recv_timeout(Duration::from_secs(10));
            }),
        );

        let writer_database = database.clone();
        let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();
        let writer = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let mut connection = SqliteConnectOptions::new()
                    .filename(&writer_database)
                    .busy_timeout(Duration::from_secs(2))
                    .connect()
                    .await
                    .expect("conexión externa");
                sqlx::query(
                    "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at)                      VALUES ('intruso', 'org-1', 'INT', 'Intruso', 100, 0, 1, '2026-01-01T00:00:00Z')",
                )
                .execute(&mut connection)
                .await
                .expect("escritura externa confirmada");
                let _ = written_tx.send(());
                // Mantiene el WAL presente mientras la utilidad intenta promover.
                let _ = hold_rx.recv_timeout(Duration::from_secs(10));
                let _ = connection.close().await;
            });
        });

        let report = restore_from_backup(&database, &backup).await;
        let _ = hold_tx.send(());
        writer.join().expect("hilo del escritor externo");

        // El WAL con datos externos impide la promoción: la base original queda
        // intacta y la escritura confirmada no se pierde.
        assert!(
            matches!(report, Err(StorageError::DatabaseInUse)),
            "se esperaba rechazo por base en uso, obtenido {report:?}"
        );
        verify_backup(&database).await.unwrap();
        assert_eq!(
            pending_count(&database).await,
            4,
            "la escritura externa confirmada se conserva"
        );
        assert!(!wal_has_data(&database), "el WAL se integró al cerrar");
    }

    #[tokio::test]
    async fn rechaza_restaurar_con_lector_externo_activo() {
        let _serial = serial();
        let (_dir, database, backup) = scenario().await;

        // Conexión externa con lectura abierta: en WAL no bloquea escritores,
        // por lo que solo el checkpoint TRUNCATE la detecta.
        let reader = SqliteConnectOptions::new()
            .filename(&database)
            .connect()
            .await
            .unwrap();
        let mut reader = reader;
        sqlx::query("BEGIN").execute(&mut reader).await.unwrap();
        sqlx::query("SELECT COUNT(*) FROM product")
            .fetch_one(&mut reader)
            .await
            .unwrap();

        let error = restore_from_backup(&database, &backup).await.unwrap_err();
        assert!(
            matches!(error, StorageError::DatabaseInUse),
            "obtenido {error:?}"
        );
        assert_eq!(pending_count(&database).await, 3, "la base no se modifica");

        sqlx::query("COMMIT").execute(&mut reader).await.unwrap();
        reader.close().await.unwrap();
        restore_from_backup(&database, &backup).await.unwrap();
        assert_eq!(pending_count(&database).await, 2);
    }
}
