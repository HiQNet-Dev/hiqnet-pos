use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{StorageError, latest_schema_version};

const REQUIRED_TABLES: [&str; 6] = [
    "organization",
    "app_user",
    "product",
    "audit_event",
    "sale",
    "sale_return",
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

/// Comprueba que un archivo sea una base SQLite íntegra, del esquema esperado y
/// no más nueva que este ejecutable. No modifica el archivo.
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
        "SELECT COUNT(*) AS total FROM sqlite_master \
         WHERE type = 'table' AND name IN ('organization', 'app_user', 'product', 'audit_event', 'sale', 'sale_return')",
    )
    .fetch_one(&mut connection)
    .await
    .map_err(|_| StorageError::InvalidBackup)?;
    let total: i64 = tables.get("total");
    if total != REQUIRED_TABLES.len() as i64 {
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

    let applied: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&mut connection)
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    if applied > latest_schema_version() {
        return Err(StorageError::IncompatibleSchema);
    }

    connection
        .close()
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    Ok(())
}

/// Restaura un respaldo sobre `database_path`.
///
/// Procedimiento seguro para uso **offline**, con la aplicación cerrada:
///
/// 1. verifica el respaldo (integridad, esquema, claves foráneas y versión);
/// 2. comprueba que no haya otro escritor activo sobre la base destino;
/// 3. crea una copia de recuperación `<base>.pre-restore-<epoch>.db` con
///    `VACUUM INTO`, que incluye el estado del WAL;
/// 4. elimina WAL/SHM obsoletos y reemplaza el archivo principal;
/// 5. verifica la base restaurada y, si falla, revierte a la copia de recuperación.
pub async fn restore_from_backup(
    database_path: &Path,
    backup_path: &Path,
) -> Result<(), StorageError> {
    verify_backup(backup_path).await?;

    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Restore)?;
    }

    let mut recovery: Option<PathBuf> = None;
    if database_path.is_file() {
        let options = SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(false)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(2));
        let mut connection = options.connect().await.map_err(|_| StorageError::Restore)?;

        // Si otro proceso mantiene una escritura, la restauración se rechaza en
        // lugar de reemplazar una base en uso.
        sqlx::query("BEGIN IMMEDIATE")
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
        recovery = Some(path);
    }

    remove_sidecars(database_path)?;
    std::fs::copy(backup_path, database_path).map_err(|_| StorageError::Restore)?;

    if let Err(error) = verify_backup(database_path).await {
        if let Some(path) = &recovery {
            // Best effort: nunca dejar una base restaurada inválida.
            let _ = std::fs::copy(path, database_path);
            let _ = remove_sidecars(database_path);
        }
        return Err(error);
    }
    Ok(())
}

fn recovery_path(database_path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let name = database_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "hiqnet-pos.db".into());
    database_path.with_file_name(format!("{name}.pre-restore-{stamp}.db"))
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
