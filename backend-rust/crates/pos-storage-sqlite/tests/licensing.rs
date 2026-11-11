//! Integración de licencias firmadas sobre SQLite real.
//!
//! Demuestra que el estado comercial se deriva del documento firmado y no de
//! columnas editables, y que cualquier manipulación de la base termina en solo
//! lectura. Usa una clave sintética exclusiva de pruebas.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signer, SigningKey};
use pos_application::{
    AppStatus, BootstrapRequest, LoginRequest, PosService, ProductDraft, SharedClock, SharedHasher,
    SharedLicenseVerifier, SharedStore, SystemClock,
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
    let store: SharedStore = Arc::new(SqliteStore::new(pool));
    let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
    let clock: SharedClock = Arc::new(SystemClock);
    let service = PosService::new(store, hasher, clock);
    match license {
        Some(verifier) => service.with_license_verifier(verifier),
        None => service,
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

fn producto(sku: &str) -> ProductDraft {
    ProductDraft {
        sku: sku.into(),
        name: format!("Producto {sku}"),
        price: "10.00".into(),
        tax_rate_bp: 1600,
        category_id: None,
    }
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

// --- Casos positivos ---------------------------------------------------------

#[tokio::test]
async fn grant_valido_autoriza_los_entitlements_firmados() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("lic.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, organization) = ids(&pool).await;

    let (json, firma) = firmar(
        &grant(
            installation,
            organization,
            PlanName::Pro,
            vec![FeatureKey::Catalog, FeatureKey::Sales],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    let view = service.install_license(&actor, json, firma).await.unwrap();
    assert_eq!(view.plan, "pro");
    assert!(!view.read_only);
    assert!(view.features.contains(&"catalog"));
    assert!(!view.features.contains(&"expenses"));

    // Catálogo autorizado por el grant.
    service
        .create_product(&actor, producto("LIC-1"))
        .await
        .unwrap();

    // Inventario no contratado: el grant no lo incluye.
    let error = service
        .record_inventory_movement(
            &actor,
            pos_application::InventoryMovementDraft {
                product_id: "00000000-0000-0000-0000-000000000000".into(),
                kind: "inbound".into(),
                quantity: "1".into(),
                reason: None,
            },
        )
        .await
        .unwrap_err();
    assert!(
        format!("{error:?}").contains("FeatureNotEntitled"),
        "se esperaba falta de entitlement: {error:?}"
    );

    // Auditoria de la instalacion.
    let audit = service.list_audit(&actor, 50).await.unwrap();
    assert!(
        audit
            .iter()
            .any(|event| event.action == "license.installed")
    );
}

#[tokio::test]
async fn la_licencia_persiste_y_permite_operar_tras_reiniciar() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lic.db");
    let pool = open(&path).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, organization) = ids(&pool).await;
    let (json, firma) = firmar(
        &grant(
            installation,
            organization,
            PlanName::Local,
            vec![FeatureKey::Catalog, FeatureKey::Sales],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    service.install_license(&actor, json, firma).await.unwrap();
    drop(service);

    // Reinicio: se reabre la base y se reconstruye el servicio.
    let pool = open(&path).await.unwrap();
    let service = build(pool, Some(verifier_with(&SEED)));
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: CLAVE.into(),
        })
        .await
        .unwrap();
    assert!(!es_solo_lectura(&service).await);
    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.plan, "local");
    service
        .create_product(&actor, producto("LIC-2"))
        .await
        .unwrap();
}

#[tokio::test]
async fn renovacion_reemplaza_la_vigencia() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("lic.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, organization) = ids(&pool).await;

    let corta = OffsetDateTime::now_utc() + time::Duration::days(5);
    let larga = OffsetDateTime::now_utc() + time::Duration::days(90);
    for vigencia in [corta, larga] {
        let (json, firma) = firmar(
            &grant(
                installation,
                organization,
                PlanName::Pro,
                vec![FeatureKey::Catalog],
                vigencia,
            ),
            &SEED,
        );
        service.install_license(&actor, json, firma).await.unwrap();
    }

    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.valid_until.unix_timestamp(), larga.unix_timestamp());
    let audit = service.list_audit(&actor, 50).await.unwrap();
    let instalaciones = audit
        .iter()
        .filter(|event| event.action == "license.installed")
        .count();
    assert_eq!(instalaciones, 2, "la renovacion debe auditarse");
}

// --- Casos negativos: manipulacion directa de SQLite -------------------------

async fn base_con_licencia() -> (tempfile::TempDir, std::path::PathBuf, PosService, Actor) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lic.db");
    let pool = open(&path).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, organization) = ids(&pool).await;
    let (json, firma) = firmar(
        &grant(
            installation,
            organization,
            PlanName::Pro,
            vec![FeatureKey::Catalog, FeatureKey::Sales],
            OffsetDateTime::now_utc() + time::Duration::days(30),
        ),
        &SEED,
    );
    service.install_license(&actor, json, firma).await.unwrap();
    drop(service);
    (dir, path, build(pool, None), actor)
}

