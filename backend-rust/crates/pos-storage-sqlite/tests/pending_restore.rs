use pos_storage_sqlite::{
    PendingRestoreOutcome, StorageError, backup_to, consume_pending_restore, open,
    preview_restore_candidate, schedule_pending_restore,
};

const TS: &str = "2026-01-01T00:00:00Z";

async fn seed_installation(
    pool: &sqlx::SqlitePool,
    installation_id: &str,
    organization_id: &str,
    valid_until: &str,
    grant_json: Option<&str>,
    signature: Option<&str>,
) {
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES (?, 'Org', ?)")
        .bind(organization_id)
        .bind(TS)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO local_installation_state \
         (id, installation_id, organization_id, plan, license_status, features_json, \
          trial_started_at, valid_until, license_grant_json, license_signature_b64, updated_at) \
         VALUES (1, ?, ?, 'trial', 'trial', '[\"catalog\",\"sales\"]', ?, ?, ?, ?, ?)",
    )
    .bind(installation_id)
    .bind(organization_id)
    .bind(TS)
    .bind(valid_until)
    .bind(grant_json)
    .bind(signature)
    .bind(TS)
    .execute(pool)
    .await
    .unwrap();
}

async fn product_count(path: &std::path::Path) -> i64 {
    let pool = open(path).await.unwrap();
    let count = sqlx::query_scalar("SELECT COUNT(*) FROM product")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    count
}

async fn seed_product(pool: &sqlx::SqlitePool, id: &str) {
    sqlx::query(
        "INSERT INTO product \
         (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
         VALUES (?, 'org-1', ?, ?, 100, 0, 1, ?)",
    )
    .bind(id)
    .bind(id)
    .bind(id)
    .bind(TS)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn restaura_solo_misma_instalacion_y_preserva_trial_vigente() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-02-01T00:00:00Z",
        None,
        None,
    )
    .await;
    seed_product(&pool, "antes").await;
    backup_to(&pool, &backup).await.unwrap();

    // El estado actual vence después que el snapshot: restaurar no debe revivir
    // ni reemplazar el Trial vigente por el plazo anterior del respaldo.
    sqlx::query("UPDATE local_installation_state SET valid_until = '2026-12-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    seed_product(&pool, "posterior").await;
    pool.close().await;

    let preview = preview_restore_candidate(&database, &backup).await.unwrap();
    assert!(preview.preserves_commercial_state);
    schedule_pending_restore(&database, &backup).await.unwrap();
    assert_eq!(
        consume_pending_restore(&database).await.unwrap(),
        PendingRestoreOutcome::Restored
    );
    assert_eq!(
        product_count(&database).await,
        1,
        "los datos vuelven al snapshot"
    );
    assert!(
        !backup.exists(),
        "la copia importada se elimina tras restaurar"
    );

    let pool = open(&database).await.unwrap();
    let valid_until: String =
        sqlx::query_scalar("SELECT valid_until FROM local_installation_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    pool.close().await;
    assert_eq!(
        valid_until, "2026-12-01T00:00:00Z",
        "el Trial vigente no retrocede"
    );
}

#[tokio::test]
async fn rechaza_respaldo_de_otra_instalacion_u_organizacion() {
    let directory = tempfile::tempdir().unwrap();
    let current = directory.path().join("actual.db");
    let foreign = directory.path().join("ajena.db");
    let backup = directory.path().join("ajena-respaldo.db");

    let current_pool = open(&current).await.unwrap();
    seed_installation(
        &current_pool,
        "install-1",
        "org-1",
        "2026-12-01T00:00:00Z",
        None,
        None,
    )
    .await;
    current_pool.close().await;

    let foreign_pool = open(&foreign).await.unwrap();
    seed_installation(
        &foreign_pool,
        "install-2",
        "org-2",
        "2026-12-01T00:00:00Z",
        None,
        None,
    )
    .await;
    backup_to(&foreign_pool, &backup).await.unwrap();
    foreign_pool.close().await;

    assert!(matches!(
        preview_restore_candidate(&current, &backup).await,
        Err(StorageError::RestoreIdentityMismatch)
    ));
}

#[tokio::test]
async fn reanuda_tras_interrumpirse_despues_de_promover_la_base() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-02-01T00:00:00Z",
        None,
        None,
    )
    .await;
    seed_product(&pool, "antes").await;
    backup_to(&pool, &backup).await.unwrap();
    sqlx::query("UPDATE local_installation_state SET valid_until = '2026-12-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    seed_product(&pool, "posterior").await;
    pool.close().await;

    schedule_pending_restore(&database, &backup).await.unwrap();
    // Interrupción simulada: se promovió el snapshot, pero no se repuso el
    // estado comercial ni se borró el marcador.
    pos_storage_sqlite::restore_from_backup(&database, &backup)
        .await
        .unwrap();
    assert_eq!(
        consume_pending_restore(&database).await.unwrap(),
        PendingRestoreOutcome::Restored
    );

    let pool = open(&database).await.unwrap();
    let valid_until: String =
        sqlx::query_scalar("SELECT valid_until FROM local_installation_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    pool.close().await;
    assert_eq!(valid_until, "2026-12-01T00:00:00Z");
    assert_eq!(product_count(&database).await, 1);
}

#[tokio::test]
async fn reabrir_varias_veces_no_repite_la_restauracion() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-12-01T00:00:00Z",
        None,
        None,
    )
    .await;
    seed_product(&pool, "antes").await;
    backup_to(&pool, &backup).await.unwrap();
    seed_product(&pool, "posterior").await;
    pool.close().await;

    schedule_pending_restore(&database, &backup).await.unwrap();
    assert_eq!(
        consume_pending_restore(&database).await.unwrap(),
        PendingRestoreOutcome::Restored
    );
    // La app puede reabrirse varias veces: sin marcador no hay segunda pasada.
    for _ in 0..3 {
        assert_eq!(
            consume_pending_restore(&database).await.unwrap(),
            PendingRestoreOutcome::None
        );
    }
    assert_eq!(product_count(&database).await, 1, "no se pierden más datos");
}

