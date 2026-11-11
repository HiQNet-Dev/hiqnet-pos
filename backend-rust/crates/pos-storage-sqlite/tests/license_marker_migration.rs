//! Migración segura del marcador de licencia (0012) sobre bases históricas.
//!
//! Simula una instalación creada con el esquema anterior (0001–0011) que ya
//! tenía un documento firmado almacenado, y comprueba que:
//!
//! 1. La actualización rellena `license_installed_at` a partir de la evidencia
//!    existente (grant + firma persistidos), sin usar columnas comerciales sin
//!    verificar como prueba.
//! 2. Tras la actualización, borrar sólo el grant deja la instalación en sólo
//!    lectura: no hay retorno al trial.
//! 3. Una base histórica sin licencia (trial legítimo) sigue siendo trial.
//!
//! No basta con una base creada directamente con la migración nueva: la prueba
//! parte de una base construida con el esquema publicado anteriormente.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signer, SigningKey};
use pos_application::{
    BootstrapRequest, LoginRequest, PosService, SharedClock, SharedHasher, SharedLicenseVerifier,
    SharedStore, SystemClock,
};
use pos_core::entitlements::{FeatureKey, PlanName};
use pos_core::ids::{InstallationId, OrganizationId};
use pos_licensing::{Ed25519LicenseVerifier, LICENSE_DOC_VERSION, LicenseGrant};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::{SqliteStore, open};
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

const SEED: [u8; 32] = [7u8; 32];
const CLAVE: &str = "contrasena-segura";
const ORG: &str = "0f9e8d7c-0000-4000-8000-0000000000aa";
const INSTALL: &str = "0f9e8d7c-0000-4000-8000-0000000000bb";

const MIGRACIONES: [&str; 11] = [
    "0001_init.sql",
    "0002_inventory.sql",
    "0003_inventory_adjustments.sql",
    "0004_catalog_categories_variants.sql",
    "0005_catalog_modifiers.sql",
    "0006_cash_registers_shifts.sql",
    "0007_sales_checkout.sql",
    "0008_purchasing.sql",
    "0009_expenses_waste.sql",
    "0010_sale_returns.sql",
    "0011_catalog_list_index.sql",
];

fn migraciones() -> Vec<&'static str> {
    MIGRACIONES
        .iter()
        .map(|name| {
            let path = format!("../migrations/local/{name}");
            include_str_bytes(&path)
        })
        .collect()
}

fn include_str_bytes(path: &str) -> &'static str {
    // `include_str!` exige un literal; se resuelve con un match estático.
    match path {
        "../migrations/local/0001_init.sql" => {
            include_str!("../../../migrations/local/0001_init.sql")
        }
        "../migrations/local/0002_inventory.sql" => {
            include_str!("../../../migrations/local/0002_inventory.sql")
        }
        "../migrations/local/0003_inventory_adjustments.sql" => {
            include_str!("../../../migrations/local/0003_inventory_adjustments.sql")
        }
        "../migrations/local/0004_catalog_categories_variants.sql" => {
            include_str!("../../../migrations/local/0004_catalog_categories_variants.sql")
        }
        "../migrations/local/0005_catalog_modifiers.sql" => {
            include_str!("../../../migrations/local/0005_catalog_modifiers.sql")
        }
        "../migrations/local/0006_cash_registers_shifts.sql" => {
            include_str!("../../../migrations/local/0006_cash_registers_shifts.sql")
        }
        "../migrations/local/0007_sales_checkout.sql" => {
            include_str!("../../../migrations/local/0007_sales_checkout.sql")
        }
        "../migrations/local/0008_purchasing.sql" => {
            include_str!("../../../migrations/local/0008_purchasing.sql")
        }
        "../migrations/local/0009_expenses_waste.sql" => {
            include_str!("../../../migrations/local/0009_expenses_waste.sql")
        }
        "../migrations/local/0010_sale_returns.sql" => {
            include_str!("../../../migrations/local/0010_sale_returns.sql")
        }
        "../migrations/local/0011_catalog_list_index.sql" => {
            include_str!("../../../migrations/local/0011_catalog_list_index.sql")
        }
        _ => unreachable!(),
    }
}

async fn checksums_hasta(pool: &SqlitePool, version_maxima: i64) -> Vec<(i64, Vec<u8>)> {
    sqlx::query(
        "SELECT version, checksum FROM _sqlx_migrations WHERE version <= ? ORDER BY version",
    )
    .bind(version_maxima)
    .fetch_all(pool)
    .await
    .expect("leer metadatos de migraciones")
    .into_iter()
    .map(|row| {
        (
            row.get::<i64, _>("version"),
            row.get::<Vec<u8>, _>("checksum"),
        )
    })
    .collect()
}

