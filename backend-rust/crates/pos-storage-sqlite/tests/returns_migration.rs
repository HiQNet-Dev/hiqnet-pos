//! Las migraciones 0010–0011 deben actualizar una instalación que ya alcanzó 0009 y
//! conservar los datos de gastos, merma y ventas.

use sqlx::Row;
use sqlx::sqlite::SqlitePoolOptions;

const FILES: [&str; 9] = [
    "0001_init.sql",
    "0002_inventory.sql",
    "0003_inventory_adjustments.sql",
    "0004_catalog_categories_variants.sql",
    "0005_catalog_modifiers.sql",
    "0006_cash_registers_shifts.sql",
    "0007_sales_checkout.sql",
    "0008_purchasing.sql",
    "0009_expenses_waste.sql",
];

#[tokio::test]
async fn actualiza_0009_a_0011_conservando_gastos_y_ventas() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    std::fs::create_dir_all(&migrations).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/local");
    for name in FILES {
        std::fs::copy(source.join(name), migrations.join(name)).unwrap();
    }

    let path = dir.path().join("legacy-0009.db");
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

    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1', 'Org', '2026-01-01T00:00:00Z')")
        .execute(&legacy)
        .await
        .unwrap();
    sqlx::query("INSERT INTO branch (id, organization_id, name, code, created_at) VALUES ('branch-1', 'org-1', 'Matriz', 'MX-01', '2026-01-01T00:00:00Z')")
        .execute(&legacy)
        .await
        .unwrap();
    sqlx::query("INSERT INTO app_user (id, organization_id, branch_id, username, display_name, role, password_hash, active, created_at) VALUES ('user-1', 'org-1', 'branch-1', 'owner', 'Owner', 'owner', 'hash', 1, '2026-01-01T00:00:00Z')")
        .execute(&legacy)
        .await
        .unwrap();
    sqlx::query("INSERT INTO expense_category (id, organization_id, name, active, created_at) VALUES ('cat-1', 'org-1', 'Operación', 1, '2026-01-01T00:00:00Z')")
        .execute(&legacy)
        .await
        .unwrap();
    sqlx::query("INSERT INTO expense (id, organization_id, branch_id, category_id, amount_minor, concept, cash_shift_id, cash_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at) VALUES ('exp-1', 'org-1', 'branch-1', 'cat-1', 1250, 'Limpieza', NULL, NULL, 'legacy-exp', 'fp', 'user-1', '2026-01-02T00:00:00Z')")
        .execute(&legacy)
        .await
        .unwrap();
    legacy.close().await;

    let upgraded = pos_storage_sqlite::open(&path).await.unwrap();
    let applied: i64 = sqlx::query("SELECT COUNT(*) AS total FROM _sqlx_migrations")
        .fetch_one(&upgraded)
        .await
        .unwrap()
        .get("total");
    assert_eq!(applied, 12);

    let expense = sqlx::query("SELECT amount_minor, concept FROM expense WHERE id = 'exp-1'")
        .fetch_one(&upgraded)
        .await
        .unwrap();
    assert_eq!(expense.get::<i64, _>("amount_minor"), 1250);
    assert_eq!(expense.get::<String, _>("concept"), "Limpieza");

    for table in ["sale_return", "sale_return_line"] {
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

    let integrity: String = sqlx::query("PRAGMA integrity_check")
        .fetch_one(&upgraded)
        .await
        .unwrap()
        .get(0);
    assert_eq!(integrity, "ok");
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&upgraded)
        .await
        .unwrap();
    assert!(violations.is_empty());
}
