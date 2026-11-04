use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use pos_core::audit::AuditEvent;
use pos_core::cash::{CashMovement, CashRegister, CashShift};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::expenses::{Expense, ExpenseCategory, WasteRecord};
use pos_core::identity::User;
use pos_core::ids::{
    BranchId, CategoryId, ExpenseCategoryId, InstallationId, OrganizationId, ProductId, UserId,
};
use pos_core::inventory::InventoryMovement;
use pos_core::modifiers::{ModifierGroup, ModifierOption};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::purchasing::{PurchaseOrder, PurchaseOrderLine, PurchaseReceipt, Supplier};
use pos_core::returns::{SaleReturn, SaleReturnLine};
use pos_core::sales::{Sale, SaleItem, SalePayment};
use serde::Serialize;
use time::OffsetDateTime;

/// Agregados de reportes calculados en SQLite con unidades menores exactas.
/// La aplicación solo los presenta; no reimplementa las sumas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SalesReportTotals {
    pub count: i64,
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSalesReportRow {
    pub product_id: ProductId,
    pub product_name: String,
    pub quantity_milli: i64,
    pub net_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReturnsReportTotals {
    pub count: i64,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CashReportTotals {
    pub shifts_opened: i64,
    pub shifts_closed: i64,
    pub entries_minor: i64,
    pub exits_minor: i64,
    pub expenses_from_cash_minor: i64,
    pub refunds_minor: i64,
    pub difference_minor: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurchasesReportTotals {
    pub orders_created: i64,
    pub pending_orders: i64,
    pub received_quantity_milli: i64,
    pub received_cost_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseCategoryReportRow {
    pub name: String,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpensesReportTotals {
    pub total_minor: i64,
    pub from_cash_minor: i64,
    pub external_minor: i64,
    pub by_category: Vec<ExpenseCategoryReportRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryReportTotals {
    pub stock_quantity_milli: i64,
    pub inbound_milli: i64,
    pub outbound_milli: i64,
    pub waste_milli: i64,
    pub adjustment_in_milli: i64,
    pub adjustment_out_milli: i64,
}

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
    async fn find_branch(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Option<Branch>>;

    async fn insert_user(&mut self, user: &User) -> AppResult<()>;
    async fn find_user_by_username(&mut self, username: &str) -> AppResult<Option<User>>;
    async fn find_user_by_id(
        &mut self,
        organization_id: OrganizationId,
        user_id: UserId,
    ) -> AppResult<Option<User>>;
    async fn list_users(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<User>>;
    async fn update_user_active(
        &mut self,
        organization_id: OrganizationId,
        user_id: UserId,
        active: bool,
    ) -> AppResult<()>;
    async fn update_user_password(
        &mut self,
        organization_id: OrganizationId,
        user_id: UserId,
        password_hash: &str,
    ) -> AppResult<()>;
    /// Propietarios activos de la organización; protege la última cuenta
    /// administrativa.
    async fn count_active_owners(&mut self, organization_id: OrganizationId) -> AppResult<i64>;
    async fn count_users(&mut self, organization_id: OrganizationId) -> AppResult<i64>;

    async fn insert_product(&mut self, product: &Product) -> AppResult<()>;
    async fn find_product(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Option<Product>>;
    async fn list_products(&mut self, organization_id: OrganizationId) -> AppResult<Vec<Product>>;

    async fn insert_category(&mut self, category: &Category) -> AppResult<()>;
    async fn find_category(
        &mut self,
        organization_id: OrganizationId,
        category_id: CategoryId,
    ) -> AppResult<Option<Category>>;
    async fn list_categories(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<Vec<Category>>;
    async fn update_product_category(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
        category_id: Option<CategoryId>,
    ) -> AppResult<()>;

    async fn insert_product_variant(&mut self, variant: &ProductVariant) -> AppResult<()>;
    async fn list_product_variants(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Vec<ProductVariant>>;

    async fn insert_cash_register(&mut self, register: &CashRegister) -> AppResult<()>;
    async fn list_cash_registers(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<CashRegister>>;

    async fn insert_cash_shift(&mut self, shift: &CashShift) -> AppResult<()>;
    async fn find_open_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        register_id: pos_core::ids::CashRegisterId,
    ) -> AppResult<Option<CashShift>>;

    async fn find_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
    ) -> AppResult<Option<CashShift>>;
    #[allow(clippy::too_many_arguments)]
    async fn close_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
        closed_by: UserId,
        closed_at: time::OffsetDateTime,
        expected_amount: Money,
        counted_amount: Money,
        difference_minor: i64,
    ) -> AppResult<()>;
    async fn insert_cash_movement(&mut self, movement: &CashMovement) -> AppResult<()>;
    async fn list_cash_movements(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
    ) -> AppResult<Vec<CashMovement>>;
    async fn list_closed_cash_shifts(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<CashShift>>;

    async fn insert_sale(&mut self, sale: &Sale) -> AppResult<()>;
    async fn insert_sale_item(&mut self, item: &SaleItem) -> AppResult<()>;
    async fn insert_sale_payment(&mut self, payment: &SalePayment) -> AppResult<()>;
    async fn find_sale_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<Sale>>;
    async fn find_sale(
        &mut self,
        organization_id: OrganizationId,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<Option<Sale>>;
    async fn list_sale_items(
        &mut self,
        organization_id: OrganizationId,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<Vec<SaleItem>>;
    async fn list_sale_payments(
        &mut self,
        organization_id: OrganizationId,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<Vec<SalePayment>>;

    async fn insert_modifier_group(&mut self, group: &ModifierGroup) -> AppResult<()>;
    async fn find_modifier_group(
        &mut self,
        organization_id: OrganizationId,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Option<ModifierGroup>>;
    async fn list_modifier_groups(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Vec<ModifierGroup>>;

    async fn insert_modifier_option(&mut self, option: &ModifierOption) -> AppResult<()>;
    async fn find_modifier_option(
        &mut self,
        organization_id: OrganizationId,
        option_id: pos_core::ids::ModifierOptionId,
    ) -> AppResult<Option<ModifierOption>>;
    async fn list_modifier_options(
        &mut self,
        organization_id: OrganizationId,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Vec<ModifierOption>>;

    async fn insert_supplier(&mut self, supplier: &Supplier) -> AppResult<()>;
    async fn list_suppliers(&mut self, organization_id: OrganizationId)
    -> AppResult<Vec<Supplier>>;
    async fn find_supplier(
        &mut self,
        organization_id: OrganizationId,
        supplier_id: pos_core::ids::SupplierId,
    ) -> AppResult<Option<Supplier>>;
    async fn insert_purchase_order(&mut self, order: &PurchaseOrder) -> AppResult<()>;
    async fn insert_purchase_order_line(&mut self, line: &PurchaseOrderLine) -> AppResult<()>;
    async fn find_purchase_order(
        &mut self,
        organization_id: OrganizationId,
        order_id: pos_core::ids::PurchaseOrderId,
    ) -> AppResult<Option<PurchaseOrder>>;
    async fn list_purchase_orders(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<PurchaseOrder>>;
    async fn list_purchase_order_lines(
        &mut self,
        organization_id: OrganizationId,
        order_id: pos_core::ids::PurchaseOrderId,
    ) -> AppResult<Vec<PurchaseOrderLine>>;
    async fn confirm_purchase_order(
        &mut self,
        organization_id: OrganizationId,
        order_id: pos_core::ids::PurchaseOrderId,
    ) -> AppResult<()>;
    async fn find_purchase_receipt_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<PurchaseReceipt>>;
    /// Inserta la clave idempotente; `false` indica que ya existía y se debe recuperar.
    async fn insert_purchase_receipt(&mut self, receipt: &PurchaseReceipt) -> AppResult<bool>;
    async fn insert_purchase_receipt_line(
        &mut self,
        receipt_id: pos_core::ids::PurchaseReceiptId,
        order_line_id: pos_core::ids::PurchaseOrderLineId,
        quantity: pos_core::quantity::Quantity,
    ) -> AppResult<()>;
    async fn apply_purchase_receipt(
        &mut self,
        organization_id: OrganizationId,
        order_id: pos_core::ids::PurchaseOrderId,
        updates: &[(
            pos_core::ids::PurchaseOrderLineId,
            pos_core::quantity::Quantity,
        )],
        status: pos_core::purchasing::PurchaseOrderStatus,
    ) -> AppResult<()>;

    async fn insert_expense_category(&mut self, category: &ExpenseCategory) -> AppResult<()>;
    async fn list_expense_categories(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<Vec<ExpenseCategory>>;
    async fn find_expense_category(
        &mut self,
        organization_id: OrganizationId,
        category_id: ExpenseCategoryId,
    ) -> AppResult<Option<ExpenseCategory>>;
    async fn find_expense_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<Expense>>;
    async fn insert_expense(&mut self, expense: &Expense) -> AppResult<bool>;
    async fn list_expenses(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<Expense>>;
    async fn find_waste_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<WasteRecord>>;
    async fn insert_waste(&mut self, waste: &WasteRecord) -> AppResult<bool>;
    async fn list_waste(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<WasteRecord>>;

    async fn find_sale_by_folio(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        folio: &str,
    ) -> AppResult<Option<Sale>>;
    /// Cantidad ya devuelta de una partida, sumando todas sus devoluciones.
    async fn returned_quantity(
        &mut self,
        organization_id: OrganizationId,
        sale_item_id: pos_core::ids::SaleItemId,
    ) -> AppResult<pos_core::quantity::Quantity>;
    /// Inserta la cabecera; `false` indica que la clave ya existía y se debe recuperar.
    async fn insert_sale_return(&mut self, value: &SaleReturn) -> AppResult<bool>;
    async fn insert_sale_return_line(&mut self, line: &SaleReturnLine) -> AppResult<()>;
    async fn find_sale_return_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<SaleReturn>>;
    async fn list_sale_return_lines(
        &mut self,
        organization_id: OrganizationId,
        return_id: pos_core::ids::SaleReturnId,
    ) -> AppResult<Vec<SaleReturnLine>>;
    async fn list_sale_returns(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        limit: i64,
    ) -> AppResult<Vec<SaleReturn>>;

    async fn sales_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<SalesReportTotals>;
    async fn sales_report_by_product(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<Vec<ProductSalesReportRow>>;
    async fn returns_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<ReturnsReportTotals>;
    async fn cash_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<CashReportTotals>;
    async fn purchases_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<PurchasesReportTotals>;
    async fn expenses_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<ExpensesReportTotals>;
    async fn inventory_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<InventoryReportTotals>;

    async fn insert_inventory_movement(&mut self, movement: &InventoryMovement) -> AppResult<()>;
    async fn current_stock(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: ProductId,
    ) -> AppResult<pos_core::quantity::Quantity>;
    /// Existencias por producto de la sucursal, para presentar disponibilidad
    /// sin una consulta por producto.
    async fn stock_levels(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<(ProductId, pos_core::quantity::Quantity)>>;
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
