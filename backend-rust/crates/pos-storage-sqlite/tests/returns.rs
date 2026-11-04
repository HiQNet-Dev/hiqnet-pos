use pos_application::{
    BootstrapRequest, CashCloseShiftDraft, CashMovementDraft, CashOpenShiftDraft,
    CashRegisterDraft, CheckoutDraft, ExpenseCategoryDraft, ExpenseDraft, InventoryMovementDraft,
    LoginRequest, PosService, ProductDraft, PurchaseOrderDraft, PurchaseOrderLineDraft,
    PurchaseReceiptDraft, PurchaseReceiptLineDraft, ReportsQuery, SaleItemDraft, SaleReturnDraft,
    SaleReturnLineDraft, SaleReturnPreviewDraft, SharedClock, SharedHasher, SharedStore,
    SupplierDraft, WasteDraft,
};
use pos_core::identity::{Actor, Role};
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

struct Fixture {
    dir: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    service: PosService,
    actor: Actor,
    product_id: String,
    register_id: String,
    shift_id: String,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("returns.db")).await.unwrap();
    let service = build_service(pool.clone());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Devoluciones SA".into(),
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
                sku: "CAF-001".into(),
                name: "Café".into(),
                price: "10.00".into(),
                tax_rate_bp: 1600,
                category_id: None,
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product.id.to_string(),
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
                opening_amount: "100.00".into(),
            },
        )
        .await
        .unwrap();
    Fixture {
        dir,
        pool,
        service,
        actor,
        product_id: product.id.to_string(),
        register_id: register.id,
        shift_id: shift.id,
    }
}

async fn sell(fixture: &Fixture, quantity: &str) -> pos_application::SaleView {
    fixture
        .service
        .checkout_sale(
            &fixture.actor,
            CheckoutDraft {
                shift_id: fixture.shift_id.clone(),
                idempotency_key: format!("sale-{quantity}"),
                items: vec![SaleItemDraft {
                    product_id: fixture.product_id.clone(),
                    variant_id: None,
                    quantity: quantity.into(),
                    selected_option_ids: Vec::new(),
                }],
                cash_received: "50.00".into(),
            },
        )
        .await
        .unwrap()
}

fn return_draft(
    sale: &pos_application::SaleView,
    shift_id: &str,
    key: &str,
    quantity: &str,
    disposition: &str,
) -> SaleReturnDraft {
    SaleReturnDraft {
        sale_id: sale.id.clone(),
        shift_id: Some(shift_id.into()),
        reason: "producto en buen estado".into(),
        idempotency_key: key.into(),
        lines: vec![SaleReturnLineDraft {
            sale_item_id: sale.items[0].id.clone(),
            quantity: quantity.into(),
            disposition: disposition.into(),
        }],
    }
}

async fn stock(fixture: &Fixture) -> String {
    fixture
        .service
        .current_stock(&fixture.actor, fixture.product_id.parse().unwrap(), None)
        .await
        .unwrap()
}

async fn new_product(fixture: &Fixture, sku: &str, price: &str, tax_rate_bp: u16) -> String {
    fixture
        .service
        .create_product(
            &fixture.actor,
            ProductDraft {
                sku: sku.into(),
                name: format!("Producto {sku}"),
                price: price.into(),
                tax_rate_bp,
                category_id: None,
            },
        )
        .await
        .unwrap()
        .id
        .to_string()
}

async fn add_stock(fixture: &Fixture, product_id: &str, quantity: &str) {
    fixture
        .service
        .record_inventory_movement(
            &fixture.actor,
            InventoryMovementDraft {
                product_id: product_id.into(),
                kind: "inbound".into(),
                quantity: quantity.into(),
                reason: None,
            },
        )
        .await
        .unwrap();
}

