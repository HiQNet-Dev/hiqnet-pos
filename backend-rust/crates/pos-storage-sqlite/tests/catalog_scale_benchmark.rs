//! Benchmark reproducible del enriquecimiento de `list_products`.
//!
//! Se ejecuta manualmente porque siembra hasta 10,000 productos en SQLite:
//! `cargo test --release -p pos-storage-sqlite --test catalog_scale_benchmark -- --ignored --nocapture`.
//! Cada caso usa una base temporal aislada; no abre ni modifica una base de una
//! instalación. Mide consultas SQLite, listado de aplicación y serialización.
//! IPC, parseo TypeScript y render React se miden en su propio arnés, no se
//! atribuyen falsamente a SQLite.

use pos_application::{
    BootstrapRequest, Clock, LoginRequest, PosService, SharedClock, SharedHasher, SharedStore,
};
use pos_core::identity::Actor;
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use serde::Serialize;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

const SAMPLES: usize = 25;
const INSERT_BATCH_SIZE: usize = 250;
const CREATED_AT: &str = "2026-01-01T00:00:00Z";

fn product_id(index: usize) -> String {
    format!("10000000-0000-4000-8000-{index:012x}")
}

fn variant_id(index: usize) -> String {
    format!("20000000-0000-4000-8000-{index:012x}")
}

fn modifier_id(index: usize) -> String {
    format!("30000000-0000-4000-8000-{index:012x}")
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Debug, Serialize)]
struct Timing {
    median_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
}

#[derive(Debug, Serialize)]
struct CatalogScaleResult {
    products: usize,
    active_variants: usize,
    products_with_modifiers: usize,
    query_count_per_list: u8,
    product_query: Timing,
    configuration_queries: Timing,
    application_list: Timing,
    json_serialization: Timing,
    cold_application_list_ms: f64,
    json_bytes: usize,
    product_query_plan: Vec<String>,
}

fn service(pool: SqlitePool) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    let clock: SharedClock = Arc::new(SystemClock);
    PosService::new(store, hasher, clock)
}

async fn setup() -> (tempfile::TempDir, SqlitePool, Actor) {
    let directory = tempfile::tempdir().expect("directorio temporal");
    let pool = open(directory.path().join("catalog-scale.db"))
        .await
        .expect("abrir SQLite temporal");
    let app = service(pool.clone());
    app.bootstrap(BootstrapRequest {
        organization_name: "Benchmark sintético".into(),
        branch_name: "Matriz".into(),
        branch_code: "BENCH-01".into(),
        owner_username: "benchmark".into(),
        owner_display_name: "Benchmark".into(),
        owner_password: "contrasena-segura".into(),
    })
    .await
    .expect("bootstrap sintético");
    let actor = app
        .login(LoginRequest {
            username: "benchmark".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .expect("login sintético");
    (directory, pool, actor)
}

async fn seed_catalog(pool: &SqlitePool, organization_id: &str, products: usize) {
    for first in (0..products).step_by(INSERT_BATCH_SIZE) {
        let end = (first + INSERT_BATCH_SIZE).min(products);
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at) ",
        );
        query.push_values(first..end, |mut values, index| {
            values
                .push_bind(product_id(index))
                .push_bind(organization_id)
                .push_bind(format!("SKU-{index:05}"))
                .push_bind(format!("Producto sintético {index:05}"))
                .push_bind(100_i64 + i64::try_from(index % 10_000).expect("índice válido"))
                .push_bind([0_i64, 800, 1600][index % 3])
                .push_bind(i64::from(index % 13 != 0))
                .push_bind(Option::<String>::None)
                .push_bind(CREATED_AT);
        });
        query
            .build()
            .execute(pool)
            .await
            .expect("sembrar productos sintéticos");
    }

    let variant_products = (0..products).step_by(10).collect::<Vec<_>>();
    for chunk in variant_products.chunks(INSERT_BATCH_SIZE) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO product_variant (id, organization_id, product_id, name, sku, unit_price_minor, active, sort_order, created_at) ",
        );
        query.push_values(chunk.iter().copied(), |mut values, index| {
            values
                .push_bind(variant_id(index))
                .push_bind(organization_id)
                .push_bind(product_id(index))
                .push_bind("Presentación estándar")
                .push_bind(Option::<String>::None)
                .push_bind(90_i64 + i64::try_from(index % 10_000).expect("índice válido"))
                .push_bind(1_i64)
                .push_bind(0_i64)
                .push_bind(CREATED_AT);
        });
        query
            .build()
            .execute(pool)
            .await
            .expect("sembrar variantes sintéticas");
    }

    let modifier_products = (0..products).step_by(20).collect::<Vec<_>>();
    for chunk in modifier_products.chunks(INSERT_BATCH_SIZE) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO modifier_group (id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at) ",
        );
        query.push_values(chunk.iter().copied(), |mut values, index| {
            values
                .push_bind(modifier_id(index))
                .push_bind(organization_id)
                .push_bind(product_id(index))
                .push_bind("Extras")
                .push_bind(0_i64)
                .push_bind(2_i64)
                .push_bind(0_i64)
                .push_bind(1_i64)
                .push_bind(CREATED_AT);
        });
        query
            .build()
            .execute(pool)
            .await
            .expect("sembrar modificadores sintéticos");
    }
}

