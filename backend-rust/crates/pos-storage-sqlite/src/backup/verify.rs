//! Verificacion de respaldos y apertura de solo lectura.

use super::*;

/// Tablas que existen desde la primera migración: cualquier base ajena a
/// HiQNet POS carece de ellas, pero un respaldo antiguo conserva su utilidad.
pub(super) const CORE_TABLES: [&str; 6] = [
    "organization",
    "branch",
    "app_user",
    "product",
    "audit_event",
    "local_installation_state",
];

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
pub(super) async fn verify_structure(path: &Path) -> Result<(), StorageError> {
    if !path.is_file() {
        return Err(StorageError::InvalidBackup);
    }
    if std::fs::metadata(path)
        .map(|meta| meta.len())
        .unwrap_or(u64::MAX)
        > MAX_BACKUP_BYTES
    {
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

pub(super) async fn open_read_only_connection(
    path: &Path,
) -> Result<sqlx::SqliteConnection, StorageError> {
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
pub(super) async fn verify_migration_history(
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
pub(super) async fn verify_snapshot(path: &Path) -> Result<(), StorageError> {
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
