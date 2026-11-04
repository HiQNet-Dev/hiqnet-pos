//! Administración local de usuarios: autorización, activación, contraseñas,
//! auditoría y persistencia.

use pos_application::{
    BootstrapRequest, ChangeOwnPasswordDraft, LoginRequest, PosService, ResetUserPasswordDraft,
    SetUserActiveDraft, SharedClock, SharedHasher, SharedStore, UserDraft,
};
use pos_core::identity::{Actor, Role};
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

fn build_service(pool: sqlx::SqlitePool) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    let clock: SharedClock = Arc::new(FixedClock(OffsetDateTime::now_utc()));
    PosService::new(store, hasher, clock)
}

async fn setup() -> (tempfile::TempDir, std::path::PathBuf, PosService, Actor) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usuarios.db");
    let service = build_service(open(&path).await.unwrap());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Cafetería Usuarios".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "propietario".into(),
            owner_display_name: "Ana Propietaria".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let owner = service
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    (dir, path, service, owner)
}

fn cashier_draft(username: &str) -> UserDraft {
    UserDraft {
        username: username.into(),
        display_name: "Luis Cajero".into(),
        role: "cashier".into(),
        password: "clave-cajero-2026".into(),
    }
}

#[tokio::test]
async fn propietario_crea_cajero_y_el_cajero_inicia_sesion() {
    let (_dir, _path, service, owner) = setup().await;

    let created = service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();
    assert_eq!(created.role, "cashier");
    assert!(created.active);
    assert_eq!(created.username, "luis.cajero");

    let cashier = service
        .login(LoginRequest {
            username: "luis.cajero".into(),
            password: "clave-cajero-2026".into(),
        })
        .await
        .unwrap();
    assert_eq!(cashier.role, Role::Cashier);
    assert!(cashier.has(pos_core::identity::Permission::Sell));
    assert!(!cashier.has(pos_core::identity::Permission::ManageUsers));

    let usuarios = service.list_users(&owner).await.unwrap();
    let nombres: Vec<&str> = usuarios.iter().map(|user| user.username.as_str()).collect();
    assert!(nombres.contains(&"propietario"));
    assert!(nombres.contains(&"luis.cajero"));
}

