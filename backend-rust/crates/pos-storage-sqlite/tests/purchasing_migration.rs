//! Las migraciones 0008–0010 deben actualizar una instalación que ya alcanzó 0007.

use sqlx::Row;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn actualiza_0007_a_0010_y_conserva_el_esquema_previo() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    std::fs::create_dir_all(&migrations).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/local");
    for version in 1..=7 {
        let name = match version {
            1 => "0001_init.sql",
            2 => "0002_inventory.sql",
            3 => "0003_inventory_adjustments.sql",
            4 => "0004_catalog_categories_variants.sql",
            5 => "0005_catalog_modifiers.sql",
            6 => "0006_cash_registers_shifts.sql",
            _ => "0007_sales_checkout.sql",
        };
        std::fs::copy(source.join(name), migrations.join(name)).unwrap();
    }
    let path = dir.path().join("legacy-0007.db");
    let legacy = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&legacy)
        .await
        .unwrap();
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1', 'Org', '2026-01-01T00:00:00Z')").execute(&legacy).await.unwrap();
    legacy.close().await;

    let upgraded = pos_storage_sqlite::open(&path).await.unwrap();
    let migrations: i64 = sqlx::query("SELECT COUNT(*) AS total FROM _sqlx_migrations")
        .fetch_one(&upgraded)
        .await
        .unwrap()
        .get("total");
    assert_eq!(migrations, 10);
    for table in [
        "organization",
        "sale",
        "supplier",
        "purchase_order",
        "purchase_receipt",
        "expense_category",
        "expense",
        "waste_record",
        "sale_return",
        "sale_return_line",
    ] {
        let exists: i64 = sqlx::query(
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(&upgraded)
        .await
        .unwrap()
        .get("total");
        assert_eq!(exists, 1, "falta {table}");
    }
    let name: String = sqlx::query("SELECT name FROM organization WHERE id = 'org-1'")
        .fetch_one(&upgraded)
        .await
        .unwrap()
        .get("name");
    assert_eq!(name, "Org");
    let integrity: String = sqlx::query("PRAGMA integrity_check")
        .fetch_one(&upgraded)
        .await
        .unwrap()
        .get(0);
    assert_eq!(integrity, "ok");
}
