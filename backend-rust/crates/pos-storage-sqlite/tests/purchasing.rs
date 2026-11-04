use pos_application::{
    BootstrapRequest, InventoryMovementDraft, LoginRequest, PosService, ProductDraft,
    PurchaseOrderDraft, PurchaseOrderLineDraft, PurchaseReceiptDraft, PurchaseReceiptLineDraft,
    SharedClock, SharedHasher, SharedStore, SupplierDraft,
};
use pos_core::identity::{Actor, Permission};
use pos_core::ids::{BranchId, OrganizationId};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use sqlx::Row;
use std::sync::Arc;
use time::OffsetDateTime;

struct FixedClock(OffsetDateTime);
impl pos_application::Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

fn build_service_with_clock(pool: sqlx::SqlitePool, now: OffsetDateTime) -> PosService {
    PosService::new(
        Arc::new(SqliteStore::new(pool)) as SharedStore,
        Arc::new(Argon2idHasher::new()) as SharedHasher,
        Arc::new(FixedClock(now)) as SharedClock,
    )
}

fn build_service(pool: sqlx::SqlitePool) -> PosService {
    build_service_with_clock(pool, OffsetDateTime::now_utc())
}

async fn setup() -> (
    tempfile::TempDir,
    sqlx::SqlitePool,
    PosService,
    Actor,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("purchasing.db")).await.unwrap();
    let service = build_service(pool.clone());
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
    (dir, pool, service, actor, product.id.to_string())
}

fn receipt_draft(
    order: &pos_application::PurchaseOrderView,
    key: &str,
    quantity: &str,
) -> PurchaseReceiptDraft {
    PurchaseReceiptDraft {
        order_id: order.id.clone(),
        idempotency_key: key.into(),
        lines: vec![PurchaseReceiptLineDraft {
            order_line_id: order.lines[0].id.clone(),
            quantity: quantity.into(),
        }],
    }
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
    let (_dir, _pool, service, actor, product_id) = setup().await;
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
    let (_dir, _pool, service, actor, product_id) = setup().await;
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
    let (_dir, _pool, service, actor, product_id) = setup().await;
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

#[tokio::test]
async fn recepciones_concurrentes_con_misma_clave_son_un_solo_movimiento() {
    let (dir, pool, service, actor, product_id) = setup().await;
    let order = ordered(&service, &actor, &product_id).await;
    let service_a = Arc::new(build_service(
        open(dir.path().join("purchasing.db")).await.unwrap(),
    ));
    let service_b = Arc::new(build_service(
        open(dir.path().join("purchasing.db")).await.unwrap(),
    ));
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let draft = receipt_draft(&order, "concurrent-same-key", "2");
    let first = {
        let service = service_a.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let draft = draft.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.receive_purchase_order(&actor, draft).await
        })
    };
    let second = {
        let service = service_b.clone();
        let actor = actor.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.receive_purchase_order(&actor, draft).await
        })
    };
    let results = [first.await.unwrap(), second.await.unwrap()];
    assert!(
        results.iter().all(Result::is_ok),
        "un reintento concurrente debe recuperar el mismo resultado: {results:?}"
    );
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "2"
    );
    let row = sqlx::query("SELECT (SELECT COUNT(*) FROM purchase_receipt WHERE idempotency_key = 'concurrent-same-key') AS receipts, (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'compra %') AS movements, (SELECT COUNT(*) FROM audit_event WHERE action = 'purchasing.received') AS audits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<i64, _>("receipts"), 1);
    assert_eq!(row.get::<i64, _>("movements"), 1);
    assert_eq!(row.get::<i64, _>("audits"), 1);
}

#[tokio::test]
async fn recepciones_concurrentes_distintas_no_exceden_el_pendiente() {
    let (dir, _pool, service, actor, product_id) = setup().await;
    let order = ordered(&service, &actor, &product_id).await;
    let service_a = Arc::new(build_service(
        open(dir.path().join("purchasing.db")).await.unwrap(),
    ));
    let service_b = Arc::new(build_service(
        open(dir.path().join("purchasing.db")).await.unwrap(),
    ));
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let first = {
        let service = service_a.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let draft = receipt_draft(&order, "concurrent-a", "3");
        tokio::spawn(async move {
            gate.wait().await;
            service.receive_purchase_order(&actor, draft).await
        })
    };
    let second = {
        let service = service_b.clone();
        let actor = actor.clone();
        let draft = receipt_draft(&order, "concurrent-b", "3");
        tokio::spawn(async move {
            gate.wait().await;
            service.receive_purchase_order(&actor, draft).await
        })
    };
    let results = [first.await.unwrap(), second.await.unwrap()];
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "solo una recepción puede consumir el pendiente"
    );
    assert_eq!(
        service
            .current_stock(&actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "3"
    );
}