#[tokio::test]
async fn un_trial_vencido_no_se_revive_con_un_respaldo_anterior() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    // El snapshot conserva un vencimiento futuro; el estado vigente ya venció.
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2027-01-01T00:00:00Z",
        None,
        None,
    )
    .await;
    backup_to(&pool, &backup).await.unwrap();
    sqlx::query(
        "UPDATE local_installation_state \
         SET license_status = 'read_only', valid_until = '2025-06-01T00:00:00Z' WHERE id = 1",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    schedule_pending_restore(&database, &backup).await.unwrap();
    assert_eq!(
        consume_pending_restore(&database).await.unwrap(),
        PendingRestoreOutcome::Restored
    );

    let pool = open(&database).await.unwrap();
    let row = sqlx::query(
        "SELECT license_status, valid_until FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    pool.close().await;
    use sqlx::Row;
    assert_eq!(row.get::<String, _>("valid_until"), "2025-06-01T00:00:00Z");
    assert_eq!(row.get::<String, _>("license_status"), "read_only");
}

#[tokio::test]
async fn reanuda_si_muere_despues_de_reponer_el_estado_comercial() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-02-01T00:00:00Z",
        None,
        None,
    )
    .await;
    seed_product(&pool, "antes").await;
    backup_to(&pool, &backup).await.unwrap();
    sqlx::query("UPDATE local_installation_state SET valid_until = '2026-12-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    seed_product(&pool, "posterior").await;
    pool.close().await;

    schedule_pending_restore(&database, &backup).await.unwrap();
    pos_storage_sqlite::restore_from_backup(&database, &backup)
        .await
        .unwrap();
    // Interrupción simulada: el estado comercial ya se repuso y el marcador sigue.
    let pool = open(&database).await.unwrap();
    sqlx::query("UPDATE local_installation_state SET valid_until = '2026-12-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    assert_eq!(
        consume_pending_restore(&database).await.unwrap(),
        PendingRestoreOutcome::Restored
    );
    let pool = open(&database).await.unwrap();
    let valid_until: String =
        sqlx::query_scalar("SELECT valid_until FROM local_installation_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    pool.close().await;
    assert_eq!(valid_until, "2026-12-01T00:00:00Z");
    assert_eq!(product_count(&database).await, 1);
}

#[tokio::test]
async fn limpia_temporales_saf_sin_tocar_la_base() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-12-01T00:00:00Z",
        None,
        None,
    )
    .await;
    pool.close().await;

    for purpose in ["exports", "imports"] {
        let dir = pos_storage_sqlite::saf_workspace(&database, purpose);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("residuo.db"), b"temporal").unwrap();
    }
    pos_storage_sqlite::cleanup_saf_staging(&database);

    for purpose in ["exports", "imports"] {
        let dir = pos_storage_sqlite::saf_workspace(&database, purpose);
        assert!(
            !dir.join("residuo.db").exists(),
            "el temporal debe eliminarse"
        );
    }
    pos_storage_sqlite::verify_backup(&database).await.unwrap();
}

#[tokio::test]
async fn rechaza_material_de_licencia_sin_verificador_configurado() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-12-01T00:00:00Z",
        Some("{\"grant\":\"sin verificar\"}"),
        Some("firma-no-verificada"),
    )
    .await;
    backup_to(&pool, &backup).await.unwrap();
    pool.close().await;

    assert!(matches!(
        preview_restore_candidate(&database, &backup).await,
        Err(StorageError::RestoreLicensePolicy)
    ));
}

#[tokio::test]
async fn archivo_modificado_despues_de_confirmar_no_toca_la_base() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("hiqnet-pos.db");
    let backup = directory.path().join("respaldo.db");
    let pool = open(&database).await.unwrap();
    seed_installation(
        &pool,
        "install-1",
        "org-1",
        "2026-12-01T00:00:00Z",
        None,
        None,
    )
    .await;
    seed_product(&pool, "antes").await;
    backup_to(&pool, &backup).await.unwrap();
    seed_product(&pool, "posterior").await;
    pool.close().await;

    schedule_pending_restore(&database, &backup).await.unwrap();
    std::fs::write(&backup, b"archivo modificado despues de confirmar").unwrap();
    assert!(matches!(
        consume_pending_restore(&database).await,
        Err(StorageError::PendingRestore)
    ));
    assert_eq!(
        product_count(&database).await,
        2,
        "la base vigente permanece intacta"
    );
}
