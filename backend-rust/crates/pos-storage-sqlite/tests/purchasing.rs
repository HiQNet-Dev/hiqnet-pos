use pos_application::{
    BootstrapRequest, InventoryMovementDraft, LoginRequest, PosService, ProductDraft,
    PurchaseOrderDraft, PurchaseOrderLineDraft, PurchaseReceiptDraft, PurchaseReceiptLineDraft,
    SharedClock, SharedHasher, SharedStore, SupplierDraft,
};
use pos_core::identity::{Actor, Permission};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use std::sync::Arc;
use time::OffsetDateTime;

struct FixedClock(OffsetDateTime);
impl pos_application::Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

fn service(pool: sqlx::SqlitePool) -> PosService {
    PosService::new(
        Arc::new(SqliteStore::new(pool)) as SharedStore,
        Arc::new(Argon2idHasher::new()) as SharedHasher,
        Arc::new(FixedClock(OffsetDateTime::now_utc())) as SharedClock,
    )
}

async fn setup() -> (tempfile::TempDir, PosService, Actor, String) {
    let dir = tempfile::tempdir().unwrap();
    let service = service(open(&dir.path().join("purchasing.db")).await.unwrap());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Compras SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let product = service
        .create_product(
            &actor,
            ProductDraft {
                sku: "INS-001".into(),
                name: "Insumo".into(),
                price: "15.00".into(),
                tax_rate_bp: 0,
                category_id: None,
            },
        )
        .await
        .unwrap();
    (dir, service, actor, product.id.to_string())
}

async fn ordered(
    service: &PosService,
    actor: &Actor,
    product_id: &str,
) -> pos_application::PurchaseOrderView {
    let supplier = service
        .create_supplier(
            actor,
            SupplierDraft {
                code: Some("SUP-01".into()),
                name: "Proveedor local".into(),
                contact: None,
                rfc: None,
            },
        )
        .await
        .unwrap();
    let order = service
        .create_purchase_order(
            actor,
            PurchaseOrderDraft {
                supplier_id: supplier.id,
                lines: vec![PurchaseOrderLineDraft {
                    product_id: product_id.into(),
                    quantity: "5".into(),
                    unit_cost: "8.25".into(),
                }],
            },
        )
        .await
        .unwrap();
    service
        .confirm_purchase_order(actor, order.id.parse().unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn orden_no_mueve_stock_y_recepciones_parciales_cuadran() {
    let (_dir, service, actor, product_id) = setup().await;
    let order = ordered(&service, &actor, &product_id).await;
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "0"
    );
    let first = service
        .receive_purchase_order(
            &actor,
            PurchaseReceiptDraft {
                order_id: order.id.clone(),
                idempotency_key: "recepcion-1".into(),
                lines: vec![PurchaseReceiptLineDraft {
                    order_line_id: order.lines[0].id.clone(),
                    quantity: "2".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(first.status, "partially_received");
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "2"
    );
    let final_order = service
        .receive_purchase_order(
            &actor,
            PurchaseReceiptDraft {
                order_id: order.id,
                idempotency_key: "recepcion-2".into(),
                lines: vec![PurchaseReceiptLineDraft {
                    order_line_id: first.lines[0].id.clone(),
                    quantity: "3".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(final_order.status, "received");
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "5"
    );
    let audit = service.list_audit(&actor, 50).await.unwrap();
    assert!(
        audit
            .iter()
            .any(|event| event.action == "purchasing.supplier_created")
    );
    assert!(
        audit
            .iter()
            .any(|event| event.action == "purchasing.order_created")
    );
    assert!(
        audit
            .iter()
            .any(|event| event.action == "purchasing.received")
    );
}

#[tokio::test]
async fn reintento_idempotente_no_duplica_ni_admite_contenido_distinto() {
    let (_dir, service, actor, product_id) = setup().await;
    let order = ordered(&service, &actor, &product_id).await;
    let draft = PurchaseReceiptDraft {
        order_id: order.id.clone(),
        idempotency_key: "mismo-intento".into(),
        lines: vec![PurchaseReceiptLineDraft {
            order_line_id: order.lines[0].id.clone(),
            quantity: "2".into(),
        }],
    };
    service
        .receive_purchase_order(&actor, draft.clone())
        .await
        .unwrap();
    service.receive_purchase_order(&actor, draft).await.unwrap();
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "2"
    );
    let error = service
        .receive_purchase_order(
            &actor,
            PurchaseReceiptDraft {
                order_id: order.id,
                idempotency_key: "mismo-intento".into(),
                lines: vec![PurchaseReceiptLineDraft {
                    order_line_id: order.lines[0].id.clone(),
                    quantity: "1".into(),
                }],
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "conflict");
}

#[tokio::test]
async fn cajero_no_administra_compras_y_exceso_hace_rollback() {
    let (_dir, service, actor, product_id) = setup().await;
    let cashier = Actor {
        user_id: actor.user_id,
        organization_id: actor.organization_id,
        branch_id: actor.branch_id,
        display_name: "Cajero".into(),
        role: pos_core::identity::Role::Cashier,
        permissions: pos_core::identity::Role::Cashier.permissions(),
    };
    assert!(!cashier.has(Permission::ManagePurchasing));
    assert!(service.list_suppliers(&cashier).await.is_err());
    let order = ordered(&service, &actor, &product_id).await;
    let error = service
        .receive_purchase_order(
            &actor,
            PurchaseReceiptDraft {
                order_id: order.id,
                idempotency_key: "exceso".into(),
                lines: vec![PurchaseReceiptLineDraft {
                    order_line_id: order.lines[0].id.clone(),
                    quantity: "6".into(),
                }],
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "conflict");
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "0"
    );
    let _ = InventoryMovementDraft {
        product_id,
        kind: "inbound".into(),
        quantity: "1".into(),
        reason: None,
    };
}