#[tokio::test]
async fn el_cajero_no_administra_usuarios() {
    let (_dir, _path, service, owner) = setup().await;
    let created = service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();
    let cashier = service
        .login(LoginRequest {
            username: "luis.cajero".into(),
            password: "clave-cajero-2026".into(),
        })
        .await
        .unwrap();

    assert!(service.list_users(&cashier).await.is_err());
    assert!(
        service
            .create_user(&cashier, cashier_draft("otro"))
            .await
            .is_err()
    );
    assert!(
        service
            .set_user_active(
                &cashier,
                SetUserActiveDraft {
                    user_id: created.id.clone(),
                    active: false,
                },
            )
            .await
            .is_err()
    );
    assert!(
        service
            .reset_user_password(
                &cashier,
                ResetUserPasswordDraft {
                    user_id: created.id.clone(),
                    password: "otra-clave-larga".into(),
                },
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn desactivar_una_cuenta_revoca_su_sesion_y_su_login() {
    let (_dir, _path, service, owner) = setup().await;
    let created = service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();
    let sesion_cajero = service
        .login(LoginRequest {
            username: "luis.cajero".into(),
            password: "clave-cajero-2026".into(),
        })
        .await
        .unwrap();

    let desactivado = service
        .set_user_active(
            &owner,
            SetUserActiveDraft {
                user_id: created.id.clone(),
                active: false,
            },
        )
        .await
        .unwrap();
    assert!(!desactivado.active);

    // Login nuevo rechazado.
    assert!(
        service
            .login(LoginRequest {
                username: "luis.cajero".into(),
                password: "clave-cajero-2026".into(),
            })
            .await
            .is_err()
    );

    // La sesión ya emitida deja de revalidarse.
    assert!(service.refresh_actor(&sesion_cajero).await.is_err());
    // El propietario sigue teniendo acceso.
    assert!(service.refresh_actor(&owner).await.is_ok());

    let reactivado = service
        .set_user_active(
            &owner,
            SetUserActiveDraft {
                user_id: created.id.clone(),
                active: true,
            },
        )
        .await
        .unwrap();
    assert!(reactivado.active);
    assert!(
        service
            .login(LoginRequest {
                username: "luis.cajero".into(),
                password: "clave-cajero-2026".into(),
            })
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn no_se_puede_dejar_la_instalacion_sin_propietario_activo() {
    let (_dir, _path, service, owner) = setup().await;

    let error = service
        .set_user_active(
            &owner,
            SetUserActiveDraft {
                user_id: owner.user_id.to_string(),
                active: false,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "last_active_owner");

    // Con un segundo propietario activo la operación sí procede.
    let segundo = service
        .create_user(
            &owner,
            UserDraft {
                username: "segundo.owner".into(),
                display_name: "Beto Propietario".into(),
                role: "owner".into(),
                password: "otra-clave-segura".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(segundo.role, "owner");
    service
        .set_user_active(
            &owner,
            SetUserActiveDraft {
                user_id: segundo.id,
                active: false,
            },
        )
        .await
        .unwrap();
    // Queda un solo propietario activo: ya no se puede desactivar.
    assert!(
        service
            .set_user_active(
                &owner,
                SetUserActiveDraft {
                    user_id: owner.user_id.to_string(),
                    active: false,
                },
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn restablecer_contrasena_invalida_la_anterior() {
    let (_dir, _path, service, owner) = setup().await;
    let created = service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();

    service
        .reset_user_password(
            &owner,
            ResetUserPasswordDraft {
                user_id: created.id,
                password: "clave-nueva-2026".into(),
            },
        )
        .await
        .unwrap();

    assert!(
        service
            .login(LoginRequest {
                username: "luis.cajero".into(),
                password: "clave-cajero-2026".into(),
            })
            .await
            .is_err()
    );
    assert!(
        service
            .login(LoginRequest {
                username: "luis.cajero".into(),
                password: "clave-nueva-2026".into(),
            })
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn cambiar_la_contrasena_propia_exige_la_actual() {
    let (_dir, _path, service, owner) = setup().await;

    let error = service
        .change_own_password(
            &owner,
            ChangeOwnPasswordDraft {
                current_password: "incorrecta".into(),
                new_password: "clave-nueva-2026".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_credentials");

    service
        .change_own_password(
            &owner,
            ChangeOwnPasswordDraft {
                current_password: "contrasena-segura".into(),
                new_password: "clave-nueva-2026".into(),
            },
        )
        .await
        .unwrap();

    assert!(
        service
            .login(LoginRequest {
                username: "propietario".into(),
                password: "contrasena-segura".into(),
            })
            .await
            .is_err()
    );
    assert!(
        service
            .login(LoginRequest {
                username: "propietario".into(),
                password: "clave-nueva-2026".into(),
            })
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn las_respuestas_no_contienen_hashes_ni_contrasenas() {
    let (_dir, _path, service, owner) = setup().await;
    service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();

    let usuarios = service.list_users(&owner).await.unwrap();
    let json = serde_json::to_string(&usuarios).unwrap();
    assert!(!json.contains("password"));
    assert!(!json.contains("hash"));
    assert!(!json.contains("clave-cajero-2026"));

    let creado = service
        .create_user(&owner, cashier_draft("otro.cajero"))
        .await
        .unwrap();
    let json = serde_json::to_string(&creado).unwrap();
    assert!(!json.contains("password"));
    assert!(!json.contains("hash"));
}

#[tokio::test]
async fn rechaza_usuarios_duplicados_roles_inexistentes_y_claves_debiles() {
    let (_dir, _path, service, owner) = setup().await;
    service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();

    let duplicado = service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap_err();
    assert_eq!(duplicado.code(), "duplicate_username");

    let rol = service
        .create_user(
            &owner,
            UserDraft {
                role: "admin".into(),
                ..cashier_draft("nuevo.usuario")
            },
        )
        .await
        .unwrap_err();
    assert_eq!(rol.code(), "validation_error");

    let debil = service
        .create_user(
            &owner,
            UserDraft {
                password: "corta".into(),
                ..cashier_draft("otro.usuario")
            },
        )
        .await
        .unwrap_err();
    assert_eq!(debil.code(), "validation_error");
}

#[tokio::test]
async fn la_auditoria_registra_las_operaciones_sensibles() {
    let (_dir, _path, service, owner) = setup().await;
    let created = service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();
    service
        .set_user_active(
            &owner,
            SetUserActiveDraft {
                user_id: created.id.clone(),
                active: false,
            },
        )
        .await
        .unwrap();
    service
        .set_user_active(
            &owner,
            SetUserActiveDraft {
                user_id: created.id.clone(),
                active: true,
            },
        )
        .await
        .unwrap();
    service
        .reset_user_password(
            &owner,
            ResetUserPasswordDraft {
                user_id: created.id,
                password: "clave-nueva-2026".into(),
            },
        )
        .await
        .unwrap();
    service
        .change_own_password(
            &owner,
            ChangeOwnPasswordDraft {
                current_password: "contrasena-segura".into(),
                new_password: "clave-owner-2026".into(),
            },
        )
        .await
        .unwrap();

    let eventos = service.list_audit(&owner, 100).await.unwrap();
    let acciones: Vec<&str> = eventos.iter().map(|event| event.action.as_str()).collect();
    for esperada in [
        "users.created",
        "users.deactivated",
        "users.reactivated",
        "users.password_reset",
        "users.password_changed",
    ] {
        assert!(
            acciones.contains(&esperada),
            "falta auditoría de {esperada}"
        );
    }
    // Ningún evento puede contener el hash ni la contraseña.
    let json = serde_json::to_string(&eventos).unwrap();
    assert!(!json.contains("$argon2"));
    assert!(!json.contains("clave-cajero-2026"));
}

#[tokio::test]
async fn los_usuarios_persisten_tras_reiniciar() {
    let (_dir, path, service, owner) = setup().await;
    service
        .create_user(&owner, cashier_draft("luis.cajero"))
        .await
        .unwrap();
    drop(service);

    let reopened = build_service(open(&path).await.unwrap());
    let cashier = reopened
        .login(LoginRequest {
            username: "luis.cajero".into(),
            password: "clave-cajero-2026".into(),
        })
        .await
        .unwrap();
    assert_eq!(cashier.role, Role::Cashier);

    let owner = reopened
        .login(LoginRequest {
            username: "propietario".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let usuarios = reopened.list_users(&owner).await.unwrap();
    assert_eq!(usuarios.len(), 2);
}
