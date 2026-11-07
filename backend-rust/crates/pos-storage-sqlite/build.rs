use std::fs;
use std::path::Path;

const MIGRATIONS_DIR: &str = "../../migrations/local";

fn main() {
    let migrations = Path::new(MIGRATIONS_DIR);
    println!("cargo:rerun-if-changed={}", migrations.display());

    let mut files = fs::read_dir(migrations)
        .expect("no fue posible leer las migraciones embebidas")
        .map(|entry| entry.expect("entrada de migración no válida").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    files.sort();

    let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        update_fingerprint(
            &mut fingerprint,
            path.file_name().unwrap().as_encoded_bytes(),
        );
        update_fingerprint(
            &mut fingerprint,
            &fs::read(&path).expect("no fue posible leer una migración embebida"),
        );
    }

    println!("cargo:rustc-env=POS_STORAGE_MIGRATIONS_FINGERPRINT={fingerprint:016x}");
}

fn update_fingerprint(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