fn summarize(mut values: Vec<Duration>) -> Timing {
    values.sort_unstable();
    let at = |percentile: f64| {
        let index = ((values.len() as f64 * percentile).ceil() as usize)
            .saturating_sub(1)
            .min(values.len().saturating_sub(1));
        values[index].as_secs_f64() * 1_000.0
    };
    Timing {
        median_ms: at(0.50),
        p95_ms: at(0.95),
        p99_ms: at(0.99),
    }
}

async fn raw_product_query(pool: &SqlitePool, organization_id: &str, expected: usize) -> Duration {
    let started = Instant::now();
    let rows = sqlx::query(
        "SELECT id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at \
         FROM product WHERE organization_id = ? ORDER BY name COLLATE NOCASE",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .expect("consultar productos");
    assert_eq!(rows.len(), expected);
    started.elapsed()
}

async fn raw_configuration_queries(pool: &SqlitePool, organization_id: &str) -> Duration {
    let started = Instant::now();
    let variants = sqlx::query(
        "SELECT id, organization_id, product_id, name, sku, unit_price_minor, active, sort_order, created_at \
         FROM product_variant WHERE organization_id = ? AND active = 1 \
         ORDER BY product_id, sort_order, name COLLATE NOCASE",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .expect("consultar variantes");
    let modifiers = sqlx::query(
        "SELECT DISTINCT product_id FROM modifier_group WHERE organization_id = ? AND active = 1",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .expect("consultar modificadores");
    assert!(!variants.is_empty());
    assert!(!modifiers.is_empty());
    started.elapsed()
}

async fn product_query_plan(pool: &SqlitePool, organization_id: &str) -> Vec<String> {
    sqlx::query(
        "EXPLAIN QUERY PLAN SELECT id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at \
         FROM product WHERE organization_id = ? ORDER BY name COLLATE NOCASE",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .expect("explicar consulta de catálogo")
    .into_iter()
    .map(|row| row.get("detail"))
    .collect()
}

async fn run_case(products: usize) -> CatalogScaleResult {
    let (_directory, pool, actor) = setup().await;
    let organization_id = actor.organization_id.to_string();
    seed_catalog(&pool, &organization_id, products).await;
    let app = service(pool.clone());

    let plan = product_query_plan(&pool, &organization_id).await;
    assert!(
        plan.iter()
            .any(|detail| detail.contains("idx_product_org_name")),
        "el plan debe usar el índice compuesto: {plan:?}"
    );

    let cold_started = Instant::now();
    let initial = app
        .list_products(&actor)
        .await
        .expect("listar catálogo frío");
    let cold_application_list_ms = cold_started.elapsed().as_secs_f64() * 1_000.0;
    assert_eq!(initial.len(), products);

    let mut product_query = Vec::with_capacity(SAMPLES);
    let mut configuration_queries = Vec::with_capacity(SAMPLES);
    let mut application_list = Vec::with_capacity(SAMPLES);
    let mut json_serialization = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        product_query.push(raw_product_query(&pool, &organization_id, products).await);
        configuration_queries.push(raw_configuration_queries(&pool, &organization_id).await);

        let started = Instant::now();
        let views = app
            .list_products(&actor)
            .await
            .expect("listar catálogo warm");
        application_list.push(started.elapsed());
        assert_eq!(views.len(), products);

        let started = Instant::now();
        let json = serde_json::to_vec(&views).expect("serializar catálogo");
        json_serialization.push(started.elapsed());
        assert!(!json.is_empty());
    }

    let json_bytes = serde_json::to_vec(&initial)
        .expect("serializar catálogo final")
        .len();
    CatalogScaleResult {
        products,
        active_variants: products.div_ceil(10),
        products_with_modifiers: products.div_ceil(20),
        query_count_per_list: 3,
        product_query: summarize(product_query),
        configuration_queries: summarize(configuration_queries),
        application_list: summarize(application_list),
        json_serialization: summarize(json_serialization),
        cold_application_list_ms,
        json_bytes,
        product_query_plan: plan,
    }
}

#[tokio::test]
#[ignore = "benchmark manual: siembra 100, 1,000 y 10,000 productos en SQLite temporal"]
async fn catalogo_escalable_100_1000_y_10000() {
    let results = [100, 1_000, 10_000]
        .into_iter()
        .map(run_case)
        .collect::<Vec<_>>();
    let mut completed = Vec::with_capacity(results.len());
    for future in results {
        completed.push(future.await);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&completed).expect("serializar resultados")
    );
}
