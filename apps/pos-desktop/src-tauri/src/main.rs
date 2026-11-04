//! Punto de entrada. Mantiene la utilidad offline de respaldo/restauración
//! separada del arranque normal de Tauri: la restauración debe ejecutarse con la
//! aplicación cerrada y nunca desde la interfaz en operación.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|value| {
        value == "--verify-backup" || value == "--restore-backup" || value == "--help-backup"
    }) {
        run_backup_cli(&args);
        return;
    }
    pos_desktop_lib::run();
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|value| value == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn run_backup_cli(args: &[String]) {
    let database = value_after(args, "--database")
        .map(PathBuf::from)
        .unwrap_or_else(pos_storage_sqlite::default_database_path);

    if args.iter().any(|value| value == "--help-backup") {
        println!("Uso:");
        println!("  pos-desktop.exe --verify-backup <respaldo.db>");
        println!("  pos-desktop.exe --restore-backup <respaldo.db> [--database <hiqnet-pos.db>]");
        println!();
        println!("Cierra HiQNet POS antes de restaurar. La restauración crea una copia");
        println!("de recuperación junto a la base actual.");
        std::process::exit(0);
    }

    let result = tauri::async_runtime::block_on(async {
        if let Some(path) = value_after(args, "--verify-backup") {
            pos_storage_sqlite::verify_backup(&PathBuf::from(path)).await
        } else if let Some(path) = value_after(args, "--restore-backup") {
            pos_storage_sqlite::restore_from_backup(&database, &PathBuf::from(path))
                .await
                .map(|_| ())
        } else {
            Err(pos_storage_sqlite::StorageError::InvalidBackup)
        }
    });

    match result {
        Ok(()) => {
            println!("resultado=OK");
            println!("database={}", database.display());
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("resultado=ERROR");
            if matches!(
                error,
                pos_storage_sqlite::StorageError::RestoreCritical { .. }
            ) {
                eprintln!("critico=true");
            }
            eprintln!("detalle={error}");
            std::process::exit(2);
        }
    }
}
