//! E2E real del enrolamiento contra una instancia local de Platform.
//!
//! Requiere Platform en ejecución, un producto `pos` y una organización con
//! owner. Se ejecuta solo manualmente para no depender de infraestructura en CI:
//!
//! ```bash
//! HIQNET_PLATFORM_E2E_URL=http://127.0.0.1:8080 \
//! HIQNET_PLATFORM_E2E_EMAIL=portal@example.test \
//! HIQNET_PLATFORM_E2E_PASSWORD=... \
//! HIQNET_PLATFORM_E2E_ORG=<uuid> \
//! cargo test --test platform_enrollment_real -- --ignored
//! ```
//!
//! La contraseña no se registra ni se escribe en el repositorio.

use pos_application::{BeginPlatformEnrollment, PlatformEnrollmentClient};
use pos_core::ids::{InstallationId, OrganizationId};
use pos_desktop_lib::platform_enrollment::HttpPlatformEnrollment;
use pos_licensing::DeviceIdentity;

#[tokio::test]
#[ignore = "requiere Platform local, producto y owner aprobados"]
async fn enrolamiento_real_begin_approve_proof() {
    let base =
        std::env::var("HIQNET_PLATFORM_E2E_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let email = std::env::var("HIQNET_PLATFORM_E2E_EMAIL").expect("email de owner");
    let password = std::env::var("HIQNET_PLATFORM_E2E_PASSWORD").expect("contraseña de owner");
    let organization_id: OrganizationId = std::env::var("HIQNET_PLATFORM_E2E_ORG")
        .expect("organización del owner")
        .parse()
        .expect("UUID de organización");

    let http = reqwest::Client::new();
    let login = http
        .post(format!("{base}/api/v1/auth/login"))
        .header("Origin", "http://127.0.0.1:5175")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("login comercial");
    assert!(
        login.status().is_success(),
        "credenciales sintéticas válidas"
    );
    let cookie = login
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| {
            value
                .split(';')
                .next()
                .filter(|cookie| cookie.starts_with("hiqnet_session="))
                .map(str::to_owned)
        })
        .expect("cookie de sesión comercial");

    let identity = DeviceIdentity::generate();
    let client = HttpPlatformEnrollment::new(base.clone()).unwrap();
    let installation_id = InstallationId::new();
    // Platform exige etiqueta única por organización.
    let device_label = format!("HiQNet POS E2E {}", &installation_id.to_string()[..8]);
    let challenge = client
        .begin(BeginPlatformEnrollment {
            installation_id,
            product_key: "pos".into(),
            public_key: identity.public_key_encoded(),
            device_label,
            app_version: "0.1.1".into(),
            platform: "linux".into(),
        })
        .await
        .expect("begin aceptado por Platform");

    let approve = http
        .post(format!(
            "{}/api/v1/enrollment-requests/{}/approve",
            base, challenge.user_code
        ))
        .header("Origin", "http://127.0.0.1:5175")
        .header("Cookie", cookie)
        .json(&serde_json::json!({ "organization_id": organization_id }))
        .send()
        .await
        .expect("aprobación del owner");
    assert_eq!(approve.status(), 200, "aprobación aceptada");

    let device_code = challenge.device_code;
    let nonce = challenge.nonce;
    let public_key = identity.public_key_encoded();
    let signature_b64 = identity.sign_proof(&device_code, installation_id, &nonce);
    let binding = client
        .prove(pos_application::PlatformProof {
            device_code,
            installation_id,
            public_key,
            nonce,
            signature_b64,
        })
        .await
        .expect("proof aceptado por Platform");
    assert_eq!(binding.installation_id, installation_id);
    assert_eq!(binding.organization_id, organization_id);
}
