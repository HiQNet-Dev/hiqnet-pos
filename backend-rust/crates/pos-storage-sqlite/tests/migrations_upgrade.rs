//! Actualización de una instalación anterior (migraciones 0001–0003) hasta la
//! última versión disponible.
//!
//! Simula una base ya publicada: aplica el SQL tal como se publicó, registra los
//! checksums reales que espera SQLx y comprueba que `open` completa el resto de
//! migraciones sin perder datos ni romper la integridad.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

const M0001: &str = include_str!("../../../migrations/local/0001_init.sql");
const M0002: &str = include_str!("../../../migrations/local/0002_inventory.sql");
const M0003: &str = include_str!("../../../migrations/local/0003_inventory_adjustments.sql");

/// Checksums que SQLx calculó al aplicar las migraciones en una base nueva.
async fn checksums_hasta(pool: &SqlitePool, version_maxima: i64) -> Vec<(i64, Vec<u8>)> {
    sqlx::query(
        "SELECT version, checksum FROM _sqlx_migrations WHERE version <= ? ORDER BY version",
    )
    .bind(version_maxima)
    .fetch_all(pool)
    .await
    .expect("leer metadatos de migraciones")
    .into_iter()
    .map(|row| {
        (
            row.get::<i64, _>("version"),
            row.get::<Vec<u8>, _>("checksum"),
        )
    })
    .collect()
}

async fn preparar_base_legacy(path: &std::path::Path, checksums: &[(i64, Vec<u8>)]) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .expect("abrir base sintética");

    for migration in [M0001, M0002, M0003] {
        sqlx::raw_sql(migration)
            .execute(&pool)
            .await
            .expect("aplicar migración publicada");
    }

    sqlx::raw_sql(
        "INSERT INTO organization (id, name, created_at) \
         VALUES ('org-legacy', 'Cafetería anterior', '2026-01-01T00:00:00Z'); \
         INSERT INTO branch (id, organization_id, name, code, created_at) \
         VALUES ('branch-legacy', 'org-legacy', 'Matriz', 'MX-01', '2026-01-01T00:00:00Z'); \
         INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
         VALUES ('product-legacy', 'org-legacy', 'CAF-001', 'Café anterior', 1000, 1600, 1, '2026-01-01T00:00:00Z'); \
         INSERT INTO inventory_movement (id, organization_id, branch_id, product_id, kind, quantity_milli, reason, actor_user_id, occurred_at) \
         VALUES ('movement-legacy', 'org-legacy', 'branch-legacy', 'product-legacy', 'inbound', 20000, NULL, NULL, '2026-01-01T00:00:00Z');",
    )
    .execute(&pool)
    .await
    .expect("insertar datos sintéticos");

    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _sqlx_migrations ( \
             version BIGINT PRIMARY KEY, \
             description TEXT NOT NULL, \
             installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, \
             success BOOLEAN NOT NULL, \
             checksum BLOB NOT NULL, \
             execution_time BIGINT NOT NULL \
         );",
    )
    .execute(&pool)
    .await
    .expect("crear tabla de migraciones");

    for (version, checksum) in checksums {
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (?, 'legacy', 1, ?, 0)",
        )
        .bind(version)
        .bind(checksum)
        .execute(&pool)
        .await
        .expect("registrar migración aplicada");
    }

    pool.close().await;
}

#[tokio::test]
async fn actualiza_una_base_0003_sin_perder_datos() {
    // Base de referencia: SQLx aplica todo y expone los checksums canónicos.
    let reference_dir = tempfile::tempdir().expect("directorio temporal");
    let reference = pos_storage_sqlite::open(reference_dir.path().join("reference.db"))
        .await
        .expect("base de referencia");
    let checksums = checksums_hasta(&reference, 3).await;
    assert_eq!(checksums.len(), 3, "se esperaban las migraciones 0001–0003");
    reference.close().await;

    // Base sintética: sólo 0001–0003, con datos comerciales previos.
    let dir = tempfile::tempdir().expect("directorio temporal");
    let path = dir.path().join("legacy.db");
    preparar_base_legacy(&path, &checksums).await;

    // La aplicación arranca y completa las migraciones restantes.
    let pool = pos_storage_sqlite::open(&path)
        .await
        .expect("actualizar base anterior");

    let aplicadas: i64 = sqlx::query("SELECT COUNT(*) AS total FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("contar migraciones")
        .get("total");
    let esperadas: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM _sqlx_migrations WHERE version > 3")
            .fetch_one(&pool)
            .await
            .expect("contar migraciones nuevas")
            .get("total");
    assert_eq!(
        aplicadas, 10,
        "deben quedar las diez migraciones registradas"
    );
    assert_eq!(esperadas, 7, "0004–0010 deben aplicarse al actualizar");

    let producto: (String, i64) = {
        let row =
            sqlx::query("SELECT sku, unit_price_minor FROM product WHERE id = 'product-legacy'")
                .fetch_one(&pool)
                .await
                .expect("el producto histórico debe conservarse");
        (row.get("sku"), row.get("unit_price_minor"))
    };
    assert_eq!(producto, ("CAF-001".to_owned(), 1000));

    let existencia: i64 = sqlx::query(
        "SELECT COALESCE(SUM(CASE kind WHEN 'outbound' THEN -quantity_milli \
         WHEN 'adjustment_out' THEN -quantity_milli ELSE quantity_milli END), 0) AS stock \
         FROM inventory_movement WHERE product_id = 'product-legacy'",
    )
    .fetch_one(&pool)
    .await
    .expect("leer existencias heredadas")
    .get("stock");
    assert_eq!(existencia, 20_000, "el ledger previo debe conservarse");

    let integridad: String = sqlx::query("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("verificar integridad")
        .get(0);
    assert_eq!(integridad, "ok");

    let fk_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("verificar claves foráneas");
    assert!(
        fk_violations.is_empty(),
        "la actualización no debe dejar referencias rotas"
    );

    for tabla in [
        "category",
        "product_variant",
        "modifier_group",
        "sale",
        "supplier",
        "purchase_order",
        "purchase_order_line",
        "purchase_receipt",
        "purchase_receipt_line",
        "expense_category",
        "expense",
        "waste_record",
        "sale_return",
        "sale_return_line",
    ] {
        let existe: i64 = sqlx::query(
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(tabla)
        .fetch_one(&pool)
        .await
        .expect("consultar esquema")
        .get("total");
        assert_eq!(existe, 1, "falta la tabla {tabla} tras actualizar");
    }

    // La columna añadida por 0004 debe existir en la tabla histórica.
    let categoria: Option<String> =
        sqlx::query("SELECT category_id FROM product WHERE id = 'product-legacy'")
            .fetch_one(&pool)
            .await
            .expect("product debe tener category_id")
            .get("category_id");
    assert!(
        categoria.is_none(),
        "un producto histórico no tiene categoría"
    );

    pool.close().await;
}