async fn sell_product(
    service: &PosService,
    actor: &Actor,
    shift_id: &str,
    product_id: &str,
    quantity: &str,
    cash_received: &str,
    key: &str,
) -> pos_application::SaleView {
    service
        .checkout_sale(
            actor,
            CheckoutDraft {
                shift_id: shift_id.into(),
                idempotency_key: key.into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.into(),
                    variant_id: None,
                    quantity: quantity.into(),
                    selected_option_ids: Vec::new(),
                }],
                cash_received: cash_received.into(),
            },
        )
        .await
        .unwrap()
}

fn money_text(minor: i64) -> String {
    format!("{}.{:02}", minor / 100, minor % 100)
}

async fn report_between(
    service: &PosService,
    actor: &Actor,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> pos_application::ReportsView {
    service
        .reports_summary(
            actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn devolucion_parcial_cuadra_reembolso_inventario_y_caja() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let preview = fixture
        .service
        .preview_sale_return(
            &fixture.actor,
            SaleReturnPreviewDraft {
                sale_id: sale.id.clone(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "1".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(preview.refund_total, "11.60");

    let created = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-1", "1", "restock"),
        )
        .await
        .unwrap();
    assert_eq!(created.refund_total, "11.60");
    assert_eq!(created.sale_folio, sale.folio);
    assert_eq!(created.lines[0].refund_net, "10.00");
    assert_eq!(created.lines[0].refund_tax, "1.60");
    assert_eq!(stock(&fixture).await, "19");

    let active = fixture
        .service
        .active_cash_shift(&fixture.actor, fixture.register_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "111.60");
    assert_eq!(active.exit_total, "11.60");
    assert_eq!(active.entry_total, "23.20");

    let audit = fixture
        .service
        .list_audit(&fixture.actor, 100)
        .await
        .unwrap();
    assert_eq!(
        audit
            .iter()
            .filter(|event| event.action == "sales.returned")
            .count(),
        1
    );
}

#[tokio::test]
async fn devoluciones_sucesivas_nunca_exceden_y_la_completa_cuadra() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-a", "1", "restock"),
        )
        .await
        .unwrap();
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-b", "1", "restock"),
        )
        .await
        .unwrap();
    assert_eq!(stock(&fixture).await, "20");
    let active = fixture
        .service
        .active_cash_shift(&fixture.actor, fixture.register_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "100.00", "todo el efectivo regresó");
    assert_eq!(active.exit_total, "23.20");

    let excess = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-c", "1", "restock"),
        )
        .await
        .unwrap_err();
    assert_eq!(excess.code(), "conflict");
    assert_eq!(stock(&fixture).await, "20");
}

#[tokio::test]
async fn devolucion_rechaza_turno_cerrado_y_saldo_insuficiente() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .register_cash_movement(
            &fixture.actor,
            CashMovementDraft {
                shift_id: fixture.shift_id.clone(),
                kind: "out".into(),
                amount: "120.00".into(),
                reason: "retiro".into(),
            },
        )
        .await
        .unwrap();
    let error = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "sin-saldo", "1", "restock"),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "validation_error");
    assert_eq!(
        stock(&fixture).await,
        "18",
        "no debe revertir sin reembolso"
    );

    fixture
        .service
        .close_cash_shift(
            &fixture.actor,
            CashCloseShiftDraft {
                shift_id: fixture.shift_id.clone(),
                counted_amount: "3.20".into(),
            },
        )
        .await
        .unwrap();
    let closed = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "turno-cerrado", "1", "restock"),
        )
        .await
        .unwrap_err();
    assert_eq!(closed.code(), "conflict");
    assert_eq!(stock(&fixture).await, "18");
    assert!(
        fixture
            .service
            .list_sale_returns(&fixture.actor)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn devolucion_idempotente_y_conflicto_por_contenido() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let draft = return_draft(&sale, &fixture.shift_id, "misma-clave", "1", "restock");
    let first = fixture
        .service
        .create_sale_return(&fixture.actor, draft.clone())
        .await
        .unwrap();
    let replay = fixture
        .service
        .create_sale_return(&fixture.actor, draft)
        .await
        .unwrap();
    assert_eq!(first.id, replay.id);
    assert_eq!(stock(&fixture).await, "19");

    let conflict = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "misma-clave", "1", "discard"),
        )
        .await
        .unwrap_err();
    assert_eq!(conflict.code(), "conflict");

    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM sale_return) AS returns, (SELECT COUNT(*) FROM sale_return_line) AS lines, (SELECT COUNT(*) FROM cash_movement WHERE kind = 'out') AS exits, (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'devolución %') AS movements",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("returns"), 1);
    assert_eq!(row.get::<i64, _>("lines"), 1);
    assert_eq!(row.get::<i64, _>("exits"), 1);
    assert_eq!(row.get::<i64, _>("movements"), 1);
}

