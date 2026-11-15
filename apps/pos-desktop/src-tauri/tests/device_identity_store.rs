//! Propiedades del archivo de identidad del dispositivo.

#[cfg(unix)]
#[test]
fn crea_la_clave_con_permisos_privados_y_la_reutiliza() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("directorio temporal");
    let database = dir.path().join("pos.db");
    let first = pos_desktop_lib::platform_enrollment::load_or_create_identity(&database)
        .expect("identidad creada");
    let path = pos_storage_sqlite::device_identity_path(&database);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "la clave privada no debe ser legible por otros"
    );

    let second = pos_desktop_lib::platform_enrollment::load_or_create_identity(&database)
        .expect("identidad reutilizada");
    assert_eq!(first.public_key_encoded(), second.public_key_encoded());
}
