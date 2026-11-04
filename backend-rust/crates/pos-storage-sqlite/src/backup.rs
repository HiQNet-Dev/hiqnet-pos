use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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
    if !path.is_file() {
        return Err(StorageError::InvalidBackup);
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .foreign_keys(true);
    let mut connection = options
        .connect()
        .await
        .map_err(|_| StorageError::InvalidBackup)?;

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

    verify_migration_history(&mut connection).await?;

    connection
        .close()
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    Ok(())
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

/// Restaura un respaldo sobre `database_path`.
///
/// Procedimiento seguro para uso **offline**, con la aplicación cerrada:
///
/// 1. verifica el respaldo (integridad, tablas, claves foráneas, historial y
///    versión de esquema);
/// 2. toma el bloqueo exclusivo del proceso; si otra instancia de HiQNet POS
///    mantiene la base, la restauración se rechaza sin tocar archivos;
/// 3. sondea SQLite con `BEGIN EXCLUSIVE` para descartar transacciones ajenas
///    abiertas por otras herramientas;
/// 4. crea y verifica una copia de recuperación `<base>.pre-restore-<epoch>.db`
///    con `VACUUM INTO`, que incluye el estado del WAL;
/// 5. copia el respaldo a `<base>.restore-tmp`, lo verifica y solo entonces lo
///    mueve sobre la base, de modo que nunca queda una base a medio escribir;
/// 6. verifica el resultado y, si falla, revierte a la copia de recuperación.
pub async fn restore_from_backup(
    database_path: &Path,
    backup_path: &Path,
) -> Result<(), StorageError> {
    verify_backup(backup_path).await?;

    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Restore)?;
    }

    // El bloqueo se mantiene durante todo el reemplazo: impide que la
    // aplicación vuelva a abrir la base a mitad del procedimiento.
    let _process_lock = try_lock(database_path)?;

    let mut recovery: Option<PathBuf> = None;
    if database_path.is_file() {
        let options = SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(false)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(2));
        let mut connection = options.connect().await.map_err(|_| StorageError::Restore)?;

        // `BEGIN EXCLUSIVE` falla si otro proceso mantiene una lectura o
        // escritura; se rechaza en lugar de reemplazar una base en uso.
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut connection)
            .await
            .map_err(|_| StorageError::DatabaseInUse)?;
        sqlx::query("ROLLBACK")
            .execute(&mut connection)
            .await
            .map_err(|_| StorageError::DatabaseInUse)?;

        let path = recovery_path(database_path);
        let target = path.to_string_lossy().to_string();
        sqlx::query("VACUUM INTO ?")
            .bind(target)
            .execute(&mut connection)
            .await
            .map_err(|_| StorageError::Restore)?;
        connection
            .close()
            .await
            .map_err(|_| StorageError::Restore)?;

        // La copia de recuperación es una salida de emergencia, no un respaldo
        // distribuible: si la base está dañada o con el historial incompleto,
        // exigir el esquema completo impediría recuperarla. Basta con que sea
        // un SQLite íntegro.
        verify_snapshot(&path).await?;
        recovery = Some(path);
    }

    remove_sidecars(database_path)?;

    if let Err(error) = replace_with(database_path, backup_path, true).await {
        if let Some(path) = &recovery {
            let _ = replace_with(database_path, path, false).await;
        }
        return Err(error);
    }

    if let Err(error) = verify_backup(database_path).await {
        if let Some(path) = &recovery {
            // Best effort: nunca dejar una base restaurada inválida.
            let _ = replace_with(database_path, path, false).await;
        }
        return Err(error);
    }
    Ok(())
}

/// Verifica que un archivo sea un SQLite legible e íntegro, sin exigir el
/// esquema de HiQNet POS. Se usa para la copia de recuperación.
async fn verify_snapshot(path: &Path) -> Result<(), StorageError> {
    if !path.is_file() {
        return Err(StorageError::Restore);
    }
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut connection = options.connect().await.map_err(|_| StorageError::Restore)?;
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

/// Copia `source` a un archivo temporal de la misma carpeta, lo verifica y lo
/// mueve sobre `database_path`. El archivo temporal nunca se deja atrás.
///
/// `strict` exige que el archivo sea un respaldo válido; sin él solo se pide
/// integridad, que es lo que necesita la copia de recuperación.
async fn replace_with(
    database_path: &Path,
    source: &Path,
    strict: bool,
) -> Result<(), StorageError> {
    let staged = staged_path(database_path);
    let _ = std::fs::remove_file(&staged);
    std::fs::copy(source, &staged).map_err(|_| StorageError::Restore)?;

    let validation = if strict {
        verify_backup(&staged).await
    } else {
        verify_snapshot(&staged).await
    };
    if let Err(error) = validation {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }

    remove_sidecars(database_path)?;
    if std::fs::rename(&staged, database_path).is_err() {
        let _ = std::fs::remove_file(&staged);
        return Err(StorageError::Restore);
    }
    // El reemplazo deja sidecars obsoletos si la copia los tuviera.
    let _ = remove_sidecars(database_path);
    Ok(())
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

fn remove_sidecars(database_path: &Path) -> Result<(), StorageError> {
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = database_path.as_os_str().to_owned();
        sidecar.push(suffix);
        let sidecar = PathBuf::from(sidecar);
        if sidecar.exists() {
            std::fs::remove_file(&sidecar).map_err(|_| StorageError::Restore)?;
        }
    }
    Ok(())
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
