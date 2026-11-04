use pos_core::DomainError;
use serde::Serialize;

/// Error de aplicación. El mensaje es apto para el usuario final en español; la
/// variante permite a los adaptadores traducir a códigos IPC estables.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("no hay una sesión autenticada")]
    Unauthenticated,

    #[error("no se encontró el recurso solicitado")]
    NotFound,

    #[error("el recurso ya existe y no puede duplicarse")]
    Conflict,

    #[error("no fue posible completar la operación de almacenamiento")]
    Storage,

    #[error("no fue posible procesar la credencial")]
    Crypto,
}

impl AppError {
    /// Código estable para contratos IPC/HTTP.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Domain(DomainError::AccessDenied { .. }) => "access_denied",
            AppError::Domain(DomainError::InvalidCredentials) => "invalid_credentials",
            AppError::Domain(DomainError::AlreadyInitialized) => "already_initialized",
            AppError::Domain(DomainError::FeatureNotEntitled { .. }) => "feature_not_entitled",
            AppError::Domain(DomainError::CommercialStateBlocked) => "commercial_state_blocked",
            AppError::Domain(DomainError::DuplicateUsername) => "duplicate_username",
            AppError::Domain(DomainError::LastActiveOwner) => "last_active_owner",
            AppError::Domain(_) => "validation_error",
            AppError::Unauthenticated => "unauthenticated",
            AppError::NotFound => "not_found",
            AppError::Conflict => "conflict",
            AppError::Storage => "storage_error",
            AppError::Crypto => "crypto_error",
        }
    }

    pub fn is_storage(&self) -> bool {
        matches!(self, AppError::Storage)
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// DTO serializable del error expuesto por IPC.
#[derive(Debug, Serialize)]
pub struct ErrorView {
    pub code: &'static str,
    pub message: String,
}

impl From<&AppError> for ErrorView {
    fn from(error: &AppError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}
