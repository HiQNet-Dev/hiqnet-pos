//! Inventario y niveles de stock.

use super::*;

pub(super) async fn insert_inventory_movement(
    tx: &mut SqliteTx,
    movement: &InventoryMovement,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO inventory_movement \
             (id, organization_id, branch_id, product_id, kind, quantity_milli, reason, \
              actor_user_id, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(movement.id.to_string())
    .bind(movement.organization_id.to_string())
    .bind(movement.branch_id.to_string())
    .bind(movement.product_id.to_string())
    .bind(movement.kind.as_str())
    .bind(movement.quantity.milli_units())
    .bind(&movement.reason)
    .bind(movement.actor_user_id.map(|id| id.to_string()))
    .bind(format_ts(movement.occurred_at))
    .execute(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn current_stock(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    product_id: ProductId,
) -> AppResult<Quantity> {
    let row = sqlx::query(
            "SELECT COALESCE(SUM(CASE kind WHEN 'outbound' THEN -quantity_milli WHEN 'adjustment_out' THEN -quantity_milli ELSE quantity_milli END), 0) AS stock \
             FROM inventory_movement WHERE organization_id = ? AND branch_id = ? AND product_id = ?",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(product_id.to_string())
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    let stock: i64 = row.get("stock");
    Quantity::from_milli_units(stock).map_err(AppError::Domain)
}

pub(super) async fn stock_levels(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<(ProductId, Quantity)>> {
    let rows = sqlx::query(
            "SELECT product_id, COALESCE(SUM(CASE kind WHEN 'outbound' THEN -quantity_milli WHEN 'adjustment_out' THEN -quantity_milli ELSE quantity_milli END), 0) AS stock \
             FROM inventory_movement WHERE organization_id = ? AND branch_id = ? GROUP BY product_id",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| -> AppResult<(ProductId, Quantity)> {
            let product_id: ProductId = parse_id(row.get("product_id"))?;
            let stock: i64 = row.get("stock");
            Ok((
                product_id,
                Quantity::from_milli_units(stock).map_err(AppError::Domain)?,
            ))
        })
        .collect()
}

pub(super) async fn list_inventory_movements(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
    product_id: Option<ProductId>,
    limit: i64,
) -> AppResult<Vec<InventoryMovement>> {
    let rows = if let Some(product_id) = product_id {
        sqlx::query(
            "SELECT id, organization_id, branch_id, product_id, kind, quantity_milli, reason, \
                 actor_user_id, occurred_at FROM inventory_movement \
                 WHERE organization_id = ? AND branch_id = ? AND product_id = ? \
                 ORDER BY occurred_at DESC LIMIT ?",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(product_id.to_string())
        .bind(limit)
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?
    } else {
        sqlx::query(
            "SELECT id, organization_id, branch_id, product_id, kind, quantity_milli, reason, \
                 actor_user_id, occurred_at FROM inventory_movement \
                 WHERE organization_id = ? AND branch_id = ? \
                 ORDER BY occurred_at DESC LIMIT ?",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(limit)
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?
    };

    rows.into_iter()
        .map(|row| -> AppResult<InventoryMovement> {
            let kind =
                MovementKind::from_key(&row.get::<String, _>("kind")).ok_or(AppError::Storage)?;
            Ok(InventoryMovement {
                id: row
                    .get::<String, _>("id")
                    .parse()
                    .map_err(|_| AppError::Storage)?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_id(row.get("branch_id"))?,
                product_id: row
                    .get::<String, _>("product_id")
                    .parse()
                    .map_err(|_| AppError::Storage)?,
                kind,
                quantity: Quantity::from_milli_units(row.get("quantity_milli"))
                    .map_err(AppError::Domain)?,
                reason: row.get("reason"),
                actor_user_id: parse_optional_id(row.get("actor_user_id"))?,
                occurred_at: parse_ts(row.get("occurred_at"))?,
            })
        })
        .collect()
}
