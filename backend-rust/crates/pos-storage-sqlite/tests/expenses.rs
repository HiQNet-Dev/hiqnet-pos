use pos_application::{
    BootstrapRequest, CashOpenShiftDraft, CashRegisterDraft, ExpenseCategoryDraft, ExpenseDraft,
    InventoryMovementDraft, LoginRequest, PosService, ProductDraft, SharedClock, SharedHasher,
    SharedStore, WasteDraft,
};
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
fn service(pool: sqlx::SqlitePool) -> PosService {
    service_at(pool, OffsetDateTime::now_utc())
}

fn service_at(pool: sqlx::SqlitePool, now: OffsetDateTime) -> PosService {
    PosService::new(
        Arc::new(SqliteStore::new(pool)) as SharedStore,
        Arc::new(Argon2idHasher::new()) as SharedHasher,
        Arc::new(FixedClock(now)) as SharedClock,
    )
}
async fn setup() -> (
    tempfile::TempDir,
    sqlx::SqlitePool,
    PosService,
    pos_core::identity::Actor,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("expenses.db")).await.unwrap();
    let service = service(pool.clone());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Pruebas".into(),
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
                sku: "MER-01".into(),
                name: "Insumo".into(),
                price: "10.00".into(),
                tax_rate_bp: 0,
                category_id: None,
            },
        )
        .await
        .unwrap();
    (dir, pool, service, actor, product.id.to_string())
}

#[tokio::test]
async fn gasto_externo_no_modifica_caja_y_reintento_no_duplica() {
    let (_dir, pool, service, actor, _product) = setup().await;
    let category = service
        .create_expense_category(
            &actor,
            ExpenseCategoryDraft {
                name: "Servicios".into(),
            },
        )
        .await
        .unwrap();
    let draft = ExpenseDraft {
        category_id: category.id,
        amount: "25.50".into(),
        concept: "Internet".into(),
        paid_from_cash: false,
        shift_id: None,
        idempotency_key: "externo-1".into(),
    };
    service.record_expense(&actor, draft.clone()).await.unwrap();
    service.record_expense(&actor, draft).await.unwrap();
    assert_eq!(service.list_expenses(&actor).await.unwrap().len(), 1);
    let movements: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cash_movement")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(movements, 0);
}

#[tokio::test]
async fn gasto_desde_caja_y_merma_actualizan_solo_su_ledger() {
    let (_dir, pool, service, actor, product) = setup().await;
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product.clone(),
                kind: "inbound".into(),
                quantity: "5".into(),
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
    let category = service
        .create_expense_category(
            &actor,
            ExpenseCategoryDraft {
                name: "Operación".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_expense(
            &actor,
            ExpenseDraft {
                category_id: category.id,
                amount: "12.50".into(),
                concept: "Limpieza".into(),
                paid_from_cash: true,
                shift_id: Some(shift.id),
                idempotency_key: "cash-1".into(),
            },
        )
        .await
        .unwrap();
    let active = service
        .active_cash_shift(&actor, register.id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "87.50");
    service
        .record_waste(
            &actor,
            WasteDraft {
                product_id: product.clone(),
                quantity: "2".into(),
                reason: "caducidad".into(),
                idempotency_key: "waste-1".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .current_stock(&actor, product.parse().unwrap(), None)
            .await
            .unwrap(),
        "3"
    );
    let cash: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cash_movement WHERE kind = 'out'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let waste: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM waste_record")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((cash, waste), (1, 1));
}

#[tokio::test]
async fn gastos_y_mermas_concurrentes_no_duplican_el_hecho_ni_sus_ledgers() {
    let (dir, pool, service, actor, product) = setup().await;
    let category = service
        .create_expense_category(
            &actor,
            ExpenseCategoryDraft {
                name: "Operación".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product.clone(),
                kind: "inbound".into(),
                quantity: "5".into(),
                reason: None,
            },
        )
        .await
        .unwrap();

    let first = Arc::new(service_at(
        open(dir.path().join("expenses.db")).await.unwrap(),
        OffsetDateTime::now_utc(),
    ));
    let second = Arc::new(service_at(
        open(dir.path().join("expenses.db")).await.unwrap(),
        OffsetDateTime::now_utc(),
    ));
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let draft = ExpenseDraft {
        category_id: category.id.clone(),
        amount: "10.00".into(),
        concept: "Luz".into(),
        paid_from_cash: false,
        shift_id: None,
        idempotency_key: "expense-concurrent".into(),
    };
    let left = {
        let service = first.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let draft = draft.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.record_expense(&actor, draft).await
        })
    };
    let right = {
        let service = second.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.record_expense(&actor, draft).await
        })
    };
    assert!(left.await.unwrap().is_ok());
    assert!(right.await.unwrap().is_ok());

    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let draft = WasteDraft {
        product_id: product.clone(),
        quantity: "2".into(),
        reason: "daño".into(),
        idempotency_key: "waste-concurrent".into(),
    };
    let left = {
        let service = first.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let draft = draft.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.record_waste(&actor, draft).await
        })
    };
    let right = {
        let service = second.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.record_waste(&actor, draft).await
        })
    };
    assert!(left.await.unwrap().is_ok());
    assert!(right.await.unwrap().is_ok());

    let row = sqlx::query("SELECT (SELECT COUNT(*) FROM expense WHERE idempotency_key = 'expense-concurrent') AS expenses, (SELECT COUNT(*) FROM waste_record WHERE idempotency_key = 'waste-concurrent') AS waste, (SELECT COUNT(*) FROM inventory_movement WHERE reason = 'merma: daño') AS movements, (SELECT COUNT(*) FROM audit_event WHERE action IN ('expenses.recorded', 'waste.recorded')) AS audits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<i64, _>("expenses"), 1);
    assert_eq!(row.get::<i64, _>("waste"), 1);
    assert_eq!(row.get::<i64, _>("movements"), 1);
    assert_eq!(row.get::<i64, _>("audits"), 2);
}

