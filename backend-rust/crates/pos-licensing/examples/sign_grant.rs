//! Firmante de licencias exclusivo de pruebas.
//!
//! La semilla se recibe por variable de entorno (`POS_LICENSE_SEED_B64`) y nunca
//! se escribe en el repositorio. No forma parte del ejecutable POS: es un
//! ejemplo de `cargo`, no de la aplicación distribuida.
//!
//! Uso:
//!   POS_LICENSE_SEED_B64=<32 bytes base64> \
//!   cargo run -p pos-licensing --example sign_grant -- \
//!       <installation_id> <organization_id> <plan> <features> <dias>
//!
//! Con `public-key` como primer argumento imprime solo la clave pública.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signer, SigningKey};
use pos_core::entitlements::{FeatureKey, PlanName};
use pos_licensing::{LICENSE_DOC_VERSION, LicenseGrant};
use time::OffsetDateTime;

fn seed() -> SigningKey {
    let raw = std::env::var("POS_LICENSE_SEED_B64").expect("falta POS_LICENSE_SEED_B64");
    let bytes = B64.decode(raw.trim()).expect("semilla base64 invalida");
    let array: [u8; 32] = bytes.try_into().expect("la semilla debe ser de 32 bytes");
    SigningKey::from_bytes(&array)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let key = seed();

    if args.first().map(String::as_str) == Some("public-key") {
        println!("{}", B64.encode(key.verifying_key().to_bytes()));
        return;
    }
    if args.len() < 5 {
        eprintln!("uso: sign_grant <installation_id> <organization_id> <plan> <features> <dias>");
        std::process::exit(2);
    }

    let plan = match args[2].as_str() {
        "trial" => PlanName::Trial,
        "local" => PlanName::Local,
        "pro" => PlanName::Pro,
        "enterprise" => PlanName::Enterprise,
        other => {
            eprintln!("plan desconocido: {other}");
            std::process::exit(2);
        }
    };
    let features: Vec<FeatureKey> = args[3]
        .split(',')
        .filter(|value| !value.is_empty())
        .filter_map(FeatureKey::from_key)
        .collect();
    let dias: i64 = args[4].parse().expect("dias invalido");

    let now = OffsetDateTime::now_utc();
    let grant = LicenseGrant {
        version: LICENSE_DOC_VERSION,
        grant_id: format!("grant-e2e-{}", now.unix_timestamp()),
        installation_id: args[0].parse().expect("installation_id invalido"),
        organization_id: args[1].parse().expect("organization_id invalido"),
        plan,
        features,
        issued_at: now,
        valid_until: now + time::Duration::days(dias),
    };
    let json = String::from_utf8(grant.to_canonical_json().expect("json")).expect("utf8");
    let signature = B64.encode(key.sign(json.as_bytes()).to_bytes());
    println!(
        "{}",
        serde_json::json!({ "grant": json, "signature": signature })
    );
}