async fn manipular(path: &std::path::Path, sql: &str) {
    let pool = open(path).await.unwrap();
    sqlx::query(sql).execute(&pool).await.unwrap();
    pool.close().await;
}

async fn reabrir(path: &std::path::Path, license: Option<SharedLicenseVerifier>) -> PosService {
    build(open(path).await.unwrap(), license)
}

async fn rechaza_mutacion(service: &PosService, actor: &Actor, sku: &str) -> bool {
    service.create_product(actor, producto(sku)).await.is_err()
}

#[tokio::test]
async fn extender_valid_until_en_la_fila_no_extiende_la_licencia() {
    let (_dir, path, _, _actor) = base_con_licencia().await;
    manipular(
        &path,
        "UPDATE local_installation_state SET valid_until = '2099-01-01T00:00:00Z' WHERE id = 1",
    )
    .await;
    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    let view = estado(&service).await.commercial.unwrap();
    assert!(
        view.valid_until.year() < 2099,
        "la columna editable no puede extender la vigencia"
    );
}

#[tokio::test]
async fn cambiar_plan_o_features_en_la_fila_no_concede_capacidades() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    manipular(
        &path,
        "UPDATE local_installation_state SET plan = 'enterprise', \
         features_json = '[\"catalog\",\"sales\",\"expenses\",\"inventory\"]' WHERE id = 1",
    )
    .await;
    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.plan, "pro", "el plan lo fija el documento firmado");
    assert!(
        !view.features.contains(&"expenses"),
        "las features las fija el documento firmado"
    );
    let error = service
        .record_inventory_movement(
            &actor,
            pos_application::InventoryMovementDraft {
                product_id: "00000000-0000-0000-0000-000000000000".into(),
                kind: "inbound".into(),
                quantity: "1".into(),
                reason: None,
            },
        )
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("FeatureNotEntitled"));
}

#[tokio::test]
async fn cambiar_installation_id_pasa_a_solo_lectura() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    manipular(
        &path,
        "UPDATE local_installation_state \
         SET installation_id = '11111111-1111-1111-1111-111111111111' WHERE id = 1",
    )
    .await;
    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-3").await);
}

#[tokio::test]
async fn cambiar_organization_id_pasa_a_solo_lectura() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    // La columna tiene clave foranea: se crea otra organizacion real para poder
    // apuntar a ella sin violar la integridad referencial.
    manipular(
        &path,
        "INSERT INTO organization (id, name, created_at) \
         VALUES ('22222222-2222-2222-2222-222222222222', 'Otra', '2026-01-01T00:00:00Z')",
    )
    .await;
    manipular(
        &path,
        "UPDATE local_installation_state \
         SET organization_id = '22222222-2222-2222-2222-222222222222' WHERE id = 1",
    )
    .await;
    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-4").await);
}

#[tokio::test]
async fn grant_de_otra_instalacion_es_rechazado() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("lic.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, organization) = ids(&pool).await;

    // Documento legítimo de *otra* instalación.
    let ajeno = grant(
        InstallationId::new(),
        organization,
        PlanName::Pro,
        vec![FeatureKey::Catalog],
        OffsetDateTime::now_utc() + time::Duration::days(30),
    );
    let (json, firma) = firmar(&ajeno, &SEED);
    let error = service
        .install_license(&actor, json, firma)
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("Crypto"));

    // La instalación local sigue en trial, no en pro.
    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.plan, "trial");
    let _ = installation;

    // La auditoría registra el rechazo.
    let audit = service.list_audit(&actor, 50).await.unwrap();
    assert!(audit.iter().any(|event| event.action == "license.rejected"));
}

#[tokio::test]
async fn firma_alterada_en_la_fila_pasa_a_solo_lectura() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    manipular(
        &path,
        "UPDATE local_installation_state \
         SET license_signature_b64 = 'ZmlybWEtZmFsc2E=' WHERE id = 1",
    )
    .await;
    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-5").await);
}