#[tokio::test]
async fn devolucion_no_reintegrable_conserva_stock() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let created = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "merma-devolucion", "1", "discard"),
        )
        .await
        .unwrap();
    assert_eq!(created.lines[0].disposition, "discard");
    assert_eq!(stock(&fixture).await, "18");
    let inbound: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'devolución %'",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(inbound, 0);
}

#[tokio::test]
async fn devoluciones_concurrentes_no_duplican_ni_exceden() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let first = Arc::new(build_service(
        open(fixture.dir.path().join("returns.db")).await.unwrap(),
    ));
    let second = Arc::new(build_service(
        open(fixture.dir.path().join("returns.db")).await.unwrap(),
    ));
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let draft = return_draft(&sale, &fixture.shift_id, "concurrente-1", "1", "restock");
    let left = {
        let service = first.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        let draft = draft.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let right = {
        let service = second.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert!(
        results.iter().all(Result::is_ok),
        "el mismo intento concurrente debe recuperar el hecho: {results:?}"
    );
    assert_eq!(stock(&fixture).await, "19");

    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let left = {
        let service = first.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        let draft = return_draft(&sale, &fixture.shift_id, "concurrente-a", "2", "restock");
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let right = {
        let service = second.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        let draft = return_draft(&sale, &fixture.shift_id, "concurrente-b", "2", "restock");
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        0,
        "solo queda una unidad devolvible"
    );
    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM sale_return) AS returns, (SELECT COALESCE(SUM(quantity_milli), 0) FROM sale_return_line) AS quantity",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("returns"), 1);
    assert_eq!(row.get::<i64, _>("quantity"), 1000);
}

#[tokio::test]
async fn rollback_ante_fallo_de_caja_inventario_o_auditoria() {
    for (name, trigger) in [
        (
            "cash",
            "CREATE TRIGGER fail_return_cash BEFORE INSERT ON cash_movement WHEN NEW.reason LIKE 'devolución %' BEGIN SELECT RAISE(ABORT, 'cash failure'); END",
        ),
        (
            "inventory",
            "CREATE TRIGGER fail_return_inventory BEFORE INSERT ON inventory_movement WHEN NEW.reason LIKE 'devolución %' BEGIN SELECT RAISE(ABORT, 'inventory failure'); END",
        ),
        (
            "audit",
            "CREATE TRIGGER fail_return_audit BEFORE INSERT ON audit_event WHEN NEW.action = 'sales.returned' BEGIN SELECT RAISE(ABORT, 'audit failure'); END",
        ),
    ] {
        let fixture = fixture().await;
        let sale = sell(&fixture, "2").await;
        sqlx::query(trigger).execute(&fixture.pool).await.unwrap();
        let error = fixture
            .service
            .create_sale_return(
                &fixture.actor,
                return_draft(&sale, &fixture.shift_id, name, "1", "restock"),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), "storage_error", "{name}");
        let row = sqlx::query(
            "SELECT (SELECT COUNT(*) FROM sale_return) AS returns, (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'devolución %') AS movements, (SELECT COUNT(*) FROM audit_event WHERE action = 'sales.returned') AS audits",
        )
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(row.get::<i64, _>("returns"), 0, "{name}");
        assert_eq!(row.get::<i64, _>("movements"), 0, "{name}");
        assert_eq!(row.get::<i64, _>("audits"), 0, "{name}");
        assert_eq!(stock(&fixture).await, "18", "{name}");
    }
}

