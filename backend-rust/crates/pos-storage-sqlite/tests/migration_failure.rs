//! Fallo controlado durante una migración.
//!
//! Una migración que no puede aplicarse debe devolver `Migrate` y dejar la base
//! en un estado conocido: datos previos intactos, sin tablas a medio crear y sin
//! marcar la migración como aplicada con éxito. El operador puede entonces
//! restaurar un respaldo en lugar de operar sobre un esquema inconsistente.

use pos_storage_sqlite::{StorageError, open};

#[tokio::test]
async fn migracion_que_falla_no_deja_esquema_parcial() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");

    // Base completa y con datos, más un respaldo utilizable.
    let pool = open(&database).await.unwrap();
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1', 'Org', '2026-01-01T00:00:00Z')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
         VALUES ('p-1', 'org-1', 'UNO', 'UNO', 100, 0, 1, '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    pos_storage_sqlite::backup_to(&pool, &backup).await.unwrap();

    // Se simula una actualización que choca con un objeto preexistente: la
    // migración 0009 no puede crear `expense_category`.
    sqlx::raw_sql(
        "DROP TRIGGER expense_same_scope;
         DROP TRIGGER waste_same_scope;
         DROP TRIGGER expense_no_update;
         DROP TRIGGER expense_no_delete;
         DROP TRIGGER waste_no_update;
         DROP TRIGGER waste_no_delete;
         DROP INDEX idx_expense_org_branch_occurred;
         DROP INDEX idx_waste_org_branch_occurred;
         DROP TABLE waste_record;
         DROP TABLE expense;
         DROP TABLE expense_category;
         DELETE FROM _sqlx_migrations WHERE version = 9;
         CREATE TABLE expense_category (id TEXT PRIMARY KEY);",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let error = open(&database).await.unwrap_err();
    assert!(matches!(error, StorageError::Migrate), "obtenido {error:?}");

    // Los datos anteriores siguen ahí y la migración no quedó como exitosa.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}?mode=rwc", database.display()))
        .await
        .unwrap();
    let products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(products, 1, "la migración fallida no debe perder datos");

    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 9")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(applied, 0, "la migración fallida no queda como aplicada");

    let waste_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'waste_record'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // SQLx detiene la migración en el primer error: no se crean tablas posteriores.
    assert_eq!(waste_exists, 0);
    pool.close().await;

    // La recuperación documentada: restaurar el respaldo previo.
    pos_storage_sqlite::restore_from_backup(&database, &backup)
        .await
        .unwrap();
    let recovered = open(&database).await.unwrap();
    let waste: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'waste_record'",
    )
    .fetch_one(&recovered)
    .await
    .unwrap();
    assert_eq!(waste, 1, "el respaldo restaura el esquema completo");
    recovered.close().await;
}
