//! Punto de entrada Tauri 2 y comandos IPC tipados.
//!
//! Toda regla de negocio vive en Rust. El frontend solo presenta estado y envía
//! comandos; la autorización se resuelve en el backend con el actor de sesión.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use pos_application::{
    AppError, AppStatus, AuditEventView, BootstrapRequest, BootstrapResponse, BranchView,
    CashCloseShiftDraft, CashMovementDraft, CashMovementView, CashOpenShiftDraft,
    CashRegisterDraft, CashRegisterView, CashShiftView, CategoryDraft, CategoryView,
    ChangeOwnPasswordDraft, CheckoutDraft, CommercialStateView, ExpenseCategoryDraft,
    ExpenseCategoryView, ExpenseDraft, ExpenseView, InventoryMovementDraft, InventoryMovementView,
    LoginRequest, ModifierConfigurationDraft, ModifierConfigurationView, ModifierGroupDraft,
    ModifierGroupView, ModifierOptionDraft, ModifierOptionView, PosService, ProductDraft,
    ProductVariantDraft, ProductVariantView, ProductView, PurchaseOrderDraft, PurchaseOrderView,
    PurchaseReceiptDraft, ReportsQuery, ReportsView, ResetUserPasswordDraft, ReturnableSaleView,
    SaleReturnDraft, SaleReturnPreviewDraft, SaleReturnPreviewView, SaleReturnView, SaleView,
    SetUserActiveDraft, SharedClock, SharedHasher, SharedStore, StockLevelView, SupplierDraft,
    SupplierView, SystemClock, UserDraft, UserView, WasteDraft, WasteView,
};
use pos_core::identity::{Actor, Permission};
use pos_core::ids::{BranchId, ProductId, PurchaseOrderId};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::SqliteStore;
use serde::Serialize;
use tauri::{Manager, State};

struct AppState {
    db_path: PathBuf,
    pool: sqlx::SqlitePool,
    service: PosService,
    session: RwLock<Option<Actor>>,
}

#[derive(Debug, Serialize)]
struct IpcError {
    code: &'static str,
    message: String,
}

impl From<pos_core::DomainError> for IpcError {
    fn from(error: pos_core::DomainError) -> Self {
        AppError::from(error).into()
    }
}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

type IpcResult<T> = Result<T, IpcError>;

