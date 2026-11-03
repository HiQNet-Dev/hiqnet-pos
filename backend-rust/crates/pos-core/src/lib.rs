//! Dominio puro de HiQNet POS.
//!
//! Este crate no conoce SQLx, HTTP, Tauri ni configuración de infraestructura.
//! Contiene entidades, value objects, invariantes y políticas puras.

pub mod audit;
pub mod catalog;
pub mod entitlements;
pub mod error;
pub mod identity;
pub mod ids;
pub mod money;
pub mod organization;
pub mod quantity;

pub use error::{DomainError, DomainResult};

/// Versión de producto del POS. El greenfield inicia en `0.1.0` (SemVer), sin
/// reciclar la secuencia histórica del repositorio anterior.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
