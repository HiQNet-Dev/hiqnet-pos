use pos_application::{
    AppError, BootstrapRequest, CategoryDraft, Clock, InventoryMovementDraft, LoginRequest,
    PosService, ProductDraft, ProductVariantDraft, SharedClock, SharedHasher, SharedStore, Store,
};
use pos_core::catalog::Product;
use pos_core::identity::Actor;
use pos_core::ids::{BranchId, OrganizationId, ProductId};
use pos_core::money::Money;
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use std::sync::Arc;
use time::OffsetDateTime;

struct FixedClock(OffsetDateTime);

impl Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

fn build_service(pool: sqlx::SqlitePool) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    let clock: SharedClock = Arc::new(FixedClock(OffsetDateTime::now_utc()));
    PosService::new(store, hasher, clock)
}

fn bootstrap_request() -> BootstrapRequest {
    BootstrapRequest {
        organization_name: "Abarrotes La Esquina".into(),
        branch_name: "Matriz".into(),
        branch_code: "mx-01".into(),
        owner_username: "propietario".into(),
        owner_display_name: "Ana Propietaria".into(),
        owner_password: "contrasena-segura".into(),
    }
}

fn product_draft(sku: &str) -> ProductDraft {
    ProductDraft {
        sku: sku.into(),
        name: "Café americano".into(),
        price: "12.50".into(),
        tax_rate_bp: 1600,
        category_id: None,
    }
}

fn movement(
    product_id: ProductId,
    kind: &str,
    quantity: &str,
    reason: Option<&str>,
) -> InventoryMovementDraft {
    InventoryMovementDraft {
        product_id: product_id.to_string(),
        kind: kind.into(),
        quantity: quantity.into(),
        reason: reason.map(str::to_owned),
    }
}

async fn setup() -> (tempfile::TempDir, sqlx::SqlitePool, Actor, ProductId) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("pos.db")).await.unwrap();
    let service = build_service(pool.clone());
    service.bootstrap(bootstrap_request()).await.unwrap();
    let actor = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let product = service
        .create_product(&actor, product_draft("CAF-001"))
        .await
        .unwrap();
    let product_id: ProductId = product.id.to_string().parse().unwrap();
    (dir, pool, actor, product_id)
}

#[tokio::test]
async fn entradas_salidas_y_ajustes_calculan_stock() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool.clone());

    service
        .record_inventory_movement(&actor, movement(product_id, "inbound", "10", None))
        .await
        .unwrap();
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "10"
    );

    service
        .record_inventory_movement(&actor, movement(product_id, "outbound", "2.5", None))
        .await
        .unwrap();
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "7.5"
    );

    service
        .record_inventory_movement(
            &actor,
            movement(product_id, "adjustment_in", "1", Some("conteo físico")),
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            movement(product_id, "adjustment_out", "0.5", Some("merma")),
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "8"
    );

    let history = service
        .list_inventory_movements(&actor, Some(product_id), 10)
        .await
        .unwrap();
    assert_eq!(history.len(), 4);
}

#[tokio::test]
async fn rechaza_stock_insuficiente_y_ajuste_sin_motivo() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool.clone());

    service
        .record_inventory_movement(&actor, movement(product_id, "inbound", "3", None))
        .await
        .unwrap();

    let insufficient = service
        .record_inventory_movement(&actor, movement(product_id, "outbound", "4", None))
        .await
        .unwrap_err();
    assert_eq!(insufficient.code(), "validation_error");

    let without_reason = service
        .record_inventory_movement(&actor, movement(product_id, "adjustment_out", "1", None))
        .await
        .unwrap_err();
    assert_eq!(without_reason.code(), "validation_error");

    // Nada de lo rechazado dejó movimientos ni auditoría parcial.
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "3"
    );
}

