use pos_application::PasswordHasher as _;
use pos_application::{
    AppError, BootstrapRequest, Clock, LoginRequest, PosService, ProductDraft, SharedClock,
    SharedHasher, SharedStore, Store,
};
use pos_core::identity::Permission;
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, backup_to, open, restore_from_backup, verify_backup};
use std::path::Path;
use std::sync::Arc;
use time::OffsetDateTime;

struct FixedClock(OffsetDateTime);

impl Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

fn build_service(pool: sqlx::SqlitePool, clock: SharedClock) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    PosService::new(store, hasher, clock)
}

fn bootstrap_request() -> BootstrapRequest {
    BootstrapRequest {
        organization_name: "Abarrotes La Esquina".into(),
        branch_name: "Matriz".into(),
        branch_code: "mx-01".into(),
        owner_username: "propietario".into(),
        owner_display_name: "Ana Propietaria".into(),
        owner_password: "contrasena-segura".into(),
    }
}

fn product_draft(sku: &str) -> ProductDraft {
    ProductDraft {
        sku: sku.into(),
        name: "Café americano".into(),
        price: "12.50".into(),
        tax_rate_bp: 1600,
    }
}

#[tokio::test]
async fn bootstrap_login_y_catalogo_persisten() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("pos.db");
    let pool = open(&db_path).await.unwrap();
    let clock: SharedClock = Arc::new(FixedClock(OffsetDateTime::now_utc()));
    let service = build_service(pool.clone(), clock);

    let status = service.status().await.unwrap();
    assert!(!status.initialized);
    assert_eq!(status.user_count, 0);

    let bootstrap = service.bootstrap(bootstrap_request()).await.unwrap();
    let status = service.status().await.unwrap();
    assert!(status.initialized);
    assert_eq!(status.user_count, 1);
    assert_eq!(status.commercial.as_ref().unwrap().status, "trial");

    let rejected = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "incorrecta".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(rejected.code(), "invalid_credentials");

    let actor = service
        .login(LoginRequest {
            username: " Propietario ".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    assert!(actor.has(Permission::ManageCatalog));

    let product = service
        .create_product(&actor, product_draft("CAF-001"))
        .await
        .unwrap();
    assert_eq!(product.unit_price, "12.50");

    let duplicate = service
        .create_product(&actor, product_draft("caf-001"))
        .await
        .unwrap_err();
    assert!(matches!(duplicate, AppError::Conflict));

    let products = service.list_products(&actor).await.unwrap();
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].sku, "CAF-001");

    let audit = service.list_audit(&actor, 50).await.unwrap();
    let actions: Vec<&str> = audit.iter().map(|event| event.action.as_str()).collect();
    assert!(actions.contains(&"catalog.product_created"));
    assert!(actions.contains(&"session.logged_in"));
    assert!(actions.contains(&"installation.bootstrapped"));

    // La auditoría es append-only incluso por SQL directo.
    let update = sqlx::query("UPDATE audit_event SET action = 'x'")
        .execute(&pool)
        .await;
    assert!(update.is_err());

    // La base sobrevive al cierre y reapertura.
    pool.close().await;
    let reopened = open(&db_path).await.unwrap();
    {
        let rows = sqlx::query("SELECT username, active, role FROM app_user")
            .fetch_all(&reopened)
            .await
            .unwrap();
        eprintln!("DEBUG users={:?}", rows.len());
        for r in &rows {
            use sqlx::Row;
            eprintln!(
                "DEBUG {:?} {:?} {:?}",
                r.get::<String, _>("username"),
                r.get::<i64, _>("active"),
                r.get::<String, _>("role")
            );
        }
    }
    let clock: SharedClock = Arc::new(FixedClock(OffsetDateTime::now_utc()));
    let service = build_service(reopened.clone(), clock);
    let actor = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    assert_eq!(service.list_products(&actor).await.unwrap().len(), 1);
    assert_eq!(
        service.status().await.unwrap().user_count,
        1,
        "el bootstrap no debe duplicar usuarios"
    );
    reopened.close().await;

    let _ = bootstrap;
}