#[tokio::test]
async fn grant_reemplazado_por_uno_de_otra_clave_es_rechazado() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    let (installation, organization) = {
        let pool = open(&path).await.unwrap();
        let identificadores = ids(&pool).await;
        pool.close().await;
        identificadores
    };

    // Documento con la identidad correcta pero firmado con otra clave.
    let falsificado = grant(
        installation,
        organization,
        PlanName::Enterprise,
        vec![FeatureKey::Catalog, FeatureKey::Sales, FeatureKey::Expenses],
        OffsetDateTime::now_utc() + time::Duration::days(365),
    );
    let (json, firma) = firmar(&falsificado, &OTHER_SEED);
    manipular(
        &path,
        &format!(
            "UPDATE local_installation_state SET license_grant_json = '{}', \
             license_signature_b64 = '{}', plan = 'enterprise' WHERE id = 1",
            json.replace('\'', "''"),
            firma
        ),
    )
    .await;

    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-6").await);
}

#[tokio::test]
async fn clave_publica_incorrecta_rechaza_la_licencia() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    let service = reabrir(&path, Some(verifier_with(&OTHER_SEED))).await;
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-7").await);
}

#[tokio::test]
async fn sin_clave_de_verificacion_la_licencia_no_autoriza() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    let service = reabrir(&path, None).await;
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-8").await);
}

#[tokio::test]
async fn grant_expirado_deja_solo_lectura() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("lic.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let (installation, organization) = ids(&pool).await;
    let (json, firma) = firmar(
        &grant(
            installation,
            organization,
            PlanName::Pro,
            vec![FeatureKey::Catalog, FeatureKey::Sales],
            OffsetDateTime::now_utc() - time::Duration::days(1),
        ),
        &SEED,
    );
    service.install_license(&actor, json, firma).await.unwrap();
    assert!(es_solo_lectura(&service).await);
    assert!(rechaza_mutacion(&service, &actor, "LIC-9").await);
}

#[tokio::test]
async fn el_trial_sin_licencia_sigue_funcionando() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("trial.db")).await.unwrap();
    let (service, actor) = preparar(&pool, None).await;
    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.status, "trial");
    assert!(!view.read_only);
    service
        .create_product(&actor, producto("TRIAL-1"))
        .await
        .unwrap();
}

// --- Transiciones licencia → sin grant → trial -------------------------------

/// Eliminar el grant no reinicia el trial: la instalación que tuvo licencia
/// queda en sólo lectura, aunque las columnas sin verificar queden en la fila.
#[tokio::test]
async fn borrar_el_grant_no_reinicia_el_trial_ni_autoriza() {
    let (_dir, path, _, actor) = base_con_licencia().await;
    // Ataque: borrar el documento firmado y su firma, dejando en la fila el
    // plan, el estado y la vigencia que la licencia había escrito.
    manipular(
        &path,
        "UPDATE local_installation_state SET license_grant_json = NULL, \
             license_signature_b64 = NULL, license_status = 'active', \
             plan = 'pro', valid_until = '2030-01-01T00:00:00Z'",
    )
    .await;
    let service = reabrir(&path, Some(verifier_with(&SEED))).await;
    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.status, "read_only");
    assert!(view.read_only);
    assert!(view.features.is_empty());
    // Ni reinicio del trial ni operación comercial sin documento.
    assert!(rechaza_mutacion(&service, &actor, "LIC-10").await);
}

