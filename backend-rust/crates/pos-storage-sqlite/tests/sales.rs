use pos_application::{
    BootstrapRequest, CashCloseShiftDraft, CashOpenShiftDraft, CashRegisterDraft, CheckoutDraft,
    Clock, InventoryMovementDraft, LoginRequest, PosService, ProductDraft, SaleItemDraft,
    SharedClock, SharedHasher, SharedStore,
};
use pos_core::identity::Actor;
use pos_core::ids::BranchId;
use pos_core::ids::ProductId;
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use sqlx::Row;
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

async fn setup() -> (tempfile::TempDir, sqlx::SqlitePool, Actor, ProductId) {
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
    let product = service
        .create_product(
            &actor,
            ProductDraft {
                sku: "CAF-001".into(),
                name: "Café".into(),
                price: "10.00".into(),
                tax_rate_bp: 1600,
                category_id: None,
            },
        )
        .await
        .unwrap();
    let product_id: ProductId = product.id;
    service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja".into(),
                code: "CJ-01".into(),
            },
        )
        .await
        .unwrap();
    (dir, pool, actor, product_id)
}

#[tokio::test]
async fn venta_en_efectivo_actualiza_inventario_y_caja() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool.clone());
    let register = service.list_cash_registers(&actor).await.unwrap().remove(0);
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "50.00".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product_id.to_string(),
                kind: "inbound".into(),
                quantity: "10".into(),
                reason: None,
            },
        )
        .await
        .unwrap();

    let sale = service
        .checkout_sale(
            &actor,
            CheckoutDraft {
                shift_id: shift.id.clone(),
                idempotency_key: "checkout-1".into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.to_string(),
                    variant_id: None,
                    quantity: "2".into(),
                    selected_option_ids: vec![],
                }],
                cash_received: "30.00".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(sale.subtotal, "20.00");
    assert_eq!(sale.tax, "3.20");
    assert_eq!(sale.total, "23.20");
    assert_eq!(sale.payment.as_ref().unwrap().change, "6.80");

    let stock = service
        .current_stock(&actor, product_id, None)
        .await
        .unwrap();
    assert_eq!(stock, "8");
    let active = service
        .active_cash_shift(&actor, register.id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "73.20");

    // Reintento idempotente.
    let again = service
        .checkout_sale(
            &actor,
            CheckoutDraft {
                shift_id: shift.id,
                idempotency_key: "checkout-1".into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.to_string(),
                    variant_id: None,
                    quantity: "2".into(),
                    selected_option_ids: vec![],
                }],
                cash_received: "30.00".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(again.id, sale.id);
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "8"
    );
    let counts = sqlx::query(
        "SELECT \
         (SELECT COUNT(*) FROM sale WHERE idempotency_key = 'checkout-1') AS sales, \
         (SELECT COUNT(*) FROM sale_payment) AS payments, \
         (SELECT COUNT(*) FROM cash_movement WHERE reason LIKE 'venta %') AS cash_movements, \
         (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'venta %') AS inventory_movements, \
         (SELECT COUNT(*) FROM audit_event WHERE action = 'sales.completed') AS audits",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts.get::<i64, _>("sales"), 1);
    assert_eq!(counts.get::<i64, _>("payments"), 1);
    assert_eq!(counts.get::<i64, _>("cash_movements"), 1);
    assert_eq!(counts.get::<i64, _>("inventory_movements"), 1);
    assert_eq!(counts.get::<i64, _>("audits"), 1);
}

#[tokio::test]
async fn idempotencia_tras_cierre_y_conflicto_por_contenido() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool);
    let register = service.list_cash_registers(&actor).await.unwrap().remove(0);
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "0".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product_id.to_string(),
                kind: "inbound".into(),
                quantity: "10".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let draft = CheckoutDraft {
        shift_id: shift.id.clone(),
        idempotency_key: "idem-1".into(),
        items: vec![SaleItemDraft {
            product_id: product_id.to_string(),
            variant_id: None,
            quantity: "1".into(),
            selected_option_ids: vec![],
        }],
        cash_received: "20.00".into(),
    };
    let sale = service.checkout_sale(&actor, draft.clone()).await.unwrap();
    service
        .close_cash_shift(
            &actor,
            CashCloseShiftDraft {
                shift_id: shift.id.clone(),
                counted_amount: "0.00".into(),
            },
        )
        .await
        .unwrap();
    let retry = service.checkout_sale(&actor, draft.clone()).await.unwrap();
    assert_eq!(retry.id, sale.id);
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "9"
    );
    let mut changed = draft;
    changed.items[0].quantity = "2".into();
    let conflict = service.checkout_sale(&actor, changed).await.unwrap_err();
    assert!(matches!(conflict, pos_application::AppError::Conflict));
}

#[tokio::test]
async fn get_sale_aisla_por_sucursal() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool);
    let register = service.list_cash_registers(&actor).await.unwrap().remove(0);
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "0".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product_id.to_string(),
                kind: "inbound".into(),
                quantity: "5".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let sale = service
        .checkout_sale(
            &actor,
            CheckoutDraft {
                shift_id: shift.id,
                idempotency_key: "branch-sale".into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.to_string(),
                    variant_id: None,
                    quantity: "1".into(),
                    selected_option_ids: vec![],
                }],
                cash_received: "20.00".into(),
            },
        )
        .await
        .unwrap();
    let other_actor = Actor {
        branch_id: Some(BranchId::new()),
        ..actor
    };
    let denied = service
        .get_sale(&other_actor, sale.id.parse().unwrap())
        .await
        .unwrap_err();
    assert_eq!(denied.code(), "not_found");
}

#[tokio::test]
async fn pago_insuficiente_revierte_todo() {
    let (_dir, pool, actor, product_id) = setup().await;
    let service = build_service(pool);
    let register = service.list_cash_registers(&actor).await.unwrap().remove(0);
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "0".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product_id.to_string(),
                kind: "inbound".into(),
                quantity: "5".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let error = service
        .checkout_sale(
            &actor,
            CheckoutDraft {
                shift_id: shift.id,
                idempotency_key: "checkout-fail".into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.to_string(),
                    variant_id: None,
                    quantity: "1".into(),
                    selected_option_ids: vec![],
                }],
                cash_received: "1.00".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "validation_error");
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "5"
    );
}
