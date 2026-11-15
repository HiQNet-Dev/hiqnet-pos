//! Integración de licencias firmadas sobre SQLite real.
//!
//! Demuestra que el estado comercial se deriva del documento firmado y no de
//! columnas editables, y que cualquier manipulación de la base termina en solo
//! lectura. Usa una clave sintética exclusiva de pruebas.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signer, SigningKey};
use pos_application::{
    AppStatus, BootstrapRequest, CheckoutDraft, Clock, LoginRequest, PosService, SaleItemDraft,
    SharedClock, SharedHasher, SharedLicenseVerifier, SharedStore, SystemClock,
};
use pos_core::entitlements::{FeatureKey, PlanName};
use pos_core::identity::Actor;
use pos_core::ids::{InstallationId, OrganizationId};
use pos_licensing::{Ed25519LicenseVerifier, LICENSE_DOC_VERSION, LicenseGrant};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

const SEED: [u8; 32] = [7u8; 32];
const OTHER_SEED: [u8; 32] = [9u8; 32];
const CLAVE: &str = "contrasena-segura";

fn verifier_with(seed: &[u8; 32]) -> SharedLicenseVerifier {
    let key = SigningKey::from_bytes(seed);
    Arc::new(
        Ed25519LicenseVerifier::from_public_key_base64(&B64.encode(key.verifying_key().to_bytes()))
            .unwrap(),
    )
}

fn build(pool: SqlitePool, license: Option<SharedLicenseVerifier>) -> PosService {
    build_at(pool, license, Arc::new(SystemClock))
}

fn build_at(
    pool: SqlitePool,
    license: Option<SharedLicenseVerifier>,
    clock: SharedClock,
) -> PosService {
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    let service = PosService::new(store, hasher, clock);
    match license {
        Some(verifier) => service.with_license_verifier(verifier),
        None => service,
    }
}

struct FixedClock(OffsetDateTime);
impl Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

async fn ids(pool: &SqlitePool) -> (InstallationId, OrganizationId) {
    let row = sqlx::query(
        "SELECT installation_id, organization_id FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (
        row.get::<String, _>("installation_id").parse().unwrap(),
        row.get::<String, _>("organization_id").parse().unwrap(),
    )
}

async fn preparar(
    pool: &SqlitePool,
    license: Option<SharedLicenseVerifier>,
) -> (PosService, Actor) {
    let service = build(pool.clone(), license);
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Licencias SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: CLAVE.into(),
        })
        .await
        .unwrap();
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: CLAVE.into(),
        })
        .await
        .unwrap();
    (service, actor)
}

fn grant(
    installation_id: InstallationId,
    organization_id: OrganizationId,
    plan: PlanName,
    features: Vec<FeatureKey>,
    valid_until: OffsetDateTime,
) -> LicenseGrant {
    LicenseGrant {
        version: LICENSE_DOC_VERSION,
        grant_id: "grant-test-001".into(),
        installation_id,
        organization_id,
        plan,
        features,
        issued_at: OffsetDateTime::now_utc(),
        valid_until,
    }
}

fn firmar(grant: &LicenseGrant, seed: &[u8; 32]) -> (String, String) {
    let json = String::from_utf8(grant.to_canonical_json().unwrap()).unwrap();
    let signature = B64.encode(
        SigningKey::from_bytes(seed)
            .sign(json.as_bytes())
            .to_bytes(),
    );
    (json, signature)
}

async fn estado(service: &PosService) -> AppStatus {
    service.status().await.unwrap()
}

async fn es_solo_lectura(service: &PosService) -> bool {
    estado(service)
        .await
        .commercial
        .map(|commercial| commercial.read_only)
        .unwrap_or(true)
}