#[tokio::test]
async fn operaciones_concurrentes_distintas_respetan_saldo_y_stock() {
    let (dir, _pool, service, actor, product) = setup().await;
    let category = service
        .create_expense_category(
            &actor,
            ExpenseCategoryDraft {
                name: "Caja".into(),
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product.clone(),
                kind: "inbound".into(),
                quantity: "5".into(),
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
                code: "CJ-02".into(),
            },
        )
        .await
        .unwrap();
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id,
                opening_amount: "100.00".into(),
            },
        )
        .await
        .unwrap();
    let first = Arc::new(service_at(
        open(dir.path().join("expenses.db")).await.unwrap(),
        OffsetDateTime::now_utc(),
    ));
    let second = Arc::new(service_at(
        open(dir.path().join("expenses.db")).await.unwrap(),
        OffsetDateTime::now_utc(),
    ));
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let left = {
        let service = first.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let category_id = category.id.clone();
        let shift_id = shift.id.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service
                .record_expense(
                    &actor,
                    ExpenseDraft {
                        category_id,
                        amount: "80.00".into(),
                        concept: "cash-a".into(),
                        paid_from_cash: true,
                        shift_id: Some(shift_id),
                        idempotency_key: "cash-a".into(),
                    },
                )
                .await
        })
    };
    let right = {
        let service = second.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let category_id = category.id.clone();
        let shift_id = shift.id.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service
                .record_expense(
                    &actor,
                    ExpenseDraft {
                        category_id,
                        amount: "80.00".into(),
                        concept: "cash-b".into(),
                        paid_from_cash: true,
                        shift_id: Some(shift_id),
                        idempotency_key: "cash-b".into(),
                    },
                )
                .await
        })
    };
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);

    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let left = {
        let service = first.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let product = product.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service
                .record_waste(
                    &actor,
                    WasteDraft {
                        product_id: product,
                        quantity: "4".into(),
                        reason: "waste-a".into(),
                        idempotency_key: "waste-a".into(),
                    },
                )
                .await
        })
    };
    let right = {
        let service = second.clone();
        let actor = actor.clone();
        let gate = gate.clone();
        let product = product.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service
                .record_waste(
                    &actor,
                    WasteDraft {
                        product_id: product,
                        quantity: "4".into(),
                        reason: "waste-b".into(),
                        idempotency_key: "waste-b".into(),
                    },
                )
                .await
        })
    };
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        service
            .current_stock(&actor, product.parse().unwrap(), None)
            .await
            .unwrap(),
        "1"
    );
}

