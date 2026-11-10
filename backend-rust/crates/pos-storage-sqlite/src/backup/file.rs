//! Copia de respaldo y utilidades de archivo.

use super::*;

/// Tope de tamaño para un respaldo. El adaptador SAF aplica además un límite
/// equivalente durante la copia; aquí protege también a la utilidad offline.
pub(super) const MAX_BACKUP_BYTES: u64 = 512 * 1024 * 1024;

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

pub(super) fn sha256_file(path: &Path) -> Result<String, StorageError> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|_| StorageError::PendingRestore)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| StorageError::PendingRestore)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
