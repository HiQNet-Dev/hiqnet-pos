//! Consultas agregadas de reportes.

use super::*;

pub(super) async fn sales_report_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<SalesReportTotals> {
    let row = sqlx::query(
            "SELECT COUNT(*) AS count, COALESCE(SUM(subtotal_minor), 0) AS subtotal, COALESCE(SUM(tax_minor), 0) AS tax, COALESCE(SUM(total_minor), 0) AS total FROM sale WHERE organization_id = ? AND branch_id = ? AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(format_ts(from))
        .bind(format_ts(to))
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(SalesReportTotals {
        count: row.get("count"),
        subtotal_minor: row.get("subtotal"),
        tax_minor: row.get("tax"),
        total_minor: row.get("total"),
    })
}

pub(super) async fn sales_report_by_product(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<Vec<ProductSalesReportRow>> {
    let rows = sqlx::query(
            "SELECT item.product_id AS product_id, item.product_name AS product_name, SUM(item.quantity_milli) AS quantity, SUM((item.unit_price_minor * item.quantity_milli + 500) / 1000) AS net, SUM((item.unit_tax_minor * item.quantity_milli + 500) / 1000) AS tax, SUM(item.line_total_minor) AS total FROM sale_item item JOIN sale ON sale.id = item.sale_id WHERE sale.organization_id = ? AND sale.branch_id = ? AND julianday(sale.occurred_at) >= julianday(?) AND julianday(sale.occurred_at) < julianday(?) GROUP BY item.product_id, item.product_name ORDER BY net DESC",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(format_ts(from))
        .bind(format_ts(to))
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| {
            Ok(ProductSalesReportRow {
                product_id: parse_id(row.get("product_id"))?,
                product_name: row.get("product_name"),
                quantity_milli: row.get("quantity"),
                net_minor: row.get("net"),
                tax_minor: row.get("tax"),
                total_minor: row.get("total"),
            })
        })
        .collect()
}

pub(super) async fn returns_report_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<ReturnsReportTotals> {
    let row = sqlx::query(
            "SELECT COUNT(*) AS count, COALESCE(SUM(refund_total_minor), 0) AS total FROM sale_return WHERE organization_id = ? AND branch_id = ? AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(format_ts(from))
        .bind(format_ts(to))
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(ReturnsReportTotals {
        count: row.get("count"),
        total_minor: row.get("total"),
    })
}

pub(super) async fn cash_report_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<CashReportTotals> {
    let from = format_ts(from);
    let to = format_ts(to);
    let organization = organization_id.to_string();
    let branch = branch_id.to_string();
    let opened: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND julianday(opened_at) >= julianday(?) AND julianday(opened_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let closed_row = sqlx::query(
            "SELECT COUNT(*) AS count, COALESCE(SUM(difference_minor), 0) AS difference FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND closed_at IS NOT NULL AND julianday(closed_at) >= julianday(?) AND julianday(closed_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let entries: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM cash_movement WHERE organization_id = ? AND branch_id = ? AND kind = 'in' AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let exits: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM cash_movement WHERE organization_id = ? AND branch_id = ? AND kind = 'out' AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let expenses_from_cash: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM expense WHERE organization_id = ? AND branch_id = ? AND cash_movement_id IS NOT NULL AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let refunds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(refund_total_minor), 0) FROM sale_return WHERE organization_id = ? AND branch_id = ? AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(CashReportTotals {
        shifts_opened: opened,
        shifts_closed: closed_row.get("count"),
        entries_minor: entries,
        exits_minor: exits,
        expenses_from_cash_minor: expenses_from_cash,
        refunds_minor: refunds,
        difference_minor: closed_row.get("difference"),
    })
}

pub(super) async fn purchases_report_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<PurchasesReportTotals> {
    let from = format_ts(from);
    let to = format_ts(to);
    let organization = organization_id.to_string();
    let branch = branch_id.to_string();
    let created: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM purchase_order WHERE organization_id = ? AND branch_id = ? AND julianday(created_at) >= julianday(?) AND julianday(created_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM purchase_order WHERE organization_id = ? AND branch_id = ? AND status IN ('ordered', 'partially_received')",
        )
        .bind(&organization)
        .bind(&branch)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let row = sqlx::query(
            "SELECT COALESCE(SUM(line.quantity_milli), 0) AS quantity, COALESCE(SUM((order_line.unit_cost_minor * line.quantity_milli + 500) / 1000), 0) AS cost FROM purchase_receipt_line line JOIN purchase_receipt receipt ON receipt.id = line.receipt_id JOIN purchase_order_line order_line ON order_line.id = line.order_line_id WHERE receipt.organization_id = ? AND receipt.branch_id = ? AND julianday(receipt.received_at) >= julianday(?) AND julianday(receipt.received_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(PurchasesReportTotals {
        orders_created: created,
        pending_orders: pending,
        received_quantity_milli: row.get("quantity"),
        received_cost_minor: row.get("cost"),
    })
}

pub(super) async fn expenses_report_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<ExpensesReportTotals> {
    let from = format_ts(from);
    let to = format_ts(to);
    let organization = organization_id.to_string();
    let branch = branch_id.to_string();
    let row = sqlx::query(
            "SELECT COALESCE(SUM(amount_minor), 0) AS total, COALESCE(SUM(CASE WHEN cash_movement_id IS NOT NULL THEN amount_minor ELSE 0 END), 0) AS from_cash, COALESCE(SUM(CASE WHEN cash_movement_id IS NULL THEN amount_minor ELSE 0 END), 0) AS external FROM expense WHERE organization_id = ? AND branch_id = ? AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let rows = sqlx::query(
            "SELECT category.name AS name, COALESCE(SUM(expense.amount_minor), 0) AS total FROM expense JOIN expense_category category ON category.id = expense.category_id WHERE expense.organization_id = ? AND expense.branch_id = ? AND julianday(expense.occurred_at) >= julianday(?) AND julianday(expense.occurred_at) < julianday(?) GROUP BY category.id, category.name ORDER BY total DESC",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(ExpensesReportTotals {
        total_minor: row.get("total"),
        from_cash_minor: row.get("from_cash"),
        external_minor: row.get("external"),
        by_category: rows
            .into_iter()
            .map(|row| ExpenseCategoryReportRow {
                name: row.get("name"),
                total_minor: row.get("total"),
            })
            .collect(),
    })
}

pub(super) async fn inventory_report_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> AppResult<InventoryReportTotals> {
    let from = format_ts(from);
    let to = format_ts(to);
    let organization = organization_id.to_string();
    let branch = branch_id.to_string();
    let stock: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(CASE WHEN kind IN ('inbound', 'adjustment_in') THEN quantity_milli ELSE -quantity_milli END), 0) FROM inventory_movement WHERE organization_id = ? AND branch_id = ?",
        )
        .bind(&organization)
        .bind(&branch)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let row = sqlx::query(
            "SELECT COALESCE(SUM(CASE WHEN kind = 'inbound' THEN quantity_milli ELSE 0 END), 0) AS inbound, COALESCE(SUM(CASE WHEN kind = 'outbound' THEN quantity_milli ELSE 0 END), 0) AS outbound, COALESCE(SUM(CASE WHEN kind = 'adjustment_in' THEN quantity_milli ELSE 0 END), 0) AS adjustment_in, COALESCE(SUM(CASE WHEN kind = 'adjustment_out' AND COALESCE(reason, '') NOT LIKE 'merma:%' THEN quantity_milli ELSE 0 END), 0) AS adjustment_out FROM inventory_movement WHERE organization_id = ? AND branch_id = ? AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let waste: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(quantity_milli), 0) FROM waste_record WHERE organization_id = ? AND branch_id = ? AND julianday(occurred_at) >= julianday(?) AND julianday(occurred_at) < julianday(?)",
        )
        .bind(&organization)
        .bind(&branch)
        .bind(&from)
        .bind(&to)
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(InventoryReportTotals {
        stock_quantity_milli: stock,
        inbound_milli: row.get("inbound"),
        outbound_milli: row.get("outbound"),
        waste_milli: waste,
        adjustment_in_milli: row.get("adjustment_in"),
        adjustment_out_milli: row.get("adjustment_out"),
    })
}
