use pos_application::{
    AppError, BootstrapRequest, CashCloseShiftDraft, CashMovementDraft, CashOpenShiftDraft,
    CashRegisterDraft, Clock, LoginRequest, PosService, SharedClock, SharedHasher, SharedStore,
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
