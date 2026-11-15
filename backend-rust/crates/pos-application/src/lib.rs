//! Casos de uso, puertos y autorización de HiQNet POS.
//!
//! La capa de aplicación orquesta el dominio y las transacciones, pero no
//! conoce SQL, Tauri ni React.

pub mod error;
pub mod ports;
pub mod service;

pub use error::{AppError, AppResult, ErrorView};
pub use ports::{
    BeginPlatformEnrollment, CashReportTotals, CatalogConfiguration, Clock, CommercialBinding,
    DeviceSigner, EnrollmentSession, EnrollmentSessionStatus, ExpenseCategoryReportRow,
    ExpensesReportTotals, InstallationState, InventoryReportTotals, LicenseVerifier,
    PasswordHasher, PlatformChallenge, PlatformEnrollmentBinding, PlatformEnrollmentClient,
    PlatformProof, ProductSalesReportRow, PurchasesReportTotals, ReturnedTotals,
    ReturnsReportTotals, SalesReportTotals, SharedClock, SharedDeviceSigner, SharedHasher,
    SharedLicenseVerifier, SharedPlatformEnrollmentClient, SharedStore, Store, StoreTx,
    SystemClock, VerifiedLicense,
};
pub use service::{
    AppStatus, AuditEventView, BootstrapRequest, BootstrapResponse, BranchView,
    CashCloseShiftDraft, CashMovementDraft, CashMovementView, CashOpenShiftDraft,
    CashRegisterDraft, CashRegisterView, CashShiftView, CategoryDraft, CategoryView,
    ChangeOwnPasswordDraft, CheckoutDraft, CommercialStateView, ExpenseCategoryDraft,
    ExpenseCategoryView, ExpenseDraft, ExpenseView, InventoryMovementDraft, InventoryMovementView,
    LoginRequest, ModifierConfigurationDraft, ModifierConfigurationView, ModifierGroupDraft,
    ModifierGroupView, ModifierOptionDraft, ModifierOptionView, PlatformEnrollmentView, PosService,
    ProductDraft, ProductPricePreviewView, ProductVariantDraft, ProductVariantView, ProductView,
    PurchaseOrderDraft, PurchaseOrderLineDraft, PurchaseOrderLineView, PurchaseOrderView,
    PurchaseReceiptDraft, PurchaseReceiptLineDraft, ReportsCashView, ReportsExpenseCategoryView,
    ReportsExpensesView, ReportsInventoryView, ReportsProductView, ReportsPurchasesView,
    ReportsQuery, ReportsSalesView, ReportsView, ResetUserPasswordDraft, ReturnableSaleItemView,
    ReturnableSaleView, SaleItemDraft, SaleItemView, SalePaymentView, SaleReturnDraft,
    SaleReturnLineDraft, SaleReturnLineView, SaleReturnPreviewDraft, SaleReturnPreviewLineView,
    SaleReturnPreviewView, SaleReturnView, SaleView, SetUserActiveDraft, StockLevelView,
    SupplierDraft, SupplierView, UserDraft, UserView, WasteDraft, WasteView,
    normalize_platform_url,
};
