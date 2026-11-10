//! Fachada `StoreTx`: delega cada operacion en su modulo de dominio.
//!
//! Rust no permite repartir un `impl` entre modulos, asi que este archivo
//! conserva el contrato completo y el resto vive en modulos por agregado.

use super::*;

#[async_trait]
impl StoreTx for SqliteTx {
    async fn installation_state(&mut self) -> AppResult<Option<InstallationState>> {
        organization::installation_state(self).await
    }

    async fn save_installation_state(&mut self, state: &InstallationState) -> AppResult<()> {
        organization::save_installation_state(self, state).await
    }

    async fn insert_organization(&mut self, organization: &Organization) -> AppResult<()> {
        organization::insert_organization(self, organization).await
    }

    async fn insert_branch(&mut self, branch: &Branch) -> AppResult<()> {
        organization::insert_branch(self, branch).await
    }

    async fn find_branch(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Option<Branch>> {
        organization::find_branch(self, organization_id, branch_id).await
    }

    async fn insert_user(&mut self, user: &User) -> AppResult<()> {
        users::insert_user(self, user).await
    }

    async fn find_user_by_username(&mut self, username: &str) -> AppResult<Option<User>> {
        users::find_user_by_username(self, username).await
    }

    async fn find_user_by_id(
        &mut self,
        organization_id: OrganizationId,
        user_id: UserId,
    ) -> AppResult<Option<User>> {
        users::find_user_by_id(self, organization_id, user_id).await
    }

    async fn list_users(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<User>> {
        users::list_users(self, organization_id, branch_id).await
    }

    async fn update_user_active(
        &mut self,
        organization_id: OrganizationId,
        user_id: UserId,
        active: bool,
    ) -> AppResult<()> {
        users::update_user_active(self, organization_id, user_id, active).await
    }

    async fn update_user_password(
        &mut self,
        organization_id: OrganizationId,
        user_id: UserId,
        password_hash: &str,
    ) -> AppResult<()> {
        users::update_user_password(self, organization_id, user_id, password_hash).await
    }

    async fn count_active_owners(&mut self, organization_id: OrganizationId) -> AppResult<i64> {
        users::count_active_owners(self, organization_id).await
    }

    async fn count_users(&mut self, organization_id: OrganizationId) -> AppResult<i64> {
        users::count_users(self, organization_id).await
    }

    async fn insert_product(&mut self, product: &Product) -> AppResult<()> {
        catalog::insert_product(self, product).await
    }

    async fn list_products(&mut self, organization_id: OrganizationId) -> AppResult<Vec<Product>> {
        catalog::list_products(self, organization_id).await
    }

    async fn list_catalog_configuration(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<CatalogConfiguration> {
        catalog::list_catalog_configuration(self, organization_id).await
    }

    async fn insert_category(&mut self, category: &Category) -> AppResult<()> {
        catalog::insert_category(self, category).await
    }

    async fn find_category(
        &mut self,
        organization_id: OrganizationId,
        category_id: CategoryId,
    ) -> AppResult<Option<Category>> {
        catalog::find_category(self, organization_id, category_id).await
    }

    async fn update_product_category(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
        category_id: Option<CategoryId>,
    ) -> AppResult<()> {
        catalog::update_product_category(self, organization_id, product_id, category_id).await
    }

    async fn list_categories(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<Vec<Category>> {
        catalog::list_categories(self, organization_id).await
    }

    async fn insert_product_variant(&mut self, variant: &ProductVariant) -> AppResult<()> {
        catalog::insert_product_variant(self, variant).await
    }

    async fn list_product_variants(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Vec<ProductVariant>> {
        catalog::list_product_variants(self, organization_id, product_id).await
    }

    async fn find_product(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Option<Product>> {
        catalog::find_product(self, organization_id, product_id).await
    }

    async fn insert_supplier(&mut self, supplier: &Supplier) -> AppResult<()> {
        purchasing::insert_supplier(self, supplier).await
    }

    async fn list_suppliers(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<Vec<Supplier>> {
        purchasing::list_suppliers(self, organization_id).await
    }

    async fn find_supplier(
        &mut self,
        organization_id: OrganizationId,
        supplier_id: SupplierId,
    ) -> AppResult<Option<Supplier>> {
        purchasing::find_supplier(self, organization_id, supplier_id).await
    }

    async fn insert_purchase_order(&mut self, order: &PurchaseOrder) -> AppResult<()> {
        purchasing::insert_purchase_order(self, order).await
    }

    async fn insert_purchase_order_line(&mut self, line: &PurchaseOrderLine) -> AppResult<()> {
        purchasing::insert_purchase_order_line(self, line).await
    }

    async fn find_purchase_order(
        &mut self,
        organization_id: OrganizationId,
        order_id: PurchaseOrderId,
    ) -> AppResult<Option<PurchaseOrder>> {
        purchasing::find_purchase_order(self, organization_id, order_id).await
    }

    async fn list_purchase_orders(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<PurchaseOrder>> {
        purchasing::list_purchase_orders(self, organization_id, branch_id).await
    }

    async fn list_purchase_order_lines(
        &mut self,
        organization_id: OrganizationId,
        order_id: PurchaseOrderId,
    ) -> AppResult<Vec<PurchaseOrderLine>> {
        purchasing::list_purchase_order_lines(self, organization_id, order_id).await
    }

    async fn confirm_purchase_order(
        &mut self,
        organization_id: OrganizationId,
        order_id: PurchaseOrderId,
    ) -> AppResult<()> {
        purchasing::confirm_purchase_order(self, organization_id, order_id).await
    }

    async fn find_purchase_receipt_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<PurchaseReceipt>> {
        purchasing::find_purchase_receipt_by_idempotency(self, organization_id, idempotency_key)
            .await
    }

    async fn insert_purchase_receipt(&mut self, receipt: &PurchaseReceipt) -> AppResult<bool> {
        purchasing::insert_purchase_receipt(self, receipt).await
    }

    async fn insert_purchase_receipt_line(
        &mut self,
        receipt_id: PurchaseReceiptId,
        order_line_id: PurchaseOrderLineId,
        quantity: Quantity,
    ) -> AppResult<()> {
        purchasing::insert_purchase_receipt_line(self, receipt_id, order_line_id, quantity).await
    }

    async fn apply_purchase_receipt(
        &mut self,
        organization_id: OrganizationId,
        order_id: PurchaseOrderId,
        updates: &[(PurchaseOrderLineId, Quantity)],
        status: PurchaseOrderStatus,
    ) -> AppResult<()> {
        purchasing::apply_purchase_receipt(self, organization_id, order_id, updates, status).await
    }

    async fn insert_expense_category(&mut self, category: &ExpenseCategory) -> AppResult<()> {
        expenses::insert_expense_category(self, category).await
    }

    async fn list_expense_categories(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<Vec<ExpenseCategory>> {
        expenses::list_expense_categories(self, organization_id).await
    }

    async fn find_expense_category(
        &mut self,
        organization_id: OrganizationId,
        category_id: ExpenseCategoryId,
    ) -> AppResult<Option<ExpenseCategory>> {
        expenses::find_expense_category(self, organization_id, category_id).await
    }

    async fn find_expense_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<Expense>> {
        expenses::find_expense_by_idempotency(self, organization_id, idempotency_key).await
    }

    async fn insert_expense(&mut self, expense: &Expense) -> AppResult<bool> {
        expenses::insert_expense(self, expense).await
    }

    async fn list_expenses(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<Expense>> {
        expenses::list_expenses(self, organization_id, branch_id).await
    }

    async fn find_waste_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<WasteRecord>> {
        expenses::find_waste_by_idempotency(self, organization_id, idempotency_key).await
    }

    async fn insert_waste(&mut self, waste: &WasteRecord) -> AppResult<bool> {
        expenses::insert_waste(self, waste).await
    }

    async fn list_waste(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<WasteRecord>> {
        expenses::list_waste(self, organization_id, branch_id).await
    }

    async fn find_sale_by_folio(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        folio: &str,
    ) -> AppResult<Option<Sale>> {
        returns::find_sale_by_folio(self, organization_id, branch_id, folio).await
    }

    async fn returned_totals(
        &mut self,
        organization_id: OrganizationId,
        sale_item_id: pos_core::ids::SaleItemId,
    ) -> AppResult<ReturnedTotals> {
        returns::returned_totals(self, organization_id, sale_item_id).await
    }

    async fn insert_sale_return(&mut self, value: &SaleReturn) -> AppResult<bool> {
        returns::insert_sale_return(self, value).await
    }

    async fn insert_sale_return_line(&mut self, line: &SaleReturnLine) -> AppResult<()> {
        returns::insert_sale_return_line(self, line).await
    }

    async fn find_sale_return_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<SaleReturn>> {
        returns::find_sale_return_by_idempotency(self, organization_id, idempotency_key).await
    }

    async fn list_sale_return_lines(
        &mut self,
        organization_id: OrganizationId,
        return_id: SaleReturnId,
    ) -> AppResult<Vec<SaleReturnLine>> {
        returns::list_sale_return_lines(self, organization_id, return_id).await
    }

    async fn list_sale_returns(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        limit: i64,
    ) -> AppResult<Vec<SaleReturn>> {
        returns::list_sale_returns(self, organization_id, branch_id, limit).await
    }

    async fn sales_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<SalesReportTotals> {
        reporting::sales_report_totals(self, organization_id, branch_id, from, to).await
    }

    async fn sales_report_by_product(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<Vec<ProductSalesReportRow>> {
        reporting::sales_report_by_product(self, organization_id, branch_id, from, to).await
    }

    async fn returns_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<ReturnsReportTotals> {
        reporting::returns_report_totals(self, organization_id, branch_id, from, to).await
    }

    async fn cash_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<CashReportTotals> {
        reporting::cash_report_totals(self, organization_id, branch_id, from, to).await
    }

    async fn purchases_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<PurchasesReportTotals> {
        reporting::purchases_report_totals(self, organization_id, branch_id, from, to).await
    }

    async fn expenses_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<ExpensesReportTotals> {
        reporting::expenses_report_totals(self, organization_id, branch_id, from, to).await
    }

    async fn inventory_report_totals(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> AppResult<InventoryReportTotals> {
        reporting::inventory_report_totals(self, organization_id, branch_id, from, to).await
    }

    async fn insert_inventory_movement(&mut self, movement: &InventoryMovement) -> AppResult<()> {
        inventory::insert_inventory_movement(self, movement).await
    }

    async fn current_stock(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: ProductId,
    ) -> AppResult<Quantity> {
        inventory::current_stock(self, organization_id, branch_id, product_id).await
    }

    async fn stock_levels(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<(ProductId, Quantity)>> {
        inventory::stock_levels(self, organization_id, branch_id).await
    }

    async fn list_inventory_movements(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: Option<ProductId>,
        limit: i64,
    ) -> AppResult<Vec<InventoryMovement>> {
        inventory::list_inventory_movements(self, organization_id, branch_id, product_id, limit)
            .await
    }

    async fn insert_modifier_group(&mut self, group: &ModifierGroup) -> AppResult<()> {
        modifiers::insert_modifier_group(self, group).await
    }

    async fn find_modifier_group(
        &mut self,
        organization_id: OrganizationId,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Option<ModifierGroup>> {
        modifiers::find_modifier_group(self, organization_id, group_id).await
    }

    async fn list_modifier_groups(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Vec<ModifierGroup>> {
        modifiers::list_modifier_groups(self, organization_id, product_id).await
    }

    async fn insert_modifier_option(&mut self, option: &ModifierOption) -> AppResult<()> {
        modifiers::insert_modifier_option(self, option).await
    }

    async fn find_modifier_option(
        &mut self,
        organization_id: OrganizationId,
        option_id: pos_core::ids::ModifierOptionId,
    ) -> AppResult<Option<ModifierOption>> {
        modifiers::find_modifier_option(self, organization_id, option_id).await
    }

    async fn list_modifier_options(
        &mut self,
        organization_id: OrganizationId,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Vec<ModifierOption>> {
        modifiers::list_modifier_options(self, organization_id, group_id).await
    }

    async fn insert_cash_register(&mut self, register: &CashRegister) -> AppResult<()> {
        cash::insert_cash_register(self, register).await
    }

    async fn list_cash_registers(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<CashRegister>> {
        cash::list_cash_registers(self, organization_id, branch_id).await
    }

    async fn insert_cash_shift(&mut self, shift: &CashShift) -> AppResult<()> {
        cash::insert_cash_shift(self, shift).await
    }

    async fn find_open_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        register_id: pos_core::ids::CashRegisterId,
    ) -> AppResult<Option<CashShift>> {
        cash::find_open_cash_shift(self, organization_id, register_id).await
    }

    async fn find_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
    ) -> AppResult<Option<CashShift>> {
        cash::find_cash_shift(self, organization_id, shift_id).await
    }

    async fn close_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
        closed_by: UserId,
        closed_at: time::OffsetDateTime,
        expected_amount: Money,
        counted_amount: Money,
        difference_minor: i64,
    ) -> AppResult<()> {
        cash::close_cash_shift(
            self,
            organization_id,
            shift_id,
            closed_by,
            closed_at,
            expected_amount,
            counted_amount,
            difference_minor,
        )
        .await
    }

    async fn insert_cash_movement(&mut self, movement: &CashMovement) -> AppResult<()> {
        cash::insert_cash_movement(self, movement).await
    }

    async fn list_cash_movements(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
    ) -> AppResult<Vec<CashMovement>> {
        cash::list_cash_movements(self, organization_id, shift_id).await
    }

    async fn list_closed_cash_shifts(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<CashShift>> {
        cash::list_closed_cash_shifts(self, organization_id, branch_id).await
    }

    async fn insert_sale(&mut self, sale: &Sale) -> AppResult<()> {
        sales::insert_sale(self, sale).await
    }

    async fn insert_sale_item(&mut self, item: &SaleItem) -> AppResult<()> {
        sales::insert_sale_item(self, item).await
    }

    async fn insert_sale_payment(&mut self, payment: &SalePayment) -> AppResult<()> {
        sales::insert_sale_payment(self, payment).await
    }

    async fn find_sale_by_idempotency(
        &mut self,
        organization_id: OrganizationId,
        idempotency_key: &str,
    ) -> AppResult<Option<Sale>> {
        sales::find_sale_by_idempotency(self, organization_id, idempotency_key).await
    }

    async fn find_sale(
        &mut self,
        organization_id: OrganizationId,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<Option<Sale>> {
        sales::find_sale(self, organization_id, sale_id).await
    }

    async fn list_sale_items(
        &mut self,
        organization_id: OrganizationId,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<Vec<SaleItem>> {
        sales::list_sale_items(self, organization_id, sale_id).await
    }

    async fn list_sale_payments(
        &mut self,
        organization_id: OrganizationId,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<Vec<SalePayment>> {
        sales::list_sale_payments(self, organization_id, sale_id).await
    }

    async fn append_audit(&mut self, event: &AuditEvent) -> AppResult<()> {
        audit::append_audit(self, event).await
    }

    async fn list_audit(
        &mut self,
        organization_id: OrganizationId,
        limit: i64,
    ) -> AppResult<Vec<AuditEvent>> {
        audit::list_audit(self, organization_id, limit).await
    }

    async fn commit(self: Box<Self>) -> AppResult<()> {
        let mut this = self;
        let tx = this.tx.take().ok_or(AppError::Storage)?;
        tx.commit().await.map_err(map_sqlx)
    }

    async fn rollback(self: Box<Self>) -> AppResult<()> {
        let mut this = self;
        let tx = this.tx.take().ok_or(AppError::Storage)?;
        tx.rollback().await.map_err(map_sqlx)
    }
}
