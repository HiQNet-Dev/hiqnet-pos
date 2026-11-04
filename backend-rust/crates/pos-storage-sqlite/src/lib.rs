//! Adaptador SQLite: conexión, migraciones, unidad de trabajo y repositorios.
//!
//! La aplicación nunca ve SQL. Toda mutación auditable se ejecuta en una sola
//! transacción `BEGIN IMMEDIATE` con su evento de auditoría.

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, SqlitePool};
use std::time::Duration;

pub mod backup;
pub mod uow;

pub use backup::{backup_to, open_read_only, restore_from_backup, verify_backup};
pub use uow::SqliteStore;

/// Error propio del adaptador, sin filtrar detalles hacia la UI.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("no fue posible abrir la base de datos local")]
    Open,

    #[error("no fue posible aplicar las migraciones")]
    Migrate,

    #[error("no fue posible respaldar la base de datos")]
    Backup,

    #[error("el respaldo no es válido")]
    InvalidBackup,

    #[error("no fue posible restaurar el respaldo")]
    Restore,

    #[error("la base de datos pertenece a una versión más reciente de HiQNet POS")]
    IncompatibleSchema,

    #[error("la base de datos local está dañada")]
    Corrupt,

    #[error("cierra HiQNet POS antes de restaurar un respaldo")]
    DatabaseInUse,
}

/// Última versión de migración conocida por este ejecutable.
pub fn latest_schema_version() -> i64 {
    sqlx::migrate!("../../migrations/local")
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

/// Abre (y crea si es necesario) la base local, aplica migraciones embebidas y
/// configura PRAGMAs seguros.
pub async fn open(path: impl AsRef<Path>) -> Result<SqlitePool, StorageError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Open)?;
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        .disable_statement_logging();

    // Una sola conexión: el POS de una caja es single-writer y así se evitan
    // bloqueos por escrituras concurrentes dentro del mismo proceso.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "no fue posible abrir la base de datos local");
            StorageError::Open
        })?;

    // Detección temprana de corrupción: `quick_check` es barato frente a
    // `integrity_check` y evita arrancar sobre un archivo dañado.
    let verdict: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "la base local no pudo verificarse");
            StorageError::Corrupt
        })?;
    if verdict != "ok" {
        tracing::error!(verdict = %verdict, "la base local está dañada");
        return Err(StorageError::Corrupt);
    }

    // Rechazar una base creada por una versión posterior: revertir el ejecutable
    // no revierte la base y las migraciones desconocidas no deben ignorarse.
    if let Some(applied) = applied_schema_version(&pool).await?
        && applied > latest_schema_version()
    {
        tracing::error!(
            applied,
            supported = latest_schema_version(),
            "la base local usa un esquema más reciente"
        );
        return Err(StorageError::IncompatibleSchema);
    }

    sqlx::migrate!("../../migrations/local")
        .run(&pool)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "no fue posible aplicar las migraciones");
            StorageError::Migrate
        })?;

    Ok(pool)
}

async fn applied_schema_version(pool: &SqlitePool) -> Result<Option<i64>, StorageError> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| StorageError::Corrupt)?;
    if exists == 0 {
        return Ok(None);
    }
    sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .map_err(|_| StorageError::Corrupt)
}

/// Ruta por defecto de la base local, fuera del repositorio.
///
/// Windows: `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db`.
pub fn default_database_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    base.join("HiQNet")
        .join("POS")
        .join("data")
        .join("hiqnet-pos.db")
}
