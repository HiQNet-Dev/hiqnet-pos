//! Hashing de credenciales locales con Argon2id.
//!
//! Los parámetros son deliberadamente explícitos y viven en un solo lugar para
//! poder ratificarlos sin tocar los casos de uso.

use argon2::PasswordHasher as _;
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordVerifier, SaltString},
};
use pos_application::{AppError, AppResult, PasswordHasher};
use rand_core::OsRng;

/// Parámetros base recomendados por OWASP (19 MiB, 2 iteraciones, 1 hilo).
/// Siguen pendientes de ratificación formal; ver ADR de credenciales.
pub const MEMORY_KIB: u32 = 19 * 1024;
pub const ITERATIONS: u32 = 2;
pub const PARALLELISM: u32 = 1;

#[derive(Debug, Clone)]
pub struct Argon2idHasher {
    argon: Argon2<'static>,
}

impl Argon2idHasher {
    pub fn new() -> Self {
        let params = Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, None)
            .expect("parámetros Argon2id válidos");
        Self {
            argon: Argon2::new(Algorithm::Argon2id, Version::V0x13, params),
        }
    }
}

impl Default for Argon2idHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordHasher for Argon2idHasher {
    fn hash(&self, password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        self.argon
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| AppError::Crypto)
    }

    fn verify(&self, password: &str, password_hash: &str) -> AppResult<bool> {
        let parsed = PasswordHash::new(password_hash).map_err(|_| AppError::Crypto)?;
        Ok(self
            .argon
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_y_verificacion_son_compatibles() {
        let hasher = Argon2idHasher::new();
        let hash = hasher.hash("contrasena-segura-123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(hash.contains("m=19456,t=2,p=1"));
        assert!(hasher.verify("contrasena-segura-123", &hash).unwrap());
        assert!(!hasher.verify("otra-contrasena", &hash).unwrap());
    }

    #[test]
    fn hash_invalido_es_error_de_criptografia() {
        let hasher = Argon2idHasher::new();
        assert!(matches!(
            hasher.verify("x", "no-es-un-hash"),
            Err(AppError::Crypto)
        ));
    }
}
