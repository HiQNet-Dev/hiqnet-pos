//! Compras, proveedores y recepciones.

use super::*;

pub(super) async fn insert_supplier(tx: &mut SqliteTx, supplier: &Supplier) -> AppResult<()> {
    sqlx::query("INSERT INTO supplier (id, organization_id, code, name, contact, rfc, active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(supplier.id.to_string()).bind(supplier.organization_id.to_string()).bind(&supplier.code)
            .bind(&supplier.name).bind(&supplier.contact).bind(&supplier.rfc).bind(i64::from(supplier.active))
            .bind(format_ts(supplier.created_at)).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_suppliers(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<Vec<Supplier>> {
    let rows = sqlx::query("SELECT id, organization_id, code, name, contact, rfc, active, created_at FROM supplier WHERE organization_id = ? ORDER BY name COLLATE NOCASE")
            .bind(organization_id.to_string()).fetch_all(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| {
            Ok(Supplier {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                code: row.get("code"),
                name: row.get("name"),
                contact: row.get("contact"),
                rfc: row.get("rfc"),
                active: row.get::<i64, _>("active") != 0,
                created_at: parse_ts(row.get("created_at"))?,
            })
        })
        .collect()
}

pub(super) async fn find_supplier(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    supplier_id: SupplierId,
) -> AppResult<Option<Supplier>> {
    let row = sqlx::query("SELECT id, organization_id, code, name, contact, rfc, active, created_at FROM supplier WHERE organization_id = ? AND id = ?")
            .bind(organization_id.to_string()).bind(supplier_id.to_string()).fetch_optional(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    row.map(|row| {
        Ok(Supplier {
            id: parse_id(row.get("id"))?,
            organization_id: parse_id(row.get("organization_id"))?,
            code: row.get("code"),
            name: row.get("name"),
            contact: row.get("contact"),
            rfc: row.get("rfc"),
            active: row.get::<i64, _>("active") != 0,
            created_at: parse_ts(row.get("created_at"))?,
        })
    })
    .transpose()
}

pub(super) async fn insert_purchase_order(
    tx: &mut SqliteTx,
    order: &PurchaseOrder,
) -> AppResult<()> {
    sqlx::query("INSERT INTO purchase_order (id, organization_id, branch_id, supplier_id, folio, status, created_by, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(order.id.to_string()).bind(order.organization_id.to_string()).bind(order.branch_id.to_string()).bind(order.supplier_id.to_string()).bind(&order.folio).bind(order.status.as_str()).bind(order.created_by.to_string()).bind(format_ts(order.created_at)).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn insert_purchase_order_line(
    tx: &mut SqliteTx,
    line: &PurchaseOrderLine,
) -> AppResult<()> {
    sqlx::query("INSERT INTO purchase_order_line (id, order_id, product_id, requested_quantity_milli, received_quantity_milli, unit_cost_minor) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(line.id.to_string()).bind(line.order_id.to_string()).bind(line.product_id.to_string()).bind(line.requested_quantity.milli_units()).bind(line.received_quantity.milli_units()).bind(line.unit_cost.minor_units()).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_purchase_order(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    order_id: PurchaseOrderId,
) -> AppResult<Option<PurchaseOrder>> {
    let row = sqlx::query("SELECT id, organization_id, branch_id, supplier_id, folio, status, created_by, created_at FROM purchase_order WHERE organization_id = ? AND id = ?")
            .bind(organization_id.to_string()).bind(order_id.to_string()).fetch_optional(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    row.map(purchase_order_from_row).transpose()
}

pub(super) async fn list_purchase_orders(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<PurchaseOrder>> {
    let rows = sqlx::query("SELECT id, organization_id, branch_id, supplier_id, folio, status, created_by, created_at FROM purchase_order WHERE organization_id = ? AND branch_id = ? ORDER BY created_at DESC")
            .bind(organization_id.to_string()).bind(branch_id.to_string()).fetch_all(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    rows.into_iter().map(purchase_order_from_row).collect()
}

pub(super) async fn list_purchase_order_lines(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    order_id: PurchaseOrderId,
) -> AppResult<Vec<PurchaseOrderLine>> {
    let rows = sqlx::query("SELECT l.id, l.order_id, l.product_id, l.requested_quantity_milli, l.received_quantity_milli, l.unit_cost_minor FROM purchase_order_line l JOIN purchase_order o ON o.id = l.order_id WHERE o.organization_id = ? AND l.order_id = ? ORDER BY l.id")
            .bind(organization_id.to_string()).bind(order_id.to_string()).fetch_all(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    rows.into_iter().map(purchase_order_line_from_row).collect()
}

pub(super) async fn confirm_purchase_order(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    order_id: PurchaseOrderId,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE purchase_order SET status = 'ordered' WHERE organization_id = ? AND id = ? AND status = 'draft'")
            .bind(organization_id.to_string()).bind(order_id.to_string()).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict);
    }
    Ok(())
}

pub(super) async fn find_purchase_receipt_by_idempotency(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    idempotency_key: &str,
) -> AppResult<Option<PurchaseReceipt>> {
    let row = sqlx::query("SELECT id, organization_id, branch_id, order_id, received_by, idempotency_key, request_fingerprint, received_at FROM purchase_receipt WHERE organization_id = ? AND idempotency_key = ?")
            .bind(organization_id.to_string()).bind(idempotency_key).fetch_optional(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    row.map(purchase_receipt_from_row).transpose()
}

pub(super) async fn insert_purchase_receipt(
    tx: &mut SqliteTx,
    receipt: &PurchaseReceipt,
) -> AppResult<bool> {
    let result = sqlx::query("INSERT INTO purchase_receipt (id, organization_id, branch_id, order_id, received_by, idempotency_key, request_fingerprint, received_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (organization_id, idempotency_key) DO NOTHING")
            .bind(receipt.id.to_string()).bind(receipt.organization_id.to_string()).bind(receipt.branch_id.to_string()).bind(receipt.order_id.to_string()).bind(receipt.received_by.to_string()).bind(&receipt.idempotency_key).bind(&receipt.request_fingerprint).bind(format_ts(receipt.received_at)).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn insert_purchase_receipt_line(
    tx: &mut SqliteTx,
    receipt_id: PurchaseReceiptId,
    order_line_id: PurchaseOrderLineId,
    quantity: Quantity,
) -> AppResult<()> {
    sqlx::query("INSERT INTO purchase_receipt_line (receipt_id, order_line_id, quantity_milli) VALUES (?, ?, ?)")
            .bind(receipt_id.to_string()).bind(order_line_id.to_string()).bind(quantity.milli_units()).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn apply_purchase_receipt(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    order_id: PurchaseOrderId,
    updates: &[(PurchaseOrderLineId, Quantity)],
    status: PurchaseOrderStatus,
) -> AppResult<()> {
    for (line_id, quantity) in updates {
        let result = sqlx::query("UPDATE purchase_order_line SET received_quantity_milli = received_quantity_milli + ? WHERE id = ? AND order_id = ? AND received_quantity_milli + ? <= requested_quantity_milli")
                .bind(quantity.milli_units()).bind(line_id.to_string()).bind(order_id.to_string()).bind(quantity.milli_units()).execute(&mut **tx.tx()?).await.map_err(map_sqlx)?;
        if result.rows_affected() != 1 {
            return Err(AppError::Conflict);
        }
    }
    sqlx::query("UPDATE purchase_order SET status = ? WHERE organization_id = ? AND id = ?")
        .bind(status.as_str())
        .bind(organization_id.to_string())
        .bind(order_id.to_string())
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}
