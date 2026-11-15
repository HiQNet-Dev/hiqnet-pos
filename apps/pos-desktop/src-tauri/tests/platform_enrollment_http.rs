//! Contrato HTTP del cliente de enrolamiento contra un servidor local.
//!
//! No sustituye una prueba contra Platform real; fija el JSON, las rutas y el
//! manejo de errores para detectar drift del contrato.

use pos_application::{BeginPlatformEnrollment, PlatformEnrollmentClient};
use pos_core::ids::InstallationId;
use pos_desktop_lib::platform_enrollment::HttpPlatformEnrollment;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&buffer[..position]).to_string();
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            let start = position + 4;
            while buffer.len() < start + length {
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
            }
            break;
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
}

async fn respond(stream: &mut tokio::net::TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    stream.shutdown().await.unwrap();
}

#[tokio::test]
async fn inicia_y_completa_contra_un_servidor_http_local() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let installation_id = InstallationId::new();
    let organization_id = pos_core::ids::OrganizationId::new();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        assert!(request.starts_with("POST /api/v1/enrollment-requests "));
        assert!(request.contains(&installation_id.to_string()));
        assert!(request.contains("ed25519:v1:"));
        respond(
            &mut stream,
            "201 Created",
            r#"{"device_code":"device-sintetico","user_code":"ABCD-EFGH","verification_uri":"http://127.0.0.1:8080/device","nonce":"nonce-1","expires_in":600}"#,
        )
        .await;

        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        assert!(request.starts_with("POST /api/v1/enrollment-requests/device-sintetico/proof "));
        assert!(request.contains("\"signature\""));
        respond(
            &mut stream,
            "201 Created",
            &format!(
                r#"{{"enrollment_request_id":"11111111-1111-4111-8111-111111111111","installation_id":"{installation_id}","organization_id":"{organization_id}","installation_identity_id":"22222222-2222-4222-8222-222222222222","installation_binding_id":"33333333-3333-4333-8333-333333333333","bound_at":"2026-11-14T00:00:00Z","status":"completed"}}"#
            ),
        )
        .await;
    });

    let client = HttpPlatformEnrollment::new(format!("http://{address}")).unwrap();
    let challenge = client
        .begin(BeginPlatformEnrollment {
            installation_id,
            product_key: "pos".into(),
            public_key: "ed25519:v1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            device_label: "HiQNet POS".into(),
            app_version: "0.1.1".into(),
            platform: "linux".into(),
        })
        .await
        .unwrap();
    assert_eq!(challenge.user_code, "ABCD-EFGH");
    assert_eq!(challenge.device_code, "device-sintetico");

    let binding = client
        .prove(pos_application::PlatformProof {
            device_code: challenge.device_code.clone(),
            installation_id,
            public_key: "ed25519:v1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            nonce: challenge.nonce.clone(),
            signature_b64: "firma".into(),
        })
        .await
        .unwrap();
    assert_eq!(binding.installation_id, installation_id);
    assert_eq!(binding.organization_id, organization_id);

    server.await.unwrap();
}
