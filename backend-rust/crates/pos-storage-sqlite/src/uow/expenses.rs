//! Gastos y merma.

use super::*;

pub(super) async fn insert_expense_category(
    tx: &mut SqliteTx,
    category: &ExpenseCategory,
) -> AppResult<()> {
    sqlx::query("INSERT INTO expense_category (id, organization_id, name, active, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(category.id.to_string()).bind(category.organization_id.to_string()).bind(&category.name).bind(i64::from(category.active)).bind(format_ts(category.created_at))
            .execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_expense_categories(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<Vec<ExpenseCategory>> {
    let rows = sqlx::query("SELECT id, organization_id, name, active, created_at FROM expense_category WHERE organization_id = ? ORDER BY name COLLATE NOCASE")
            .bind(organization_id.to_string()).fetch_all(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    rows.into_iter().map(expense_category_from_row).collect()
}

pub(super) async fn find_expense_category(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    category_id: ExpenseCategoryId,
) -> AppResult<Option<ExpenseCategory>> {
    sqlx::query("SELECT id, organization_id, name, active, created_at FROM expense_category WHERE organization_id = ? AND id = ?")
            .bind(organization_id.to_string()).bind(category_id.to_string()).fetch_optional(&mut **tx.tx()?).await.map_err(map_sqlx)?.map(expense_category_from_row).transpose()
}

pub(super) async fn find_expense_by_idempotency(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    idempotency_key: &str,
) -> AppResult<Option<Expense>> {
    sqlx::query("SELECT id, organization_id, branch_id, category_id, amount_minor, concept, cash_shift_id, cash_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at FROM expense WHERE organization_id = ? AND idempotency_key = ?")
            .bind(organization_id.to_string()).bind(idempotency_key).fetch_optional(&mut **tx.tx()?).await.map_err(map_sqlx)?.map(expense_from_row).transpose()
}

pub(super) async fn insert_expense(tx: &mut SqliteTx, expense: &Expense) -> AppResult<bool> {
    let result = sqlx::query("INSERT INTO expense (id, organization_id, branch_id, category_id, amount_minor, concept, cash_shift_id, cash_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (organization_id, idempotency_key) DO NOTHING")
            .bind(expense.id.to_string()).bind(expense.organization_id.to_string()).bind(expense.branch_id.to_string()).bind(expense.category_id.to_string()).bind(expense.amount.minor_units()).bind(&expense.concept).bind(expense.cash_shift_id.map(|id| id.to_string())).bind(expense.cash_movement_id.map(|id| id.to_string())).bind(&expense.idempotency_key).bind(&expense.request_fingerprint).bind(expense.created_by.to_string()).bind(format_ts(expense.occurred_at)).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn list_expenses(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<Expense>> {
    let rows = sqlx::query("SELECT id, organization_id, branch_id, category_id, amount_minor, concept, cash_shift_id, cash_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at FROM expense WHERE organization_id = ? AND branch_id = ? ORDER BY occurred_at DESC")
            .bind(organization_id.to_string()).bind(branch_id.to_string()).fetch_all(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    rows.into_iter().map(expense_from_row).collect()
}

pub(super) async fn find_waste_by_idempotency(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    idempotency_key: &str,
) -> AppResult<Option<WasteRecord>> {
    sqlx::query("SELECT id, organization_id, branch_id, product_id, quantity_milli, reason, inventory_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at FROM waste_record WHERE organization_id = ? AND idempotency_key = ?")
            .bind(organization_id.to_string()).bind(idempotency_key).fetch_optional(&mut **tx.tx()?).await.map_err(map_sqlx)?.map(waste_from_row).transpose()
}

pub(super) async fn insert_waste(tx: &mut SqliteTx, waste: &WasteRecord) -> AppResult<bool> {
    let result = sqlx::query("INSERT INTO waste_record (id, organization_id, branch_id, product_id, quantity_milli, reason, inventory_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (organization_id, idempotency_key) DO NOTHING")
            .bind(waste.id.to_string()).bind(waste.organization_id.to_string()).bind(waste.branch_id.to_string()).bind(waste.product_id.to_string()).bind(waste.quantity.milli_units()).bind(&waste.reason).bind(waste.inventory_movement_id.to_string()).bind(&waste.idempotency_key).bind(&waste.request_fingerprint).bind(waste.created_by.to_string()).bind(format_ts(waste.occurred_at)).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn list_waste(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<WasteRecord>> {
    let rows = sqlx::query("SELECT id, organization_id, branch_id, product_id, quantity_milli, reason, inventory_movement_id, idempotency_key, request_fingerprint, created_by, occurred_at FROM waste_record WHERE organization_id = ? AND branch_id = ? ORDER BY occurred_at DESC")
            .bind(organization_id.to_string()).bind(branch_id.to_string()).fetch_all(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    rows.into_iter().map(waste_from_row).collect()
}
