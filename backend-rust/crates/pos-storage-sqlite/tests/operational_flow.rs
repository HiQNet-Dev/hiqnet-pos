use pos_application::{
    BootstrapRequest, CashCloseShiftDraft, CashOpenShiftDraft, CashRegisterDraft, CategoryDraft,
    CheckoutDraft, Clock, InventoryMovementDraft, LoginRequest, ModifierGroupDraft,
    ModifierOptionDraft, PosService, ProductDraft, ProductVariantDraft, SaleItemDraft, SharedClock,
    SharedHasher, SharedStore,
};
use pos_core::identity::Actor;
use pos_core::ids::ProductId;
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

async fn login(service: &PosService) -> Actor {
    service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn piloto_operativo_completo_con_persistencia() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("piloto.db");
    let service = build_service(open(&db_path).await.unwrap());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Cafetería Piloto".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "propietario".into(),
            owner_display_name: "Ana".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let actor = login(&service).await;

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
    let product = service
        .create_product(
            &actor,
            ProductDraft {
                sku: "LIC-001".into(),
                name: "Licuado de fresa".into(),
                price: "30.00".into(),
                tax_rate_bp: 1600,
                category_id: Some(category.id),
            },
        )
        .await
        .unwrap();
    let product_id: ProductId = product.id;
    let variant = service
        .create_product_variant(
            &actor,
            ProductVariantDraft {
                product_id: product_id.to_string(),
                name: "Grande".into(),
                sku: Some("LIC-001-G".into()),
                price: "35.00".into(),
                sort_order: 1,
            },
        )
        .await
        .unwrap();
    let group = service
        .create_modifier_group(
            &actor,
            ModifierGroupDraft {
                product_id: product_id.to_string(),
                name: "Extras".into(),
                min: 1,
                max: 2,
                sort_order: 1,
            },
        )
        .await
        .unwrap();
    let option = service
        .create_modifier_option(
            &actor,
            ModifierOptionDraft {
                group_id: group.id,
                name: "Proteína".into(),
                kind: "extra".into(),
                price_delta: "5.00".into(),
                sort_order: 1,
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
                quantity: "20".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let register = service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja principal".into(),
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
                opening_amount: "100.00".into(),
            },
        )
        .await
        .unwrap();
    let sale = service
        .checkout_sale(
            &actor,
            CheckoutDraft {
                shift_id: shift.id.clone(),
                idempotency_key: "piloto-1".into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.to_string(),
                    variant_id: Some(variant.id),
                    quantity: "2".into(),
                    selected_option_ids: vec![option.id],
                }],
                cash_received: "100.00".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(sale.subtotal, "80.00");
    assert_eq!(sale.tax, "12.80");
    assert_eq!(sale.total, "92.80");
    assert_eq!(
        service
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "18"
    );
    let active = service
        .active_cash_shift(&actor, register.id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "192.80");

    // Reinicio completo de la aplicación/base.
    drop(service);
    let reopened = build_service(open(&db_path).await.unwrap());
    let actor = login(&reopened).await;
    let recovered = reopened
        .get_sale(&actor, sale.id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(recovered.id, sale.id);
    assert_eq!(recovered.folio, sale.folio);
    assert_eq!(
        reopened
            .current_stock(&actor, product_id, None)
            .await
            .unwrap(),
        "18"
    );
    assert_eq!(
        reopened
            .active_cash_shift(&actor, register.id.parse().unwrap())
            .await
            .unwrap()
            .unwrap()
            .expected_amount,
        "192.80"
    );

    let closed = reopened
        .close_cash_shift(
            &actor,
            CashCloseShiftDraft {
                shift_id: shift.id,
                counted_amount: "192.80".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(closed.difference_minor, Some(0));
    let history = reopened.list_cash_shift_history(&actor).await.unwrap();
    assert_eq!(history.len(), 1);
    let audit = reopened.list_audit(&actor, 100).await.unwrap();
    let actions: Vec<&str> = audit.iter().map(|event| event.action.as_str()).collect();
    assert!(actions.contains(&"sales.completed"));
    assert!(actions.contains(&"cash.shift_opened"));
    assert!(actions.contains(&"cash.shift_closed"));
}
