//! Respaldo, verificacion y restauracion de la base local.
//!
//! La secuencia critica (preparar destino, promover y revertir) vive completa
//! en `restore`: no debe fragmentarse, porque su atomicidad depende del orden
//! y de la ventana entre el checkpoint y el reemplazo del archivo.

use crate::lock::try_lock;
use crate::{StorageError, embedded_migrations, latest_schema_version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteLockingMode, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod failure_tests;
#[cfg(test)]
mod fault_injector;
mod file;
mod hooks;
mod pending;
mod policy;
#[cfg(test)]
mod race;
mod restore;
mod verify;

use file::*;
use hooks::*;
#[cfg(test)]
use pending::*;
use policy::*;
use restore::*;
use verify::*;

pub use file::backup_to;
pub use pending::{
    PendingRestoreOutcome, cleanup_saf_staging, consume_pending_restore, saf_workspace,
    schedule_pending_restore,
};
pub use policy::{RestorePreview, preview_restore_candidate};
pub use restore::{RestoreReport, restore_from_backup};
pub use verify::{open_read_only, verify_backup};
