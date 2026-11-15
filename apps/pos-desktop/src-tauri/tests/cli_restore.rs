use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Directorio temporal único, portable entre Linux y Windows.
fn scratch_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj de prueba")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("hiqnet-cli-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&directory).expect("crear directorio de prueba");
    directory
}

#[test]
fn cli_restore_falla_cerrado_sin_base_verificable_y_no_toca_archivos() {
    let directory = scratch_dir();
    let backup = directory.join("respaldo.db");
    std::fs::write(&backup, b"respaldo sintetico").expect("crear respaldo de prueba");

    // Base ausente: no se inventa una recuperación sin identidad verificable.
    let result = Command::new(env!("CARGO_BIN_EXE_pos-desktop"))
        .current_dir(&directory)
        .args(["--restore-backup", "respaldo.db", "--database", "actual.db"])
        .output()
        .expect("ejecutar CLI");
    assert_eq!(result.status.code(), Some(2));
    let stderr = String::from_utf8(result.stderr).expect("stderr UTF-8");
    assert!(stderr.contains("resultado=ERROR"));
    assert!(!directory.join("actual.db").exists());
    assert_eq!(std::fs::read(&backup).unwrap(), b"respaldo sintetico");

    // Base corrupta: tampoco se reemplaza.
    std::fs::write(directory.join("actual.db"), b"base rota").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_pos-desktop"))
        .current_dir(&directory)
        .args(["--restore-backup", "respaldo.db", "--database", "actual.db"])
        .output()
        .expect("ejecutar CLI");
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        std::fs::read(directory.join("actual.db")).unwrap(),
        b"base rota"
    );
    assert_eq!(std::fs::read(&backup).unwrap(), b"respaldo sintetico");

    std::fs::remove_dir_all(directory).expect("limpiar archivos de prueba propios");
}

#[test]
fn cli_verify_backup_sigue_disponible_sin_base_instalada() {
    let directory = scratch_dir();
    let pool =
        tauri::async_runtime::block_on(pos_storage_sqlite::open(&directory.join("verificable.db")))
            .expect("SQLite temporal verificable");
    tauri::async_runtime::block_on(pool.close());

    let verify = Command::new(env!("CARGO_BIN_EXE_pos-desktop"))
        .current_dir(&directory)
        .args(["--verify-backup", "verificable.db"])
        .output()
        .expect("ejecutar verificación aislada");
    assert_eq!(verify.status.code(), Some(0));
    assert!(
        String::from_utf8(verify.stdout)
            .unwrap()
            .contains("resultado=OK")
    );

    std::fs::remove_dir_all(directory).expect("limpiar archivos de prueba propios");
}