#[tokio::test]
async fn fallos_de_ledger_hecho_y_auditoria_hacen_rollback_completo() {
    for (name, trigger, from_cash) in [
        (
            "cash",
            "CREATE TRIGGER fail_expense_cash BEFORE INSERT ON cash_movement WHEN NEW.reason LIKE 'gasto:%' BEGIN SELECT RAISE(ABORT, 'cash failure'); END",
            true,
        ),
        (
            "expense",
            "CREATE TRIGGER fail_expense_record BEFORE INSERT ON expense BEGIN SELECT RAISE(ABORT, 'expense failure'); END",
            true,
        ),
        (
            "audit",
            "CREATE TRIGGER fail_expense_audit BEFORE INSERT ON audit_event WHEN NEW.action = 'expenses.recorded' BEGIN SELECT RAISE(ABORT, 'audit failure'); END",
            true,
        ),
    ] {
        let (_dir, pool, service, actor, _product) = setup().await;
        let category = service
            .create_expense_category(
                &actor,
                ExpenseCategoryDraft {
                    name: format!("{name}-category"),
                },
            )
            .await
            .unwrap();
        let register = service
            .create_cash_register(
                &actor,
                CashRegisterDraft {
                    name: format!("Caja {name}"),
                    code: format!("C-{name}"),
                },
            )
            .await
            .unwrap();
        let shift = service
            .open_cash_shift(
                &actor,
                CashOpenShiftDraft {
                    register_id: register.id,
                    opening_amount: "100.00".into(),
                },
            )
            .await
            .unwrap();
        sqlx::query(trigger).execute(&pool).await.unwrap();
        assert_eq!(
            service
                .record_expense(
                    &actor,
                    ExpenseDraft {
                        category_id: category.id,
                        amount: "10.00".into(),
                        concept: name.into(),
                        paid_from_cash: from_cash,
                        shift_id: Some(shift.id.clone()),
                        idempotency_key: format!("failure-{name}")
                    }
                )
                .await
                .unwrap_err()
                .code(),
            "storage_error"
        );
        let row = sqlx::query("SELECT (SELECT COUNT(*) FROM expense) AS expenses, (SELECT COUNT(*) FROM cash_movement) AS cash, (SELECT COUNT(*) FROM audit_event WHERE action = 'expenses.recorded') AS audits").fetch_one(&pool).await.unwrap();
        assert_eq!(row.get::<i64, _>("expenses"), 0, "{name}");
        assert_eq!(row.get::<i64, _>("cash"), 0, "{name}");
        assert_eq!(row.get::<i64, _>("audits"), 0, "{name}");
    }
}

#[tokio::test]
async fn merma_rechaza_stock_insuficiente_y_licencia_vencida() {
    let (_dir, pool, service, actor, product) = setup().await;
    assert_eq!(
        service
            .record_waste(
                &actor,
                WasteDraft {
                    product_id: product.clone(),
                    quantity: "1".into(),
                    reason: "daño".into(),
                    idempotency_key: "no-stock".into()
                }
            )
            .await
            .unwrap_err()
            .code(),
        "validation_error"
    );
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product.clone(),
                kind: "inbound".into(),
                quantity: "2".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let expired = service_at(pool, OffsetDateTime::now_utc() + time::Duration::days(31));
    assert_eq!(
        expired
            .record_waste(
                &actor,
                WasteDraft {
                    product_id: product,
                    quantity: "1".into(),
                    reason: "daño".into(),
                    idempotency_key: "expired".into()
                }
            )
            .await
            .unwrap_err()
            .code(),
        "commercial_state_blocked"
    );
}

#[tokio::test]
async fn merma_sin_stock_rechaza_y_no_persiste() {
    let (_dir, pool, service, actor, product) = setup().await;
    assert!(
        service
            .record_waste(
                &actor,
                WasteDraft {
                    product_id: product,
                    quantity: "1".into(),
                    reason: "daño".into(),
                    idempotency_key: "none".into()
                }
            )
            .await
            .is_err()
    );
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM waste_record")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}