#[tokio::test]
async fn ledger_es_append_only_en_sqlite() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool.clone());
    service
        .record_inventory_movement(&actor, movement(product_id, "inbound", "1", None))
        .await
        .unwrap();

    let update = sqlx::query("UPDATE inventory_movement SET quantity_milli = 99")
        .execute(&pool)
        .await;
    assert!(update.is_err());
    let delete = sqlx::query("DELETE FROM inventory_movement")
        .execute(&pool)
        .await;
    assert!(delete.is_err());
}

#[tokio::test]
async fn aislamiento_entre_organizaciones_y_sucursal() {
    let (_dir, pool, actor, _product_id) = setup().await;
    let service = build_service(pool.clone());

    // Producto de otra organización insertado directamente por el adaptador.
    let other_org = OrganizationId::new();
    let other_branch = BranchId::new();
    let foreign_product = Product::new(
        ProductId::new(),
        other_org,
        "FOR-001",
        "Producto ajeno",
        Money::from_minor_units(1000).unwrap(),
        0,
    )
    .unwrap();
    let store = SqliteStore::new(pool.clone());
    let mut tx = store.begin().await.unwrap();
    tx.insert_inventory_movement(
        &pos_core::inventory::InventoryMovement::new(
            other_org,
            other_branch,
            foreign_product.id,
            pos_core::inventory::MovementKind::Inbound,
            pos_core::quantity::Quantity::from_milli_units(1000).unwrap(),
            None,
            None,
        )
        .unwrap(),
    )
    .await
    .unwrap_err();
    let _ = tx.rollback().await;
    assert!(store.begin().await.is_ok());

    // Consultar una sucursal distinta a la del actor se rechaza.
    let denied = service
        .current_stock(&actor, foreign_product.id, Some(BranchId::new()))
        .await
        .unwrap_err();
    assert_eq!(denied.code(), "access_denied");
}

#[tokio::test]
async fn salidas_concurrentes_no_sobrevenden() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = Arc::new(build_service(pool.clone()));

    service
        .record_inventory_movement(&actor, movement(product_id, "inbound", "5", None))
        .await
        .unwrap();

    let first = {
        let service = service.clone();
        let actor = actor.clone();
        tokio::spawn(async move {
            service
                .record_inventory_movement(&actor, movement(product_id, "outbound", "4", None))
                .await
        })
    };
    let second = {
        let service = service.clone();
        let actor = actor.clone();
        tokio::spawn(async move {
            service
                .record_inventory_movement(&actor, movement(product_id, "outbound", "4", None))
                .await
        })
    };

    let results = [first.await.unwrap(), second.await.unwrap()];
    let successes = results.iter().filter(|result| result.is_ok()).count();
    assert_eq!(successes, 1, "solo una salida debe confirmarse");
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "1"
    );
}

#[tokio::test]
async fn migracion_0002_a_0003_conserva_datos_y_preserva_append_only() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    std::fs::create_dir_all(&migrations).unwrap();
    let repo_migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/local");
    std::fs::copy(
        repo_migrations.join("0001_init.sql"),
        migrations.join("0001_init.sql"),
    )
    .unwrap();
    std::fs::copy(
        repo_migrations.join("0002_inventory.sql"),
        migrations.join("0002_inventory.sql"),
    )
    .unwrap();

    let db_path = dir.path().join("migracion.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    let migrator = sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .unwrap();
    migrator.run(&pool).await.unwrap();

    // Datos sintéticos de una instalación previa a 0003.
    let now = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1','Org','2026-01-01T00:00:00Z')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO branch (id, organization_id, name, code, created_at) VALUES ('br-1','org-1','Sucursal','MX-01','2026-01-01T00:00:00Z')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) VALUES ('pr-1','org-1','CAF-001','Café',1000,0,1,'2026-01-01T00:00:00Z')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO inventory_movement (id, organization_id, branch_id, product_id, kind, quantity_milli, reason, actor_user_id, occurred_at) VALUES ('mv-1','org-1','br-1','pr-1','adjustment',1000,'ajuste histórico',NULL,?)")
        .bind(&now).execute(&pool).await.unwrap();

    pool.close().await;

    // Abrir con el código actual aplica 0003.
    let upgraded = open(&db_path).await.unwrap();
    let row = sqlx::query("SELECT kind, quantity_milli FROM inventory_movement WHERE id = 'mv-1'")
        .fetch_one(&upgraded)
        .await
        .unwrap();
    use sqlx::Row;
    assert_eq!(row.get::<String, _>("kind"), "adjustment_in");
    assert_eq!(row.get::<i64, _>("quantity_milli"), 1000);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_movement")
        .fetch_one(&upgraded)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        sqlx::query("UPDATE inventory_movement SET quantity_milli = 1")
            .execute(&upgraded)
            .await
            .is_err()
    );
    upgraded.close().await;
}

