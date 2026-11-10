//! Fixtures y utilidades compartidas por los escenarios de devoluciones.
//!
//! Se reexportan los tipos para que cada escenario solo importe este modulo.

pub use pos_application::{
    BootstrapRequest, CashCloseShiftDraft, CashMovementDraft, CashOpenShiftDraft,
    CashRegisterDraft, CheckoutDraft, ExpenseCategoryDraft, ExpenseDraft, InventoryMovementDraft,
    LoginRequest, PosService, ProductDraft, PurchaseOrderDraft, PurchaseOrderLineDraft,
    PurchaseReceiptDraft, PurchaseReceiptLineDraft, ReportsQuery, SaleItemDraft, SaleReturnDraft,
    SaleReturnLineDraft, SaleReturnPreviewDraft, SharedClock, SharedHasher, SharedStore,
    SupplierDraft, WasteDraft,
};
pub use pos_core::identity::{Actor, Role};
pub use pos_core::ids::{BranchId, OrganizationId};
pub use pos_security::Argon2idHasher;
pub use pos_storage_sqlite::{SqliteStore, open};
pub use sqlx::Row;
pub use std::sync::Arc;
pub use time::OffsetDateTime;

pub struct FixedClock(pub OffsetDateTime);
impl pos_application::Clock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

pub fn build_service_with_clock(pool: sqlx::SqlitePool, now: OffsetDateTime) -> PosService {
    PosService::new(
        Arc::new(SqliteStore::new(pool)) as SharedStore,
        Arc::new(Argon2idHasher::new()) as SharedHasher,
        Arc::new(FixedClock(now)) as SharedClock,
    )
}

pub fn build_service(pool: sqlx::SqlitePool) -> PosService {
    build_service_with_clock(pool, OffsetDateTime::now_utc())
}

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub pool: sqlx::SqlitePool,
    pub service: PosService,
    pub actor: Actor,
    pub product_id: String,
    pub register_id: String,
    pub shift_id: String,
}

pub async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path().join("returns.db")).await.unwrap();
    let service = build_service(pool.clone());
    service
        .bootstrap(BootstrapRequest {
            organization_name: "Devoluciones SA".into(),
            branch_name: "Matriz".into(),
            branch_code: "MX-01".into(),
            owner_username: "owner".into(),
            owner_display_name: "Owner".into(),
            owner_password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let product = service
        .create_product(
            &actor,
            ProductDraft {
                sku: "CAF-001".into(),
                name: "Café".into(),
                price: "10.00".into(),
                tax_rate_bp: 1600,
                category_id: None,
            },
        )
        .await
        .unwrap();
    service
        .record_inventory_movement(
            &actor,
            InventoryMovementDraft {
                product_id: product.id.to_string(),
                kind: "inbound".into(),
                quantity: "20".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let register = service
        .create_cash_register(
            &actor,
            CashRegisterDraft {
                name: "Caja".into(),
                code: "CJ-01".into(),
            },
        )
        .await
        .unwrap();
    let shift = service
        .open_cash_shift(
            &actor,
            CashOpenShiftDraft {
                register_id: register.id.clone(),
                opening_amount: "100.00".into(),
            },
        )
        .await
        .unwrap();
    Fixture {
        dir,
        pool,
        service,
        actor,
        product_id: product.id.to_string(),
        register_id: register.id,
        shift_id: shift.id,
    }
}

pub async fn sell(fixture: &Fixture, quantity: &str) -> pos_application::SaleView {
    fixture
        .service
        .checkout_sale(
            &fixture.actor,
            CheckoutDraft {
                shift_id: fixture.shift_id.clone(),
                idempotency_key: format!("sale-{quantity}"),
                items: vec![SaleItemDraft {
                    product_id: fixture.product_id.clone(),
                    variant_id: None,
                    quantity: quantity.into(),
                    selected_option_ids: Vec::new(),
                }],
                cash_received: "50.00".into(),
            },
        )
        .await
        .unwrap()
}

pub fn return_draft(
    sale: &pos_application::SaleView,
    shift_id: &str,
    key: &str,
    quantity: &str,
    disposition: &str,
) -> SaleReturnDraft {
    SaleReturnDraft {
        sale_id: sale.id.clone(),
        shift_id: Some(shift_id.into()),
        reason: "producto en buen estado".into(),
        idempotency_key: key.into(),
        lines: vec![SaleReturnLineDraft {
            sale_item_id: sale.items[0].id.clone(),
            quantity: quantity.into(),
            disposition: disposition.into(),
        }],
    }
}

pub async fn stock(fixture: &Fixture) -> String {
    fixture
        .service
        .current_stock(&fixture.actor, fixture.product_id.parse().unwrap(), None)
        .await
        .unwrap()
}

pub async fn new_product(fixture: &Fixture, sku: &str, price: &str, tax_rate_bp: u16) -> String {
    fixture
        .service
        .create_product(
            &fixture.actor,
            ProductDraft {
                sku: sku.into(),
                name: format!("Producto {sku}"),
                price: price.into(),
                tax_rate_bp,
                category_id: None,
            },
        )
        .await
        .unwrap()
        .id
        .to_string()
}

pub async fn add_stock(fixture: &Fixture, product_id: &str, quantity: &str) {
    fixture
        .service
        .record_inventory_movement(
            &fixture.actor,
            InventoryMovementDraft {
                product_id: product_id.into(),
                kind: "inbound".into(),
                quantity: quantity.into(),
                reason: None,
            },
        )
        .await
        .unwrap();
}

pub async fn sell_product(
    service: &PosService,
    actor: &Actor,
    shift_id: &str,
    product_id: &str,
    quantity: &str,
    cash_received: &str,
    key: &str,
) -> pos_application::SaleView {
    service
        .checkout_sale(
            actor,
            CheckoutDraft {
                shift_id: shift_id.into(),
                idempotency_key: key.into(),
                items: vec![SaleItemDraft {
                    product_id: product_id.into(),
                    variant_id: None,
                    quantity: quantity.into(),
                    selected_option_ids: Vec::new(),
                }],
                cash_received: cash_received.into(),
            },
        )
        .await
        .unwrap()
}

pub fn money_text(minor: i64) -> String {
    format!("{}.{:02}", minor / 100, minor % 100)
}

pub async fn report_between(
    service: &PosService,
    actor: &Actor,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> pos_application::ReportsView {
    service
        .reports_summary(
            actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap()
}

/// Vista auxiliar para comparar importes exactos en las pruebas.
pub struct MoneyView(pub i64);

impl std::str::FromStr for MoneyView {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (units, cents) = value.split_once('.').ok_or(())?;
        let units: i64 = units.parse().map_err(|_| ())?;
        let cents: i64 = cents.parse().map_err(|_| ())?;
        Ok(MoneyView(units * 100 + cents))
    }
}
