use crate::error::{AppError, AppResult};
use crate::ports::{InstallationState, SharedClock, SharedHasher, SharedStore};
use pos_core::audit::{AuditAction, AuditEvent};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::entitlements::{
    CommercialState, FeatureKey, LicenseStatus, PlanName, local_features,
};
use pos_core::identity::{Actor, Role, User, normalize_username};
use pos_core::ids::{BranchId, InstallationId, OrganizationId, ProductId, UserId};
use pos_core::inventory::{InventoryMovement, MovementKind};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::quantity::Quantity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;

pub const MIN_PASSWORD_LEN: usize = 10;
const MAX_AUDIT_PAGE: i64 = 200;

#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapRequest {
    pub organization_name: String,
    pub branch_name: String,
    pub branch_code: String,
    pub owner_username: String,
    pub owner_display_name: String,
    pub owner_password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapResponse {
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub user_id: UserId,
    pub trial_ends_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductDraft {
    pub sku: String,
    pub name: String,
    /// Precio como texto decimal ("12.50") para no perder exactitud en JSON.
    pub price: String,
    pub tax_rate_bp: u16,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProductView {
    pub id: ProductId,
    pub sku: String,
    pub name: String,
    pub unit_price: String,
    pub tax_rate_bp: u16,
    pub active: bool,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryDraft {
    pub name: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryView {
    pub id: String,
    pub name: String,
    pub sort_order: i32,
    pub active: bool,
}

impl From<&Category> for CategoryView {
    fn from(category: &Category) -> Self {
        Self {
            id: category.id.to_string(),
            name: category.name.clone(),
            sort_order: category.sort_order,
            active: category.active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductVariantDraft {
    pub product_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub price: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductVariantView {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub unit_price: String,
    pub active: bool,
    pub sort_order: i32,
}

impl From<&ProductVariant> for ProductVariantView {
    fn from(variant: &ProductVariant) -> Self {
        Self {
            id: variant.id.to_string(),
            product_id: variant.product_id.to_string(),
            name: variant.name.clone(),
            sku: variant.sku.clone(),
            unit_price: variant.unit_price.to_string(),
            active: variant.active,
            sort_order: variant.sort_order,
        }
    }
}

impl From<&Product> for ProductView {
    fn from(product: &Product) -> Self {
        Self {
            id: product.id,
            sku: product.sku.clone(),
            name: product.name.clone(),
            unit_price: product.unit_price.to_string(),
            tax_rate_bp: product.tax_rate_bp,
            active: product.active,
            category_id: product.category_id.map(|id| id.to_string()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InventoryMovementDraft {
    pub product_id: String,
    pub kind: String,
    pub quantity: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryMovementView {
    pub id: String,
    pub product_id: String,
    pub kind: String,
    pub quantity: String,
    pub reason: Option<String>,
    pub occurred_at: OffsetDateTime,
}

impl From<&InventoryMovement> for InventoryMovementView {
    fn from(movement: &InventoryMovement) -> Self {
        Self {
            id: movement.id.to_string(),
            product_id: movement.product_id.to_string(),
            kind: movement.kind.as_str().to_owned(),
            quantity: movement.quantity.to_string(),
            reason: movement.reason.clone(),
            occurred_at: movement.occurred_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEventView {
    pub id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub actor_user_id: Option<String>,
    pub occurred_at: OffsetDateTime,
}

impl From<&AuditEvent> for AuditEventView {
    fn from(event: &AuditEvent) -> Self {
        Self {
            id: event.id.to_string(),
            action: event.action.as_str().to_owned(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            actor_user_id: event.actor_user_id.map(|id| id.to_string()),
            occurred_at: event.occurred_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CommercialStateView {
    pub plan: &'static str,
    pub status: &'static str,
    pub valid_until: OffsetDateTime,
    pub features: Vec<&'static str>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub initialized: bool,
    pub product_version: &'static str,
    pub commercial: Option<CommercialStateView>,
    pub user_count: i64,
}

/// Fachada de casos de uso del POS. No conoce SQL, Tauri ni React.
pub struct PosService {
    store: SharedStore,
    hasher: SharedHasher,
    clock: SharedClock,
}

impl PosService {
    pub fn new(store: SharedStore, hasher: SharedHasher, clock: SharedClock) -> Self {
        Self {
            store,
            hasher,
            clock,
        }
    }

    pub async fn status(&self) -> AppResult<AppStatus> {
        let mut tx = self.store.begin().await?;
        let state = tx.installation_state().await?;
        let user_count = match &state {
            Some(state) => tx.count_users(state.organization_id).await?,
            None => 0,
        };
        tx.rollback().await?;

        let commercial = match state {
            Some(state) => Some(commercial_view(&state, self.clock.now())?),
            None => None,
        };

        Ok(AppStatus {
            initialized: commercial.is_some(),
            product_version: pos_core::PRODUCT_VERSION,
            commercial,
            user_count,
        })
    }

    /// Inicializa organización, sucursal, propietario y Trial de 30 días en una
    /// sola transacción (mutación + auditoría + commit).
    pub async fn bootstrap(&self, request: BootstrapRequest) -> AppResult<BootstrapResponse> {
        validate_password(&request.owner_password)?;

        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if tx.installation_state().await?.is_some() {
            return Err(pos_core::DomainError::AlreadyInitialized.into());
        }

        let organization = Organization::new(OrganizationId::new(), request.organization_name)?;
        let branch = Branch::new(
            BranchId::new(),
            organization.id,
            request.branch_name,
            request.branch_code,
        )?;

        let password_hash = self.hasher.hash(&request.owner_password)?;
        let user = User::new(
            UserId::new(),
            organization.id,
            Some(branch.id),
            request.owner_username,
            request.owner_display_name,
            Role::Owner,
            password_hash,
        )?;

        tx.insert_organization(&organization).await?;
        tx.insert_branch(&branch).await?;
        tx.insert_user(&user).await?;

        let trial = CommercialState::new_trial(InstallationId::new(), organization.id, now);
        let state = installation_from_commercial(&trial);
        tx.save_installation_state(&state).await?;

        tx.append_audit(&AuditEvent::new(
            organization.id,
            Some(branch.id),
            Some(user.id),
            AuditAction::InstallationBootstrapped,
            "organization",
            organization.id.to_string(),
            serde_json::json!({ "branch_code": branch.code }),
        ))
        .await?;

        tx.commit().await?;

        Ok(BootstrapResponse {
            organization_id: organization.id,
            branch_id: branch.id,
            user_id: user.id,
            trial_ends_at: trial.valid_until,
        })
    }

    pub async fn login(&self, request: LoginRequest) -> AppResult<Actor> {
        let username = normalize_username(&request.username)?;
        let mut tx = self.store.begin().await?;
        let user = match tx.find_user_by_username(&username).await? {
            Some(user) if user.active => user,
            _ => return Err(pos_core::DomainError::InvalidCredentials.into()),
        };

        if !self.hasher.verify(&request.password, &user.password_hash)? {
            return Err(pos_core::DomainError::InvalidCredentials.into());
        }

        tx.append_audit(&AuditEvent::new(
            user.organization_id,
            user.branch_id,
            Some(user.id),
            AuditAction::SessionLoggedIn,
            "user",
            user.id.to_string(),
            serde_json::json!({ "role": user.role.to_string() }),
        ))
        .await?;
        tx.commit().await?;

        Ok(Actor::from_user(&user))
    }

    pub async fn create_product(
        &self,
        actor: &Actor,
        draft: ProductDraft,
    ) -> AppResult<ProductView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let price: Money = draft.price.parse()?;
        let category_id = draft
            .category_id
            .as_deref()
            .map(str::parse::<pos_core::ids::CategoryId>)
            .transpose()
            .map_err(|_| pos_core::DomainError::CategoryNotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        if let Some(category_id) = category_id {
            let category = tx
                .find_category(actor.organization_id, category_id)
                .await?
                .ok_or(pos_core::DomainError::CategoryNotFound)?;
            if !category.active {
                return Err(pos_core::DomainError::InactiveCategory.into());
            }
        }

        let mut product = Product::new(
            ProductId::new(),
            actor.organization_id,
            draft.sku,
            draft.name,
            price,
            draft.tax_rate_bp,
        )?;
        product.category_id = category_id;

        tx.insert_product(&product).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ProductCreated,
            "product",
            product.id.to_string(),
            serde_json::json!({ "sku": product.sku, "price": product.unit_price.to_string() }),
        ))
        .await?;
        tx.commit().await?;

        Ok(ProductView::from(&product))
    }

    pub async fn assign_product_category(
        &self,
        actor: &Actor,
        product_id: ProductId,
        category_id: Option<String>,
    ) -> AppResult<ProductView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let category_id = category_id
            .as_deref()
            .map(str::parse::<pos_core::ids::CategoryId>)
            .transpose()
            .map_err(|_| pos_core::DomainError::CategoryNotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        tx.find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if let Some(category_id) = category_id {
            let category = tx
                .find_category(actor.organization_id, category_id)
                .await?
                .ok_or(pos_core::DomainError::CategoryNotFound)?;
            if !category.active {
                return Err(pos_core::DomainError::InactiveCategory.into());
            }
        }
        tx.update_product_category(actor.organization_id, product_id, category_id)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ProductCategoryChanged,
            "product",
            product_id.to_string(),
            serde_json::json!({ "category_id": category_id.map(|id| id.to_string()) }),
        ))
        .await?;
        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        tx.commit().await?;
        Ok(ProductView::from(&product))
    }

    pub async fn list_products(&self, actor: &Actor) -> AppResult<Vec<ProductView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let products = tx.list_products(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(products.iter().map(ProductView::from).collect())
    }

    pub async fn create_category(
        &self,
        actor: &Actor,
        draft: CategoryDraft,
    ) -> AppResult<CategoryView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        let category = Category::new(actor.organization_id, draft.name, draft.sort_order)?;
        tx.insert_category(&category).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::CategoryCreated,
            "category",
            category.id.to_string(),
            serde_json::json!({ "name": category.name, "sort_order": category.sort_order }),
        ))
        .await?;
        tx.commit().await?;
        Ok(CategoryView::from(&category))
    }

    pub async fn list_categories(&self, actor: &Actor) -> AppResult<Vec<CategoryView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let categories = tx.list_categories(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(categories.iter().map(CategoryView::from).collect())
    }

    pub async fn create_product_variant(
        &self,
        actor: &Actor,
        draft: ProductVariantDraft,
    ) -> AppResult<ProductVariantView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let product_id: ProductId = draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let price: Money = draft.price.parse()?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        tx.find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;

        let variant = ProductVariant::new(
            actor.organization_id,
            product_id,
            draft.name,
            draft.sku,
            price,
            draft.sort_order,
        )?;
        tx.insert_product_variant(&variant).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ProductVariantCreated,
            "product_variant",
            variant.id.to_string(),
            serde_json::json!({ "product_id": product_id.to_string(), "name": variant.name, "price": variant.unit_price.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ProductVariantView::from(&variant))
    }

    pub async fn list_product_variants(
        &self,
        actor: &Actor,
        product_id: ProductId,
    ) -> AppResult<Vec<ProductVariantView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let variants = tx
            .list_product_variants(actor.organization_id, product_id)
            .await?;
        tx.rollback().await?;
        Ok(variants.iter().map(ProductVariantView::from).collect())
    }

    pub async fn record_inventory_movement(
        &self,
        actor: &Actor,
        draft: InventoryMovementDraft,
    ) -> AppResult<InventoryMovementView> {
        actor.require(pos_core::identity::Permission::ManageInventory)?;
        let product_id: ProductId = draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let kind = MovementKind::from_key(&draft.kind).ok_or(AppError::NotFound)?;
        let quantity: Quantity = draft.quantity.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Inventory, self.clock.now())?;

        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if !product.active {
            return Err(pos_core::DomainError::InactiveProduct.into());
        }

        if matches!(kind, MovementKind::Outbound | MovementKind::AdjustmentOut) {
            let current = tx
                .current_stock(actor.organization_id, branch_id, product_id)
                .await?;
            if current.milli_units() < quantity.milli_units() {
                return Err(pos_core::DomainError::InsufficientStock.into());
            }
        }

        let movement = InventoryMovement::new(
            actor.organization_id,
            branch_id,
            product_id,
            kind,
            quantity,
            draft.reason,
            Some(actor.user_id),
        )?;

        tx.insert_inventory_movement(&movement).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::InventoryMovementRegistered,
            "inventory_movement",
            movement.id.to_string(),
            serde_json::json!({ "product_id": product_id.to_string(), "kind": kind.as_str(), "quantity": quantity.to_string() }),
        ))
        .await?;
        tx.commit().await?;

        Ok(InventoryMovementView::from(&movement))
    }

    pub async fn current_stock(
        &self,
        actor: &Actor,
        product_id: ProductId,
        branch_id: Option<BranchId>,
    ) -> AppResult<String> {
        actor.require(pos_core::identity::Permission::ViewInventory)?;
        if let Some(requested_branch) = branch_id
            && Some(requested_branch) != actor.branch_id
        {
            return Err(pos_core::DomainError::AccessDenied {
                required: "inventory.view",
            }
            .into());
        }
        let branch_id = branch_id.or(actor.branch_id).ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let stock = tx
            .current_stock(actor.organization_id, branch_id, product_id)
            .await?;
        tx.rollback().await?;
        Ok(stock.to_string())
    }

    pub async fn list_inventory_movements(
        &self,
        actor: &Actor,
        product_id: Option<ProductId>,
        limit: u32,
    ) -> AppResult<Vec<InventoryMovementView>> {
        actor.require(pos_core::identity::Permission::ViewInventory)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let limit = i64::from(limit).clamp(1, MAX_AUDIT_PAGE);
        let mut tx = self.store.begin().await?;
        let movements = tx
            .list_inventory_movements(actor.organization_id, branch_id, product_id, limit)
            .await?;
        tx.rollback().await?;
        Ok(movements.iter().map(InventoryMovementView::from).collect())
    }

    pub async fn list_audit(&self, actor: &Actor, limit: u32) -> AppResult<Vec<AuditEventView>> {
        actor.require(pos_core::identity::Permission::ViewAudit)?;
        let limit = i64::from(limit).clamp(1, MAX_AUDIT_PAGE);
        let mut tx = self.store.begin().await?;
        let events = tx.list_audit(actor.organization_id, limit).await?;
        tx.rollback().await?;
        Ok(events.iter().map(AuditEventView::from).collect())
    }

    /// Registra un respaldo exitoso. El respaldo es una operación permitida
    /// incluso con licencia vencida (lectura, exportación y respaldo).
    pub async fn record_backup(&self, actor: &Actor, backup_path: &str) -> AppResult<()> {
        actor.require(pos_core::identity::Permission::ManageBackups)?;
        let mut tx = self.store.begin().await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::BackupCreated,
            "backup",
            backup_path.to_owned(),
            serde_json::json!({}),
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn commercial_state(&self) -> AppResult<CommercialStateView> {
        let mut tx = self.store.begin().await?;
        let state = tx.installation_state().await?.ok_or(AppError::NotFound)?;
        tx.rollback().await?;
        commercial_view(&state, self.clock.now())
    }
}

fn validate_password(password: &str) -> AppResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(pos_core::DomainError::WeakPassword {
            min: MIN_PASSWORD_LEN,
        }
        .into());
    }
    if password.trim().is_empty() {
        return Err(pos_core::DomainError::WeakPassword {
            min: MIN_PASSWORD_LEN,
        }
        .into());
    }
    Ok(())
}

async fn require_commercial(tx: &mut dyn crate::ports::StoreTx) -> AppResult<CommercialState> {
    let state = tx
        .installation_state()
        .await?
        .ok_or(pos_core::DomainError::AlreadyInitialized)?;
    commercial_from_installation(&state)
}

fn commercial_from_installation(state: &InstallationState) -> AppResult<CommercialState> {
    let plan = match state.plan.as_str() {
        "trial" => PlanName::Trial,
        "local" => PlanName::Local,
        "pro" => PlanName::Pro,
        "enterprise" => PlanName::Enterprise,
        _ => PlanName::Trial,
    };
    let status = match state.license_status.as_str() {
        "active" => LicenseStatus::Active,
        "grace" => LicenseStatus::Grace,
        "read_only" => LicenseStatus::ReadOnly,
        "expired" => LicenseStatus::Expired,
        _ => LicenseStatus::Trial,
    };
    let features: BTreeSet<FeatureKey> = state
        .features
        .iter()
        .filter_map(|key| FeatureKey::from_key(key))
        .collect();

    // La verificación de la firma del grant pertenece a `pos-licensing`; este
    // estado refleja únicamente lo observado por la aplicación.
    Ok(CommercialState {
        installation_id: state.installation_id,
        organization_id: state.organization_id,
        plan,
        status,
        features,
        valid_until: state.valid_until,
    })
}

fn commercial_view(
    state: &InstallationState,
    now: OffsetDateTime,
) -> AppResult<CommercialStateView> {
    let commercial = commercial_from_installation(state)?;
    let effective = commercial.effective_status(now);
    Ok(CommercialStateView {
        plan: commercial.plan.as_str(),
        status: match effective {
            LicenseStatus::Trial => "trial",
            LicenseStatus::Active => "active",
            LicenseStatus::Grace => "grace",
            LicenseStatus::ReadOnly => "read_only",
            LicenseStatus::Expired => "expired",
        },
        valid_until: commercial.valid_until,
        features: commercial
            .features
            .iter()
            .map(|feature| feature.as_str())
            .collect(),
        read_only: matches!(effective, LicenseStatus::ReadOnly | LicenseStatus::Expired),
    })
}

fn installation_from_commercial(commercial: &CommercialState) -> InstallationState {
    InstallationState {
        installation_id: commercial.installation_id,
        organization_id: commercial.organization_id,
        trial_started_at: commercial.valid_until - time::Duration::days(30),
        plan: commercial.plan.as_str().to_owned(),
        license_status: LicenseStatus::Trial.as_str().to_owned(),
        features: local_features()
            .iter()
            .map(|feature| feature.as_str().to_owned())
            .collect(),
        valid_until: commercial.valid_until,
        license_grant_json: None,
        license_signature_b64: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn politica_de_contrasena_exige_longitud_minima() {
        assert!(validate_password("123456789").is_err());
        assert!(validate_password("1234567890").is_ok());
    }

    #[test]
    fn vista_comercial_marca_solo_lectura_al_vencer() {
        let start = OffsetDateTime::now_utc();
        let commercial =
            CommercialState::new_trial(InstallationId::new(), OrganizationId::new(), start);
        let state = installation_from_commercial(&commercial);
        let fresh = commercial_view(&state, start).unwrap();
        assert!(!fresh.read_only);
        assert_eq!(fresh.status, "trial");

        let expired = commercial_view(&state, start + time::Duration::days(31)).unwrap();
        assert!(expired.read_only);
        assert_eq!(expired.status, "read_only");
    }
}