#[tokio::test]
async fn vinculo_firmado_conserva_identidades_locales_y_rechaza_otro_tenant() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("bind.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, local_org) = ids(&pool).await;
    let commercial_org = OrganizationId::new();
    let (json, signature) = firmar(
        &grant(
            installation,
            commercial_org,
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );

    let (bad_json, bad_signature) = firmar(
        &grant(
            installation,
            commercial_org,
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &OTHER_SEED,
    );
    assert!(
        service
            .install_license(&actor, bad_json, bad_signature)
            .await
            .is_err()
    );
    let mut altered_signature = signature.clone();
    altered_signature.replace_range(
        ..1,
        if altered_signature.starts_with('A') {
            "B"
        } else {
            "A"
        },
    );
    assert!(
        service
            .install_license(&actor, json.clone(), altered_signature)
            .await
            .is_err()
    );
    let (wrong_json, wrong_signature) = firmar(
        &grant(
            InstallationId::new(),
            commercial_org,
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    assert!(
        service
            .install_license(&actor, wrong_json, wrong_signature)
            .await
            .is_err()
    );
    let binding_count: i64 = sqlx::query_scalar("SELECT count(*) FROM commercial_binding")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(binding_count, 0);
    let bound_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_event WHERE action = 'commercial.identity_bound'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bound_audits, 0);
    service
        .install_license(&actor, json.clone(), signature.clone())
        .await
        .unwrap();
    let audit = service.list_audit(&actor, 50).await.unwrap();
    assert!(
        audit
            .iter()
            .any(|event| event.action == "commercial.identity_bound")
    );
    let (persisted_installation, persisted_org) = ids(&pool).await;
    assert_eq!(
        (persisted_installation, persisted_org),
        (installation, local_org)
    );
    let row = sqlx::query("SELECT organization_id, installation_id, proof_source FROM commercial_binding WHERE id = 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        row.get::<String, _>("organization_id"),
        commercial_org.to_string()
    );
    assert_eq!(
        row.get::<String, _>("installation_id"),
        installation.to_string()
    );
    assert_eq!(
        row.get::<String, _>("proof_source"),
        "signed_license_grant_v1"
    );
    // Reinstalar el mismo grant está permitido; no debe crear otro vínculo.
    service
        .install_license(&actor, json.clone(), signature.clone())
        .await
        .unwrap();
    let bound_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_event WHERE action = 'commercial.identity_bound'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bound_audits, 1);
    assert!(!es_solo_lectura(&service).await);

    let (other_json, other_sig) = firmar(
        &grant(
            installation,
            OrganizationId::new(),
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    assert!(
        service
            .install_license(&actor, other_json.clone(), other_sig.clone())
            .await
            .is_err()
    );
    assert!(
        service
            .install_license(&actor, other_json, other_sig)
            .await
            .is_err()
    );
    let persisted: (String, String) =
        sqlx::query_as("SELECT organization_id, proof_json FROM commercial_binding WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(persisted, (commercial_org.to_string(), json));
    let (persisted_installation, persisted_local_org) = ids(&pool).await;
    assert_eq!(
        (persisted_installation, persisted_local_org),
        (installation, local_org)
    );
    assert!(!es_solo_lectura(&service).await);
}

#[tokio::test]
async fn primer_grant_comercial_vincula_instala_y_audita_en_una_transaccion() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("automatic.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, local_org) = ids(&pool).await;
    let remote_org = OrganizationId::new();
    let (json, signature) = firmar(
        &grant(
            installation,
            remote_org,
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    let mut foreign_actor = actor.clone();
    foreign_actor.organization_id = OrganizationId::new();
    assert!(
        service
            .install_license(&foreign_actor, json.clone(), signature.clone())
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM commercial_binding")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    service
        .install_license(&actor, json, signature)
        .await
        .unwrap();
    assert_eq!(ids(&pool).await, (installation, local_org));
    let row: String =
        sqlx::query_scalar("SELECT organization_id FROM commercial_binding WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, remote_org.to_string());
    let audit = service.list_audit(&actor, 50).await.unwrap();
    assert!(
        audit
            .iter()
            .any(|event| event.action == "commercial.identity_bound")
    );
    assert!(
        audit
            .iter()
            .any(|event| event.action == "license.installed")
    );
    assert!(!es_solo_lectura(&service).await);
}

#[tokio::test]
async fn licencia_comercial_vencida_no_reanuda_trial() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("expired-bind.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, _) = ids(&pool).await;
    let (json, signature) = firmar(
        &grant(
            installation,
            OrganizationId::new(),
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() - time::Duration::days(1),
        ),
        &SEED,
    );
    service
        .install_license(&actor, json, signature)
        .await
        .unwrap();
    assert!(es_solo_lectura(&service).await);
}

#[tokio::test]
async fn vinculo_sin_grant_persistido_no_reanuda_trial() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("incomplete.db")).await.unwrap();
    let (service, _) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, _) = ids(&pool).await;
    let org = OrganizationId::new();
    let (json, signature) = firmar(
        &grant(
            installation,
            org,
            PlanName::Pro,
            vec![FeatureKey::Sales],
            OffsetDateTime::now_utc() - time::Duration::days(1),
        ),
        &SEED,
    );
    sqlx::query("INSERT INTO commercial_binding \
        (id, installation_id, organization_id, proof_source, proof_json, proof_signature_b64, bound_at) \
        VALUES (1, ?, ?, 'signed_license_grant_v1', ?, ?, ?)")
        .bind(installation.to_string()).bind(org.to_string())
        .bind(json).bind(signature).bind("2026-01-01T00:00:00Z")
        .execute(&pool).await.unwrap();
    assert!(es_solo_lectura(&service).await);
}

#[tokio::test]
async fn ventas_bloqueadas_exactamente_en_valid_until() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("boundary.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, _) = ids(&pool).await;
    let boundary = OffsetDateTime::now_utc() + time::Duration::days(1);
    let (json, signature) = firmar(
        &grant(
            installation,
            OrganizationId::new(),
            PlanName::Pro,
            vec![FeatureKey::Sales],
            boundary,
        ),
        &SEED,
    );
    service
        .install_license(&actor, json, signature)
        .await
        .unwrap();
    let at_boundary = build_at(
        pool.clone(),
        Some(verifier_with(&SEED)),
        Arc::new(FixedClock(boundary)),
    );
    let view = at_boundary.status().await.unwrap().commercial.unwrap();
    assert!(view.read_only);
    let result = at_boundary
        .checkout_sale(
            &actor,
            CheckoutDraft {
                shift_id: pos_core::ids::CashShiftId::new().to_string(),
                idempotency_key: "valid-until-boundary".into(),
                items: vec![SaleItemDraft {
                    product_id: pos_core::ids::ProductId::new().to_string(),
                    variant_id: None,
                    quantity: "1".into(),
                    selected_option_ids: vec![],
                }],
                cash_received: "10.00".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(result.code(), "commercial_state_blocked");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sale")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn vinculo_alterado_no_autoriza() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("tamper.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, _) = ids(&pool).await;
    let org = OrganizationId::new();
    let (json, sig) = firmar(
        &grant(
            installation,
            org,
            PlanName::Pro,
            vec![FeatureKey::Catalog],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    service.install_license(&actor, json, sig).await.unwrap();
    sqlx::query("UPDATE commercial_binding SET organization_id = ? WHERE id = 1")
        .bind(OrganizationId::new().to_string())
        .execute(&pool)
        .await
        .unwrap();
    assert!(es_solo_lectura(&service).await);
}

// --- Casos positivos ---------------------------------------------------------
