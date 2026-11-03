/// Errores de dominio. El texto es apto para mostrarse al usuario en español;
/// los consumidores nunca deben ramificar por el mensaje, sino por la variante.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("el importe no puede ser negativo")]
    NegativeAmount,

    #[error("el importe excede el máximo admitido")]
    AmountOverflow,

    #[error("el importe tiene más de dos decimales")]
    InvalidMoneyScale,

    #[error("la cantidad no puede ser negativa")]
    NegativeQuantity,

    #[error("la cantidad excede el máximo admitido")]
    QuantityOverflow,

    #[error("la cantidad tiene más de tres decimales")]
    InvalidQuantityScale,

    #[error("el nombre es obligatorio")]
    EmptyName,

    #[error("el identificador de negocio (SKU) no es válido: {0}")]
    InvalidSku(String),

    #[error("el impuesto debe estar entre 0 y 10000 puntos base")]
    InvalidTaxRate,

    #[error("la contraseña debe tener al menos {min} caracteres")]
    WeakPassword { min: usize },

    #[error("el usuario ya existe")]
    DuplicateUsername,

    #[error("acceso denegado: falta el permiso {required}")]
    AccessDenied { required: &'static str },

    #[error("la capacidad comercial '{feature}' no está habilitada para el plan actual")]
    FeatureNotEntitled { feature: &'static str },

    #[error("la instalación ya fue inicializada")]
    AlreadyInitialized,

    #[error("el usuario o la contraseña no son válidos")]
    InvalidCredentials,

    #[error("el estado comercial no permite esta operación")]
    CommercialStateBlocked,

    #[error("la versión de autorización comercial no es compatible")]
    UnsupportedLicenseVersion,
}

pub type DomainResult<T> = Result<T, DomainError>;
