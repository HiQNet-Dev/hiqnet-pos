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
