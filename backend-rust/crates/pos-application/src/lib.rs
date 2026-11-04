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
    AppStatus, AuditEventView, BootstrapRequest, BootstrapResponse, CashCloseShiftDraft,
    CashMovementDraft, CashMovementView, CashOpenShiftDraft, CashRegisterDraft, CashRegisterView,
    CashShiftView, CategoryDraft, CategoryView, CheckoutDraft, CommercialStateView,
    InventoryMovementDraft, InventoryMovementView, LoginRequest, ModifierConfigurationDraft,
    ModifierConfigurationView, ModifierGroupDraft, ModifierGroupView, ModifierOptionDraft,
    ModifierOptionView, PosService, ProductDraft, ProductVariantDraft, ProductVariantView,
    ProductView, SaleItemDraft, SaleItemView, SalePaymentView, SaleView,
};
