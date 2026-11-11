//! Contrato de licencias locales y verificación de firmas Ed25519.
//!
//! El POS solo recibe materiales públicos. La clave privada de emisión vive en
//! HiQNet Platform y no se implementa aquí. La emisión productiva y los plazos
//! de gracia siguen pendientes (D-04, D-09).

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use pos_application::AppError;
use pos_core::entitlements::{FeatureKey, LicenseStatus, PlanName};
use pos_core::ids::{InstallationId, OrganizationId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Versión del documento de autorización comercial. Incrementar exige una
/// migración explícita del verificador.
pub const LICENSE_DOC_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum LicenseError {
    #[error("la firma de la licencia no es válida")]
    InvalidSignature,

    #[error("la clave pública de licencias no es válida")]
    InvalidPublicKey,

    #[error("el documento de licencia no es válido")]
    InvalidDocument,

    #[error("la versión de licencia no es compatible")]
    UnsupportedVersion,
}

/// Derechos comerciales firmados por HiQNet Platform.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LicenseGrant {
    pub version: u32,
    pub grant_id: String,
    pub installation_id: InstallationId,
    pub organization_id: OrganizationId,
    pub plan: PlanName,
    pub features: Vec<FeatureKey>,
    pub issued_at: OffsetDateTime,
    pub valid_until: OffsetDateTime,
}

impl LicenseGrant {
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, LicenseError> {
        // La forma canónica es la serialización JSON del contrato con orden de
        // campos fijo por el struct. Cualquier cambio de campos exige versionar.
        serde_json::to_vec(self).map_err(|_| LicenseError::InvalidDocument)
    }

    pub fn from_canonical_json(bytes: &[u8]) -> Result<Self, LicenseError> {
        let grant: LicenseGrant =
            serde_json::from_slice(bytes).map_err(|_| LicenseError::InvalidDocument)?;
        if grant.version != LICENSE_DOC_VERSION {
            return Err(LicenseError::UnsupportedVersion);
        }
        Ok(grant)
    }

    pub fn status_at(&self, now: OffsetDateTime) -> LicenseStatus {
        if now > self.valid_until {
            LicenseStatus::ReadOnly
        } else if self.plan == PlanName::Trial {
            LicenseStatus::Trial
        } else {
            LicenseStatus::Active
        }
    }
}

/// Verificador con clave pública embebida en el binario. Nunca contiene la
/// clave privada de emisión.
#[derive(Debug, Clone)]
pub struct Ed25519LicenseVerifier {
    key: VerifyingKey,
}

impl Ed25519LicenseVerifier {
    pub fn from_public_key_base64(value: &str) -> Result<Self, LicenseError> {
        let bytes = B64
            .decode(value.trim())
            .map_err(|_| LicenseError::InvalidPublicKey)?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| LicenseError::InvalidPublicKey)?;
        let key = VerifyingKey::from_bytes(&array).map_err(|_| LicenseError::InvalidPublicKey)?;
        Ok(Self { key })
    }

    pub fn verify_grant(
        &self,
        grant_json: &str,
        signature_b64: &str,
    ) -> Result<LicenseGrant, LicenseError> {
        let signature_bytes = B64
            .decode(signature_b64.trim())
            .map_err(|_| LicenseError::InvalidSignature)?;
        let signature =
            Signature::from_slice(&signature_bytes).map_err(|_| LicenseError::InvalidSignature)?;

        self.key
            .verify(grant_json.as_bytes(), &signature)
            .map_err(|_| LicenseError::InvalidSignature)?;

        LicenseGrant::from_canonical_json(grant_json.as_bytes())
    }
}

