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

    #[error("el producto no existe")]
    ProductNotFound,

    #[error("el recurso no existe")]
    NotFound,

    #[error("el producto está inactivo")]
    InactiveProduct,

    #[error("la categoría no existe")]
    CategoryNotFound,

    #[error("la categoría está inactiva")]
    InactiveCategory,

    #[error("el nombre es obligatorio")]
    EmptyName,

    #[error("el identificador de negocio (SKU) no es válido: {0}")]
    InvalidSku(String),

    #[error("el impuesto debe estar entre 0 y 10000 puntos base")]
    InvalidTaxRate,

    #[error("límites de modificador inválidos")]
    InvalidModifierLimits,

    #[error("regla de modificador no cumplida")]
    ModifierRuleViolation,

    #[error("la opción está duplicada")]
    DuplicateOption,

    #[error("la opción de modificador está inactiva")]
    InactiveOption,

    #[error("el delta de precio no es válido")]
    InvalidPriceDelta,

    #[error("la contraseña debe tener al menos {min} caracteres")]
    WeakPassword { min: usize },

    #[error("el usuario ya existe")]
    DuplicateUsername,

    #[error("debe quedar al menos un propietario activo")]
    LastActiveOwner,

    #[error("el rol indicado no existe")]
    UnknownRole,

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

    #[error("stock insuficiente para registrar la salida")]
    InsufficientStock,

    #[error("el efectivo recibido es insuficiente")]
    InsufficientPayment,

    #[error("el efectivo disponible en el turno es insuficiente")]
    InsufficientCash,

    #[error("la versión de autorización comercial no es compatible")]
    UnsupportedLicenseVersion,
}

pub type DomainResult<T> = Result<T, DomainError>;
