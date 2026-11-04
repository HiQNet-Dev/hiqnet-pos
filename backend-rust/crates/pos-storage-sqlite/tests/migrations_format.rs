//! Las migraciones se embeben y SQLx valida su checksum contra la base.
//! Un checkout de Windows con CRLF cambia el hash y bloquea el arranque.

#[test]
fn las_migraciones_estan_en_lf() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("migrations")
        .join("local");

    for entry in std::fs::read_dir(&dir).expect("leer migraciones") {
        let path = entry.expect("entrada de migración").path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("leer migración");
        assert!(
            !bytes.windows(2).any(|window| window == b"\r\n"),
            "{} contiene CRLF; las migraciones deben permanecer en LF",
            path.display()
        );
    }
}
