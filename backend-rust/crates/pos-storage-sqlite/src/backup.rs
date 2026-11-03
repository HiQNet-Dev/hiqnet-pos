use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, Row, SqlitePool};
use std::path::Path;

use crate::StorageError;

/// Crea un respaldo consistente usando `VACUUM INTO`, que no requiere detener
/// la aplicación y produce un archivo SQLite íntegro.
pub async fn backup_to(pool: &SqlitePool, target: &Path) -> Result<(), StorageError> {
    if target.exists() {
        return Err(StorageError::Backup);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Backup)?;
    }
    let target = target.to_string_lossy().to_string();
    sqlx::query("VACUUM INTO ?")
        .bind(target)
        .execute(pool)
        .await
        .map_err(|_| StorageError::Backup)?;
    Ok(())
}

/// Comprueba que un archivo sea una base SQLite íntegra con el esquema de la
/// aplicación. No modifica el archivo.
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
         WHERE type = 'table' AND name IN ('organization', 'app_user', 'product', 'audit_event')",
    )
    .fetch_one(&mut connection)
    .await
    .map_err(|_| StorageError::InvalidBackup)?;
    let total: i64 = tables.get("total");
    if total != 4 {
        return Err(StorageError::InvalidBackup);
    }

    connection
        .close()
        .await
        .map_err(|_| StorageError::InvalidBackup)?;
    Ok(())
}

/// Restaura un respaldo sobre `database_path`.
///
/// Debe invocarse **sin** una conexión abierta a la base destino. La aplicación
/// de escritorio exige reinicio antes de restaurar para no mezclar conexiones;
/// ver `docs/05-operations/backup-and-restore.md`.
pub async fn restore_from_backup(
    database_path: &Path,
    backup_path: &Path,
) -> Result<(), StorageError> {
    verify_backup(backup_path).await?;

    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Restore)?;
    }

    // Se eliminan WAL/SHM obsoletos antes de reemplazar el archivo principal.
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = database_path.as_os_str().to_owned();
        sidecar.push(suffix);
        let sidecar = std::path::PathBuf::from(sidecar);
        if sidecar.exists() {
            std::fs::remove_file(&sidecar).map_err(|_| StorageError::Restore)?;
        }
    }

    std::fs::copy(backup_path, database_path).map_err(|_| StorageError::Restore)?;
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