#[tokio::test]
async fn devolucion_aislada_y_denegada_a_cajero() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let cashier = Actor {
        role: Role::Cashier,
        permissions: Role::Cashier.permissions(),
        ..fixture.actor.clone()
    };
    assert_eq!(
        fixture
            .service
            .create_sale_return(
                &cashier,
                return_draft(&sale, &fixture.shift_id, "cashier", "1", "restock"),
            )
            .await
            .unwrap_err()
            .code(),
        "access_denied"
    );
    let foreign_branch = Actor {
        branch_id: Some(BranchId::new()),
        ..fixture.actor.clone()
    };
    assert_eq!(
        fixture
            .service
            .create_sale_return(
                &foreign_branch,
                return_draft(&sale, &fixture.shift_id, "foreign", "1", "restock"),
            )
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    let foreign_organization = Actor {
        organization_id: OrganizationId::new(),
        ..fixture.actor.clone()
    };
    assert_eq!(
        fixture
            .service
            .find_sale_for_return(&foreign_organization, sale.folio.clone())
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
}

#[tokio::test]
async fn devolucion_y_reportes_persisten_tras_reinicio() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "persistente", "1", "restock"),
        )
        .await
        .unwrap();
    fixture.pool.close().await;

    let reopened = open(fixture.dir.path().join("returns.db")).await.unwrap();
    let service = build_service(reopened);
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let returns = service.list_sale_returns(&actor).await.unwrap();
    assert_eq!(returns.len(), 1);
    assert_eq!(returns[0].refund_total, "11.60");
    assert_eq!(
        service
            .current_stock(&actor, fixture.product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "19"
    );
    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let report = service
        .reports_summary(
            &actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(report.sales.count, 1);
    assert_eq!(report.sales.total, "23.20");
    assert_eq!(report.sales.returned_total, "11.60");
    assert_eq!(report.sales.net_total, "11.60");
}

#[tokio::test]
async fn reportes_agregan_cada_fuente_sin_mezclar_ingresos() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "reporte-ret", "1", "restock"),
        )
        .await
        .unwrap();
    let category = fixture
        .service
        .create_expense_category(
            &fixture.actor,
            ExpenseCategoryDraft {
                name: "Operación".into(),
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .record_expense(
            &fixture.actor,
            ExpenseDraft {
                category_id: category.id.clone(),
                amount: "9.00".into(),
                concept: "Externo".into(),
                paid_from_cash: false,
                shift_id: None,
                idempotency_key: "rep-exp-a".into(),
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .record_expense(
            &fixture.actor,
            ExpenseDraft {
                category_id: category.id,
                amount: "12.00".into(),
                concept: "Caja".into(),
                paid_from_cash: true,
                shift_id: Some(fixture.shift_id.clone()),
                idempotency_key: "rep-exp-b".into(),
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .record_waste(
            &fixture.actor,
            WasteDraft {
                product_id: fixture.product_id.clone(),
                quantity: "1".into(),
                reason: "caducidad".into(),
                idempotency_key: "rep-waste".into(),
            },
        )
        .await
        .unwrap();
    let supplier = fixture
        .service
        .create_supplier(
            &fixture.actor,
            SupplierDraft {
                code: Some("SUP-1".into()),
                name: "Proveedor".into(),
                contact: None,
                rfc: None,
            },
        )
        .await
        .unwrap();
    let order = fixture
        .service
        .create_purchase_order(
            &fixture.actor,
            PurchaseOrderDraft {
                supplier_id: supplier.id,
                lines: vec![PurchaseOrderLineDraft {
                    product_id: fixture.product_id.clone(),
                    quantity: "5".into(),
                    unit_cost: "8.00".into(),
                }],
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .confirm_purchase_order(&fixture.actor, order.id.parse().unwrap())
        .await
        .unwrap();
    fixture
        .service
        .receive_purchase_order(
            &fixture.actor,
            PurchaseReceiptDraft {
                order_id: order.id.clone(),
                idempotency_key: "rep-receipt".into(),
                lines: vec![PurchaseReceiptLineDraft {
                    order_line_id: order.lines[0].id.clone(),
                    quantity: "2".into(),
                }],
            },
        )
        .await
        .unwrap();

    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let report = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();

    assert_eq!(report.sales.count, 1);
    assert_eq!(report.sales.subtotal, "20.00");
    assert_eq!(report.sales.tax, "3.20");
    assert_eq!(report.sales.total, "23.20");
    assert_eq!(report.sales.returned_count, 1);
    assert_eq!(report.sales.returned_total, "11.60");
    assert_eq!(report.sales.net_total, "11.60");
    assert_eq!(report.sales.products[0].quantity, "2");
    assert_eq!(report.sales.products[0].net, "20.00");

    assert_eq!(report.cash.shifts_opened, 1);
    assert_eq!(report.cash.entries, "23.20");
    assert_eq!(report.cash.exits, "23.60");
    assert_eq!(report.cash.expenses_from_cash, "12.00");
    assert_eq!(report.cash.refunds, "11.60");

    assert_eq!(report.purchases.orders_created, 1);
    assert_eq!(report.purchases.pending_orders, 1);
    assert_eq!(report.purchases.received_quantity, "2");
    assert_eq!(report.purchases.received_cost, "16.00");

    assert_eq!(report.expenses.total, "21.00");
    assert_eq!(report.expenses.from_cash, "12.00");
    assert_eq!(report.expenses.external, "9.00");
    assert_eq!(report.expenses.categories.len(), 1);

    assert_eq!(report.inventory.stock_quantity, "20");
    assert_eq!(
        report.inventory.inbound, "23",
        "entradas: 20 iniciales + 2 de compra + 1 devuelta"
    );
    assert_eq!(report.inventory.outbound, "2");
    assert_eq!(report.inventory.waste, "1");
    assert_eq!(report.inventory.adjustment_out, "0");
}

#[tokio::test]
async fn reportes_sin_datos_y_filtro_de_fechas() {
    let fixture = fixture().await;
    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let empty = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(empty.sales.count, 0);
    assert_eq!(empty.sales.total, "0.00");
    assert!(empty.sales.products.is_empty());

    sell(&fixture, "2").await;
    let future_from = OffsetDateTime::now_utc() + time::Duration::days(2);
    let future_to = OffsetDateTime::now_utc() + time::Duration::days(3);
    let outside = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: future_from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: future_to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(outside.sales.count, 0, "el intervalo es semiabierto");

    let invalid = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: future_to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: future_from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(invalid.code(), "validation_error");
}

#[tokio::test]
async fn reportes_requieren_permiso_de_consulta() {
    let fixture = fixture().await;
    let cashier = Actor {
        role: Role::Cashier,
        permissions: Role::Cashier.permissions(),
        ..fixture.actor.clone()
    };
    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let error = fixture
        .service
        .reports_summary(
            &cashier,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "access_denied");
}

#[tokio::test]
async fn reembolso_fraccionario_acumulado_no_excede_la_linea() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "CENT-1", "0.01", 0).await;
    add_stock(&fixture, &product, "10").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "3",
        "1.00",
        "frac-sale",
    )
    .await;
    assert_eq!(sale.total, "0.03", "cinco centavos de línea");

    for index in 0..6 {
        fixture
            .service
            .create_sale_return(
                &fixture.actor,
                return_draft(
                    &sale,
                    &fixture.shift_id,
                    &format!("frac-{index}"),
                    "0.5",
                    "restock",
                ),
            )
            .await
            .unwrap();
    }

    let row = sqlx::query(
        "SELECT COALESCE(SUM(refund_total_minor), 0) AS refunded, COUNT(*) AS returns FROM sale_return",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("refunded"), 3, "nunca más que la línea");
    assert_eq!(row.get::<i64, _>("returns"), 6);

    // Los incrementos de cero centavos no crean movimientos de caja ficticios.
    let cash = sqlx::query(
        "SELECT COALESCE(SUM(amount_minor), 0) AS total, COUNT(*) AS movements FROM cash_movement WHERE kind = 'out'",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(cash.get::<i64, _>("total"), 3);
    assert_eq!(cash.get::<i64, _>("movements"), 3);

    let active = fixture
        .service
        .active_cash_shift(&fixture.actor, fixture.register_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "100.00");
    assert_eq!(
        fixture
            .service
            .current_stock(&fixture.actor, product.parse().unwrap(), None)
            .await
            .unwrap(),
        "10"
    );
}

#[tokio::test]
async fn preview_coincide_con_la_transaccion_en_cantidades_fraccionarias() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "TRE-3", "0.03", 0).await;
    add_stock(&fixture, &product, "2").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "0.5",
        "1.00",
        "frac-preview-sale",
    )
    .await;
    assert_eq!(sale.total, "0.02");

    let preview = fixture
        .service
        .preview_sale_return(
            &fixture.actor,
            SaleReturnPreviewDraft {
                sale_id: sale.id.clone(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "0.25".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(preview.refund_total, "0.01");
    let first = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(
                &sale,
                &fixture.shift_id,
                "frac-preview-a",
                "0.25",
                "restock",
            ),
        )
        .await
        .unwrap();
    assert_eq!(first.refund_total, preview.refund_total);

    let second_preview = fixture
        .service
        .preview_sale_return(
            &fixture.actor,
            SaleReturnPreviewDraft {
                sale_id: sale.id.clone(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "0.25".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(second_preview.refund_total, "0.01");
    let second = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(
                &sale,
                &fixture.shift_id,
                "frac-preview-b",
                "0.25",
                "restock",
            ),
        )
        .await
        .unwrap();
    assert_eq!(second.refund_total, second_preview.refund_total);
    assert_eq!(
        first.refund_total.parse::<MoneyView>().unwrap().0
            + second.refund_total.parse::<MoneyView>().unwrap().0,
        2,
        "la devolución completa cuadra con el total vendido"
    );
}

#[tokio::test]
async fn devolucion_de_cero_centavos_no_exige_turno_ni_mueve_caja() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "MIL-1", "0.01", 0).await;
    add_stock(&fixture, &product, "10").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "1",
        "1.00",
        "zero-sale",
    )
    .await;
    fixture
        .service
        .close_cash_shift(
            &fixture.actor,
            CashCloseShiftDraft {
                shift_id: fixture.shift_id.clone(),
                counted_amount: "100.01".into(),
            },
        )
        .await
        .unwrap();

    let created = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            SaleReturnDraft {
                sale_id: sale.id.clone(),
                shift_id: None,
                reason: "reintegro sin importe".into(),
                idempotency_key: "zero-refund".into(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "0.001".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(created.refund_total, "0.00");
    assert_eq!(created.lines[0].refund_total, "0.00");

    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM cash_movement WHERE kind = 'out') AS exits, (SELECT shift_id IS NULL AND cash_movement_id IS NULL FROM sale_return WHERE id = ?) AS sin_caja",
    )
    .bind(&created.id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(
        row.get::<i64, _>("exits"),
        0,
        "sin movimiento de caja ficticio"
    );
    assert_eq!(row.get::<i64, _>("sin_caja"), 1);
    assert_eq!(
        fixture
            .service
            .current_stock(&fixture.actor, product.parse().unwrap(), None)
            .await
            .unwrap(),
        "9.001",
        "la restitución física sí ocurre"
    );
}

#[tokio::test]
async fn reportes_con_devolucion_total_dejan_neto_cero() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "neto-cero", "2", "restock"),
        )
        .await
        .unwrap();

    let report = report_between(
        &fixture.service,
        &fixture.actor,
        OffsetDateTime::now_utc() - time::Duration::days(1),
        OffsetDateTime::now_utc() + time::Duration::days(1),
    )
    .await;
    assert_eq!(report.sales.total, "23.20");
    assert_eq!(report.sales.returned_total, "23.20");
    assert_eq!(report.sales.net_total, "0.00");
}

#[tokio::test]
async fn reportes_neto_negativo_cuando_la_venta_cae_fuera_del_periodo() {
    let fixture = fixture().await;
    let past = build_service_with_clock(
        open(fixture.dir.path().join("returns.db")).await.unwrap(),
        OffsetDateTime::now_utc() - time::Duration::days(2),
    );
    let past_actor = past
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let sale = sell_product(
        &past,
        &past_actor,
        &fixture.shift_id,
        &fixture.product_id,
        "2",
        "50.00",
        "past-sale",
    )
    .await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "neto-fuera", "2", "restock"),
        )
        .await
        .unwrap();

    let report = report_between(
        &fixture.service,
        &fixture.actor,
        OffsetDateTime::now_utc() - time::Duration::days(1),
        OffsetDateTime::now_utc() + time::Duration::days(1),
    )
    .await;
    assert_eq!(report.sales.count, 0, "la venta quedó fuera del período");
    assert_eq!(report.sales.total, "0.00");
    assert_eq!(report.sales.returned_total, "23.20");
    assert_eq!(report.sales.net_total, "-23.20");
}

