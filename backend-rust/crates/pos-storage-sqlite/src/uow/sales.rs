//! Ventas y sus componentes.

use super::*;

pub(super) async fn insert_sale(tx: &mut SqliteTx, sale: &Sale) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO sale (id, organization_id, branch_id, register_id, shift_id, operator_id, folio, occurred_at, status, subtotal_minor, tax_minor, total_minor, idempotency_key, request_fingerprint) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(sale.id.to_string())
        .bind(sale.organization_id.to_string())
        .bind(sale.branch_id.to_string())
        .bind(sale.register_id.to_string())
        .bind(sale.shift_id.to_string())
        .bind(sale.operator_id.to_string())
        .bind(&sale.folio)
        .bind(format_ts(sale.occurred_at))
        .bind(sale.status.as_str())
        .bind(sale.subtotal.minor_units())
        .bind(sale.tax.minor_units())
        .bind(sale.total.minor_units())
        .bind(&sale.idempotency_key)
        .bind(&sale.request_fingerprint)
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn insert_sale_item(tx: &mut SqliteTx, item: &SaleItem) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO sale_item (id, sale_id, product_id, variant_id, product_name, variant_name, quantity_milli, unit_price_minor, unit_tax_minor, line_total_minor, modifiers_summary) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(item.id.to_string())
        .bind(item.sale_id.to_string())
        .bind(item.product_id.to_string())
        .bind(item.variant_id.map(|id| id.to_string()))
        .bind(&item.product_name)
        .bind(&item.variant_name)
        .bind(item.quantity.milli_units())
        .bind(item.unit_price.minor_units())
        .bind(item.unit_tax.minor_units())
        .bind(item.line_total.minor_units())
        .bind(&item.modifiers_summary)
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn insert_sale_payment(tx: &mut SqliteTx, payment: &SalePayment) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO sale_payment (id, sale_id, method, amount_minor, change_minor, occurred_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(payment.id.to_string())
        .bind(payment.sale_id.to_string())
        .bind(&payment.method)
        .bind(payment.amount.minor_units())
        .bind(payment.change_given.minor_units())
        .bind(format_ts(payment.occurred_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_sale_by_idempotency(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    idempotency_key: &str,
) -> AppResult<Option<Sale>> {
    let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, shift_id, operator_id, folio, occurred_at, status, subtotal_minor, tax_minor, total_minor, idempotency_key, request_fingerprint FROM sale WHERE organization_id = ? AND idempotency_key = ?",
        )
        .bind(organization_id.to_string())
        .bind(idempotency_key)
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|row| sale_from_row(&row)).transpose()
}

pub(super) async fn find_sale(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    sale_id: pos_core::ids::SaleId,
) -> AppResult<Option<Sale>> {
    let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, shift_id, operator_id, folio, occurred_at, status, subtotal_minor, tax_minor, total_minor, idempotency_key, request_fingerprint FROM sale WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.to_string())
        .bind(sale_id.to_string())
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|row| sale_from_row(&row)).transpose()
}

pub(super) async fn list_sale_items(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    sale_id: pos_core::ids::SaleId,
) -> AppResult<Vec<SaleItem>> {
    let rows = sqlx::query(
            "SELECT si.id, si.sale_id, si.product_id, si.variant_id, si.product_name, si.variant_name, si.quantity_milli, si.unit_price_minor, si.unit_tax_minor, si.line_total_minor, si.modifiers_summary FROM sale_item si JOIN sale s ON s.id = si.sale_id WHERE s.organization_id = ? AND si.sale_id = ?",
        )
        .bind(organization_id.to_string())
        .bind(sale_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| -> AppResult<SaleItem> {
            Ok(SaleItem {
                id: parse_id(row.get("id"))?,
                sale_id: parse_id(row.get("sale_id"))?,
                product_id: parse_id(row.get("product_id"))?,
                variant_id: parse_optional_id(row.get("variant_id"))?,
                product_name: row.get("product_name"),
                variant_name: row.get("variant_name"),
                quantity: Quantity::from_milli_units(row.get("quantity_milli"))
                    .map_err(AppError::Domain)?,
                unit_price: Money::from_minor_units(row.get("unit_price_minor"))
                    .map_err(AppError::Domain)?,
                unit_tax: Money::from_minor_units(row.get("unit_tax_minor"))
                    .map_err(AppError::Domain)?,
                line_total: Money::from_minor_units(row.get("line_total_minor"))
                    .map_err(AppError::Domain)?,
                modifiers_summary: row.get("modifiers_summary"),
            })
        })
        .collect()
}

pub(super) async fn list_sale_payments(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    sale_id: pos_core::ids::SaleId,
) -> AppResult<Vec<SalePayment>> {
    let rows = sqlx::query(
            "SELECT p.id, p.sale_id, p.method, p.amount_minor, p.change_minor, p.occurred_at FROM sale_payment p JOIN sale s ON s.id = p.sale_id WHERE s.organization_id = ? AND p.sale_id = ?",
        )
        .bind(organization_id.to_string())
        .bind(sale_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| -> AppResult<SalePayment> {
            Ok(SalePayment {
                id: parse_id(row.get("id"))?,
                sale_id: parse_id(row.get("sale_id"))?,
                method: row.get("method"),
                amount: Money::from_minor_units(row.get("amount_minor"))
                    .map_err(AppError::Domain)?,
                change_given: Money::from_minor_units(row.get("change_minor"))
                    .map_err(AppError::Domain)?,
                occurred_at: parse_ts(row.get("occurred_at"))?,
            })
        })
        .collect()
}