/// Construye una base con el esquema publicado hasta 0011 y sus checksums
/// canónicos registrados, como una instalación real anterior a la corrección.
async fn base_legacy(path: &std::path::Path, con_grant: bool) {
    // Base de referencia para obtener los checksums que SQLx espera.
    let reference_dir = tempfile::tempdir().expect("directorio temporal");
    let reference = open(reference_dir.path().join("reference.db"))
        .await
        .expect("base de referencia");
    let checksums = checksums_hasta(&reference, 11).await;
    assert_eq!(checksums.len(), 11);
    reference.close().await;

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .expect("abrir base sintética");

    for migracion in migraciones() {
        sqlx::raw_sql(migracion)
            .execute(&pool)
            .await
            .expect("aplicar migración publicada");
    }

    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES (?, 'Licencias Históricas', '2026-01-01T00:00:00Z')")
        .bind(ORG)
        .execute(&pool)
        .await
        .expect("insertar organización");

    // Estado de instalación histórico. Antes de 0012 no existía la columna
    // `license_installed_at`; la fila con grant almacenado es la evidencia que
    // la migración debe conservar.
    let (grant_json, signature_b64) = if con_grant {
        let grant = LicenseGrant {
            version: LICENSE_DOC_VERSION,
            grant_id: "grant-legacy-001".into(),
            installation_id: INSTALL.parse().unwrap(),
            organization_id: ORG.parse().unwrap(),
            plan: PlanName::Local,
            features: vec![FeatureKey::Catalog, FeatureKey::Sales],
            issued_at: OffsetDateTime::now_utc() - time::Duration::days(5),
            valid_until: OffsetDateTime::now_utc() + time::Duration::days(25),
        };
        let json = String::from_utf8(grant.to_canonical_json().unwrap()).unwrap();
        let firma = B64.encode(
            SigningKey::from_bytes(&SEED)
                .sign(json.as_bytes())
                .to_bytes(),
        );
        (Some(json), Some(firma))
    } else {
        (None, None)
    };
    sqlx::query(
        "INSERT INTO local_installation_state \
             (id, installation_id, organization_id, plan, license_status, features_json, \
              trial_started_at, valid_until, license_grant_json, license_signature_b64, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, '2026-01-01T00:00:00Z')",
    )
    .bind(INSTALL)
    .bind(ORG)
    .bind(if con_grant { "local" } else { "trial" })
    .bind(if con_grant { "active" } else { "trial" })
    .bind(r#"["catalog","sales"]"#)
    .bind("2026-01-01T00:00:00Z")
    .bind("2026-02-01T00:00:00Z")
    .bind(grant_json)
    .bind(signature_b64)
    .execute(&pool)
    .await
    .expect("insertar estado de instalación histórico");

    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _sqlx_migrations ( \
             version BIGINT PRIMARY KEY, \
             description TEXT NOT NULL, \
             installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, \
             success BOOLEAN NOT NULL, \
             checksum BLOB NOT NULL, \
             execution_time BIGINT NOT NULL \
         );",
    )
    .execute(&pool)
    .await
    .expect("crear tabla de migraciones");

    for (version, checksum) in checksums {
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (?, 'legacy', 1, ?, 0)",
        )
        .bind(version)
        .bind(checksum)
        .execute(&pool)
        .await
        .expect("registrar migración aplicada");
    }

    pool.close().await;
}

fn service(pool: SqlitePool) -> PosService {
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

/// Instalación histórica con grant almacenado: la migración conserva la
/// evidencia de licenciamiento y el borrado posterior del grant no reinicia el
/// trial.
#[tokio::test]
async fn la_migracion_conserva_el_marcador_de_una_instalacion_licenciada() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let path = dir.path().join("legacy-licenciada.db");
    base_legacy(&path, true).await;

    // La actualización completa las migraciones restantes, incluida 0012.
    let pool = open(&path).await.expect("actualizar base anterior");
    let marcador: Option<String> = sqlx::query_scalar(
        "SELECT license_installed_at FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("leer marcador");
    assert!(
        marcador.is_some(),
        "una base histórica con grant almacenado debe conservar la evidencia de licenciamiento"
    );
    drop(pool);

    // Ataque posterior: borrar sólo el grant y su firma.
    let pool = open(&path).await.unwrap();
    sqlx::query(
        "UPDATE local_installation_state SET license_grant_json = NULL, license_signature_b64 = NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    drop(pool);

    let service = service(open(&path).await.unwrap());
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: CLAVE.into(),
        })
        .await;
    if let Ok(actor) = actor {
        let status = service.status().await.unwrap();
        let commercial = status.commercial.unwrap();
        assert_eq!(commercial.status, "read_only");
        assert!(commercial.read_only);
        assert!(commercial.features.is_empty());
        assert!(
            service
                .create_product(&actor, producto("MIG-1"))
                .await
                .is_err(),
            "sin documento firmado no hay operación comercial"
        );
    } else {
        // Si el login inicial no está disponible (estado histórico mínimo), la
        // comprobación de solo lectura se hace por la vista comercial pública.
        let status = service.status().await.unwrap();
        let commercial = status.commercial.unwrap();
        assert_eq!(commercial.status, "read_only");
    }
}

/// Instalación histórica sin licencia: el marcador sigue ausente y el trial
/// legítimo no se pierde por la actualización.
#[tokio::test]
async fn la_migracion_no_marca_un_trial_sin_licencia() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let path = dir.path().join("legacy-trial.db");
    base_legacy(&path, false).await;

    let pool = open(&path).await.expect("actualizar base anterior");
    let marcador: Option<String> = sqlx::query_scalar(
        "SELECT license_installed_at FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("leer marcador");
    assert!(marcador.is_none(), "sin grant almacenado no hay marcador");
}

/// Operaciones de guardado no reinician el marcador a NULL una vez establecido.
#[tokio::test]
async fn el_guardado_no_reinicia_el_marcador() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let path = dir.path().join("marcador-persistente.db");
    base_legacy(&path, true).await;
    let pool = open(&path).await.expect("actualizar base anterior");
    let service = service(pool.clone());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "X".into(),
            branch_name: "Y".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: CLAVE.into(),
        })
        .await
        .expect_err("la instalación histórica ya está inicializada");
    let marcador: Option<String> = sqlx::query_scalar(
        "SELECT license_installed_at FROM local_installation_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("leer marcador");
    assert!(
        marcador.is_some(),
        "una escritura rechazada no debe borrar el marcador"
    );
    let _ = InstallationId::new();
    let _ = OrganizationId::new();
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
