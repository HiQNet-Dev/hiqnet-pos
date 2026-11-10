//! Utilidades de SQLite: formato de valores y mapeo de filas.

use super::*;

pub(super) fn format_ts(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_default()
}

pub(super) fn parse_ts(value: String) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| AppError::Storage)
}

pub(super) fn parse_id<T: FromStr>(value: String) -> AppResult<T> {
    value.parse::<T>().map_err(|_| AppError::Storage)
}

pub(super) fn parse_optional_id<T: FromStr>(value: Option<String>) -> AppResult<Option<T>> {
    value
        .map(|raw| raw.parse::<T>().map_err(|_| AppError::Storage))
        .transpose()
}

pub(super) fn user_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<User> {
    let role = Role::from_key(&row.get::<String, _>("role")).ok_or(AppError::Storage)?;
    Ok(User {
        id: parse_id(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_optional_id(row.get("branch_id"))?,
        username: row.get("username"),
        display_name: row.get("display_name"),
        role,
        password_hash: row.get("password_hash"),
        active: row.get::<i64, _>("active") != 0,
        created_at: parse_ts(row.get("created_at"))?,
    })
}

pub(super) fn purchase_order_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<PurchaseOrder> {
    Ok(PurchaseOrder {
        id: parse_id(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_id(row.get("branch_id"))?,
        supplier_id: parse_id(row.get("supplier_id"))?,
        folio: row.get("folio"),
        status: PurchaseOrderStatus::from_key(&row.get::<String, _>("status"))
            .ok_or(AppError::Storage)?,
        created_by: parse_id(row.get("created_by"))?,
        created_at: parse_ts(row.get("created_at"))?,
    })
}

pub(super) fn purchase_order_line_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<PurchaseOrderLine> {
    Ok(PurchaseOrderLine {
        id: parse_id(row.get("id"))?,
        order_id: parse_id(row.get("order_id"))?,
        product_id: parse_id(row.get("product_id"))?,
        requested_quantity: Quantity::from_milli_units(row.get("requested_quantity_milli"))
            .map_err(AppError::Domain)?,
        received_quantity: Quantity::from_milli_units(row.get("received_quantity_milli"))
            .map_err(AppError::Domain)?,
        unit_cost: Money::from_minor_units(row.get("unit_cost_minor")).map_err(AppError::Domain)?,
    })
}

pub(super) fn expense_category_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<ExpenseCategory> {
    Ok(ExpenseCategory {
        id: parse_id(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        name: row.get("name"),
        active: row.get::<i64, _>("active") != 0,
        created_at: parse_ts(row.get("created_at"))?,
    })
}

pub(super) fn expense_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<Expense> {
    Ok(Expense {
        id: parse_id::<ExpenseId>(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_id(row.get("branch_id"))?,
        category_id: parse_id(row.get("category_id"))?,
        amount: Money::from_minor_units(row.get("amount_minor"))?,
        concept: row.get("concept"),
        cash_shift_id: row
            .get::<Option<String>, _>("cash_shift_id")
            .map(parse_id)
            .transpose()?,
        cash_movement_id: row
            .get::<Option<String>, _>("cash_movement_id")
            .map(parse_id)
            .transpose()?,
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint: row.get("request_fingerprint"),
        created_by: parse_id(row.get("created_by"))?,
        occurred_at: parse_ts(row.get("occurred_at"))?,
    })
}

pub(super) fn waste_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<WasteRecord> {
    Ok(WasteRecord {
        id: parse_id::<WasteId>(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_id(row.get("branch_id"))?,
        product_id: parse_id(row.get("product_id"))?,
        quantity: Quantity::from_milli_units(row.get("quantity_milli"))?,
        reason: row.get("reason"),
        inventory_movement_id: parse_id(row.get("inventory_movement_id"))?,
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint: row.get("request_fingerprint"),
        created_by: parse_id(row.get("created_by"))?,
        occurred_at: parse_ts(row.get("occurred_at"))?,
    })
}

pub(super) fn sale_return_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<SaleReturn> {
    Ok(SaleReturn {
        id: parse_id::<SaleReturnId>(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_id(row.get("branch_id"))?,
        sale_id: parse_id(row.get("sale_id"))?,
        shift_id: parse_optional_id(row.get("shift_id"))?,
        cash_movement_id: parse_optional_id(row.get("cash_movement_id"))?,
        operator_id: parse_id(row.get("operator_id"))?,
        reason: row.get("reason"),
        refund_total: Money::from_minor_units(row.get("refund_total_minor"))
            .map_err(AppError::Domain)?,
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint: row.get("request_fingerprint"),
        occurred_at: parse_ts(row.get("occurred_at"))?,
    })
}

pub(super) fn sale_return_line_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<SaleReturnLine> {
    Ok(SaleReturnLine {
        id: parse_id::<SaleReturnLineId>(row.get("id"))?,
        return_id: parse_id(row.get("return_id"))?,
        sale_item_id: parse_id(row.get("sale_item_id"))?,
        quantity: Quantity::from_milli_units(row.get("quantity_milli"))?,
        disposition: ReturnDisposition::from_key(&row.get::<String, _>("disposition"))
            .ok_or(AppError::Storage)?,
        refund_net: Money::from_minor_units(row.get("refund_net_minor"))
            .map_err(AppError::Domain)?,
        refund_tax: Money::from_minor_units(row.get("refund_tax_minor"))
            .map_err(AppError::Domain)?,
        refund_total: Money::from_minor_units(row.get("refund_total_minor"))
            .map_err(AppError::Domain)?,
        inbound_movement_id: parse_optional_id(row.get("inbound_movement_id"))?,
    })
}

pub(super) fn purchase_receipt_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<PurchaseReceipt> {
    Ok(PurchaseReceipt {
        id: parse_id(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_id(row.get("branch_id"))?,
        order_id: parse_id(row.get("order_id"))?,
        received_by: parse_id(row.get("received_by"))?,
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint: row.get("request_fingerprint"),
        received_at: parse_ts(row.get("received_at"))?,
    })
}

pub(super) fn sale_from_row(row: &sqlx::sqlite::SqliteRow) -> AppResult<Sale> {
    Ok(Sale {
        id: parse_id(row.get("id"))?,
        organization_id: parse_id(row.get("organization_id"))?,
        branch_id: parse_id(row.get("branch_id"))?,
        register_id: parse_id(row.get("register_id"))?,
        shift_id: parse_id(row.get("shift_id"))?,
        operator_id: parse_id(row.get("operator_id"))?,
        folio: row.get("folio"),
        occurred_at: parse_ts(row.get("occurred_at"))?,
        status: SaleStatus::Completed,
        subtotal: Money::from_minor_units(row.get("subtotal_minor")).map_err(AppError::Domain)?,
        tax: Money::from_minor_units(row.get("tax_minor")).map_err(AppError::Domain)?,
        total: Money::from_minor_units(row.get("total_minor")).map_err(AppError::Domain)?,
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint: row.get("request_fingerprint"),
    })
}

pub(super) fn map_sqlx(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_error) = &error
        && db_error.is_unique_violation()
    {
        return AppError::Conflict;
    }
    tracing::error!(error = %error, "fallo de almacenamiento SQLite");
    AppError::Storage
}
