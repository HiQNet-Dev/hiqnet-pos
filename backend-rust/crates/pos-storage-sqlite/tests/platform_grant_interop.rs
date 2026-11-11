//! Interoperabilidad real Platform → POS con el fixture canónico v1.
//!
//! El documento y su firma provienen del fixture generado por la
//! implementación real de Platform
//! (`crates/platform-licensing/tests/interop_fixture.rs`; copia idéntica en
//! `tests/fixtures/platform-grant-v1.json`). El POS instala los bytes tal
//! cual, sin reconstruir el documento ni volver a firmarlo.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::SigningKey;
use pos_application::{
    BootstrapRequest, LoginRequest, PosService, SharedClock, SharedHasher, SharedLicenseVerifier,
    SharedStore, SystemClock,
};
use pos_core::entitlements::{FeatureKey, PlanName};
use pos_licensing::{Ed25519LicenseVerifier, LICENSE_DOC_VERSION, LicenseGrant};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use time::OffsetDateTime;

const SEED: [u8; 32] = [7u8; 32];
const CLAVE: &str = "contrasena-segura";

/// Documento emitido por la implementación real de Platform (bytes exactos).
const FIXTURE: &str = include_str!("fixtures/platform-grant-v1.json");

struct Fixture {
    grant_json: String,
    signature_b64: String,
    grant: LicenseGrant,
}

fn fixture() -> Fixture {
    let parsed: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let grant_json = parsed["grant_json"].as_str().unwrap().to_owned();
    let signature_b64 = parsed["signature_b64"].as_str().unwrap().to_owned();
    let public_key = parsed["public_key_b64"].as_str().unwrap().to_owned();

    // Verificación con la clave pública que Platform declaró en el propio
    // fixture: par exclusivamente sintético de pruebas.
    let verifier =
        Ed25519LicenseVerifier::from_public_key_base64(&public_key).expect("clave pública válida");
    let grant = verifier
        .verify_grant(&grant_json, &signature_b64)
        .expect("el grant emitido por Platform debe verificar en el POS");
    Fixture {
        grant_json,
        signature_b64,
        grant,
    }
}

fn service(pool: sqlx::SqlitePool) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    let clock: SharedClock = Arc::new(SystemClock);
    let verifier: SharedLicenseVerifier = Arc::new(
        Ed25519LicenseVerifier::from_public_key_base64(
            &B64.encode(SigningKey::from_bytes(&SEED).verifying_key().to_bytes()),
        )
        .unwrap(),
    );
    PosService::new(store, hasher, clock).with_license_verifier(verifier)
}