#[tokio::test]
async fn categorias_y_variantes_se_persisten_y_auditan() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool.clone());

    let category = service
        .create_category(
            &actor,
            CategoryDraft {
                name: "Bebidas".into(),
                sort_order: 1,
            },
        )
        .await
        .unwrap();
    let categories = service.list_categories(&actor).await.unwrap();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].name, "Bebidas");

    let assigned = service
        .assign_product_category(&actor, product_id, Some(category.id.clone()))
        .await
        .unwrap();
    assert_eq!(assigned.category_id.as_deref(), Some(category.id.as_str()));
    let products = service.list_products(&actor).await.unwrap();
    assert_eq!(
        products[0].category_id.as_deref(),
        Some(category.id.as_str())
    );

    let variant = service
        .create_product_variant(
            &actor,
            ProductVariantDraft {
                product_id: product_id.to_string(),
                name: "Grande".into(),
                sku: Some("CAF-001-G".into()),
                price: "15.00".into(),
                sort_order: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(variant.unit_price, "15.00");
    let variants = service
        .list_product_variants(&actor, product_id)
        .await
        .unwrap();
    assert_eq!(variants.len(), 1);

    // SKU duplicado de variante en la misma organización.
    let duplicate = service
        .create_product_variant(
            &actor,
            ProductVariantDraft {
                product_id: product_id.to_string(),
                name: "Familiar".into(),
                sku: Some("CAF-001-G".into()),
                price: "20.00".into(),
                sort_order: 2,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(duplicate, AppError::Conflict));
    // Asignar una categoría de otra organización debe rechazarse.
    let foreign_store = SqliteStore::new(pool.clone());
    let mut foreign_tx = foreign_store.begin().await.unwrap();
    let foreign_org =
        pos_core::organization::Organization::new(OrganizationId::new(), "Otra organización")
            .unwrap();
    foreign_tx.insert_organization(&foreign_org).await.unwrap();
    let foreign_category = pos_core::catalog::Category::new(foreign_org.id, "Ajena", 0).unwrap();
    foreign_tx.insert_category(&foreign_category).await.unwrap();
    foreign_tx.commit().await.unwrap();
    let cross_org = service
        .assign_product_category(&actor, product_id, Some(foreign_category.id.to_string()))
        .await
        .unwrap_err();
    assert_eq!(cross_org.code(), "validation_error");

    let audit = service.list_audit(&actor, 50).await.unwrap();
    let actions: Vec<&str> = audit.iter().map(|event| event.action.as_str()).collect();
    assert!(actions.contains(&"catalog.category_created"));
    assert!(actions.contains(&"catalog.variant_created"));
}

#[tokio::test]
async fn producto_inactivo_no_acepta_movimientos() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool.clone());
    sqlx::query("UPDATE product SET active = 0 WHERE id = ?")
        .bind(product_id.to_string())
        .execute(&pool)
        .await
        .unwrap();

    let result = service
        .record_inventory_movement(&actor, movement(product_id, "inbound", "1", None))
        .await
        .unwrap_err();
    assert_eq!(result.code(), "validation_error");
}