#[tokio::test]
async fn reportes_soportan_faltantes_sobrantes_y_arqueo_exacto() {
    for (delta_minor, expected) in [(0_i64, "0.00"), (500, "5.00"), (-1000, "-10.00")] {
        let fixture = fixture().await;
        sell(&fixture, "2").await;
        let counted = money_text(12_320 + delta_minor);
        fixture
            .service
            .close_cash_shift(
                &fixture.actor,
                CashCloseShiftDraft {
                    shift_id: fixture.shift_id.clone(),
                    counted_amount: counted,
                },
            )
            .await
            .unwrap();
        let report = report_between(
            &fixture.service,
            &fixture.actor,
            OffsetDateTime::now_utc() - time::Duration::days(1),
            OffsetDateTime::now_utc() + time::Duration::days(1),
        )
        .await;
        assert_eq!(report.cash.difference, expected, "diferencia {delta_minor}");
        assert_eq!(report.cash.shifts_closed, 1);
        assert_eq!(report.cash.entries, "23.20");
    }
}

#[tokio::test]
async fn reportes_por_producto_reconcilian_con_cantidades_fraccionarias() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "REC-3", "0.03", 0).await;
    add_stock(&fixture, &product, "2").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "0.5",
        "1.00",
        "reconcile-sale",
    )
    .await;
    assert_eq!(sale.subtotal, "0.02");
    assert_eq!(sale.total, "0.02");

    let report = report_between(
        &fixture.service,
        &fixture.actor,
        OffsetDateTime::now_utc() - time::Duration::days(1),
        OffsetDateTime::now_utc() + time::Duration::days(1),
    )
    .await;
    assert_eq!(report.sales.subtotal, "0.02");
    assert_eq!(report.sales.total, "0.02");
    assert_eq!(report.sales.products.len(), 1);
    let row = &report.sales.products[0];
    assert_eq!(row.quantity, "0.5");
    assert_eq!(row.net, "0.02", "half-up, no truncamiento");
    assert_eq!(row.tax, "0.00");
    assert_eq!(row.total, "0.02");
}

/// Vista auxiliar para comparar importes exactos en las pruebas.
struct MoneyView(i64);

impl std::str::FromStr for MoneyView {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (units, cents) = value.split_once('.').ok_or(())?;
        let units: i64 = units.parse().map_err(|_| ())?;
        let cents: i64 = cents.parse().map_err(|_| ())?;
        Ok(MoneyView(units * 100 + cents))
    }
}
