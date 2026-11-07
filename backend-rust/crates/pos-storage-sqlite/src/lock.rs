//! Bloqueo exclusivo del proceso sobre la base local.
//!
//! SQLite protege el archivo mientras hay transacciones activas, pero no impide
//! que otra utilidad lo reemplace con la aplicación abierta y ociosa. Este
//! bloqueo es de ámbito de proceso: quien lo obtiene mantiene la base para sí
//! hasta que cierra, y la utilidad de restauración lo exige en exclusiva antes
//! de tocar ningún archivo.
//!
//! Es un bloqueo cooperativo (advisory): protege frente a otras instancias de
//! HiQNet POS, no frente a un programa externo que ignore el archivo de bloqueo.
//! Esa limitación se documenta y se compensa reemplazando el archivo solo bajo
//! el bloqueo y verificando el resultado.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::StorageError;

/// Bloqueo mantenido mientras el proceso usa la base.
#[derive(Debug)]
pub struct DatabaseLock {
    #[allow(dead_code)]
    file: File,
    path: PathBuf,
}

impl DatabaseLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Ruta del archivo de bloqueo asociado a una base.
pub fn lock_path(database_path: &Path) -> PathBuf {
    let mut value = database_path.as_os_str().to_owned();
    value.push(".lock");
    PathBuf::from(value)
}

/// Intenta tomar el bloqueo exclusivo del proceso. No espera: si otra instancia
/// lo mantiene devuelve [`StorageError::DatabaseInUse`].
pub fn try_lock(database_path: &Path) -> Result<DatabaseLock, StorageError> {
    let path = lock_path(database_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StorageError::Open)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|_| StorageError::Open)?;

    match lock_file(&file) {
        Ok(()) => Ok(DatabaseLock { file, path }),
        Err(error) if is_contended(&error) => Err(StorageError::DatabaseInUse),
        Err(_) => Err(StorageError::Open),
    }
}

#[cfg(windows)]
fn lock_file(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_LOCK_VIOLATION, HANDLE};
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    let handle = file.as_raw_handle() as HANDLE;
    // Bloquea desde el inicio del archivo por un byte: suficiente para excluir
    // a otra instancia del mismo bloqueo cooperativo.
    let result = unsafe {
        LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if result == 0 {
        let error = io::Error::last_os_error();
        // `windows-sys` expone la constante como tipo u32; se compara numérico.
        if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, error));
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(windows)]
fn is_contended(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

#[cfg(unix)]
fn lock_file(file: &File) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;

    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, error));
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(unix)]
fn is_contended(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_bloqueo_excluye_a_un_segundo_candidato_y_se_libera_al_cerrar() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("hiqnet-pos.db");

        let first = try_lock(&database).unwrap();
        assert_eq!(first.path(), lock_path(&database));

        let second = try_lock(&database).unwrap_err();
        assert!(
            matches!(second, StorageError::DatabaseInUse),
            "obtenido {second:?}"
        );

        drop(first);

        // Mismo proceso, nuevo descriptor: el bloqueo anterior ya se liberó.
        let third = try_lock(&database).unwrap();
        drop(third);
    }
}
