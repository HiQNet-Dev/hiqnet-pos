use crate::error::{AppError, AppResult};
use crate::ports::{InstallationState, SharedClock, SharedHasher, SharedStore, StoreTx};
use pos_core::audit::{AuditAction, AuditEvent};
use pos_core::cash::{CashMovement, CashMovementKind, CashRegister, CashShift, CashShiftStatus};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::entitlements::{
    CommercialState, FeatureKey, LicenseStatus, PlanName, local_features,
};
use pos_core::expenses::{Expense, ExpenseCategory, WasteRecord};
use pos_core::identity::{Actor, Role, User, normalize_username};
use pos_core::ids::{
    BranchId, CashRegisterId, CashShiftId, ExpenseCategoryId, ExpenseId, InstallationId,
    OrganizationId, ProductId, PurchaseOrderId, PurchaseOrderLineId, SaleItemId, SupplierId,
    UserId, WasteId,
};
use pos_core::inventory::{InventoryMovement, MovementKind};
use pos_core::modifiers::{ModifierConfiguration, ModifierGroup, ModifierOption, OptionKind};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::purchasing::{
    PurchaseOrder, PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, Supplier,
};
use pos_core::quantity::Quantity;
use pos_core::returns::{
    RefundBasis, ReturnDisposition, SaleReturn, SaleReturnLine, refund_increment,
};
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

