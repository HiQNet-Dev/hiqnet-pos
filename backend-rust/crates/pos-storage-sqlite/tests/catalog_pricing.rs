//! Precio al público con IVA incluido: contrato de catálogo, vista previa y
//! coherencia con la venta confirmada por Rust.

use pos_application::{
    BootstrapRequest, CashOpenShiftDraft, CashRegisterDraft, CheckoutDraft, Clock,
    InventoryMovementDraft, LoginRequest, ModifierGroupDraft, PosService, ProductDraft,
    ProductVariantDraft, SaleItemDraft, SharedClock, SharedHasher, SharedStore,
};
use pos_core::identity::Actor;
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

async fn setup() -> (tempfile::TempDir, sqlx::SqlitePool, Actor) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("pos.db")).await.unwrap();
    let service = build_service(pool.clone());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Abarrotes".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "propietario".into(),
            owner_display_name: "Ana".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let actor = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    (dir, pool, actor)
}

async fn product(service: &PosService, actor: &Actor, sku: &str, price: &str, rate: u16) -> String {
    service
        .create_product(
            actor,
            ProductDraft {
                sku: sku.into(),
                name: format!("Producto {sku}"),
                price: price.into(),
                tax_rate_bp: rate,
                category_id: None,
            },
        )
        .await
        .unwrap()
        .id
        .to_string()
}

#[tokio::test]
async fn el_catalogo_publica_el_precio_final_con_impuesto() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let id = product(&service, &actor, "CAF-001", "100.00", 1600).await;

    let view = service
        .list_products(&actor)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id.to_string() == id)
        .unwrap();

    assert_eq!(view.unit_price, "100.00");
    assert_eq!(view.public_unit_total, "116.00");
    assert_eq!(view.tax_rate_bp, 1600);
    assert_eq!(view.active_variants, 0);
    assert_eq!(view.variant_public_from, None);
    assert!(!view.has_modifiers);
}

#[tokio::test]
async fn una_tasa_cero_no_cambia_el_precio_ni_miente() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    product(&service, &actor, "PAN-001", "12.50", 0).await;

    let view = &service.list_products(&actor).await.unwrap()[0];
    assert_eq!(view.unit_price, "12.50");
    assert_eq!(view.public_unit_total, "12.50");
    assert_eq!(view.tax_rate_bp, 0);
}

#[tokio::test]
async fn el_redondeo_del_impuesto_se_aplica_por_unidad() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    // 0.05 * 16 % = 0.008 → 0.01; el precio al público es 0.06, no 0.05.
    product(&service, &actor, "MIN-001", "0.05", 1600).await;

    let view = &service.list_products(&actor).await.unwrap()[0];
    assert_eq!(view.unit_price, "0.05");
    assert_eq!(view.public_unit_total, "0.06");
}

#[tokio::test]
async fn las_variantes_publican_desde_el_precio_mas_bajo() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let id = product(&service, &actor, "LIC-001", "40.00", 1600).await;
    for (name, price, order) in [("Chico", "30.00", 0), ("Grande", "50.00", 1)] {
        service
            .create_product_variant(
                &actor,
                ProductVariantDraft {
                    product_id: id.clone(),
                    name: name.into(),
                    sku: None,
                    price: price.into(),
                    sort_order: order,
                },
            )
            .await
            .unwrap();
    }

    let view = &service.list_products(&actor).await.unwrap()[0];
    // 30.00 + 4.80 = 34.80 es la presentación más barata con impuesto.
    assert_eq!(view.active_variants, 2);
    assert_eq!(view.variant_public_from.as_deref(), Some("34.80"));
    assert_eq!(view.public_unit_total, "46.40");
}

#[tokio::test]
async fn el_catalogo_marca_los_productos_con_modificadores() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let id = product(&service, &actor, "CAF-002", "25.00", 1600).await;
    service
        .create_modifier_group(
            &actor,
            ModifierGroupDraft {
                product_id: id.clone(),
                name: "Extras".into(),
                min: 0,
                max: 2,
                sort_order: 0,
            },
        )
        .await
        .unwrap();

    let view = &service.list_products(&actor).await.unwrap()[0];
    assert!(view.has_modifiers);
    assert_eq!(view.public_unit_total, "29.00");
}

#[tokio::test]
async fn la_vista_previa_coincide_con_el_producto_guardado() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let preview = service
        .preview_product_price(&actor, "100.00", 1600)
        .unwrap();
    assert_eq!(preview.unit_price, "100.00");
    assert_eq!(preview.tax, "16.00");
    assert_eq!(preview.public_unit_total, "116.00");

    product(&service, &actor, "CAF-003", "100.00", 1600).await;
    let view = &service.list_products(&actor).await.unwrap()[0];
    assert_eq!(preview.public_unit_total, view.public_unit_total);

    let zero = service.preview_product_price(&actor, "7.00", 0).unwrap();
    assert_eq!(zero.tax, "0.00");
    assert_eq!(zero.public_unit_total, "7.00");

    assert!(service.preview_product_price(&actor, "abc", 1600).is_err());
    assert!(
        service
            .preview_product_price(&actor, "10.00", 10_001)
            .is_err()
    );
}

#[tokio::test]
async fn la_venta_cobra_exactamente_el_precio_publicado() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let id = product(&service, &actor, "CAF-004", "100.00", 1600).await;
    let published = service.list_products(&actor).await.unwrap()[0]
        .public_unit_total
        .clone();
    assert_eq!(published, "116.00");

    let register = service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja".into(),
                code: "CJ-01".into(),
            },
        )
        .await
        .unwrap();
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "0.00".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: id.clone(),
                kind: "inbound".into(),
                quantity: "10".into(),
                reason: None,
            },
        )
        .await
        .unwrap();

    let draft = |key: &str| CheckoutDraft {
        shift_id: shift.id.clone(),
        idempotency_key: key.into(),
        items: vec![SaleItemDraft {
            product_id: id.clone(),
            variant_id: None,
            quantity: "2".into(),
            selected_option_ids: vec![],
        }],
        cash_received: "250.00".into(),
    };

    let sale = service
        .checkout_sale(&actor, draft("precio-publico-1"))
        .await
        .unwrap();
    assert_eq!(sale.subtotal, "200.00");
    assert_eq!(sale.tax, "32.00");
    assert_eq!(sale.total, "232.00");
    // La partida conserva el neto y el impuesto por separado; el total de la
    // partida es el precio publicado por la cantidad.
    assert_eq!(sale.items[0].unit_price, "100.00");
    assert_eq!(sale.items[0].unit_tax, "16.00");
    assert_eq!(sale.items[0].line_total, "232.00");
    assert_eq!(sale.payment.as_ref().unwrap().change, "18.00");

    // Reintento idempotente: el mismo cobro no duplica la venta ni el importe.
    let again = service
        .checkout_sale(&actor, draft("precio-publico-1"))
        .await
        .unwrap();
    assert_eq!(again.folio, sale.folio);
    assert_eq!(again.total, sale.total);
    assert_eq!(
        service.list_products(&actor).await.unwrap()[0].public_unit_total,
        published
    );
}
