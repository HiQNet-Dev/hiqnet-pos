//! Casos de uso, puertos y autorización de HiQNet POS.
//!
//! La capa de aplicación orquesta el dominio y las transacciones, pero no
//! conoce SQL, Tauri ni React.

pub mod error;
pub mod ports;
pub mod service;

pub use error::{AppError, AppResult, ErrorView};
pub use ports::{
    Clock, InstallationState, LicenseVerifier, PasswordHasher, SharedClock, SharedHasher,
    SharedStore, Store, StoreTx, SystemClock,
};
pub use service::{
    AppStatus, AuditEventView, BootstrapRequest, BootstrapResponse, CategoryDraft, CategoryView,
    CommercialStateView, InventoryMovementDraft, InventoryMovementView, LoginRequest, PosService,
    ProductDraft, ProductVariantDraft, ProductVariantView, ProductView,
};