impl pos_application::LicenseVerifier for Ed25519LicenseVerifier {
    /// Verifica la firma y devuelve el contenido del documento ya validado.
    ///
    /// El error hacia capas superiores es siempre `Crypto`: no distingue entre
    /// firma incorrecta, documento malformado o version no soportada.
    fn verify(
        &self,
        grant_json: &str,
        signature_b64: &str,
    ) -> pos_application::AppResult<pos_application::VerifiedLicense> {
        let grant = match self.verify_grant(grant_json, signature_b64) {
            Ok(grant) => grant,
            Err(error) => {
                tracing::warn!(error = %error, "licencia rechazada");
                return Err(AppError::Crypto);
            }
        };
        Ok(pos_application::VerifiedLicense {
            installation_id: grant.installation_id,
            organization_id: grant.organization_id,
            plan: grant.plan,
            features: grant.features.into_iter().collect(),
            issued_at: grant.issued_at,
            valid_until: grant.valid_until,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn grant(valid_until: OffsetDateTime) -> LicenseGrant {
        LicenseGrant {
            version: LICENSE_DOC_VERSION,
            grant_id: "grant-dev-001".into(),
            installation_id: InstallationId::new(),
            organization_id: OrganizationId::new(),
            plan: PlanName::Trial,
            features: vec![FeatureKey::Catalog, FeatureKey::Sales],
            issued_at: OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap(),
            valid_until,
        }
    }

    fn signer() -> SigningKey {
        // Clave exclusivamente de pruebas; nunca se usa en instaladores.
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn acepta_firma_valida() {
        let grant = grant(OffsetDateTime::now_utc() + time::Duration::days(30));
        let json = String::from_utf8(grant.to_canonical_json().unwrap()).unwrap();
        let signature = B64.encode(signer().sign(json.as_bytes()).to_bytes());
        let verifier = Ed25519LicenseVerifier::from_public_key_base64(
            &B64.encode(signer().verifying_key().to_bytes()),
        )
        .unwrap();

        assert_eq!(verifier.verify_grant(&json, &signature).unwrap(), grant);
    }

    #[test]
    fn rechaza_payload_manipulado() {
        let grant = grant(OffsetDateTime::now_utc() + time::Duration::days(30));
        let mut json = String::from_utf8(grant.to_canonical_json().unwrap()).unwrap();
        let signature = B64.encode(signer().sign(json.as_bytes()).to_bytes());
        json = json.replace("\"trial\"", "\"enterprise\"");

        let verifier = Ed25519LicenseVerifier::from_public_key_base64(
            &B64.encode(signer().verifying_key().to_bytes()),
        )
        .unwrap();
        assert!(matches!(
            verifier.verify_grant(&json, &signature),
            Err(LicenseError::InvalidSignature)
        ));
    }

    #[test]
    fn rechaza_firma_de_otra_clave() {
        let grant = grant(OffsetDateTime::now_utc() + time::Duration::days(30));
        let json = String::from_utf8(grant.to_canonical_json().unwrap()).unwrap();
        let other = SigningKey::from_bytes(&[9u8; 32]);
        let signature = B64.encode(other.sign(json.as_bytes()).to_bytes());

        let verifier = Ed25519LicenseVerifier::from_public_key_base64(
            &B64.encode(signer().verifying_key().to_bytes()),
        )
        .unwrap();
        assert!(matches!(
            verifier.verify_grant(&json, &signature),
            Err(LicenseError::InvalidSignature)
        ));
    }

    #[test]
    fn vencimiento_pasa_a_solo_lectura() {
        let now = OffsetDateTime::now_utc();
        assert_eq!(
            grant(now + time::Duration::days(1)).status_at(now),
            LicenseStatus::Trial
        );
        assert_eq!(
            grant(now - time::Duration::days(1)).status_at(now),
            LicenseStatus::ReadOnly
        );
    }

    /// El JSON canónico de Platform serializa las fechas de `time` en su
    /// formato por defecto (array año/ordinal); RFC 3339 no es el formato del
    /// contrato v1. Ambas implementaciones comparten el mismo behaviour; si
    /// Platform lo cambia, el verificador debe migrarse con versión nueva.
    #[test]
    fn el_formato_canonico_de_fechas_es_el_de_time() {
        let with_array = r#"{"version":1,"grant_id":"c0ffee00-0000-4000-8000-000000000001","installation_id":"0f9e8d7c-0000-4000-8000-0000000000bb","organization_id":"0f9e8d7c-0000-4000-8000-0000000000aa","plan":"pro","features":["catalog"],"issued_at":[2026,1,0,0,0,0,0,0,0],"valid_until":[2099,1,0,0,0,0,0,0,0]}"#;
        let parsed: LicenseGrant = serde_json::from_str(with_array).unwrap();
        assert_eq!(
            parsed.valid_until,
            OffsetDateTime::from_unix_timestamp(4_070_908_800).unwrap()
        );

        let with_rfc3339 = r#"{"version":1,"grant_id":"c0ffee00-0000-4000-8000-000000000001","installation_id":"0f9e8d7c-0000-4000-8000-0000000000bb","organization_id":"0f9e8d7c-0000-4000-8000-0000000000aa","plan":"pro","features":["catalog"],"issued_at":"2026-01-01T00:00:00Z","valid_until":"2099-01-01T00:00:00Z"}"#;
        assert!(serde_json::from_str::<LicenseGrant>(with_rfc3339).is_err());
    }
}