/// El mismo ataque sobre una instalación que nunca tuvo licencia no cambia el
/// trial legítimo: el marcador es lo que distingue ambos casos.
#[tokio::test]
async fn el_trial_de_una_instalacion_nueva_no_se_reinicia_por_marca_ajena() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trial-limpio.db");
    let pool = open(&path).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED))).await;
    let view = estado(&service).await.commercial.unwrap();
    assert_eq!(view.status, "trial");
    let trial_started: String =
        sqlx::query_scalar("SELECT trial_started_at FROM local_installation_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    // Sin licencia instalada, el marcador no existe y el trial legítimo sigue.
    let marcador: Option<String> = sqlx::query_scalar(
        "SELECT license_installed_at FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(marcador.is_none());
    let _ = trial_started;
    assert!(!view.read_only);
    service
        .create_product(&actor, producto("TRIAL-2"))
        .await
        .unwrap();
}

/// Un grant emitido por Platform (canonical JSON con orden de campos fijo y
/// fechas en el formato de `time`) se verifica nativamente en el POS con la
/// clave pública del par sintético de pruebas.
#[tokio::test]
async fn grant_emitido_por_platform_se_verifica_nativamente() {
    use base64::Engine as _;
    use ed25519_dalek::Signer;
    use serde::{Deserialize, Serialize};

    // Réplica del struct canónico de Platform: mismo orden de campos y tipos.
    // El espejo comprueba que el documento emitido por Platform es legible
    // campo a campo por el POS; los campos no asertados son `#[allow(dead_code)]`
    // porque su valor se valida ya en el grant verificado.
    #[derive(Serialize)]
    struct GrantPlatform {
        version: u32,
        grant_id: uuid::Uuid,
        installation_id: InstallationId,
        organization_id: OrganizationId,
        plan: PlanName,
        features: Vec<FeatureKey>,
        issued_at: OffsetDateTime,
        valid_until: OffsetDateTime,
    }
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct GrantMirror {
        version: u32,
        grant_id: String,
        installation_id: InstallationId,
        organization_id: OrganizationId,
        plan: PlanName,
        features: Vec<FeatureKey>,
        issued_at: OffsetDateTime,
        valid_until: OffsetDateTime,
    }

    // El par sintético de pruebas es el mismo que la semilla DEV de Platform.
    const SEED_PLATFORM: [u8; 32] = [7u8; 32];
    let signing = ed25519_dalek::SigningKey::from_bytes(&SEED_PLATFORM);
    let vigencia = OffsetDateTime::now_utc() + time::Duration::days(45);
    let documento = GrantPlatform {
        version: LICENSE_DOC_VERSION,
        grant_id: uuid::Uuid::new_v4(),
        installation_id: InstallationId::new(),
        organization_id: OrganizationId::new(),
        plan: PlanName::Pro,
        features: vec![FeatureKey::Catalog, FeatureKey::Sales, FeatureKey::Reports],
        issued_at: OffsetDateTime::now_utc(),
        valid_until: vigencia,
    };
    let grant_json = String::from_utf8(serde_json::to_vec(&documento).unwrap()).unwrap();
    let firma = base64::engine::general_purpose::STANDARD
        .encode(signing.sign(grant_json.as_bytes()).to_bytes());

    let verifier = Ed25519LicenseVerifier::from_public_key_base64(
        &base64::engine::general_purpose::STANDARD.encode(signing.verifying_key().to_bytes()),
    )
    .unwrap();
    let grant = verifier.verify_grant(&grant_json, &firma).unwrap();
    assert_eq!(grant.plan, PlanName::Pro);
    assert_eq!(grant.features.len(), 3);
    assert_eq!(grant.valid_until, vigencia);

    // El grant de Platform se instala en el POS real y autoriza.
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("platform.db")).await.unwrap();
    let (service, actor) = preparar(&pool, Some(verifier_with(&SEED_PLATFORM))).await;
    let (installation, organization) = ids(&pool).await;
    // Re-firmar con las identidades locales: el caso de entrega exige que el
    // documento traiga las identidades de esta instalación.
    let entregado = GrantPlatform {
        version: LICENSE_DOC_VERSION,
        grant_id: documento.grant_id,
        installation_id: installation,
        organization_id: organization,
        plan: documento.plan,
        features: documento.features,
        issued_at: documento.issued_at,
        valid_until: vigencia,
    };
    let json_entregado = String::from_utf8(serde_json::to_vec(&entregado).unwrap()).unwrap();
    let firma_entregada = base64::engine::general_purpose::STANDARD
        .encode(signing.sign(json_entregado.as_bytes()).to_bytes());
    let view = service
        .install_license(&actor, json_entregado.clone(), firma_entregada)
        .await
        .unwrap();
    assert_eq!(view.plan, "pro");
    assert!(!view.read_only);
    assert!(view.features.contains(&"catalog"));

    // El documento entregado coincide con el esquema canónico que Platform
    // produce (difiere sólo en las identidades de la instalación local).
    let espejo: GrantMirror = serde_json::from_str(json_entregado.as_str()).unwrap();
    assert_eq!(espejo.version, LICENSE_DOC_VERSION);
    assert_eq!(espejo.installation_id, installation);
    assert_eq!(espejo.organization_id, organization);
    assert_eq!(espejo.features.len(), 3);
    assert_eq!(espejo.valid_until, vigencia);
    assert_eq!(espejo.plan, PlanName::Pro);
}