/// El driver WebView no propaga las variables de entorno al binario de la app.
/// Esta ruta solo se acepta desde el proceso que Tauri marcó para automatización.
fn automation_database_path() -> Option<PathBuf> {
    if std::env::var_os("TAURI_WEBVIEW_AUTOMATION")?.as_os_str() != "true" {
        return None;
    }
    let mut args = std::env::args_os();
    while let Some(argument) = args.next() {
        if argument == "--hiqnet-e2e-db-path" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn session_actor(state: &AppState) -> IpcResult<Actor> {
    state
        .session
        .read()
        .map_err(|_| AppError::Storage)?
        .clone()
        .ok_or(AppError::Unauthenticated.into())
}

/// Revalida la sesión contra la base en cada comando: una cuenta desactivada,
/// eliminada o con otro rol pierde el acceso aunque su `Actor` siga en memoria
/// dentro de la capa IPC.
async fn authenticated(state: &AppState) -> IpcResult<Actor> {
    let stored = session_actor(state)?;
    match state.service.refresh_actor(&stored).await {
        Ok(actor) => {
            *state.session.write().map_err(|_| AppError::Storage)? = Some(actor.clone());
            Ok(actor)
        }
        Err(_) => {
            *state.session.write().map_err(|_| AppError::Storage)? = None;
            Err(AppError::Unauthenticated.into())
        }
    }
}

// Todos los comandos declaran `rename_all = "snake_case"` para que los
// argumentos conserven los nombres publicados en `docs/03-contracts/ipc.md`
// (`product_id`, `register_id`, `shift_id`, ...). Tauri 2 los renombraría a
// camelCase por defecto y el frontend dejaría de poder invocarlos.
#[tauri::command(rename_all = "snake_case")]
async fn app_status(state: State<'_, AppState>) -> IpcResult<AppStatus> {
    Ok(state.service.status().await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn bootstrap(
    state: State<'_, AppState>,
    request: BootstrapRequest,
) -> IpcResult<BootstrapResponse> {
    Ok(state.service.bootstrap(request).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn login(state: State<'_, AppState>, request: LoginRequest) -> IpcResult<Actor> {
    let actor = state.service.login(request).await?;
    *state.session.write().map_err(|_| IpcError {
        code: "storage_error",
        message: "no fue posible conservar la sesión".into(),
    })? = Some(actor.clone());
    Ok(actor)
}

#[tauri::command(rename_all = "snake_case")]
fn logout(state: State<'_, AppState>) -> IpcResult<()> {
    *state.session.write().map_err(|_| IpcError {
        code: "storage_error",
        message: "no fue posible cerrar la sesión".into(),
    })? = None;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn current_session(state: State<'_, AppState>) -> IpcResult<Option<Actor>> {
    if state
        .session
        .read()
        .map_err(|_| AppError::Storage)?
        .is_none()
    {
        return Ok(None);
    }
    Ok(authenticated(&state).await.ok())
}

#[tauri::command(rename_all = "snake_case")]
async fn create_user(state: State<'_, AppState>, draft: UserDraft) -> IpcResult<UserView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_user(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_users(state: State<'_, AppState>) -> IpcResult<Vec<UserView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_users(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn set_user_active(
    state: State<'_, AppState>,
    draft: SetUserActiveDraft,
) -> IpcResult<UserView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.set_user_active(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn reset_user_password(
    state: State<'_, AppState>,
    draft: ResetUserPasswordDraft,
) -> IpcResult<()> {
    let actor = authenticated(&state).await?;
    Ok(state.service.reset_user_password(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn change_password(
    state: State<'_, AppState>,
    draft: ChangeOwnPasswordDraft,
) -> IpcResult<()> {
    let actor = authenticated(&state).await?;
    Ok(state.service.change_own_password(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_product(state: State<'_, AppState>, draft: ProductDraft) -> IpcResult<ProductView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_product(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_products(state: State<'_, AppState>) -> IpcResult<Vec<ProductView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_products(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn assign_product_category(
    state: State<'_, AppState>,
    product_id: String,
    category_id: Option<String>,
) -> IpcResult<ProductView> {
    let actor = authenticated(&state).await?;
    let product_id: ProductId = product_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state
        .service
        .assign_product_category(&actor, product_id, category_id)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_modifier_group(
    state: State<'_, AppState>,
    draft: ModifierGroupDraft,
) -> IpcResult<ModifierGroupView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_modifier_group(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_modifier_groups(
    state: State<'_, AppState>,
    product_id: String,
) -> IpcResult<Vec<ModifierGroupView>> {
    let actor = authenticated(&state).await?;
    let product_id: ProductId = product_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state
        .service
        .list_modifier_groups(&actor, product_id)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_modifier_option(
    state: State<'_, AppState>,
    draft: ModifierOptionDraft,
) -> IpcResult<ModifierOptionView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_modifier_option(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_modifier_options(
    state: State<'_, AppState>,
    group_id: String,
) -> IpcResult<Vec<ModifierOptionView>> {
    let actor = authenticated(&state).await?;
    let group_id: pos_core::ids::ModifierGroupId = group_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state
        .service
        .list_modifier_options(&actor, group_id)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn configure_product(
    state: State<'_, AppState>,
    draft: ModifierConfigurationDraft,
) -> IpcResult<ModifierConfigurationView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.configure_product(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn checkout_sale(state: State<'_, AppState>, draft: CheckoutDraft) -> IpcResult<SaleView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.checkout_sale(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn find_sale_for_return(
    state: State<'_, AppState>,
    folio: String,
) -> IpcResult<ReturnableSaleView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.find_sale_for_return(&actor, folio).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn preview_sale_return(
    state: State<'_, AppState>,
    draft: SaleReturnPreviewDraft,
) -> IpcResult<SaleReturnPreviewView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.preview_sale_return(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_sale_return(
    state: State<'_, AppState>,
    draft: SaleReturnDraft,
) -> IpcResult<SaleReturnView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_sale_return(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_sale_returns(state: State<'_, AppState>) -> IpcResult<Vec<SaleReturnView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_sale_returns(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn reports_summary(
    state: State<'_, AppState>,
    query: ReportsQuery,
) -> IpcResult<ReportsView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.reports_summary(&actor, query).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn get_sale(state: State<'_, AppState>, sale_id: String) -> IpcResult<SaleView> {
    let actor = authenticated(&state).await?;
    let sale_id: pos_core::ids::SaleId = sale_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state.service.get_sale(&actor, sale_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn branch_summary(state: State<'_, AppState>) -> IpcResult<BranchView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.branch_summary(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_supplier(
    state: State<'_, AppState>,
    draft: SupplierDraft,
) -> IpcResult<SupplierView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_supplier(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_suppliers(state: State<'_, AppState>) -> IpcResult<Vec<SupplierView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_suppliers(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_purchase_order(
    state: State<'_, AppState>,
    draft: PurchaseOrderDraft,
) -> IpcResult<PurchaseOrderView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_purchase_order(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_purchase_orders(state: State<'_, AppState>) -> IpcResult<Vec<PurchaseOrderView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_purchase_orders(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn get_purchase_order(
    state: State<'_, AppState>,
    order_id: String,
) -> IpcResult<PurchaseOrderView> {
    let actor = authenticated(&state).await?;
    let order_id: PurchaseOrderId = order_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state.service.get_purchase_order(&actor, order_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn confirm_purchase_order(
    state: State<'_, AppState>,
    order_id: String,
) -> IpcResult<PurchaseOrderView> {
    let actor = authenticated(&state).await?;
    let order_id: PurchaseOrderId = order_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state
        .service
        .confirm_purchase_order(&actor, order_id)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn receive_purchase_order(
    state: State<'_, AppState>,
    draft: PurchaseReceiptDraft,
) -> IpcResult<PurchaseOrderView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.receive_purchase_order(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_expense_category(
    state: State<'_, AppState>,
    draft: ExpenseCategoryDraft,
) -> IpcResult<ExpenseCategoryView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_expense_category(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_expense_categories(
    state: State<'_, AppState>,
) -> IpcResult<Vec<ExpenseCategoryView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_expense_categories(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn record_expense(state: State<'_, AppState>, draft: ExpenseDraft) -> IpcResult<ExpenseView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.record_expense(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_expenses(state: State<'_, AppState>) -> IpcResult<Vec<ExpenseView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_expenses(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn record_waste(state: State<'_, AppState>, draft: WasteDraft) -> IpcResult<WasteView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.record_waste(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_waste(state: State<'_, AppState>) -> IpcResult<Vec<WasteView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_waste(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_cash_register(
    state: State<'_, AppState>,
    draft: CashRegisterDraft,
) -> IpcResult<CashRegisterView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_cash_register(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_cash_registers(state: State<'_, AppState>) -> IpcResult<Vec<CashRegisterView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_cash_registers(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn open_cash_shift(
    state: State<'_, AppState>,
    draft: CashOpenShiftDraft,
) -> IpcResult<CashShiftView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.open_cash_shift(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn active_cash_shift(
    state: State<'_, AppState>,
    register_id: String,
) -> IpcResult<Option<CashShiftView>> {
    let actor = authenticated(&state).await?;
    let register_id: pos_core::ids::CashRegisterId = register_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state.service.active_cash_shift(&actor, register_id).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn register_cash_movement(
    state: State<'_, AppState>,
    draft: CashMovementDraft,
) -> IpcResult<CashMovementView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.register_cash_movement(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_cash_shift_history(state: State<'_, AppState>) -> IpcResult<Vec<CashShiftView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_cash_shift_history(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn close_cash_shift(
    state: State<'_, AppState>,
    draft: CashCloseShiftDraft,
) -> IpcResult<CashShiftView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.close_cash_shift(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_category(
    state: State<'_, AppState>,
    draft: CategoryDraft,
) -> IpcResult<CategoryView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_category(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_categories(state: State<'_, AppState>) -> IpcResult<Vec<CategoryView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_categories(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_product_variant(
    state: State<'_, AppState>,
    draft: ProductVariantDraft,
) -> IpcResult<ProductVariantView> {
    let actor = authenticated(&state).await?;
    Ok(state.service.create_product_variant(&actor, draft).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_product_variants(
    state: State<'_, AppState>,
    product_id: String,
) -> IpcResult<Vec<ProductVariantView>> {
    let actor = authenticated(&state).await?;
    let product_id: ProductId = product_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    Ok(state
        .service
        .list_product_variants(&actor, product_id)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn current_stock(
    state: State<'_, AppState>,
    product_id: String,
    branch_id: Option<String>,
) -> IpcResult<String> {
    let actor = authenticated(&state).await?;
    let product_id: ProductId = product_id
        .parse()
        .map_err(|_| IpcError::from(AppError::NotFound))?;
    let branch_id: Option<BranchId> = branch_id
        .map(|value| {
            value
                .parse()
                .map_err(|_| IpcError::from(AppError::NotFound))
        })
        .transpose()?;
    Ok(state
        .service
        .current_stock(&actor, product_id, branch_id)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_stock_levels(state: State<'_, AppState>) -> IpcResult<Vec<StockLevelView>> {
    let actor = authenticated(&state).await?;
    Ok(state.service.list_stock_levels(&actor).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn record_inventory_movement(
    state: State<'_, AppState>,
    draft: InventoryMovementDraft,
) -> IpcResult<InventoryMovementView> {
    let actor = authenticated(&state).await?;
    Ok(state
        .service
        .record_inventory_movement(&actor, draft)
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_inventory_movements(
    state: State<'_, AppState>,
    product_id: Option<String>,
    limit: Option<u32>,
) -> IpcResult<Vec<InventoryMovementView>> {
    let actor = authenticated(&state).await?;
    let product_id: Option<ProductId> = product_id
        .map(|value| {
            value
                .parse()
                .map_err(|_| IpcError::from(AppError::NotFound))
        })
        .transpose()?;
    Ok(state
        .service
        .list_inventory_movements(&actor, product_id, limit.unwrap_or(50))
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_audit_events(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> IpcResult<Vec<AuditEventView>> {
    let actor = authenticated(&state).await?;
    Ok(state
        .service
        .list_audit(&actor, limit.unwrap_or(50))
        .await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn commercial_state(state: State<'_, AppState>) -> IpcResult<CommercialStateView> {
    Ok(state.service.commercial_state().await?)
}

/// Crea un respaldo consistente de la base local y lo registra en auditoría.
#[tauri::command(rename_all = "snake_case")]
async fn backup_database(state: State<'_, AppState>) -> IpcResult<String> {
    let actor = authenticated(&state).await?;
    actor.require(Permission::ManageBackups)?;

    let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
    let target = state
        .db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("backups")
        .join(format!("hiqnet-pos-{stamp}.db"));

    let store = SqliteStore::new(state.pool.clone());
    pos_storage_sqlite::backup_to(store.pool(), &target)
        .await
        .map_err(|_| AppError::Storage)?;
    state
        .service
        .record_backup(&actor, &target.to_string_lossy())
        .await?;
    Ok(target.to_string_lossy().to_string())
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let db_path = automation_database_path()
                .or_else(|| std::env::var_os("HIQNET_POS_DB_PATH").map(PathBuf::from))
                .unwrap_or_else(pos_storage_sqlite::default_database_path);

            let pool = tauri::async_runtime::block_on(pos_storage_sqlite::open(&db_path))
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;

            let store: SharedStore = Arc::new(SqliteStore::new(pool.clone()));
            let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
            let clock: SharedClock = Arc::new(SystemClock);
            let service = PosService::new(store, hasher, clock);

            app.manage(AppState {
                db_path,
                pool,
                service,
                session: RwLock::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            bootstrap,
            login,
            logout,
            current_session,
            checkout_sale,
            get_sale,
            find_sale_for_return,
            preview_sale_return,
            create_sale_return,
            list_sale_returns,
            reports_summary,
            branch_summary,
            create_user,
            list_users,
            set_user_active,
            reset_user_password,
            change_password,
            create_supplier,
            list_suppliers,
            create_purchase_order,
            list_purchase_orders,
            get_purchase_order,
            confirm_purchase_order,
            receive_purchase_order,
            create_expense_category,
            list_expense_categories,
            record_expense,
            list_expenses,
            record_waste,
            list_waste,
            create_cash_register,
            list_cash_registers,
            open_cash_shift,
            active_cash_shift,
            register_cash_movement,
            list_cash_shift_history,
            close_cash_shift,
            create_product,
            list_products,
            assign_product_category,
            create_category,
            create_modifier_group,
            create_modifier_option,
            list_categories,
            list_modifier_groups,
            list_modifier_options,
            configure_product,
            create_product_variant,
            list_product_variants,
            current_stock,
            list_stock_levels,
            record_inventory_movement,
            list_inventory_movements,
            list_audit_events,
            commercial_state,
            backup_database
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar HiQNet POS");
}
