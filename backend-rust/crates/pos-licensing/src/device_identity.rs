//! Identidad criptográfica del dispositivo para el enrolamiento con Platform.
//!
//! La clave privada nunca sale del equipo ni se incluye en respaldos por
//! defecto. El registro del enrolamiento solo necesita la clave pública y una
//! firma sobre el mensaje canónico definido por Platform.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use pos_core::ids::InstallationId;

/// Prefijo exacto del mensaje firmado por el dispositivo. Debe coincidir con
/// `platform-core::enrollment::PROOF_PREFIX`.
pub const PROOF_PREFIX: &str = "hiqnet-enrollment-proof:v1";

#[derive(Debug, thiserror::Error)]
pub enum DeviceIdentityError {
    #[error("la clave del dispositivo no es válida")]
    InvalidKey,
}

/// Par de claves Ed25519 del dispositivo.
#[derive(Clone)]
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl std::fmt::Debug for DeviceIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceIdentity")
            .field("public_key", &self.public_key_encoded())
            .finish_non_exhaustive()
    }
}

impl DeviceIdentity {
    /// Genera una identidad nueva con el generador del sistema operativo.
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut rand_core::OsRng),
        }
    }

    /// Reconstruye la identidad desde la semilla persistida (32 bytes).
    pub fn from_seed_bytes(seed: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    /// Semilla privada para persistir en el almacén del sistema. No registrar
    /// ni incluir en respaldos.
    pub fn seed_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Clave pública en el formato aceptado por Platform: `ed25519:v1:<base64url>`.
    pub fn public_key_encoded(&self) -> String {
        let encoded = URL_SAFE_NO_PAD.encode(self.signing_key.verifying_key().to_bytes());
        format!("ed25519:v1:{encoded}")
    }

    /// Firma el mensaje canónico de proof. Devuelve la firma en base64url sin
    /// relleno, como exige Platform.
    pub fn sign_proof(
        &self,
        device_code: &str,
        installation_id: InstallationId,
        nonce: &str,
    ) -> String {
        let message = canonical_proof_message(
            device_code,
            installation_id,
            &self.public_key_encoded(),
            nonce,
        );
        URL_SAFE_NO_PAD.encode(self.signing_key.sign(&message).to_bytes())
    }
}

/// Mensaje canónico idéntico al de Platform; cualquier cambio exige versión.
pub fn canonical_proof_message(
    device_code: &str,
    installation_id: InstallationId,
    public_key: &str,
    nonce: &str,
) -> Vec<u8> {
    format!(
        "{PROOF_PREFIX}\nrequest={device_code}\ninstallation={installation_id}\nkey={public_key}\nnonce={nonce}"
    )
    .into_bytes()
}

/// Decodifica una clave persistida de 32 bytes en base64 estándar.
pub fn seed_from_base64(value: &str) -> Result<[u8; 32], DeviceIdentityError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| DeviceIdentityError::InvalidKey)?;
    bytes
        .try_into()
        .map_err(|_| DeviceIdentityError::InvalidKey)
}

/// Codifica la semilla para el almacén local.
pub fn seed_to_base64(seed: &[u8; 32]) -> String {
    base64::engine::general_purpose::STANDARD.encode(seed)
}

impl pos_application::DeviceSigner for DeviceIdentity {
    fn public_key(&self) -> String {
        self.public_key_encoded()
    }

    fn sign_proof(
        &self,
        device_code: &str,
        installation_id: InstallationId,
        nonce: &str,
    ) -> pos_application::AppResult<String> {
        Ok(DeviceIdentity::sign_proof(
            self,
            device_code,
            installation_id,
            nonce,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    #[test]
    fn publica_el_formato_que_platform_acepta() {
        let identity = DeviceIdentity::generate();
        let encoded = identity.public_key_encoded();
        let raw = encoded.strip_prefix("ed25519:v1:").unwrap();
        let bytes = URL_SAFE_NO_PAD.decode(raw).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn firma_el_mensaje_canonico_y_verifica_con_la_publica() {
        let identity = DeviceIdentity::generate();
        let installation = InstallationId::new();
        let signature = identity.sign_proof("device-code", installation, "nonce");
        let message = canonical_proof_message(
            "device-code",
            installation,
            &identity.public_key_encoded(),
            "nonce",
        );
        let raw = URL_SAFE_NO_PAD.decode(signature).unwrap();
        let signature = Signature::from_slice(&raw).unwrap();
        let public = URL_SAFE_NO_PAD
            .decode(
                identity
                    .public_key_encoded()
                    .strip_prefix("ed25519:v1:")
                    .unwrap(),
            )
            .unwrap();
        let verifying = VerifyingKey::from_bytes(public.as_slice().try_into().unwrap()).unwrap();
        verifying.verify(&message, &signature).unwrap();
    }

    #[test]
    fn la_semilla_se_reconstruye_y_mantiene_la_clave_publica() {
        let identity = DeviceIdentity::generate();
        let restored = DeviceIdentity::from_seed_bytes(
            seed_from_base64(&seed_to_base64(&identity.seed_bytes())).unwrap(),
        );
        assert_eq!(identity.public_key_encoded(), restored.public_key_encoded());
    }
}
