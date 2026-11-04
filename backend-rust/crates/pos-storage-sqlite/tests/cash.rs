use pos_application::{
    AppError, BootstrapRequest, CashCloseShiftDraft, CashMovementDraft, CashOpenShiftDraft,
    CashRegisterDraft, Clock, LoginRequest, PosService, SharedClock, SharedHasher, SharedStore,
    Store,
};
use pos_core::cash::CashRegister;
use pos_core::identity::Actor;
use pos_core::ids::BranchId;
use pos_core::organization::Branch;
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

#[tokio::test]
async fn flujo_completo_de_caja() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
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
    assert_eq!(shift.expected_amount, "100.00");

    service
        .register_cash_movement(
            &actor,
            CashMovementDraft {
                shift_id: shift.id.clone(),
                kind: "in".into(),
                amount: "20.00".into(),
                reason: "cambio".into(),
            },
        )
        .await
        .unwrap();
    service
        .register_cash_movement(
            &actor,
            CashMovementDraft {
                shift_id: shift.id.clone(),
                kind: "out".into(),
                amount: "5.00".into(),
                reason: "proveedor".into(),
            },
        )
        .await
        .unwrap();

    let active = service
        .active_cash_shift(&actor, register.id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "115.00");

    let closed = service
        .close_cash_shift(
            &actor,
            CashCloseShiftDraft {
                shift_id: shift.id.clone(),
                counted_amount: "113.00".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(closed.difference_minor, Some(-200));

    let history = service.list_cash_shift_history(&actor).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, shift.id);
    assert_eq!(history[0].difference_minor, Some(-200));
}

#[tokio::test]
async fn no_permite_dos_turnos_abiertos() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
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
    service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "0".into(),
            },
        )
        .await
        .unwrap();
    let second = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "0".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(second, AppError::Conflict));
}

#[tokio::test]
async fn turno_cerrado_rechaza_movimientos() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let register = service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja".into(),
                code: "CJ-03".into(),
            },
        )
        .await
        .unwrap();
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "10.00".into(),
            },
        )
        .await
        .unwrap();
    service
        .close_cash_shift(
            &actor,
            CashCloseShiftDraft {
                shift_id: shift.id.clone(),
                counted_amount: "10.00".into(),
            },
        )
        .await
        .unwrap();
    let rejected = service
        .register_cash_movement(
            &actor,
            CashMovementDraft {
                shift_id: shift.id.clone(),
                kind: "in".into(),
                amount: "1.00".into(),
                reason: "tarde".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(rejected.code(), "not_found");
}

#[tokio::test]
async fn migracion_0006_conserva_datos_y_bloquea_doble_apertura() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    std::fs::create_dir_all(&migrations).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/local");
    for file in [
        "0001_init.sql",
        "0002_inventory.sql",
        "0003_inventory_adjustments.sql",
        "0004_catalog_categories_variants.sql",
        "0005_catalog_modifiers.sql",
    ] {
        std::fs::copy(repo.join(file), migrations.join(file)).unwrap();
    }
    let path = dir.path().join("cash.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let migrator = sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .unwrap();
    migrator.run(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1','Org','2026-01-01T00:00:00Z')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO branch (id, organization_id, name, code, created_at) VALUES ('br-1','org-1','Matriz','MX-01','2026-01-01T00:00:00Z')")
        .execute(&pool).await.unwrap();
    pool.close().await;

    let upgraded = open(&path).await.unwrap();
    let org_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organization")
        .fetch_one(&upgraded)
        .await
        .unwrap();
    assert_eq!(org_count, 1, "0006 no debe perder datos previos");
    sqlx::query("INSERT INTO cash_register (id, organization_id, branch_id, name, code, active, created_at) VALUES ('reg-1','org-1','br-1','Caja','CJ-01',1,'2026-01-01T00:00:00Z')")
        .execute(&upgraded).await.unwrap();
    sqlx::query("INSERT INTO cash_shift (id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status) VALUES ('sh-1','org-1','br-1','reg-1','u-1','2026-01-01T00:00:00Z',1000,'open')")
        .execute(&upgraded).await.unwrap();
    let duplicate = sqlx::query("INSERT INTO cash_shift (id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status) VALUES ('sh-2','org-1','br-1','reg-1','u-2','2026-01-01T00:00:00Z',0,'open')")
        .execute(&upgraded)
        .await;
    assert!(
        duplicate.is_err(),
        "el índice parcial debe impedir dos turnos abiertos"
    );
    upgraded.close().await;
}

#[tokio::test]
async fn doble_apertura_concurrente_con_conexiones_independientes() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool.clone());
    let register = service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja".into(),
                code: "CJ-05".into(),
            },
        )
        .await
        .unwrap();
    let path = _dir.path().join("pos.db");
    let service_a = Arc::new(build_service(open(&path).await.unwrap()));
    let service_b = Arc::new(build_service(open(&path).await.unwrap()));
    let draft_a = CashOpenShiftDraft {
        register_id: register.id.clone(),
        opening_amount: "0".into(),
    };
    let draft_b = draft_a.clone();
    let actor_a = actor.clone();
    let actor_b = actor.clone();
    let a = tokio::spawn(async move { service_a.open_cash_shift(&actor_a, draft_a).await });
    let b = tokio::spawn(async move { service_b.open_cash_shift(&actor_b, draft_b).await });
    let (result_a, result_b) = (a.await.unwrap(), b.await.unwrap());
    let successes = i32::from(result_a.is_ok()) + i32::from(result_b.is_ok());
    assert_eq!(successes, 1);
}

#[tokio::test]
async fn aislamiento_por_sucursal_en_apertura() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool.clone());
    let store = SqliteStore::new(pool);
    let mut tx = store.begin().await.unwrap();
    let other_branch =
        Branch::new(BranchId::new(), actor.organization_id, "Otra", "MX-02").unwrap();
    tx.insert_branch(&other_branch).await.unwrap();
    let foreign = CashRegister::new(
        other_branch.organization_id,
        other_branch.id,
        "Ajena",
        "CJ-X",
    )
    .unwrap();
    tx.insert_cash_register(&foreign).await.unwrap();
    tx.commit().await.unwrap();
    let result = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: foreign.id.to_string(),
                opening_amount: "0".into(),
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn salida_no_deja_saldo_negativo() {
    let (_dir, pool, actor) = setup().await;
    let service = build_service(pool);
    let register = service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja".into(),
                code: "CJ-04".into(),
            },
        )
        .await
        .unwrap();
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "5.00".into(),
            },
        )
        .await
        .unwrap();
    let rejected = service
        .register_cash_movement(
            &actor,
            CashMovementDraft {
                shift_id: shift.id.clone(),
                kind: "out".into(),
                amount: "10.00".into(),
                reason: "exceso".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(rejected, AppError::Conflict));
}
