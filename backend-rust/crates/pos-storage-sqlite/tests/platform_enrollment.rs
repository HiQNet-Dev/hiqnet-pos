//! Enrolamiento con un cliente Platform simulado.
//!
//! Verifica la orquestación y la persistencia; no sustituye una prueba contra
//! una API real. El contrato HTTP se prueba en el adaptador de escritorio.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use pos_application::{
    AppResult, BeginPlatformEnrollment, BootstrapRequest, LoginRequest, PlatformChallenge,
    PlatformEnrollmentBinding, PlatformEnrollmentClient, PlatformProof, PosService, SharedClock,
    SharedDeviceSigner, SharedPlatformEnrollmentClient, SharedStore,
};
use pos_core::identity::Actor;
use pos_core::ids::{InstallationId, OrganizationId};
use pos_licensing::DeviceIdentity;
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, backup_to};
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

struct FakeClient {
    organization_id: OrganizationId,
    installation_id: InstallationId,
    proofs: Mutex<Vec<PlatformProof>>,
}

#[async_trait::async_trait]
impl PlatformEnrollmentClient for FakeClient {
    async fn begin(&self, _request: BeginPlatformEnrollment) -> AppResult<PlatformChallenge> {
        Ok(PlatformChallenge {
            device_code: "device-secreto".into(),
            user_code: "ABCD-EFGH".into(),
            verification_uri: "http://127.0.0.1:8080/device".into(),
            nonce: "nonce-sintetico".into(),
            expires_in_seconds: 600,
        })
    }

    async fn prove(&self, request: PlatformProof) -> AppResult<PlatformEnrollmentBinding> {
        // Comprueba que el POS firmó el mensaje canónico con la clave pública.
        let key = request
            .public_key
            .strip_prefix("ed25519:v1:")
            .expect("clave con prefijo");
        let key_bytes = URL_SAFE_NO_PAD.decode(key).expect("clave base64url");
        let verifying = VerifyingKey::from_bytes(key_bytes.as_slice().try_into().unwrap()).unwrap();
        let signature = Signature::from_slice(
            &URL_SAFE_NO_PAD
                .decode(&request.signature_b64)
                .expect("firma base64url"),
        )
        .unwrap();
        let message = format!(
            "hiqnet-enrollment-proof:v1\nrequest={}\ninstallation={}\nkey={}\nnonce={}",
            request.device_code, request.installation_id, request.public_key, request.nonce
        );
        verifying
            .verify(message.as_bytes(), &signature)
            .expect("firma del dispositivo válida");
        self.proofs.lock().unwrap().push(request);
        Ok(PlatformEnrollmentBinding {
            installation_id: self.installation_id,
            organization_id: self.organization_id,
            bound_at: OffsetDateTime::now_utc(),
        })
    }
}

fn service(
    pool: sqlx::SqlitePool,
    installation_id: InstallationId,
    organization_id: OrganizationId,
) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher = Arc::new(Argon2idHasher::new());
    let clock: SharedClock = Arc::new(pos_application::SystemClock);
    let signer: SharedDeviceSigner = Arc::new(DeviceIdentity::generate());
    let client: SharedPlatformEnrollmentClient = Arc::new(FakeClient {
        organization_id,
        installation_id,
        proofs: Mutex::new(Vec::new()),
    });
    PosService::new(store, hasher, clock).with_platform_enrollment(
        signer,
        client,
        "http://127.0.0.1:8080".into(),
    )
}