/// Ajusta la identidad local a la del grant (equivalente a un dispositivo
/// enrolado con esas identidades en Platform). No toca el documento ni la
/// firma: sólo hace que la instalación local sea la destinataria del grant.
async fn reidentificar(pool: &sqlx::SqlitePool, fixture: &Fixture) {
    let organizacion = fixture.grant.organization_id.as_uuid().to_string();
    let instalacion = fixture.grant.installation_id.as_uuid().to_string();
    sqlx::query(
        "INSERT OR IGNORE INTO organization (id, name, created_at) \
             VALUES (?, 'Organización del fixture', '2026-01-01T00:00:00Z')",
    )
    .bind(&organizacion)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE local_installation_state \
             SET installation_id = ?, organization_id = ? WHERE id = 1",
    )
    .bind(&instalacion)
    .bind(&organizacion)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn el_grant_de_platform_se_instala_verifica_y_sobrevive_al_reinicio() {
    let fixture = fixture();

    // Compatibilidad completa del documento: versión, identidades, plan,
    // features y vigencia exacta (la representación de fechas de `time` debe
    // parsear idéntica en ambas implementaciones).
    assert_eq!(fixture.grant.version, LICENSE_DOC_VERSION);
    assert_eq!(fixture.grant.plan, PlanName::Pro);
    assert_eq!(
        fixture.grant.features,
        vec![
            FeatureKey::Catalog,
            FeatureKey::Sales,
            FeatureKey::LocalBackups
        ]
    );
    assert_eq!(
        fixture.grant.valid_until,
        OffsetDateTime::from_unix_timestamp(4_070_908_800).unwrap()
    );
    assert_eq!(
        fixture.grant.issued_at,
        OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap()
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("interop.db");
    let pool = open(&path).await.unwrap();
    let instalador = service(pool.clone());
    instalador
        .bootstrap(BootstrapRequest {
            organization_name: "Interop SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: CLAVE.into(),
        })
        .await
        .unwrap();
    let actor = instalador
        .login(LoginRequest {
            username: "owner".into(),
            password: CLAVE.into(),
        })
        .await
        .unwrap();
    reidentificar(&pool, &fixture).await;

    // Instalación con los bytes originales de Platform, sin reconstruir ni
    // volver a firmar.
    let view = instalador
        .install_license(
            &actor,
            fixture.grant_json.clone(),
            fixture.signature_b64.clone(),
        )
        .await
        .expect("el grant de Platform debe instalarse");
    assert_eq!(view.plan, "pro");
    assert!(!view.read_only);
    assert!(view.features.contains(&"catalog"));
    assert!(view.features.contains(&"sales"));
    assert!(view.features.contains(&"backups.local"));

    // Entitlements operativos derivados del documento firmado.
    instalador
        .create_product(&actor, producto("INT-1"))
        .await
        .unwrap();

    // Reinicio: la resolución se repite desde SQLite con el documento original.
    drop(instalador);
    let pool = open(&path).await.unwrap();
    let reiniciado = service(pool);
    let actor = reiniciado
        .login(LoginRequest {
            username: "owner".into(),
            password: CLAVE.into(),
        })
        .await
        .unwrap();
    let view = reiniciado.status().await.unwrap().commercial.unwrap();
    assert_eq!(view.plan, "pro");
    assert!(!view.read_only);
    reiniciado
        .create_product(&actor, producto("INT-2"))
        .await
        .unwrap();
}

/// Manipular los bytes del documento produce rechazo: la firma cubre todo el
/// JSON canónico generado por Platform.
#[tokio::test]
async fn el_documento_manipulado_del_fixture_se_rechaza() {
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("interop-tamper.db")).await.unwrap();
    let servicio = service(pool.clone());
    servicio
        .bootstrap(BootstrapRequest {
            organization_name: "Interop SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: CLAVE.into(),
        })
        .await
        .unwrap();
    let actor = servicio
        .login(LoginRequest {
            username: "owner".into(),
            password: CLAVE.into(),
        })
        .await
        .unwrap();
    reidentificar(&pool, &fixture).await;

    // Manipulación mínima del JSON canónico: la vigencia cambia de mes.
    let manipulado = fixture
        .grant_json
        .replace("[2099,1,0,0,0,0,0,0,0]", "[2099,2,0,0,0,0,0,0,0]");
    assert_ne!(manipulado, fixture.grant_json);
    let error = servicio
        .install_license(&actor, manipulado.clone(), fixture.signature_b64.clone())
        .await
        .unwrap_err();
    assert!(
        format!("{error:?}").contains("Crypto"),
        "se esperaba rechazo criptográfico: {error:?}"
    );

    // El rechazo queda auditado (append-only) y el estado comercial no concede
    // el documento manipulado. La consulta directa evita el filtro por
    // organización de la sesión, que tras la reidentificación difiere de la
    // organización del grant.
    let rechazos: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_event WHERE action = 'license.rejected'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(rechazos >= 1, "el rechazo debe auditarse");
    let marcador: Option<String> =
        sqlx::query_scalar("SELECT license_grant_json FROM local_installation_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(marcador.as_deref(), Some(manipulado.as_str()));
}

fn producto(sku: &str) -> pos_application::ProductDraft {
    pos_application::ProductDraft {
        sku: sku.into(),
        name: format!("Producto {sku}"),
        price: "10.00".into(),
        tax_rate_bp: 1600,
        category_id: None,
    }
}
