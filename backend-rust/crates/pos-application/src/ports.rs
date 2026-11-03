use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use pos_core::audit::AuditEvent;
use pos_core::catalog::Product;
use pos_core::identity::User;
use pos_core::ids::{BranchId, InstallationId, OrganizationId, ProductId};
use pos_core::inventory::InventoryMovement;
use pos_core::organization::{Branch, Organization};
use serde::Serialize;
use time::OffsetDateTime;

/// Estado persistido de la instalación local y su licencia comercial.
#[derive(Debug, Clone, Serialize)]
pub struct InstallationState {
    pub installation_id: InstallationId,
    pub organization_id: OrganizationId,
    pub trial_started_at: OffsetDateTime,
    pub plan: String,
    pub license_status: String,
    pub features: Vec<String>,
    pub valid_until: OffsetDateTime,
    pub license_grant_json: Option<String>,
    pub license_signature_b64: Option<String>,
}

/// Puertos de persistencia. La implementación transaccional vive en
/// `pos-storage-sqlite`; la capa de aplicación solo conoce operaciones.
#[async_trait]
pub trait Store: Send + Sync {
    async fn begin(&self) -> AppResult<Box<dyn StoreTx>>;
}

#[async_trait]
pub trait StoreTx: Send {
    async fn installation_state(&mut self) -> AppResult<Option<InstallationState>>;
    async fn save_installation_state(&mut self, state: &InstallationState) -> AppResult<()>;

    async fn insert_organization(&mut self, organization: &Organization) -> AppResult<()>;
    async fn insert_branch(&mut self, branch: &Branch) -> AppResult<()>;

    async fn insert_user(&mut self, user: &User) -> AppResult<()>;
    async fn find_user_by_username(&mut self, username: &str) -> AppResult<Option<User>>;
    async fn count_users(&mut self, organization_id: OrganizationId) -> AppResult<i64>;

    async fn insert_product(&mut self, product: &Product) -> AppResult<()>;
    async fn find_product(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Option<Product>>;
    async fn list_products(&mut self, organization_id: OrganizationId) -> AppResult<Vec<Product>>;

    async fn insert_inventory_movement(&mut self, movement: &InventoryMovement) -> AppResult<()>;
    async fn current_stock(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: ProductId,
    ) -> AppResult<pos_core::quantity::Quantity>;
    async fn list_inventory_movements(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: Option<ProductId>,
        limit: i64,
    ) -> AppResult<Vec<InventoryMovement>>;

    async fn append_audit(&mut self, event: &AuditEvent) -> AppResult<()>;
    async fn list_audit(
        &mut self,
        organization_id: OrganizationId,
        limit: i64,
    ) -> AppResult<Vec<AuditEvent>>;

    async fn commit(self: Box<Self>) -> AppResult<()>;
    async fn rollback(self: Box<Self>) -> AppResult<()>;
}

/// Hash de contraseñas. La implementación Argon2id vive en `pos-security`.
pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: &str) -> AppResult<String>;
    fn verify(&self, password: &str, password_hash: &str) -> AppResult<bool>;
}

/// Reloj inyectable para pruebas deterministas de licencia y auditoría.
pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// Verificador de firmas de licencia. En esta fundación solo se exige la
/// interfaz; la emisión productiva pertenece a HiQNet Platform (pendiente).
pub trait LicenseVerifier: Send + Sync {
    fn verify(&self, grant_json: &str, signature_b64: &str) -> AppResult<()>;
}

pub type SharedStore = std::sync::Arc<dyn Store>;
pub type SharedHasher = std::sync::Arc<dyn PasswordHasher>;
pub type SharedClock = std::sync::Arc<dyn Clock>;

/// Utilidad para los adaptadores: convierte fallos inesperados sin filtrar
/// detalles internos hacia la UI.
pub fn storage_error(_detail: impl std::fmt::Display) -> AppError {
    AppError::Storage
}
