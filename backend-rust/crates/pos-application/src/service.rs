use crate::error::{AppError, AppResult};
use crate::ports::{InstallationState, SharedClock, SharedHasher, SharedStore};
use pos_core::audit::{AuditAction, AuditEvent};
use pos_core::cash::{CashMovement, CashMovementKind, CashRegister, CashShift, CashShiftStatus};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::entitlements::{
    CommercialState, FeatureKey, LicenseStatus, PlanName, local_features,
};
use pos_core::identity::{Actor, Role, User, normalize_username};
use pos_core::ids::{
    BranchId, CashRegisterId, CashShiftId, InstallationId, OrganizationId, ProductId, UserId,
};
use pos_core::inventory::{InventoryMovement, MovementKind};
use pos_core::modifiers::{ModifierConfiguration, ModifierGroup, ModifierOption, OptionKind};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::quantity::Quantity;
use pos_core::sales::{Sale, SaleItem, SalePayment, SaleStatus, validate_payment};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    #[serde(with = "time::serde::rfc3339")]
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
pub struct ModifierGroupDraft {
    pub product_id: String,
    pub name: String,
    pub min: u32,
    pub max: u32,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierGroupView {
    pub id: String,
    pub name: String,
    pub min: u32,
    pub max: u32,
    pub sort_order: i32,
    pub active: bool,
}

impl From<&ModifierGroup> for ModifierGroupView {
    fn from(group: &ModifierGroup) -> Self {
        Self {
            id: group.id.to_string(),
            name: group.name.clone(),
            min: group.min,
            max: group.max,
            sort_order: group.sort_order,
            active: group.active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierOptionDraft {
    pub group_id: String,
    pub name: String,
    pub kind: String,
    pub price_delta: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierOptionView {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub kind: String,
    pub price_delta: String,
    pub sort_order: i32,
    pub active: bool,
}

impl From<&ModifierOption> for ModifierOptionView {
    fn from(option: &ModifierOption) -> Self {
        Self {
            id: option.id.to_string(),
            group_id: option.group_id.to_string(),
            name: option.name.clone(),
            kind: option.kind.as_str().to_owned(),
            price_delta: option.price_delta.to_string(),
            sort_order: option.sort_order,
            active: option.active,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierSelectionView {
    pub selected_option_id: String,
    pub name: String,
    pub kind: String,
    pub price_delta: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifierConfigurationView {
    pub product_id: String,
    pub product_name: String,
    pub variant_id: Option<String>,
    pub variant_name: Option<String>,
    pub base_price: String,
    pub modifiers_total: String,
    pub tax: String,
    pub unit_price: String,
    pub tax_rate_bp: u16,
    pub selected_options: Vec<ModifierSelectionView>,
}

impl From<ModifierConfiguration> for ModifierConfigurationView {
    fn from(configuration: ModifierConfiguration) -> Self {
        Self {
            product_id: configuration.product_id.to_string(),
            product_name: configuration.product_name,
            variant_id: configuration.variant_id.map(|id| id.to_string()),
            variant_name: configuration.variant_name,
            base_price: Money::from_minor_units(configuration.base_price_minor)
                .unwrap()
                .to_string(),
            modifiers_total: Money::from_minor_units(configuration.modifiers_total_minor)
                .unwrap()
                .to_string(),
            tax: Money::from_minor_units(configuration.tax_minor)
                .unwrap()
                .to_string(),
            unit_price: Money::from_minor_units(configuration.total_minor)
                .unwrap()
                .to_string(),
            tax_rate_bp: configuration.tax_rate_bp,
            selected_options: configuration
                .selected_options
                .into_iter()
                .map(|option| ModifierSelectionView {
                    selected_option_id: option.id.to_string(),
                    name: option.name,
                    kind: option.kind.as_str().to_owned(),
                    price_delta: option.price_delta.to_string(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierConfigurationDraft {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub selected_option_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashRegisterDraft {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockLevelView {
    pub product_id: String,
    pub quantity: String,
}

/// Identidad legible de la sucursal del actor autenticado.
#[derive(Debug, Clone, Serialize)]
pub struct BranchView {
    pub id: String,
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CashRegisterView {
    pub id: String,
    pub name: String,
    pub code: String,
    pub active: bool,
}

impl From<&CashRegister> for CashRegisterView {
    fn from(register: &CashRegister) -> Self {
        Self {
            id: register.id.to_string(),
            name: register.name.clone(),
            code: register.code.clone(),
            active: register.active,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CashMovementView {
    pub id: String,
    pub kind: String,
    pub amount: String,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: time::OffsetDateTime,
}

impl From<&CashMovement> for CashMovementView {
    fn from(movement: &CashMovement) -> Self {
        Self {
            id: movement.id.to_string(),
            kind: movement.kind.as_str().to_owned(),
            amount: movement.amount.to_string(),
            reason: movement.reason.clone(),
            occurred_at: movement.occurred_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CashShiftView {
    pub id: String,
    pub register_id: String,
    pub status: String,
    pub opened_by: String,
    #[serde(with = "time::serde::rfc3339")]
    pub opened_at: time::OffsetDateTime,
    pub closed_by: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<time::OffsetDateTime>,
    pub opening_amount: String,
    pub expected_amount: String,
    pub entry_total: String,
    pub exit_total: String,
    pub counted_amount: Option<String>,
    pub difference_minor: Option<i64>,
    pub movements: Vec<CashMovementView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashMovementDraft {
    pub shift_id: String,
    pub kind: String,
    pub amount: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashOpenShiftDraft {
    pub register_id: String,
    pub opening_amount: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashCloseShiftDraft {
    pub shift_id: String,
    pub counted_amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleItemDraft {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub quantity: String,
    #[serde(default)]
    pub selected_option_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutDraft {
    pub shift_id: String,
    pub idempotency_key: String,
    pub items: Vec<SaleItemDraft>,
    pub cash_received: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleItemView {
    pub id: String,
    pub product_id: String,
    pub product_name: String,
    pub variant_name: Option<String>,
    pub quantity: String,
    pub unit_price: String,
    pub unit_tax: String,
    pub line_total: String,
    pub modifiers_summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SalePaymentView {
    pub method: String,
    pub amount: String,
    pub change: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleView {
    pub id: String,
    pub folio: String,
    pub status: String,
    pub subtotal: String,
    pub tax: String,
    pub total: String,
    pub items: Vec<SaleItemView>,
    pub payment: Option<SalePaymentView>,
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
    #[serde(with = "time::serde::rfc3339")]
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
    #[serde(with = "time::serde::rfc3339")]
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
    #[serde(with = "time::serde::rfc3339")]
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

    pub async fn create_modifier_group(
        &self,
        actor: &Actor,
        draft: ModifierGroupDraft,
    ) -> AppResult<ModifierGroupView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let product_id: pos_core::ids::ProductId =
            draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;
        tx.find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        let group = ModifierGroup::new(
            actor.organization_id,
            product_id,
            draft.name,
            draft.min,
            draft.max,
            draft.sort_order,
        )?;
        tx.insert_modifier_group(&group).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ModifierGroupCreated,
            "modifier_group",
            group.id.to_string(),
            serde_json::json!({ "product_id": product_id.to_string(), "name": group.name }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ModifierGroupView::from(&group))
    }

    pub async fn list_modifier_groups(
        &self,
        actor: &Actor,
        product_id: pos_core::ids::ProductId,
    ) -> AppResult<Vec<ModifierGroupView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let groups = tx
            .list_modifier_groups(actor.organization_id, product_id)
            .await?;
        tx.rollback().await?;
        Ok(groups.iter().map(ModifierGroupView::from).collect())
    }

    pub async fn create_modifier_option(
        &self,
        actor: &Actor,
        draft: ModifierOptionDraft,
    ) -> AppResult<ModifierOptionView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let group_id: pos_core::ids::ModifierGroupId =
            draft.group_id.parse().map_err(|_| AppError::NotFound)?;
        let kind = OptionKind::from_key(&draft.kind).ok_or(AppError::NotFound)?;
        let price_delta: Money = draft.price_delta.parse()?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;
        let group = tx
            .find_modifier_group(actor.organization_id, group_id)
            .await?
            .ok_or(pos_core::DomainError::CategoryNotFound)?;
        // product validity ensured when the group was created; re-check organization only.
        let _ = group.product_id;
        let option = ModifierOption::new(
            actor.organization_id,
            group_id,
            draft.name,
            kind,
            price_delta,
            draft.sort_order,
        )?;
        tx.insert_modifier_option(&option).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ModifierOptionCreated,
            "modifier_option",
            option.id.to_string(),
            serde_json::json!({ "group_id": group_id.to_string(), "name": option.name }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ModifierOptionView::from(&option))
    }

    pub async fn list_modifier_options(
        &self,
        actor: &Actor,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Vec<ModifierOptionView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let options = tx
            .list_modifier_options(actor.organization_id, group_id)
            .await?;
        tx.rollback().await?;
        Ok(options.iter().map(ModifierOptionView::from).collect())
    }

    pub async fn configure_product(
        &self,
        actor: &Actor,
        draft: ModifierConfigurationDraft,
    ) -> AppResult<ModifierConfigurationView> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let product_id: pos_core::ids::ProductId =
            draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let variant_id = draft
            .variant_id
            .as_deref()
            .map(str::parse::<pos_core::ids::ProductVariantId>)
            .transpose()
            .map_err(|_| AppError::NotFound)?;
        let selected_ids: Vec<pos_core::ids::ModifierOptionId> = draft
            .selected_option_ids
            .iter()
            .map(|value| value.parse())
            .collect::<Result<_, _>>()
            .map_err(|_| AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        let variant = if let Some(variant_id) = variant_id {
            Some(
                tx.list_product_variants(actor.organization_id, product_id)
                    .await?
                    .into_iter()
                    .find(|value| value.id == variant_id)
                    .ok_or(AppError::NotFound)?,
            )
        } else {
            None
        };
        let groups = tx
            .list_modifier_groups(actor.organization_id, product_id)
            .await?;
        let mut options = Vec::new();
        for group in &groups {
            options.extend(
                tx.list_modifier_options(actor.organization_id, group.id)
                    .await?,
            );
        }
        tx.rollback().await?;

        let configuration = pos_core::modifiers::configure(
            &product,
            variant.as_ref(),
            &groups,
            &options,
            &selected_ids,
        )?;
        Ok(ModifierConfigurationView::from(configuration))
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

    /// Existencias de todos los productos de la sucursal del actor. Permite
    /// mostrar disponibilidad en la pantalla de venta sin N consultas.
    pub async fn list_stock_levels(&self, actor: &Actor) -> AppResult<Vec<StockLevelView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let levels = tx.stock_levels(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(levels
            .into_iter()
            .map(|(product_id, quantity)| StockLevelView {
                product_id: product_id.to_string(),
                quantity: quantity.to_string(),
            })
            .collect())
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

    pub async fn create_cash_register(
        &self,
        actor: &Actor,
        draft: CashRegisterDraft,
    ) -> AppResult<CashRegisterView> {
        actor.require(pos_core::identity::Permission::ManageCashRegisters)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        let register = CashRegister::new(actor.organization_id, branch_id, draft.name, draft.code)?;
        tx.insert_cash_register(&register).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashRegisterCreated,
            "cash_register",
            register.id.to_string(),
            serde_json::json!({ "name": register.name, "code": register.code }),
        ))
        .await?;
        tx.commit().await?;
        Ok(CashRegisterView::from(&register))
    }

    /// Devuelve nombre y código de la sucursal del actor. Solo lectura y sin
    /// datos de otras sucursales u organizaciones.
    pub async fn branch_summary(&self, actor: &Actor) -> AppResult<BranchView> {
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let branch = tx
            .find_branch(actor.organization_id, branch_id)
            .await?
            .ok_or(AppError::NotFound)?;
        tx.rollback().await?;
        Ok(BranchView {
            id: branch.id.to_string(),
            name: branch.name,
            code: branch.code,
        })
    }

    pub async fn list_cash_registers(&self, actor: &Actor) -> AppResult<Vec<CashRegisterView>> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let registers = tx
            .list_cash_registers(actor.organization_id, branch_id)
            .await?;
        tx.rollback().await?;
        Ok(registers.iter().map(CashRegisterView::from).collect())
    }

    pub async fn open_cash_shift(
        &self,
        actor: &Actor,
        draft: CashOpenShiftDraft,
    ) -> AppResult<CashShiftView> {
        actor.require(pos_core::identity::Permission::OpenCashShift)?;
        let register_id: CashRegisterId =
            draft.register_id.parse().map_err(|_| AppError::NotFound)?;
        let opening: Money = draft.opening_amount.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        let registers = tx
            .list_cash_registers(actor.organization_id, branch_id)
            .await?;
        let register = registers
            .iter()
            .find(|item| item.id == register_id && item.active)
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if tx
            .find_open_cash_shift(actor.organization_id, register_id)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict);
        }
        let shift = CashShift::new(
            actor.organization_id,
            branch_id,
            register.id,
            actor.user_id,
            opening,
        );
        tx.insert_cash_shift(&shift).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashShiftOpened,
            "cash_shift",
            shift.id.to_string(),
            serde_json::json!({ "register_id": register_id.to_string(), "opening_amount": opening.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        let expected = shift.opening_amount;
        self.shift_view(actor, shift, Vec::new(), expected, Money::ZERO, 0)
    }

    pub async fn active_cash_shift(
        &self,
        actor: &Actor,
        register_id: CashRegisterId,
    ) -> AppResult<Option<CashShiftView>> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let mut tx = self.store.begin().await?;
        let Some(shift) = tx
            .find_open_cash_shift(actor.organization_id, register_id)
            .await?
        else {
            tx.rollback().await?;
            return Ok(None);
        };
        if shift.branch_id != actor.branch_id.ok_or(AppError::NotFound)? {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.view",
            }
            .into());
        }
        let movements = tx
            .list_cash_movements(actor.organization_id, shift.id)
            .await?;
        let expected = expected_amount(&shift, &movements)?;
        tx.rollback().await?;
        Ok(Some(self.shift_view(
            actor,
            shift,
            movements,
            expected,
            Money::ZERO,
            0,
        )?))
    }

    pub async fn register_cash_movement(
        &self,
        actor: &Actor,
        draft: CashMovementDraft,
    ) -> AppResult<CashMovementView> {
        actor.require(pos_core::identity::Permission::RecordCashMovement)?;
        let shift_id: CashShiftId = draft.shift_id.parse().map_err(|_| AppError::NotFound)?;
        let kind = CashMovementKind::from_key(&draft.kind).ok_or(AppError::NotFound)?;
        let amount: Money = draft.amount.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let open_shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if open_shift.status != CashShiftStatus::Open || open_shift.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        if kind == CashMovementKind::Out {
            let movements = tx
                .list_cash_movements(actor.organization_id, open_shift.id)
                .await?;
            let expected = expected_amount(&open_shift, &movements)?;
            if expected.minor_units() < amount.minor_units() {
                return Err(AppError::Conflict);
            }
        }
        let movement = CashMovement::new(
            actor.organization_id,
            branch_id,
            open_shift.register_id,
            open_shift.id,
            kind,
            amount,
            draft.reason,
            Some(actor.user_id),
        )?;
        tx.insert_cash_movement(&movement).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashMovementRegistered,
            "cash_movement",
            movement.id.to_string(),
            serde_json::json!({ "shift_id": shift_id.to_string(), "kind": kind.as_str(), "amount": amount.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(CashMovementView::from(&movement))
    }

    pub async fn close_cash_shift(
        &self,
        actor: &Actor,
        draft: CashCloseShiftDraft,
    ) -> AppResult<CashShiftView> {
        actor.require(pos_core::identity::Permission::CloseCashShift)?;
        let shift_id: CashShiftId = draft.shift_id.parse().map_err(|_| AppError::NotFound)?;
        let counted: Money = draft.counted_amount.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let open_shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if open_shift.status != CashShiftStatus::Open || open_shift.branch_id != branch_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.view",
            }
            .into());
        }
        if actor.role != Role::Owner && open_shift.opened_by != actor.user_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.close",
            }
            .into());
        }
        let movements = tx
            .list_cash_movements(actor.organization_id, open_shift.id)
            .await?;
        let (entry_total, exit_total) = cash_totals(&movements)?;
        let expected = open_shift
            .opening_amount
            .checked_add(entry_total)?
            .checked_sub(exit_total)?;
        let difference_minor = counted.minor_units() - expected.minor_units();
        tx.close_cash_shift(
            actor.organization_id,
            open_shift.id,
            actor.user_id,
            OffsetDateTime::now_utc(),
            expected,
            counted,
            difference_minor,
        )
        .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashShiftClosed,
            "cash_shift",
            open_shift.id.to_string(),
            serde_json::json!({ "counted": counted.to_string(), "expected": expected.to_string(), "difference_minor": difference_minor }),
        ))
        .await?;
        tx.commit().await?;
        let expected_string = expected.to_string();
        let counted_string = counted.to_string();
        Ok(CashShiftView {
            id: open_shift.id.to_string(),
            register_id: open_shift.register_id.to_string(),
            status: "closed".into(),
            opened_by: open_shift.opened_by.to_string(),
            opened_at: open_shift.opened_at,
            closed_by: Some(actor.user_id.to_string()),
            closed_at: Some(OffsetDateTime::now_utc()),
            opening_amount: open_shift.opening_amount.to_string(),
            expected_amount: expected_string,
            entry_total: entry_total.to_string(),
            exit_total: exit_total.to_string(),
            counted_amount: Some(counted_string),
            difference_minor: Some(difference_minor),
            movements: movements.iter().map(CashMovementView::from).collect(),
        })
    }

    pub async fn cash_shift_summary(
        &self,
        actor: &Actor,
        shift_id: CashShiftId,
    ) -> AppResult<CashShiftView> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let mut tx = self.store.begin().await?;
        let shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if shift.branch_id != actor.branch_id.ok_or(AppError::NotFound)? {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.view",
            }
            .into());
        }
        let movements = tx
            .list_cash_movements(actor.organization_id, shift.id)
            .await?;
        tx.rollback().await?;
        let expected = if shift.status == CashShiftStatus::Open {
            expected_amount(&shift, &movements)?
        } else {
            shift.expected_amount.ok_or(AppError::NotFound)?
        };
        let counted = shift.counted_amount.unwrap_or(Money::ZERO);
        let diff = shift.difference_minor.unwrap_or(0);
        let view = self.shift_view(actor, shift, movements, expected, counted, diff)?;
        Ok(view)
    }

    pub async fn list_cash_shift_history(&self, actor: &Actor) -> AppResult<Vec<CashShiftView>> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let closed = tx
            .list_closed_cash_shifts(actor.organization_id, branch_id)
            .await?;
        let mut views = Vec::new();
        for shift in closed {
            let movements = tx
                .list_cash_movements(actor.organization_id, shift.id)
                .await?;
            let expected = shift.expected_amount.unwrap_or(shift.opening_amount);
            let counted = shift.counted_amount.unwrap_or(Money::ZERO);
            let diff = shift.difference_minor.unwrap_or(0);
            views.push(self.shift_view(actor, shift, movements, expected, counted, diff)?);
        }
        tx.rollback().await?;
        Ok(views)
    }

    fn shift_view(
        &self,
        _actor: &Actor,
        shift: CashShift,
        movements: Vec<CashMovement>,
        expected: Money,
        counted: Money,
        difference_minor: i64,
    ) -> AppResult<CashShiftView> {
        let (entry_total, exit_total) = cash_totals(&movements)?;
        Ok(CashShiftView {
            id: shift.id.to_string(),
            register_id: shift.register_id.to_string(),
            status: shift.status.as_str().to_owned(),
            opened_by: shift.opened_by.to_string(),
            opened_at: shift.opened_at,
            closed_by: shift.closed_by.map(|id| id.to_string()),
            closed_at: shift.closed_at,
            opening_amount: shift.opening_amount.to_string(),
            expected_amount: expected.to_string(),
            entry_total: entry_total.to_string(),
            exit_total: exit_total.to_string(),
            counted_amount: shift.counted_amount.map(|m| m.to_string()).or_else(|| {
                if shift.status == CashShiftStatus::Closed {
                    Some(counted.to_string())
                } else {
                    None
                }
            }),
            difference_minor: shift.difference_minor.or(Some(difference_minor)),
            movements: movements.iter().map(CashMovementView::from).collect(),
        })
    }

    pub async fn checkout_sale(&self, actor: &Actor, draft: CheckoutDraft) -> AppResult<SaleView> {
        actor.require(pos_core::identity::Permission::Sell)?;
        if draft.items.is_empty() {
            return Err(pos_core::DomainError::InvalidQuantityScale.into());
        }
        let shift_id: CashShiftId = draft.shift_id.parse().map_err(|_| AppError::NotFound)?;
        let cash_received: Money = draft.cash_received.parse()?;
        let fingerprint = serde_json::to_string(&draft).map_err(|_| AppError::Storage)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        // La recuperación idempotente se resuelve antes de exigir un turno abierto:
        // un reintento de una venta ya confirmada debe funcionar aunque el turno se
        // haya cerrado después.
        if let Some(existing) = tx
            .find_sale_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if existing.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            if existing.request_fingerprint != fingerprint {
                return Err(AppError::Conflict);
            }
            let items = tx
                .list_sale_items(actor.organization_id, existing.id)
                .await?;
            let payment = tx
                .list_sale_payments(actor.organization_id, existing.id)
                .await?
                .into_iter()
                .next();
            tx.rollback().await?;
            return self.sale_view(existing, items, payment);
        }

        let shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if shift.status != CashShiftStatus::Open || shift.branch_id != branch_id {
            return Err(AppError::NotFound);
        }

        let sale_id = pos_core::ids::SaleId::new();
        let folio = format!(
            "V-{}",
            sale_id
                .to_string()
                .replace('-', "")
                .chars()
                .take(10)
                .collect::<String>()
                .to_uppercase()
        );
        let mut subtotal = Money::ZERO;
        let mut tax_total = Money::ZERO;
        let mut sale_items = Vec::new();
        let mut stock_needed: BTreeMap<pos_core::ids::ProductId, Quantity> = BTreeMap::new();

        for draft_item in &draft.items {
            let product_id: pos_core::ids::ProductId = draft_item
                .product_id
                .parse()
                .map_err(|_| AppError::NotFound)?;
            let variant_id = draft_item
                .variant_id
                .as_deref()
                .map(str::parse::<pos_core::ids::ProductVariantId>)
                .transpose()
                .map_err(|_| AppError::NotFound)?;
            let quantity: Quantity = draft_item.quantity.parse()?;
            if quantity.is_zero() {
                return Err(pos_core::DomainError::NegativeQuantity.into());
            }
            let product = tx
                .find_product(actor.organization_id, product_id)
                .await?
                .ok_or(pos_core::DomainError::ProductNotFound)?;
            if !product.active {
                return Err(pos_core::DomainError::InactiveProduct.into());
            }
            let variant = if let Some(variant_id) = variant_id {
                Some(
                    tx.list_product_variants(actor.organization_id, product_id)
                        .await?
                        .into_iter()
                        .find(|item| item.id == variant_id)
                        .ok_or(AppError::NotFound)?,
                )
            } else {
                None
            };
            let groups = tx
                .list_modifier_groups(actor.organization_id, product_id)
                .await?;
            let mut options = Vec::new();
            for group in &groups {
                options.extend(
                    tx.list_modifier_options(actor.organization_id, group.id)
                        .await?,
                );
            }
            let selected_ids = draft_item
                .selected_option_ids
                .iter()
                .map(|value| value.parse())
                .collect::<Result<Vec<pos_core::ids::ModifierOptionId>, _>>()
                .map_err(|_| AppError::NotFound)?;
            let configuration = pos_core::modifiers::configure(
                &product,
                variant.as_ref(),
                &groups,
                &options,
                &selected_ids,
            )?;

            let unit_net = Money::from_minor_units(configuration.base_price_minor)
                .map_err(AppError::Domain)?
                .checked_add(
                    Money::from_minor_units(configuration.modifiers_total_minor)
                        .map_err(AppError::Domain)?,
                )?;
            let unit_tax =
                Money::from_minor_units(configuration.tax_minor).map_err(AppError::Domain)?;
            let line_net = unit_net.multiply_by(quantity)?;
            let line_tax = unit_tax.multiply_by(quantity)?;
            let line_total = line_net.checked_add(line_tax)?;
            subtotal = subtotal.checked_add(line_net)?;
            tax_total = tax_total.checked_add(line_tax)?;
            let required = stock_needed.entry(product_id).or_insert(Quantity::ZERO);
            *required = required.checked_add(quantity)?;
            let modifiers_summary = configuration
                .selected_options
                .iter()
                .map(|option| option.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            sale_items.push(SaleItem::new(
                sale_id,
                product_id,
                variant.as_ref().map(|item| item.id),
                product.name.clone(),
                variant.as_ref().map(|item| item.name.clone()),
                quantity,
                unit_net,
                unit_tax,
                line_total,
                modifiers_summary,
            ));
        }

        let total = subtotal.checked_add(tax_total)?;
        let change = validate_payment(total, cash_received)?;
        for (product_id, quantity) in &stock_needed {
            let available = tx
                .current_stock(actor.organization_id, branch_id, *product_id)
                .await?;
            if available.milli_units() < quantity.milli_units() {
                return Err(pos_core::DomainError::InsufficientStock.into());
            }
        }

        let occurred_at = OffsetDateTime::now_utc();
        let sale = Sale {
            id: sale_id,
            organization_id: actor.organization_id,
            branch_id,
            register_id: shift.register_id,
            shift_id: shift.id,
            operator_id: actor.user_id,
            folio: folio.clone(),
            occurred_at,
            status: SaleStatus::Completed,
            subtotal,
            tax: tax_total,
            total,
            idempotency_key: draft.idempotency_key.clone(),
            request_fingerprint: fingerprint,
        };
        tx.insert_sale(&sale).await?;
        for item in &sale_items {
            tx.insert_sale_item(item).await?;
        }
        let payment = SalePayment {
            id: pos_core::ids::SalePaymentId::new(),
            sale_id,
            method: "cash".into(),
            amount: cash_received,
            change_given: change,
            occurred_at,
        };
        tx.insert_sale_payment(&payment).await?;

        for (product_id, quantity) in &stock_needed {
            let movement = InventoryMovement::new(
                actor.organization_id,
                branch_id,
                *product_id,
                MovementKind::Outbound,
                *quantity,
                Some(format!("venta {folio}")),
                Some(actor.user_id),
            )?;
            tx.insert_inventory_movement(&movement).await?;
        }
        let cash_movement = CashMovement::new(
            actor.organization_id,
            branch_id,
            shift.register_id,
            shift.id,
            CashMovementKind::In,
            total,
            format!("venta {folio}"),
            Some(actor.user_id),
        )?;
        tx.insert_cash_movement(&cash_movement).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::SaleCompleted,
            "sale",
            sale_id.to_string(),
            serde_json::json!({ "folio": folio, "total": total.to_string(), "shift_id": shift_id.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        self.sale_view(sale, sale_items, Some(payment))
    }

    pub async fn get_sale(
        &self,
        actor: &Actor,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<SaleView> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let sale = tx
            .find_sale(actor.organization_id, sale_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if sale.branch_id != actor.branch_id.ok_or(AppError::NotFound)? {
            return Err(AppError::NotFound);
        }
        let items = tx.list_sale_items(actor.organization_id, sale_id).await?;
        let payment = tx
            .list_sale_payments(actor.organization_id, sale_id)
            .await?
            .into_iter()
            .next();
        tx.rollback().await?;
        self.sale_view(sale, items, payment)
    }

    fn sale_view(
        &self,
        sale: Sale,
        items: Vec<SaleItem>,
        payment: Option<SalePayment>,
    ) -> AppResult<SaleView> {
        Ok(SaleView {
            id: sale.id.to_string(),
            folio: sale.folio,
            status: sale.status.as_str().to_owned(),
            subtotal: sale.subtotal.to_string(),
            tax: sale.tax.to_string(),
            total: sale.total.to_string(),
            items: items
                .into_iter()
                .map(|item| SaleItemView {
                    id: item.id.to_string(),
                    product_id: item.product_id.to_string(),
                    product_name: item.product_name,
                    variant_name: item.variant_name,
                    quantity: item.quantity.to_string(),
                    unit_price: item.unit_price.to_string(),
                    unit_tax: item.unit_tax.to_string(),
                    line_total: item.line_total.to_string(),
                    modifiers_summary: item.modifiers_summary,
                })
                .collect(),
            payment: payment.map(|payment| SalePaymentView {
                method: payment.method,
                amount: payment.amount.to_string(),
                change: payment.change_given.to_string(),
            }),
        })
    }

    pub async fn commercial_state(&self) -> AppResult<CommercialStateView> {
        let mut tx = self.store.begin().await?;
        let state = tx.installation_state().await?.ok_or(AppError::NotFound)?;
        tx.rollback().await?;
        commercial_view(&state, self.clock.now())
    }
}

fn cash_totals(movements: &[CashMovement]) -> pos_core::DomainResult<(Money, Money)> {
    let mut entries = Money::ZERO;
    let mut exits = Money::ZERO;
    for movement in movements {
        match movement.kind {
            CashMovementKind::In => entries = entries.checked_add(movement.amount)?,
            CashMovementKind::Out => exits = exits.checked_add(movement.amount)?,
        }
    }
    Ok((entries, exits))
}

fn expected_amount(shift: &CashShift, movements: &[CashMovement]) -> pos_core::DomainResult<Money> {
    let (entries, exits) = cash_totals(movements)?;
    shift
        .opening_amount
        .checked_add(entries)?
        .checked_sub(exits)
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

    #[test]
    fn las_fechas_del_contrato_ipc_son_texto_rfc3339() {
        let movement = CashMovementView {
            id: "m-1".into(),
            kind: "in".into(),
            amount: "1.00".into(),
            reason: "prueba".into(),
            occurred_at: OffsetDateTime::now_utc(),
        };
        let value = serde_json::to_value(movement).unwrap();
        assert!(
            value["occurred_at"].is_string(),
            "el contrato IPC debe serializar fechas como texto: {value}"
        );
    }
}