async fn owner(service: &PosService) -> Actor {
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Enrolamiento SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn inicia_y_completa_el_enrolamiento_sin_exponer_el_device_code() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pos_storage_sqlite::open(dir.path().join("pos.db"))
        .await
        .unwrap();
    // Las identidades se conocen tras bootstrap; se inyectan después con un
    // servicio equivalente al real.
    let bootstrap_service = service(pool.clone(), InstallationId::new(), OrganizationId::new());
    bootstrap_service
        .bootstrap(BootstrapRequest {
            organization_name: "Enrolamiento SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let actor = bootstrap_service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let public_status = bootstrap_service.status().await.unwrap();
    assert!(
        !serde_json::to_string(&public_status)
            .unwrap()
            .contains("installation_id"),
        "el estado público no expone la identidad de instalación"
    );
    let installation_id = sqlx::query_scalar::<_, String>(
        "SELECT installation_id FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .parse()
    .unwrap();

    let organization_id = OrganizationId::new();
    let service = service(pool.clone(), installation_id, organization_id);
    let challenge = service.begin_platform_enrollment(&actor).await.unwrap();
    assert_eq!(challenge.status, "pending");
    assert_eq!(challenge.user_code.as_deref(), Some("ABCD-EFGH"));
    assert!(!challenge.completed);
    // El device_code nunca cruza la frontera de la interfaz.
    let serialized = serde_json::to_string(&challenge).unwrap();
    assert!(!serialized.contains("device-secreto"));

    let completed = service.complete_platform_enrollment(&actor).await.unwrap();
    assert!(completed.completed);
    assert_eq!(
        completed.organization_id.as_deref(),
        Some(organization_id.to_string().as_str())
    );
    assert_eq!(
        service
            .platform_enrollment_status(&actor)
            .await
            .unwrap()
            .status,
        "completed"
    );
    let device_code: Option<String> =
        sqlx::query_scalar("SELECT device_code FROM platform_enrollment WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(device_code, None, "el secreto se borra al completar");

    let events = service.list_audit(&actor, 50).await.unwrap();
    let keys: Vec<_> = events.iter().map(|event| event.action.as_str()).collect();
    assert!(keys.contains(&"platform.enrollment_started"));
    assert!(keys.contains(&"platform.enrollment_completed"));
}

#[tokio::test]
async fn sqlite_impone_los_estados_y_el_respaldo_no_transporta_device_code() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("pos.db");
    let backup = dir.path().join("backup.db");
    let pool = pos_storage_sqlite::open(&database).await.unwrap();
    let bootstrap_service = service(pool.clone(), InstallationId::new(), OrganizationId::new());
    let actor = owner(&bootstrap_service).await;
    let state_installation: InstallationId = sqlx::query_scalar::<_, String>(
        "SELECT installation_id FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .parse()
    .unwrap();
    let service = service(pool.clone(), state_installation, actor.organization_id);
    service.begin_platform_enrollment(&actor).await.unwrap();

    assert!(
        sqlx::query(
            "UPDATE platform_enrollment SET status = 'completed', organization_id = NULL, bound_at = NULL WHERE id = 1",
        )
        .execute(&pool)
        .await
        .is_err(),
        "completed exige organización y fecha de vínculo"
    );
    assert!(
        sqlx::query("UPDATE platform_enrollment SET device_code = NULL WHERE id = 1")
            .execute(&pool)
            .await
            .is_err(),
        "pending exige conservar el device_code"
    );
    assert!(
        sqlx::query(
            "UPDATE platform_enrollment \
             SET status = 'completed', organization_id = 'org-vinculada', \
                 bound_at = '2026-01-01T00:00:00Z' WHERE id = 1",
        )
        .execute(&pool)
        .await
        .is_err(),
        "completed exige eliminar el device_code"
    );

    backup_to(&pool, &backup).await.unwrap();
    let backup_pool = pos_storage_sqlite::open_read_only(&backup).await.unwrap();
    let pending_codes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM platform_enrollment \
         WHERE status = 'pending' AND device_code IS NOT NULL",
    )
    .fetch_one(&backup_pool)
    .await
    .unwrap();
    backup_pool.close().await;
    assert_eq!(
        pending_codes, 0,
        "el backup publicado no exporta device_code de challenges pendientes"
    );

    assert!(
        sqlx::query_scalar::<_, String>("SELECT device_code FROM platform_enrollment WHERE id = 1")
            .fetch_one(&pool)
            .await
            .is_ok(),
        "el polling pendiente conserva el secreto local"
    );

    // Un snapshot legado no sanitizado tampoco puede reintroducir el secreto.
    let legacy_snapshot = dir.path().join("legacy-with-secret.db");
    sqlx::query("VACUUM INTO ?")
        .bind(legacy_snapshot.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let report = pos_storage_sqlite::restore_from_backup(&database, &legacy_snapshot)
        .await
        .unwrap();
    let recovery = report
        .recovery_copy
        .expect("la restauración conserva una copia de recuperación");
    let recovery_pool = pos_storage_sqlite::open_read_only(&recovery).await.unwrap();
    let recovery_pending_codes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM platform_enrollment \
         WHERE status = 'pending' AND device_code IS NOT NULL",
    )
    .fetch_one(&recovery_pool)
    .await
    .unwrap();
    recovery_pool.close().await;
    assert_eq!(
        recovery_pending_codes, 0,
        "la copia de recuperación conservada no retiene device_code pendiente"
    );
    let restored = pos_storage_sqlite::open(&database).await.unwrap();
    let restored_pending_codes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM platform_enrollment \
         WHERE status = 'pending' AND device_code IS NOT NULL",
    )
    .fetch_one(&restored)
    .await
    .unwrap();
    restored.close().await;
    assert_eq!(
        restored_pending_codes, 0,
        "la base publicada por restauración no reintroduce device_code pendiente"
    );
}

#[tokio::test]
async fn elimina_el_device_code_cuando_el_challenge_caduca() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pos_storage_sqlite::open(dir.path().join("pos.db"))
        .await
        .unwrap();
    let bootstrap_service = service(pool.clone(), InstallationId::new(), OrganizationId::new());
    let actor = owner(&bootstrap_service).await;
    let installation_id: InstallationId = sqlx::query_scalar::<_, String>(
        "SELECT installation_id FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .parse()
    .unwrap();
    let service = service(pool.clone(), installation_id, actor.organization_id);
    service.begin_platform_enrollment(&actor).await.unwrap();
    sqlx::query("UPDATE platform_enrollment SET expires_at = '2020-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        service
            .platform_enrollment_status(&actor)
            .await
            .unwrap()
            .status,
        "none"
    );
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM platform_enrollment")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn un_cajero_no_puede_iniciar_el_enrolamiento() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pos_storage_sqlite::open(dir.path().join("pos.db"))
        .await
        .unwrap();
    let service = service(pool, InstallationId::new(), OrganizationId::new());
    let _actor = owner(&service).await;
    // Sin usuarios adicionales, se comprueba la regla con un actor sintético.
    let cashier = Actor {
        user_id: pos_core::ids::UserId::new(),
        organization_id: OrganizationId::new(),
        branch_id: None,
        display_name: "Cajero".into(),
        role: pos_core::identity::Role::Cashier,
        permissions: pos_core::identity::Role::Cashier.permissions(),
    };
    assert!(matches!(
        service.begin_platform_enrollment(&cashier).await,
        Err(pos_application::AppError::Domain(
            pos_core::DomainError::AccessDenied { .. }
        ))
    ));
    assert!(matches!(
        service.platform_enrollment_status(&cashier).await,
        Err(pos_application::AppError::Domain(
            pos_core::DomainError::AccessDenied { .. }
        ))
    ));
    assert!(matches!(
        service.commercial_state(&cashier).await,
        Err(pos_application::AppError::Domain(
            pos_core::DomainError::AccessDenied { .. }
        ))
    ));
}
