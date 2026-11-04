use pos_storage_sqlite::{
    StorageError, backup_to, latest_schema_version, open, restore_from_backup, try_lock,
    verify_backup,
};
use sqlx::Row;

const TS: &str = "2026-01-01T00:00:00Z";

async fn seed_product(pool: &sqlx::SqlitePool, id: &str, sku: &str) {
    sqlx::query(
        "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) VALUES (?, 'org-1', ?, ?, 100, 0, 1, ?)",
    )
    .bind(id)
    .bind(sku)
    .bind(sku)
    .bind(TS)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_organization(pool: &sqlx::SqlitePool) {
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1', 'Org', ?)")
        .bind(TS)
        .execute(pool)
        .await
        .unwrap();
}

async fn product_count(path: &std::path::Path) -> i64 {
    let pool = open(path).await.unwrap();
    let value: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    value
}

#[tokio::test]
async fn respaldo_verificado_y_restauracion_con_copia_de_recuperacion() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");

    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    seed_product(&pool, "p-1", "UNO").await;
    backup_to(&pool, &backup).await.unwrap();
    verify_backup(&backup).await.unwrap();

    // Cambio posterior que debe perderse al restaurar.
    seed_product(&pool, "p-2", "DOS").await;
    pool.close().await;

    restore_from_backup(&database, &backup).await.unwrap();
    assert_eq!(
        product_count(&database).await,
        1,
        "vuelve al estado respaldado"
    );

    let recovery = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().contains(".pre-restore-"))
        .expect("debe existir copia de recuperación");
    let recovery_pool = open(&recovery).await.unwrap();
    let recovered: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product")
        .fetch_one(&recovery_pool)
        .await
        .unwrap();
    recovery_pool.close().await;
    assert_eq!(recovered, 2, "la copia conserva el estado previo completo");

    // La base restaurada queda íntegra y con claves foráneas válidas.
    verify_backup(&database).await.unwrap();
}

#[tokio::test]
async fn rechaza_archivo_ajeno_o_corrupto() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("no-es-sqlite.db");
    std::fs::write(&text, b"esto no es una base").unwrap();
    assert!(matches!(
        verify_backup(&text).await,
        Err(StorageError::InvalidBackup)
    ));

    let foreign = dir.path().join("ajena.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&foreign)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    sqlx::query("CREATE TABLE otra (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    assert!(matches!(
        verify_backup(&foreign).await,
        Err(StorageError::InvalidBackup)
    ));
}

#[tokio::test]
async fn rechaza_esquema_mas_nuevo_que_el_ejecutable() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("futuro.db");
    let pool = open(&database).await.unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (?, 'desconocida', 1, X'00', 0)",
    )
    .bind(latest_schema_version() + 1)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    assert!(matches!(
        verify_backup(&database).await,
        Err(StorageError::IncompatibleSchema)
    ));
    assert!(matches!(
        open(&database).await,
        Err(StorageError::IncompatibleSchema)
    ));
}

#[tokio::test]
async fn rechaza_restaurar_mientras_hay_escritura_activa() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");

    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    seed_product(&pool, "p-1", "UNO").await;
    backup_to(&pool, &backup).await.unwrap();

    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) VALUES ('p-2', 'org-1', 'DOS', 'DOS', 100, 0, 1, ?)")
        .bind(TS)
        .execute(&mut *transaction)
        .await
        .unwrap();

    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(
        matches!(error, StorageError::DatabaseInUse),
        "obtenido {error:?}"
    );

    transaction.rollback().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn no_sobrescribe_un_respaldo_existente() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    std::fs::write(&backup, b"ocupado").unwrap();
    assert!(matches!(
        backup_to(&pool, &backup).await,
        Err(StorageError::Backup)
    ));
    pool.close().await;
}

#[tokio::test]
async fn base_corrupta_se_rechaza_al_abrir() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("corrupta.db");
    std::fs::write(&database, vec![0x42_u8; 4096]).unwrap();
    let error = open(&database).await.unwrap_err();
    assert!(
        matches!(error, StorageError::Corrupt | StorageError::Open),
        "obtenido {error:?}"
    );
}

#[tokio::test]
async fn rechaza_restaurar_con_otra_instancia_abierta() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");

    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    seed_product(&pool, "p-1", "UNO").await;
    backup_to(&pool, &backup).await.unwrap();
    seed_product(&pool, "p-2", "DOS").await;
    pool.close().await;

    // Otra instancia de HiQNet POS abierta y ociosa: sin transacción activa,
    // pero con la base en uso.
    let running = try_lock(&database).unwrap();

    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(
        matches!(error, StorageError::DatabaseInUse),
        "obtenido {error:?}"
    );
    assert_eq!(
        product_count(&database).await,
        2,
        "la base en uso no se modifica"
    );

    drop(running);
    restore_from_backup(&database, &backup).await.unwrap();
    assert_eq!(product_count(&database).await, 1);
}

#[tokio::test]
async fn restauracion_no_deja_archivos_temporales() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");

    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    seed_product(&pool, "p-1", "UNO").await;
    backup_to(&pool, &backup).await.unwrap();
    pool.close().await;

    restore_from_backup(&database, &backup).await.unwrap();

    let leftovers: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.contains("restore-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "archivos sobrantes: {leftovers:?}");
}

#[tokio::test]
async fn respaldo_invalido_deja_la_base_intacta() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let corrupt = dir.path().join("roto.db");

    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    seed_product(&pool, "p-1", "UNO").await;
    pool.close().await;

    std::fs::write(&corrupt, vec![0x41_u8; 8192]).unwrap();
    assert!(restore_from_backup(&database, &corrupt).await.is_err());
    assert_eq!(product_count(&database).await, 1);
}

#[tokio::test]
async fn rechaza_historial_de_migraciones_manipulado() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 1")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    assert!(matches!(
        verify_backup(&database).await,
        Err(StorageError::InvalidBackup)
    ));
}

#[tokio::test]
async fn restauraciones_sucesivas_conservan_cada_copia_de_recuperacion() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let older = dir.path().join("anterior.db");
    let newer = dir.path().join("reciente.db");

    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    seed_product(&pool, "p-1", "UNO").await;
    backup_to(&pool, &older).await.unwrap();
    seed_product(&pool, "p-2", "DOS").await;
    backup_to(&pool, &newer).await.unwrap();
    pool.close().await;

    restore_from_backup(&database, &older).await.unwrap();
    restore_from_backup(&database, &newer).await.unwrap();

    let recovery_copies = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains(".pre-restore-")
        })
        .count();
    assert_eq!(recovery_copies, 2, "ninguna copia se sobrescribe");
    assert_eq!(product_count(&database).await, 2);
}

#[tokio::test]
async fn respaldo_conserva_todas_las_fuentes_del_negocio() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_organization(&pool).await;
    // Un respaldo recién creado debe incluir las tablas de todas las fases.
    backup_to(&pool, &backup).await.unwrap();
    let restored = pos_storage_sqlite::open_read_only(&backup).await.unwrap();
    let tables: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'table' AND name IN \
         ('organization','app_user','product','sale','sale_return','expense','waste_record','supplier','purchase_order','cash_shift','audit_event')",
    )
    .fetch_one(&restored)
    .await
    .unwrap()
    .get("total");
    restored.close().await;
    assert_eq!(tables, 11);
    pool.close().await;
}