#[tokio::test]
async fn fallos_despues_de_cabecera_ledger_o_auditoria_hacen_rollback() {
    let failures = [
        (
            "receipt_line",
            "CREATE TRIGGER fail_receipt_line BEFORE INSERT ON purchase_receipt_line BEGIN SELECT RAISE(ABORT, 'receipt line failure'); END",
        ),
        (
            "ledger",
            "CREATE TRIGGER fail_purchase_ledger BEFORE INSERT ON inventory_movement WHEN NEW.reason LIKE 'compra %' BEGIN SELECT RAISE(ABORT, 'ledger failure'); END",
        ),
        (
            "audit",
            "CREATE TRIGGER fail_purchase_audit BEFORE INSERT ON audit_event WHEN NEW.action = 'purchasing.received' BEGIN SELECT RAISE(ABORT, 'audit failure'); END",
        ),
    ];
    for (name, trigger) in failures {
        let (_dir, pool, service, actor, product_id) = setup().await;
        let order = ordered(&service, &actor, &product_id).await;
        sqlx::query(trigger).execute(&pool).await.unwrap();
        let error = service
            .receive_purchase_order(&actor, receipt_draft(&order, name, "2"))
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            "storage_error",
            "{name} debe propagar el fallo SQLite"
        );
        assert_eq!(
            service
                .current_stock(&actor, product_id.parse().unwrap(), None)
                .await
                .unwrap(),
            "0"
        );
        let row = sqlx::query("SELECT COUNT(*) AS receipts, (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'compra %') AS movements, (SELECT COUNT(*) FROM audit_event WHERE action = 'purchasing.received') AS audits FROM purchase_receipt")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(
            row.get::<i64, _>("receipts"),
            0,
            "{name} no debe dejar cabecera"
        );
        assert_eq!(
            row.get::<i64, _>("movements"),
            0,
            "{name} no debe dejar ledger"
        );
        assert_eq!(
            row.get::<i64, _>("audits"),
            0,
            "{name} no debe dejar auditoría"
        );
        assert_eq!(
            service
                .get_purchase_order(&actor, order.id.parse().unwrap())
                .await
                .unwrap()
                .lines[0]
                .received_quantity,
            "0"
        );
    }
}

#[tokio::test]
async fn compras_respetan_licencia_aislamiento_y_reinicio() {
    let (dir, pool, service, actor, product_id) = setup().await;
    let order = ordered(&service, &actor, &product_id).await;
    let foreign_branch = Actor {
        branch_id: Some(BranchId::new()),
        ..actor.clone()
    };
    let foreign_organization = Actor {
        organization_id: OrganizationId::new(),
        ..actor.clone()
    };
    assert_eq!(
        service
            .get_purchase_order(&foreign_branch, order.id.parse().unwrap())
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    assert_eq!(
        service
            .receive_purchase_order(&foreign_organization, receipt_draft(&order, "foreign", "2"))
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    let expired = build_service_with_clock(
        pool.clone(),
        OffsetDateTime::now_utc() + time::Duration::days(31),
    );
    assert_eq!(
        expired
            .receive_purchase_order(&actor, receipt_draft(&order, "expired", "2"))
            .await
            .unwrap_err()
            .code(),
        "commercial_state_blocked"
    );
    let received = service
        .receive_purchase_order(&actor, receipt_draft(&order, "persisted", "2"))
        .await
        .unwrap();
    assert_eq!(received.status, "partially_received");
    pool.close().await;
    let reopened = open(dir.path().join("purchasing.db")).await.unwrap();
    let reopened_service = build_service(reopened.clone());
    let reopened_actor = reopened_service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        reopened_service
            .get_purchase_order(&reopened_actor, order.id.parse().unwrap())
            .await
            .unwrap()
            .lines[0]
            .received_quantity,
        "2"
    );
    assert_eq!(
        reopened_service
            .current_stock(&reopened_actor, product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "2"
    );
}