#[tokio::test]
async fn rol_cajero_no_puede_administrar_catalogo() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("pos.db")).await.unwrap();
    let clock: SharedClock = Arc::new(FixedClock(OffsetDateTime::now_utc()));
    let service = build_service(pool.clone(), clock);
    service.bootstrap(bootstrap_request()).await.unwrap();

    // Se crea un cajero directamente con el puerto de persistencia para probar
    // la autorización sin añadir todavía un caso de uso de gestión de usuarios.
    let actor = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let hasher = Argon2idHasher::new();
    let cashier = pos_core::identity::User::new(
        pos_core::ids::UserId::new(),
        actor.organization_id,
        actor.branch_id,
        "cajero",
        "Carlos Cajero",
        pos_core::identity::Role::Cashier,
        hasher.hash("contrasena-cajero").unwrap(),
    )
    .unwrap();
    let store = SqliteStore::new(pool.clone());
    let mut tx = store.begin().await.unwrap();
    tx.insert_user(&cashier).await.unwrap();
    tx.commit().await.unwrap();

    let cashier_actor = service
        .login(LoginRequest {
            username: "cajero".into(),
            password: "contrasena-cajero".into(),
        })
        .await
        .unwrap();
    let error = service
        .create_product(&cashier_actor, product_draft("CAF-002"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "access_denied");
    assert_eq!(
        service.list_products(&cashier_actor).await.unwrap().len(),
        0
    );
    pool.close().await;
}

#[tokio::test]
async fn licencia_vencida_no_permite_nuevas_operaciones() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("pos.db")).await.unwrap();
    let start = OffsetDateTime::now_utc();
    let clock: SharedClock = Arc::new(FixedClock(start));
    let service = build_service(pool.clone(), clock);
    service.bootstrap(bootstrap_request()).await.unwrap();
    let actor = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();

    let expired_clock: SharedClock = Arc::new(FixedClock(start + time::Duration::days(31)));
    let expired_service = service_with_shared_store(pool.clone(), expired_clock);
    let status = expired_service.status().await.unwrap();
    assert!(status.commercial.as_ref().unwrap().read_only);
    let error = expired_service
        .create_product(&actor, product_draft("CAF-003"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "commercial_state_blocked");

    // La lectura histórica sigue disponible con licencia vencida.
    assert_eq!(
        expired_service.list_products(&actor).await.unwrap().len(),
        0
    );
    pool.close().await;
}

fn service_with_shared_store(pool: sqlx::SqlitePool, clock: SharedClock) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    PosService::new(store, hasher, clock)
}

#[tokio::test]
async fn respaldo_y_restauracion_conservan_datos() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("pos.db");
    let backup_path = dir.path().join("backups").join("pos-backup.db");
    let pool = open(&db_path).await.unwrap();
    let clock: SharedClock = Arc::new(FixedClock(OffsetDateTime::now_utc()));
    let service = build_service(pool.clone(), clock);
    service.bootstrap(bootstrap_request()).await.unwrap();
    let actor = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    service
        .create_product(&actor, product_draft("CAF-001"))
        .await
        .unwrap();

    backup_to(&pool, &backup_path).await.unwrap();
    verify_backup(&backup_path).await.unwrap();

    service
        .create_product(&actor, product_draft("CAF-002"))
        .await
        .unwrap();
    assert_eq!(service.list_products(&actor).await.unwrap().len(), 2);
    pool.close().await;

    restore_from_backup(&db_path, &backup_path).await.unwrap();
    let restored = open(&db_path).await.unwrap();
    let restored_service = service_with_shared_store(
        restored.clone(),
        Arc::new(FixedClock(OffsetDateTime::now_utc())),
    );
    let actor = restored_service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let products = restored_service.list_products(&actor).await.unwrap();
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].sku, "CAF-001");
    restored.close().await;

    // Un archivo que no es SQLite no puede restaurarse.
    let invalid = dir.path().join("invalid.db");
    std::fs::write(&invalid, b"no es sqlite").unwrap();
    assert!(restore_from_backup(&db_path, &invalid).await.is_err());
    assert!(verify_backup(Path::new(&invalid)).await.is_err());
}
