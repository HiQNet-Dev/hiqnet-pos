//! Devoluciones.

use super::*;

pub(super) async fn find_sale_by_folio(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    folio: &str,
) -> AppResult<Option<Sale>> {
    let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, shift_id, operator_id, folio, occurred_at, status, subtotal_minor, tax_minor, total_minor, idempotency_key, request_fingerprint FROM sale WHERE organization_id = ? AND branch_id = ? AND folio = ?",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(folio.trim())
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|row| sale_from_row(&row)).transpose()
}

pub(super) async fn returned_totals(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    sale_item_id: pos_core::ids::SaleItemId,
) -> AppResult<ReturnedTotals> {
    let row = sqlx::query(
            "SELECT COALESCE(SUM(line.quantity_milli), 0) AS quantity, COALESCE(SUM(line.refund_net_minor), 0) AS net, COALESCE(SUM(line.refund_tax_minor), 0) AS tax FROM sale_return_line line JOIN sale_return ret ON ret.id = line.return_id WHERE ret.organization_id = ? AND line.sale_item_id = ?",
        )
        .bind(organization_id.to_string())
        .bind(sale_item_id.to_string())
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(ReturnedTotals {
        quantity_milli: row.get("quantity"),
        net_minor: row.get("net"),
        tax_minor: row.get("tax"),
    })
}

pub(super) async fn insert_sale_return(tx: &mut SqliteTx, value: &SaleReturn) -> AppResult<bool> {
    let result = sqlx::query(
            "INSERT INTO sale_return (id, organization_id, branch_id, sale_id, shift_id, cash_movement_id, operator_id, reason, refund_total_minor, idempotency_key, request_fingerprint, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (organization_id, idempotency_key) DO NOTHING",
        )
        .bind(value.id.to_string())
        .bind(value.organization_id.to_string())
        .bind(value.branch_id.to_string())
        .bind(value.sale_id.to_string())
        .bind(value.shift_id.map(|id| id.to_string()))
        .bind(value.cash_movement_id.map(|id| id.to_string()))
        .bind(value.operator_id.to_string())
        .bind(&value.reason)
        .bind(value.refund_total.minor_units())
        .bind(&value.idempotency_key)
        .bind(&value.request_fingerprint)
        .bind(format_ts(value.occurred_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn insert_sale_return_line(
    tx: &mut SqliteTx,
    line: &SaleReturnLine,
) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO sale_return_line (id, return_id, sale_item_id, quantity_milli, disposition, refund_net_minor, refund_tax_minor, refund_total_minor, inbound_movement_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(line.id.to_string())
        .bind(line.return_id.to_string())
        .bind(line.sale_item_id.to_string())
        .bind(line.quantity.milli_units())
        .bind(line.disposition.as_str())
        .bind(line.refund_net.minor_units())
        .bind(line.refund_tax.minor_units())
        .bind(line.refund_total.minor_units())
        .bind(line.inbound_movement_id.map(|id| id.to_string()))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_sale_return_by_idempotency(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    idempotency_key: &str,
) -> AppResult<Option<SaleReturn>> {
    let row = sqlx::query(
            "SELECT id, organization_id, branch_id, sale_id, shift_id, cash_movement_id, operator_id, reason, refund_total_minor, idempotency_key, request_fingerprint, occurred_at FROM sale_return WHERE organization_id = ? AND idempotency_key = ?",
        )
        .bind(organization_id.to_string())
        .bind(idempotency_key)
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(sale_return_from_row).transpose()
}

pub(super) async fn list_sale_return_lines(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    return_id: SaleReturnId,
) -> AppResult<Vec<SaleReturnLine>> {
    let rows = sqlx::query(
            "SELECT line.id, line.return_id, line.sale_item_id, line.quantity_milli, line.disposition, line.refund_net_minor, line.refund_tax_minor, line.refund_total_minor, line.inbound_movement_id FROM sale_return_line line JOIN sale_return ret ON ret.id = line.return_id WHERE ret.organization_id = ? AND line.return_id = ? ORDER BY line.sale_item_id",
        )
        .bind(organization_id.to_string())
        .bind(return_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter().map(sale_return_line_from_row).collect()
}

pub(super) async fn list_sale_returns(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    limit: i64,
) -> AppResult<Vec<SaleReturn>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, sale_id, shift_id, cash_movement_id, operator_id, reason, refund_total_minor, idempotency_key, request_fingerprint, occurred_at FROM sale_return WHERE organization_id = ? AND branch_id = ? ORDER BY occurred_at DESC LIMIT ?",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(limit)
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter().map(sale_return_from_row).collect()
}