#[derive(Debug, Clone, Deserialize)]
pub struct UserDraft {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetUserActiveDraft {
    pub user_id: String,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResetUserPasswordDraft {
    pub user_id: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChangeOwnPasswordDraft {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupplierDraft {
    pub code: Option<String>,
    pub name: String,
    pub contact: Option<String>,
    pub rfc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupplierView {
    pub id: String,
    pub code: Option<String>,
    pub name: String,
    pub contact: Option<String>,
    pub rfc: Option<String>,
    pub active: bool,
}

impl From<&Supplier> for SupplierView {
    fn from(value: &Supplier) -> Self {
        Self {
            id: value.id.to_string(),
            code: value.code.clone(),
            name: value.name.clone(),
            contact: value.contact.clone(),
            rfc: value.rfc.clone(),
            active: value.active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseOrderLineDraft {
    pub product_id: String,
    pub quantity: String,
    pub unit_cost: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseOrderDraft {
    pub supplier_id: String,
    pub lines: Vec<PurchaseOrderLineDraft>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseReceiptLineDraft {
    pub order_line_id: String,
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseReceiptDraft {
    pub order_id: String,
    pub idempotency_key: String,
    pub lines: Vec<PurchaseReceiptLineDraft>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseOrderLineView {
    pub id: String,
    pub product_id: String,
    pub quantity: String,
    pub received_quantity: String,
    pub unit_cost: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseOrderView {
    pub id: String,
    pub supplier_id: String,
    pub folio: String,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub lines: Vec<PurchaseOrderLineView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserView {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl UserView {
    fn from_user(user: &User) -> Self {
        Self {
            id: user.id.to_string(),
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            role: user.role.to_string(),
            active: user.active,
            created_at: user.created_at,
        }
    }
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

#[derive(Debug, Clone, Deserialize)]
pub struct ExpenseCategoryDraft {
    pub name: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ExpenseDraft {
    pub category_id: String,
    pub amount: String,
    pub concept: String,
    pub paid_from_cash: bool,
    pub shift_id: Option<String>,
    pub idempotency_key: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct WasteDraft {
    pub product_id: String,
    pub quantity: String,
    pub reason: String,
    pub idempotency_key: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ExpenseCategoryView {
    pub id: String,
    pub name: String,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct ExpenseView {
    pub id: String,
    pub category_id: String,
    pub amount: String,
    pub concept: String,
    pub paid_from_cash: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}
#[derive(Debug, Clone, Serialize)]
pub struct WasteView {
    pub id: String,
    pub product_id: String,
    pub quantity: String,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaleReturnLineDraft {
    pub sale_item_id: String,
    pub quantity: String,
    pub disposition: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaleReturnDraft {
    pub sale_id: String,
    /// Turno de reembolso. Puede faltar cuando la devolución no mueve efectivo.
    pub shift_id: Option<String>,
    pub reason: String,
    pub idempotency_key: String,
    pub lines: Vec<SaleReturnLineDraft>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaleReturnPreviewDraft {
    pub sale_id: String,
    pub lines: Vec<SaleReturnLineDraft>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReportsQuery {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnLineView {
    pub sale_item_id: String,
    pub product_name: String,
    pub quantity: String,
    pub disposition: String,
    pub refund_net: String,
    pub refund_tax: String,
    pub refund_total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnView {
    pub id: String,
    pub sale_id: String,
    pub sale_folio: String,
    pub reason: String,
    pub refund_total: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub lines: Vec<SaleReturnLineView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReturnableSaleItemView {
    pub id: String,
    pub product_name: String,
    pub variant_name: Option<String>,
    pub modifiers_summary: String,
    pub quantity: String,
    pub returned_quantity: String,
    pub returnable_quantity: String,
    pub unit_refund: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReturnableSaleView {
    pub id: String,
    pub folio: String,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub total: String,
    pub items: Vec<ReturnableSaleItemView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnPreviewLineView {
    pub sale_item_id: String,
    pub quantity: String,
    pub refund_total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleReturnPreviewView {
    pub refund_total: String,
    pub lines: Vec<SaleReturnPreviewLineView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsProductView {
    pub product_id: String,
    pub product_name: String,
    pub quantity: String,
    pub net: String,
    pub tax: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsSalesView {
    pub count: i64,
    pub subtotal: String,
    pub tax: String,
    pub total: String,
    pub returned_count: i64,
    pub returned_total: String,
    pub net_total: String,
    pub products: Vec<ReportsProductView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsCashView {
    pub shifts_opened: i64,
    pub shifts_closed: i64,
    pub entries: String,
    pub exits: String,
    pub expenses_from_cash: String,
    pub refunds: String,
    pub difference: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsPurchasesView {
    pub orders_created: i64,
    pub pending_orders: i64,
    pub received_quantity: String,
    pub received_cost: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsExpenseCategoryView {
    pub name: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsExpensesView {
    pub total: String,
    pub from_cash: String,
    pub external: String,
    pub categories: Vec<ReportsExpenseCategoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsInventoryView {
    pub stock_quantity: String,
    pub inbound: String,
    pub outbound: String,
    pub waste: String,
    pub adjustment_in: String,
    pub adjustment_out: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportsView {
    #[serde(with = "time::serde::rfc3339")]
    pub from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub to: OffsetDateTime,
    pub sales: ReportsSalesView,
    pub cash: ReportsCashView,
    pub purchases: ReportsPurchasesView,
    pub expenses: ReportsExpensesView,
    pub inventory: ReportsInventoryView,
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

    /// Crea un usuario local en la sucursal del actor. Requiere `users.manage`.
    pub async fn create_user(&self, actor: &Actor, draft: UserDraft) -> AppResult<UserView> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let role = Role::from_key(&draft.role).ok_or(pos_core::DomainError::UnknownRole)?;
        let username = normalize_username(&draft.username)?;
        validate_password(&draft.password)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        if tx.find_user_by_username(&username).await?.is_some() {
            return Err(pos_core::DomainError::DuplicateUsername.into());
        }

        let password_hash = self.hasher.hash(&draft.password)?;
        let user = User::new(
            UserId::new(),
            actor.organization_id,
            Some(branch_id),
            username,
            draft.display_name,
            role,
            password_hash,
        )?;
        tx.insert_user(&user).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::UserCreated,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username, "role": user.role.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(UserView::from_user(&user))
    }

    /// Empleados de la organización en la sucursal del actor.
    pub async fn list_users(&self, actor: &Actor) -> AppResult<Vec<UserView>> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let users = tx.list_users(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(users.iter().map(UserView::from_user).collect())
    }

    /// Activa o desactiva una cuenta. Nunca deja la instalación sin propietario
    /// activo; la cuenta propia puede desactivarse si permanece otro propietario activo.
    pub async fn set_user_active(
        &self,
        actor: &Actor,
        draft: SetUserActiveDraft,
    ) -> AppResult<UserView> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let user_id: UserId = draft.user_id.parse().map_err(|_| AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let mut user = tx
            .find_user_by_id(actor.organization_id, user_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if user.branch_id != Some(branch_id) {
            return Err(AppError::NotFound);
        }

        if !draft.active {
            // Defensa en profundidad: ninguna operación puede dejar la
            // instalación sin un propietario activo. Cubre también la
            // desactivación de la cuenta propia.
            if user.active && user.is_owner() {
                let restantes = tx.count_active_owners(actor.organization_id).await? - 1;
                if restantes < 1 {
                    return Err(pos_core::DomainError::LastActiveOwner.into());
                }
            }
            user.deactivate();
        } else {
            user.activate();
        }

        tx.update_user_active(actor.organization_id, user.id, user.active)
            .await?;
        let action = if user.active {
            AuditAction::UserReactivated
        } else {
            AuditAction::UserDeactivated
        };
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            action,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username, "role": user.role.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(UserView::from_user(&user))
    }

    /// Restablece la contraseña de un empleado. El hash nunca sale del backend.
    pub async fn reset_user_password(
        &self,
        actor: &Actor,
        draft: ResetUserPasswordDraft,
    ) -> AppResult<()> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        validate_password(&draft.password)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let user_id: UserId = draft.user_id.parse().map_err(|_| AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let user = tx
            .find_user_by_id(actor.organization_id, user_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if user.branch_id != Some(branch_id) {
            return Err(AppError::NotFound);
        }

        let password_hash = self.hasher.hash(&draft.password)?;
        tx.update_user_password(actor.organization_id, user.id, &password_hash)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::UserPasswordReset,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username }),
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Cambio de contraseña de la cuenta autenticada. Exige la contraseña actual.
    pub async fn change_own_password(
        &self,
        actor: &Actor,
        draft: ChangeOwnPasswordDraft,
    ) -> AppResult<()> {
        validate_password(&draft.new_password)?;
        let branch_id = actor.branch_id;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let user = tx
            .find_user_by_id(actor.organization_id, actor.user_id)
            .await?
            .filter(|user| user.active)
            .ok_or(AppError::Unauthenticated)?;
        if !self
            .hasher
            .verify(&draft.current_password, &user.password_hash)?
        {
            return Err(pos_core::DomainError::InvalidCredentials.into());
        }

        let password_hash = self.hasher.hash(&draft.new_password)?;
        tx.update_user_password(actor.organization_id, user.id, &password_hash)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            branch_id,
            Some(actor.user_id),
            AuditAction::UserPasswordChanged,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username }),
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Revalida la sesión contra la base. Una cuenta desactivada o eliminada
    /// pierde el acceso en el siguiente comando, aunque su `Actor` siga en
    /// memoria en la capa IPC.
    pub async fn refresh_actor(&self, actor: &Actor) -> AppResult<Actor> {
        let mut tx = self.store.begin().await?;
        let user = tx
            .find_user_by_id(actor.organization_id, actor.user_id)
            .await?;
        tx.rollback().await?;
        match user {
            Some(user) if user.active => Ok(Actor::from_user(&user)),
            _ => Err(AppError::Unauthenticated),
        }
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
            self.clock.now(),
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

        let occurred_at = self.clock.now();
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

    pub async fn create_supplier(
        &self,
        actor: &Actor,
        draft: SupplierDraft,
    ) -> AppResult<SupplierView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let supplier = Supplier::new(
            actor.organization_id,
            draft.code,
            draft.name,
            draft.contact,
            draft.rfc,
        )?;
        tx.insert_supplier(&supplier).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::SupplierCreated,
            "supplier",
            supplier.id.to_string(),
            serde_json::json!({ "name": supplier.name, "code": supplier.code }),
        ))
        .await?;
        tx.commit().await?;
        Ok(SupplierView::from(&supplier))
    }

    pub async fn list_suppliers(&self, actor: &Actor) -> AppResult<Vec<SupplierView>> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let mut tx = self.store.begin().await?;
        let suppliers = tx.list_suppliers(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(suppliers.iter().map(SupplierView::from).collect())
    }

    pub async fn create_purchase_order(
        &self,
        actor: &Actor,
        draft: PurchaseOrderDraft,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let supplier_id: SupplierId = draft.supplier_id.parse().map_err(|_| AppError::NotFound)?;
        if draft.lines.is_empty() {
            return Err(pos_core::DomainError::EmptyName.into());
        }
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let supplier = tx
            .find_supplier(actor.organization_id, supplier_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if !supplier.active {
            return Err(AppError::Conflict);
        }
        let order_id = PurchaseOrderId::new();
        let folio = format!(
            "OC-{}-{}",
            self.clock.now().unix_timestamp(),
            &order_id.to_string()[..8]
        );
        let order = PurchaseOrder {
            id: order_id,
            organization_id: actor.organization_id,
            branch_id,
            supplier_id,
            folio,
            status: PurchaseOrderStatus::Draft,
            created_by: actor.user_id,
            created_at: self.clock.now(),
        };
        let mut lines = Vec::with_capacity(draft.lines.len());
        let mut seen = BTreeSet::new();
        for line in draft.lines {
            let product_id: ProductId = line.product_id.parse().map_err(|_| AppError::NotFound)?;
            if !seen.insert(product_id) {
                return Err(AppError::Conflict);
            }
            let product = tx
                .find_product(actor.organization_id, product_id)
                .await?
                .ok_or(pos_core::DomainError::ProductNotFound)?;
            if !product.active {
                return Err(pos_core::DomainError::InactiveProduct.into());
            }
            let quantity: Quantity = line.quantity.parse()?;
            let unit_cost: Money = line.unit_cost.parse()?;
            lines.push(PurchaseOrderLine::new(
                order.id, product_id, quantity, unit_cost,
            )?);
        }
        tx.insert_purchase_order(&order).await?;
        for line in &lines {
            tx.insert_purchase_order_line(line).await?;
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::PurchaseOrderCreated, "purchase_order", order.id.to_string(), serde_json::json!({ "folio": order.folio, "supplier_id": supplier_id.to_string(), "lines": lines.len() }))).await?;
        tx.commit().await?;
        Ok(purchase_order_view(order, lines))
    }

    pub async fn list_purchase_orders(&self, actor: &Actor) -> AppResult<Vec<PurchaseOrderView>> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let orders = tx
            .list_purchase_orders(actor.organization_id, branch_id)
            .await?;
        let mut views = Vec::with_capacity(orders.len());
        for order in orders {
            let lines = tx
                .list_purchase_order_lines(actor.organization_id, order.id)
                .await?;
            views.push(purchase_order_view(order, lines));
        }
        tx.rollback().await?;
        Ok(views)
    }

    pub async fn get_purchase_order(
        &self,
        actor: &Actor,
        order_id: PurchaseOrderId,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let order = tx
            .find_purchase_order(actor.organization_id, order_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if order.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        let lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        tx.rollback().await?;
        Ok(purchase_order_view(order, lines))
    }

    pub async fn confirm_purchase_order(
        &self,
        actor: &Actor,
        order_id: PurchaseOrderId,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let mut order = tx
            .find_purchase_order(actor.organization_id, order_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if order.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        if order.status != PurchaseOrderStatus::Draft {
            return Err(AppError::Conflict);
        }
        tx.confirm_purchase_order(actor.organization_id, order_id)
            .await?;
        order.status = PurchaseOrderStatus::Ordered;
        let lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::PurchaseOrderConfirmed,
            "purchase_order",
            order.id.to_string(),
            serde_json::json!({ "folio": order.folio }),
        ))
        .await?;
        tx.commit().await?;
        Ok(purchase_order_view(order, lines))
    }

    pub async fn receive_purchase_order(
        &self,
        actor: &Actor,
        draft: PurchaseReceiptDraft,
    ) -> AppResult<PurchaseOrderView> {
        // Dos procesos locales pueden intentar la misma clave simultáneamente.
        // SQLite puede devolver una contención transitoria antes de que la segunda
        // transacción alcance la restricción única; se reintenta el mismo payload
        // inmutable para recuperar el resultado idempotente confirmado.
        for delay_ms in [20_u64, 50_u64] {
            match self.receive_purchase_order_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.receive_purchase_order_once(actor, draft).await
    }

    async fn receive_purchase_order_once(
        &self,
        actor: &Actor,
        draft: PurchaseReceiptDraft,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let order_id: PurchaseOrderId = draft.order_id.parse().map_err(|_| AppError::NotFound)?;
        if draft.idempotency_key.trim().is_empty() || draft.lines.is_empty() {
            return Err(AppError::Conflict);
        }
        let fingerprint = purchase_receipt_fingerprint(order_id, &draft.lines)?;
        let mut tx = self.store.begin().await?;
        if let Some(receipt) = tx
            .find_purchase_receipt_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if receipt.request_fingerprint != fingerprint {
                return Err(AppError::Conflict);
            }
            let order = tx
                .find_purchase_order(actor.organization_id, receipt.order_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if order.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let lines = tx
                .list_purchase_order_lines(actor.organization_id, order.id)
                .await?;
            tx.rollback().await?;
            return Ok(purchase_order_view(order, lines));
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let order = tx
            .find_purchase_order(actor.organization_id, order_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if order.branch_id != branch_id
            || !matches!(
                order.status,
                PurchaseOrderStatus::Ordered | PurchaseOrderStatus::PartiallyReceived
            )
        {
            return Err(AppError::Conflict);
        }
        let lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        let mut receipt_lines = Vec::new();
        let mut seen = BTreeSet::new();
        for input in &draft.lines {
            let line_id: PurchaseOrderLineId = input
                .order_line_id
                .parse()
                .map_err(|_| AppError::NotFound)?;
            if !seen.insert(line_id) {
                return Err(AppError::Conflict);
            }
            let quantity: Quantity = input.quantity.parse()?;
            if quantity.is_zero() {
                return Err(pos_core::DomainError::NegativeQuantity.into());
            }
            let line = lines
                .iter()
                .find(|line| line.id == line_id)
                .ok_or(AppError::NotFound)?;
            if quantity.milli_units() > line.remaining()?.milli_units() {
                return Err(AppError::Conflict);
            }
            receipt_lines.push((line.clone(), quantity));
        }
        let receipt = PurchaseReceipt {
            id: pos_core::ids::PurchaseReceiptId::new(),
            organization_id: actor.organization_id,
            branch_id,
            order_id,
            received_by: actor.user_id,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint,
            received_at: self.clock.now(),
        };
        if !tx.insert_purchase_receipt(&receipt).await? {
            let existing = tx
                .find_purchase_receipt_by_idempotency(
                    actor.organization_id,
                    &receipt.idempotency_key,
                )
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != receipt.request_fingerprint
                || existing.order_id != order_id
            {
                return Err(AppError::Conflict);
            }
            let replay_order = tx
                .find_purchase_order(actor.organization_id, existing.order_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if replay_order.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let replay_lines = tx
                .list_purchase_order_lines(actor.organization_id, replay_order.id)
                .await?;
            tx.rollback().await?;
            return Ok(purchase_order_view(replay_order, replay_lines));
        }
        for (line, quantity) in &receipt_lines {
            tx.insert_purchase_receipt_line(receipt.id, line.id, *quantity)
                .await?;
        }
        // Each conditional update verifies the pending quantity again at write time.
        tx.apply_purchase_receipt(
            actor.organization_id,
            order_id,
            &receipt_lines
                .iter()
                .map(|(line, quantity)| (line.id, *quantity))
                .collect::<Vec<_>>(),
            PurchaseOrderStatus::PartiallyReceived,
        )
        .await?;
        let updated_lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        let complete = updated_lines.iter().all(|line| {
            line.remaining()
                .map(|value| value.is_zero())
                .unwrap_or(false)
        });
        let status = if complete {
            PurchaseOrderStatus::Received
        } else {
            PurchaseOrderStatus::PartiallyReceived
        };
        if status == PurchaseOrderStatus::Received {
            tx.apply_purchase_receipt(actor.organization_id, order_id, &[], status)
                .await?;
        }
        for (line, quantity) in &receipt_lines {
            let movement = InventoryMovement::new(
                actor.organization_id,
                branch_id,
                line.product_id,
                MovementKind::Inbound,
                *quantity,
                Some(format!("compra {}", order.folio)),
                Some(actor.user_id),
            )?;
            tx.insert_inventory_movement(&movement).await?;
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::PurchaseReceived, "purchase_receipt", receipt.id.to_string(), serde_json::json!({ "folio": order.folio, "order_id": order.id.to_string(), "lines": receipt_lines.len() }))).await?;
        tx.commit().await?;
        let mut view_order = order;
        view_order.status = status;
        Ok(purchase_order_view(view_order, updated_lines))
    }

    pub async fn create_expense_category(
        &self,
        actor: &Actor,
        draft: ExpenseCategoryDraft,
    ) -> AppResult<ExpenseCategoryView> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let category = ExpenseCategory::new(actor.organization_id, draft.name)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Expenses, self.clock.now())?;
        tx.insert_expense_category(&category).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ExpenseCategoryCreated,
            "expense_category",
            category.id.to_string(),
            serde_json::json!({ "name": category.name }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ExpenseCategoryView {
            id: category.id.to_string(),
            name: category.name,
            active: category.active,
        })
    }

    pub async fn list_expense_categories(
        &self,
        actor: &Actor,
    ) -> AppResult<Vec<ExpenseCategoryView>> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let mut tx = self.store.begin().await?;
        let categories = tx.list_expense_categories(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(categories
            .into_iter()
            .map(|item| ExpenseCategoryView {
                id: item.id.to_string(),
                name: item.name,
                active: item.active,
            })
            .collect())
    }

    pub async fn list_expenses(&self, actor: &Actor) -> AppResult<Vec<ExpenseView>> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let expenses = tx.list_expenses(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(expenses.into_iter().map(expense_view).collect())
    }

    pub async fn record_expense(
        &self,
        actor: &Actor,
        draft: ExpenseDraft,
    ) -> AppResult<ExpenseView> {
        // Dos procesos locales pueden alcanzar la escritura del movimiento físico
        // antes de observar la clave idempotente. Reintentamos sólo la misma
        // solicitud inmutable para recuperar el hecho confirmado por SQLite.
        for delay_ms in [20_u64, 50_u64] {
            match self.record_expense_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.record_expense_once(actor, draft).await
    }

    async fn record_expense_once(
        &self,
        actor: &Actor,
        draft: ExpenseDraft,
    ) -> AppResult<ExpenseView> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let category_id: ExpenseCategoryId =
            draft.category_id.parse().map_err(|_| AppError::NotFound)?;
        let amount: Money = draft.amount.parse()?;
        let concept = draft.concept.trim().to_owned();
        if concept.is_empty()
            || draft.idempotency_key.trim().is_empty()
            || (!draft.paid_from_cash && draft.shift_id.is_some())
        {
            return Err(AppError::Conflict);
        }
        let fingerprint = format!(
            "{category_id}:{amount}:{}:{}:{}",
            concept,
            draft.paid_from_cash,
            draft.shift_id.as_deref().unwrap_or("")
        );
        let mut tx = self.store.begin().await?;
        if let Some(existing) = tx
            .find_expense_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if existing.request_fingerprint != fingerprint || existing.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(expense_view(existing));
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Expenses, self.clock.now())?;
        let category = tx
            .find_expense_category(actor.organization_id, category_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if !category.active {
            return Err(AppError::Conflict);
        }
        let (cash_shift_id, cash_movement_id) = if draft.paid_from_cash {
            let shift_id: CashShiftId = draft
                .shift_id
                .as_deref()
                .ok_or(AppError::NotFound)?
                .parse()
                .map_err(|_| AppError::NotFound)?;
            let shift = tx
                .find_cash_shift(actor.organization_id, shift_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if shift.status != CashShiftStatus::Open || shift.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let expected = expected_amount(
                &shift,
                &tx.list_cash_movements(actor.organization_id, shift.id)
                    .await?,
            )?;
            if expected.minor_units() < amount.minor_units() {
                return Err(AppError::Conflict);
            }
            let movement = CashMovement::new(
                actor.organization_id,
                branch_id,
                shift.register_id,
                shift.id,
                CashMovementKind::Out,
                amount,
                format!("gasto: {concept}"),
                Some(actor.user_id),
            )?;
            tx.insert_cash_movement(&movement).await?;
            (Some(shift.id), Some(movement.id))
        } else {
            (None, None)
        };
        let expense = Expense {
            id: ExpenseId::new(),
            organization_id: actor.organization_id,
            branch_id,
            category_id,
            amount,
            concept,
            cash_shift_id,
            cash_movement_id,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint,
            created_by: actor.user_id,
            occurred_at: self.clock.now(),
        };
        if !tx.insert_expense(&expense).await? {
            let existing = tx
                .find_expense_by_idempotency(actor.organization_id, &expense.idempotency_key)
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != expense.request_fingerprint {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(expense_view(existing));
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::ExpenseRecorded, "expense", expense.id.to_string(), serde_json::json!({ "amount": amount.to_string(), "paid_from_cash": draft.paid_from_cash }))).await?;
        tx.commit().await?;
        Ok(expense_view(expense))
    }

    pub async fn list_waste(&self, actor: &Actor) -> AppResult<Vec<WasteView>> {
        actor.require(pos_core::identity::Permission::ManageWaste)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let rows = tx.list_waste(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(rows.into_iter().map(waste_view).collect())
    }

    pub async fn record_waste(&self, actor: &Actor, draft: WasteDraft) -> AppResult<WasteView> {
        // Igual que los cobros y las recepciones, una contención SQLite no debe
        // convertir un reintento idéntico en una segunda merma.
        for delay_ms in [20_u64, 50_u64] {
            match self.record_waste_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.record_waste_once(actor, draft).await
    }

    async fn record_waste_once(&self, actor: &Actor, draft: WasteDraft) -> AppResult<WasteView> {
        actor.require(pos_core::identity::Permission::ManageWaste)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let product_id: ProductId = draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let quantity: Quantity = draft.quantity.parse()?;
        let reason = draft.reason.trim().to_owned();
        if reason.is_empty() || draft.idempotency_key.trim().is_empty() {
            return Err(AppError::Conflict);
        }
        let fingerprint = format!("{product_id}:{}:{reason}", quantity.milli_units());
        let mut tx = self.store.begin().await?;
        if let Some(existing) = tx
            .find_waste_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if existing.request_fingerprint != fingerprint || existing.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(waste_view(existing));
        }
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
        if tx
            .current_stock(actor.organization_id, branch_id, product_id)
            .await?
            .milli_units()
            < quantity.milli_units()
        {
            return Err(pos_core::DomainError::InsufficientStock.into());
        }
        let movement = InventoryMovement::new(
            actor.organization_id,
            branch_id,
            product_id,
            MovementKind::AdjustmentOut,
            quantity,
            Some(format!("merma: {reason}")),
            Some(actor.user_id),
        )?;
        tx.insert_inventory_movement(&movement).await?;
        let waste = WasteRecord {
            id: WasteId::new(),
            organization_id: actor.organization_id,
            branch_id,
            product_id,
            quantity,
            reason,
            inventory_movement_id: movement.id,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint,
            created_by: actor.user_id,
            occurred_at: self.clock.now(),
        };
        if !tx.insert_waste(&waste).await? {
            let existing = tx
                .find_waste_by_idempotency(actor.organization_id, &waste.idempotency_key)
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != waste.request_fingerprint
                || existing.branch_id != branch_id
            {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(waste_view(existing));
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::WasteRecorded, "waste_record", waste.id.to_string(), serde_json::json!({ "product_id": product_id.to_string(), "quantity": quantity.to_string() }))).await?;
        tx.commit().await?;
        Ok(waste_view(waste))
    }

    pub async fn find_sale_for_return(
        &self,
        actor: &Actor,
        folio: String,
    ) -> AppResult<ReturnableSaleView> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let folio = folio.trim().to_owned();
        if folio.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        let mut tx = self.store.begin().await?;
        let sale = tx
            .find_sale_by_folio(actor.organization_id, branch_id, &folio)
            .await?
            .ok_or(AppError::NotFound)?;
        let items = tx.list_sale_items(actor.organization_id, sale.id).await?;
        let mut views = Vec::new();
        for item in &items {
            let totals = tx.returned_totals(actor.organization_id, item.id).await?;
            let returnable_milli = (item.quantity.milli_units() - totals.quantity_milli).max(0);
            views.push(ReturnableSaleItemView {
                id: item.id.to_string(),
                product_name: item.product_name.clone(),
                variant_name: item.variant_name.clone(),
                modifiers_summary: item.modifiers_summary.clone(),
                quantity: item.quantity.to_string(),
                returned_quantity: Quantity::from_milli_units(totals.quantity_milli)?.to_string(),
                returnable_quantity: Quantity::from_milli_units(returnable_milli)?.to_string(),
                unit_refund: item.unit_price.checked_add(item.unit_tax)?.to_string(),
            });
        }
        tx.rollback().await?;
        Ok(ReturnableSaleView {
            id: sale.id.to_string(),
            folio: sale.folio,
            occurred_at: sale.occurred_at,
            total: sale.total.to_string(),
            items: views,
        })
    }

    pub async fn preview_sale_return(
        &self,
        actor: &Actor,
        draft: SaleReturnPreviewDraft,
    ) -> AppResult<SaleReturnPreviewView> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let sale_id = draft.sale_id.parse().map_err(|_| AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let sale = tx
            .find_sale(actor.organization_id, sale_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if sale.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        let items = tx.list_sale_items(actor.organization_id, sale_id).await?;
        let resolved =
            resolve_return_lines(tx.as_mut(), actor.organization_id, &items, &draft.lines).await?;
        let mut total = Money::ZERO;
        let mut lines = Vec::new();
        for line in &resolved {
            total = total.checked_add(line.total)?;
            lines.push(SaleReturnPreviewLineView {
                sale_item_id: line.item.id.to_string(),
                quantity: line.quantity.to_string(),
                refund_total: line.total.to_string(),
            });
        }
        tx.rollback().await?;
        Ok(SaleReturnPreviewView {
            refund_total: total.to_string(),
            lines,
        })
    }

    pub async fn create_sale_return(
        &self,
        actor: &Actor,
        draft: SaleReturnDraft,
    ) -> AppResult<SaleReturnView> {
        // Misma garantía que compras y gastos: una contención de SQLite se
        // reintenta con el payload inmutable para recuperar el hecho confirmado.
        for delay_ms in [20_u64, 50_u64] {
            match self.create_sale_return_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.create_sale_return_once(actor, draft).await
    }

    async fn create_sale_return_once(
        &self,
        actor: &Actor,
        draft: SaleReturnDraft,
    ) -> AppResult<SaleReturnView> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let reason = draft.reason.trim().to_owned();
        if reason.is_empty() || draft.idempotency_key.trim().is_empty() || draft.lines.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        let sale_id: pos_core::ids::SaleId =
            draft.sale_id.parse().map_err(|_| AppError::NotFound)?;
        let fingerprint = sale_return_fingerprint(&draft, &reason)?;
        let mut tx = self.store.begin().await?;
        if let Some(existing) = tx
            .find_sale_return_by_idempotency(actor.organization_id, draft.idempotency_key.trim())
            .await?
        {
            if existing.request_fingerprint != fingerprint {
                return Err(AppError::Conflict);
            }
            if existing.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let sale = tx
                .find_sale(actor.organization_id, existing.sale_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if sale.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let lines = tx
                .list_sale_return_lines(actor.organization_id, existing.id)
                .await?;
            let items = tx.list_sale_items(actor.organization_id, sale.id).await?;
            let view = sale_return_view(existing, &sale, &lines, &items);
            tx.rollback().await?;
            return Ok(view);
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        let sale = tx
            .find_sale(actor.organization_id, sale_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if sale.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        let items = tx.list_sale_items(actor.organization_id, sale_id).await?;
        let resolved =
            resolve_return_lines(tx.as_mut(), actor.organization_id, &items, &draft.lines).await?;
        let mut refund_total = Money::ZERO;
        for line in &resolved {
            refund_total = refund_total.checked_add(line.total)?;
        }
        // Un reembolso de cero centavos solo restituye inventario: no exige turno
        // abierto ni crea un movimiento de caja ficticio.
        let (shift_id, cash_movement_id) = if refund_total.minor_units() > 0 {
            let raw_shift = draft
                .shift_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(AppError::InvalidRequest)?;
            let shift_id: CashShiftId = raw_shift.parse().map_err(|_| AppError::NotFound)?;
            let shift = tx
                .find_cash_shift(actor.organization_id, shift_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if shift.status != CashShiftStatus::Open || shift.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            let expected = expected_amount(
                &shift,
                &tx.list_cash_movements(actor.organization_id, shift.id)
                    .await?,
            )?;
            if expected.minor_units() < refund_total.minor_units() {
                return Err(pos_core::DomainError::InsufficientCash.into());
            }
            let movement = CashMovement::new(
                actor.organization_id,
                branch_id,
                shift.register_id,
                shift.id,
                CashMovementKind::Out,
                refund_total,
                format!("devolución {}", sale.folio),
                Some(actor.user_id),
            )?;
            tx.insert_cash_movement(&movement).await?;
            (Some(shift.id), Some(movement.id))
        } else {
            (None, None)
        };
        let return_entity = SaleReturn {
            id: pos_core::ids::SaleReturnId::new(),
            organization_id: actor.organization_id,
            branch_id,
            sale_id,
            shift_id,
            cash_movement_id,
            operator_id: actor.user_id,
            reason: reason.clone(),
            refund_total,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint.clone(),
            occurred_at: self.clock.now(),
        };
        if !tx.insert_sale_return(&return_entity).await? {
            let existing = tx
                .find_sale_return_by_idempotency(
                    actor.organization_id,
                    &return_entity.idempotency_key,
                )
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != fingerprint || existing.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            let replay_lines = tx
                .list_sale_return_lines(actor.organization_id, existing.id)
                .await?;
            let view = sale_return_view(existing, &sale, &replay_lines, &items);
            tx.rollback().await?;
            return Ok(view);
        }
        let mut stored_lines = Vec::new();
        for line in &resolved {
            let inbound_movement_id = if line.disposition == ReturnDisposition::Restock {
                let inbound = InventoryMovement::new(
                    actor.organization_id,
                    branch_id,
                    line.item.product_id,
                    MovementKind::Inbound,
                    line.quantity,
                    Some(format!("devolución {}", sale.folio)),
                    Some(actor.user_id),
                )?;
                tx.insert_inventory_movement(&inbound).await?;
                Some(inbound.id)
            } else {
                None
            };
            let stored = SaleReturnLine {
                id: pos_core::ids::SaleReturnLineId::new(),
                return_id: return_entity.id,
                sale_item_id: line.item.id,
                quantity: line.quantity,
                disposition: line.disposition,
                refund_net: line.net,
                refund_tax: line.tax,
                refund_total: line.total,
                inbound_movement_id,
            };
            tx.insert_sale_return_line(&stored).await?;
            stored_lines.push(stored);
        }
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::SaleReturned,
            "sale_return",
            return_entity.id.to_string(),
            serde_json::json!({
                "sale_folio": sale.folio,
                "refund": refund_total.to_string(),
                "lines": resolved.len(),
                "shift_id": shift_id.map(|id| id.to_string()),
            }),
        ))
        .await?;
        tx.commit().await?;
        Ok(sale_return_view(
            return_entity,
            &sale,
            &stored_lines,
            &items,
        ))
    }

    pub async fn list_sale_returns(&self, actor: &Actor) -> AppResult<Vec<SaleReturnView>> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let returns = tx
            .list_sale_returns(actor.organization_id, branch_id, 50)
            .await?;
        let mut views = Vec::new();
        for item in returns {
            let sale = tx
                .find_sale(actor.organization_id, item.sale_id)
                .await?
                .ok_or(AppError::NotFound)?;
            let lines = tx
                .list_sale_return_lines(actor.organization_id, item.id)
                .await?;
            let sale_items = tx
                .list_sale_items(actor.organization_id, item.sale_id)
                .await?;
            views.push(sale_return_view(item, &sale, &lines, &sale_items));
        }
        tx.rollback().await?;
        Ok(views)
    }

    pub async fn reports_summary(
        &self,
        actor: &Actor,
        query: ReportsQuery,
    ) -> AppResult<ReportsView> {
        actor.require(pos_core::identity::Permission::ViewReports)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let from = parse_rfc3339(&query.from)?;
        let to = parse_rfc3339(&query.to)?;
        if from >= to {
            return Err(AppError::InvalidRequest);
        }
        let mut tx = self.store.begin().await?;
        let sales = tx
            .sales_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let by_product = tx
            .sales_report_by_product(actor.organization_id, branch_id, from, to)
            .await?;
        let returns = tx
            .returns_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let cash = tx
            .cash_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let purchases = tx
            .purchases_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let expenses = tx
            .expenses_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let inventory = tx
            .inventory_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        tx.rollback().await?;

        let returned_total =
            Money::from_minor_units(returns.total_minor).map_err(AppError::Domain)?;
        let net_minor = sales
            .total_minor
            .checked_sub(returns.total_minor)
            .ok_or(AppError::Storage)?;
        Ok(ReportsView {
            from,
            to,
            sales: ReportsSalesView {
                count: sales.count,
                subtotal: money_minor(sales.subtotal_minor)?,
                tax: money_minor(sales.tax_minor)?,
                total: money_minor(sales.total_minor)?,
                returned_count: returns.count,
                returned_total: returned_total.to_string(),
                net_total: signed_money_minor(net_minor),
                products: by_product
                    .into_iter()
                    .map(|row| {
                        Ok(ReportsProductView {
                            product_id: row.product_id.to_string(),
                            product_name: row.product_name,
                            quantity: Quantity::from_milli_units(row.quantity_milli)?.to_string(),
                            net: money_minor(row.net_minor)?,
                            tax: money_minor(row.tax_minor)?,
                            total: money_minor(row.total_minor)?,
                        })
                    })
                    .collect::<AppResult<Vec<_>>>()?,
            },
            cash: ReportsCashView {
                shifts_opened: cash.shifts_opened,
                shifts_closed: cash.shifts_closed,
                entries: money_minor(cash.entries_minor)?,
                exits: money_minor(cash.exits_minor)?,
                expenses_from_cash: money_minor(cash.expenses_from_cash_minor)?,
                refunds: money_minor(cash.refunds_minor)?,
                difference: signed_money_minor(cash.difference_minor),
            },
            purchases: ReportsPurchasesView {
                orders_created: purchases.orders_created,
                pending_orders: purchases.pending_orders,
                received_quantity: Quantity::from_milli_units(purchases.received_quantity_milli)?
                    .to_string(),
                received_cost: money_minor(purchases.received_cost_minor)?,
            },
            expenses: ReportsExpensesView {
                total: money_minor(expenses.total_minor)?,
                from_cash: money_minor(expenses.from_cash_minor)?,
                external: money_minor(expenses.external_minor)?,
                categories: expenses
                    .by_category
                    .into_iter()
                    .map(|row| {
                        Ok(ReportsExpenseCategoryView {
                            name: row.name,
                            total: money_minor(row.total_minor)?,
                        })
                    })
                    .collect::<AppResult<Vec<_>>>()?,
            },
            inventory: ReportsInventoryView {
                stock_quantity: Quantity::from_milli_units(inventory.stock_quantity_milli)?
                    .to_string(),
                inbound: Quantity::from_milli_units(inventory.inbound_milli)?.to_string(),
                outbound: Quantity::from_milli_units(inventory.outbound_milli)?.to_string(),
                waste: Quantity::from_milli_units(inventory.waste_milli)?.to_string(),
                adjustment_in: Quantity::from_milli_units(inventory.adjustment_in_milli)?
                    .to_string(),
                adjustment_out: Quantity::from_milli_units(inventory.adjustment_out_milli)?
                    .to_string(),
            },
        })
    }

    pub async fn commercial_state(&self) -> AppResult<CommercialStateView> {
        let mut tx = self.store.begin().await?;
        let state = tx.installation_state().await?.ok_or(AppError::NotFound)?;
        tx.rollback().await?;
        commercial_view(&state, self.clock.now())
    }
}

struct ResolvedReturnLine {
    item: SaleItem,
    quantity: Quantity,
    disposition: ReturnDisposition,
    net: Money,
    tax: Money,
    total: Money,
}

/// Valida pertenencia, cantidades pendientes y calcula el reembolso exacto a
/// partir de los snapshots de la venta original.
async fn resolve_return_lines(
    tx: &mut dyn StoreTx,
    organization_id: OrganizationId,
    items: &[SaleItem],
    drafts: &[SaleReturnLineDraft],
) -> AppResult<Vec<ResolvedReturnLine>> {
    if drafts.is_empty() {
        return Err(AppError::InvalidRequest);
    }
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::new();
    for draft in drafts {
        let item_id: SaleItemId = draft.sale_item_id.parse().map_err(|_| AppError::NotFound)?;
        if !seen.insert(item_id) {
            return Err(AppError::InvalidRequest);
        }
        let quantity: Quantity = draft.quantity.parse()?;
        if quantity.is_zero() {
            return Err(pos_core::DomainError::NegativeQuantity.into());
        }
        let disposition = ReturnDisposition::from_key(draft.disposition.trim())
            .ok_or(AppError::InvalidRequest)?;
        let item = items
            .iter()
            .find(|item| item.id == item_id)
            .ok_or(AppError::NotFound)?;
        let totals = tx.returned_totals(organization_id, item_id).await?;
        let remaining = item.quantity.milli_units() - totals.quantity_milli;
        if remaining < 0 || quantity.milli_units() > remaining {
            return Err(AppError::Conflict);
        }
        let basis = RefundBasis {
            returned_quantity: Quantity::from_milli_units(totals.quantity_milli)?,
            refunded_net: Money::from_minor_units(totals.net_minor).map_err(AppError::Domain)?,
            refunded_tax: Money::from_minor_units(totals.tax_minor).map_err(AppError::Domain)?,
        };
        let refund = refund_increment(item, &basis, quantity)?;
        resolved.push(ResolvedReturnLine {
            item: item.clone(),
            quantity,
            disposition,
            net: refund.net,
            tax: refund.tax,
            total: refund.total,
        });
    }
    Ok(resolved)
}

fn sale_return_fingerprint(draft: &SaleReturnDraft, reason: &str) -> AppResult<String> {
    let mut rows = Vec::new();
    for line in &draft.lines {
        let item_id: SaleItemId = line.sale_item_id.parse().map_err(|_| AppError::NotFound)?;
        let quantity: Quantity = line.quantity.parse()?;
        let disposition =
            ReturnDisposition::from_key(line.disposition.trim()).ok_or(AppError::InvalidRequest)?;
        rows.push(format!(
            "{item_id}:{}:{}",
            quantity.milli_units(),
            disposition.as_str()
        ));
    }
    rows.sort();
    Ok(format!(
        "{}|{}|{reason}|{}",
        draft.sale_id.trim(),
        draft.shift_id.as_deref().unwrap_or("").trim(),
        rows.join(",")
    ))
}

fn sale_return_view(
    value: SaleReturn,
    sale: &Sale,
    lines: &[SaleReturnLine],
    items: &[SaleItem],
) -> SaleReturnView {
    SaleReturnView {
        id: value.id.to_string(),
        sale_id: sale.id.to_string(),
        sale_folio: sale.folio.clone(),
        reason: value.reason,
        refund_total: value.refund_total.to_string(),
        occurred_at: value.occurred_at,
        lines: lines
            .iter()
            .map(|line| SaleReturnLineView {
                sale_item_id: line.sale_item_id.to_string(),
                product_name: items
                    .iter()
                    .find(|item| item.id == line.sale_item_id)
                    .map(|item| item.product_name.clone())
                    .unwrap_or_default(),
                quantity: line.quantity.to_string(),
                disposition: line.disposition.as_str().to_owned(),
                refund_net: line.refund_net.to_string(),
                refund_tax: line.refund_tax.to_string(),
                refund_total: line.refund_total.to_string(),
            })
            .collect(),
    }
}

fn parse_rfc3339(value: &str) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(value.trim(), &time::format_description::well_known::Rfc3339)
        .map_err(|_| AppError::InvalidRequest)
}

fn money_minor(value: i64) -> AppResult<String> {
    Ok(Money::from_minor_units(value)
        .map_err(AppError::Domain)?
        .to_string())
}

/// Métrica firmada del período (ventas netas, diferencias de arqueo).
/// `Money` sigue siendo no negativo para cobros y reembolsos.
fn signed_money_minor(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.unsigned_abs();
    format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
}

fn expense_view(expense: Expense) -> ExpenseView {
    ExpenseView {
        id: expense.id.to_string(),
        category_id: expense.category_id.to_string(),
        amount: expense.amount.to_string(),
        concept: expense.concept,
        paid_from_cash: expense.cash_movement_id.is_some(),
        occurred_at: expense.occurred_at,
    }
}

fn waste_view(waste: WasteRecord) -> WasteView {
    WasteView {
        id: waste.id.to_string(),
        product_id: waste.product_id.to_string(),
        quantity: waste.quantity.to_string(),
        reason: waste.reason,
        occurred_at: waste.occurred_at,
    }
}

fn purchase_order_view(order: PurchaseOrder, lines: Vec<PurchaseOrderLine>) -> PurchaseOrderView {
    PurchaseOrderView {
        id: order.id.to_string(),
        supplier_id: order.supplier_id.to_string(),
        folio: order.folio,
        status: order.status.as_str().to_owned(),
        created_at: order.created_at,
        lines: lines
            .into_iter()
            .map(|line| PurchaseOrderLineView {
                id: line.id.to_string(),
                product_id: line.product_id.to_string(),
                quantity: line.requested_quantity.to_string(),
                received_quantity: line.received_quantity.to_string(),
                unit_cost: line.unit_cost.to_string(),
            })
            .collect(),
    }
}

fn purchase_receipt_fingerprint(
    order_id: PurchaseOrderId,
    lines: &[PurchaseReceiptLineDraft],
) -> AppResult<String> {
    let mut values = lines
        .iter()
        .map(|line| {
            let id: PurchaseOrderLineId =
                line.order_line_id.parse().map_err(|_| AppError::NotFound)?;
            let quantity: Quantity = line.quantity.parse()?;
            Ok(format!("{id}:{}", quantity.milli_units()))
        })
        .collect::<AppResult<Vec<_>>>()?;
    values.sort();
    Ok(format!("{}|{}", order_id, values.join("|")))
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
