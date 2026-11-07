//! Punto de entrada. Mantiene la utilidad offline de respaldo/restauración
//! separada del arranque normal de Tauri: la restauración debe ejecutarse con la
//! aplicación cerrada y nunca desde la interfaz en operación.

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|value| {
        value == "--verify-backup" || value == "--restore-backup" || value == "--help-backup"
    }) {
        run_backup_cli(&args);
    }
    pos_desktop_lib::run();
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|value| value == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn run_backup_cli(args: &[String]) -> ! {
    if args.iter().any(|value| value == "--help-backup") {
        println!("Uso:");
        println!("  pos-desktop --verify-backup <respaldo.db>");
        println!("  pos-desktop --restore-backup <respaldo.db> [--database <hiqnet-pos.db>]");
        println!();
        println!("Sin --database se usa la ubicación persistente de la plataforma.");
        println!("En desarrollo y QA puede aislarse con la variable HIQNET_POS_DB_PATH.");
        println!();
        println!("Cierra HiQNet POS antes de restaurar. La restauración crea una copia");
        println!("de recuperación junto a la base actual.");
        std::process::exit(0);
    }

    // Verificar un respaldo no abre ni resuelve la base instalada.
    if let Some(path) = value_after(args, "--verify-backup") {
        let result = tauri::async_runtime::block_on(pos_storage_sqlite::verify_backup(
            &PathBuf::from(path),
        ));
        report(result, None);
    }

    let Some(backup) = value_after(args, "--restore-backup").map(PathBuf::from) else {
        eprintln!("resultado=ERROR");
        eprintln!("detalle=Operación no reconocida. Usa --help-backup.");
        std::process::exit(2);
    };

    let database = match value_after(args, "--database")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(pos_storage_sqlite::resolved_database_path)
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("resultado=ERROR");
            eprintln!("detalle={error}");
            std::process::exit(2);
        }
    };

    let result = tauri::async_runtime::block_on(pos_storage_sqlite::restore_from_backup(
        &database, &backup,
    ))
    .map(|_| ());
    report(result, Some(&database));
}

fn report(result: Result<(), pos_storage_sqlite::StorageError>, database: Option<&Path>) -> ! {
    match result {
        Ok(()) => {
            println!("resultado=OK");
            if let Some(path) = database {
                println!("database={}", path.display());
            }
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
