//! Modificadores de producto.

use super::*;

pub(super) async fn insert_modifier_group(
    tx: &mut SqliteTx,
    group: &ModifierGroup,
) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO modifier_group \
             (id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(group.id.to_string())
        .bind(group.organization_id.to_string())
        .bind(group.product_id.to_string())
        .bind(&group.name)
        .bind(group.min as i64)
        .bind(group.max as i64)
        .bind(group.sort_order)
        .bind(i64::from(group.active))
        .bind(format_ts(group.created_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_modifier_group(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    group_id: pos_core::ids::ModifierGroupId,
) -> AppResult<Option<ModifierGroup>> {
    let row = sqlx::query(
            "SELECT id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at \
             FROM modifier_group WHERE id = ? AND organization_id = ?",
        )
        .bind(group_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|r| -> AppResult<ModifierGroup> {
        Ok(ModifierGroup {
            id: parse_id(r.get("id"))?,
            organization_id: parse_id(r.get("organization_id"))?,
            product_id: parse_id(r.get("product_id"))?,
            name: r.get("name"),
            min: u32::try_from(r.get::<i64, _>("min_selectable")).map_err(|_| AppError::Storage)?,
            max: u32::try_from(r.get::<i64, _>("max_selectable")).map_err(|_| AppError::Storage)?,
            sort_order: r.get("sort_order"),
            active: r.get::<i64, _>("active") != 0,
            created_at: parse_ts(r.get("created_at"))?,
        })
    })
    .transpose()
}

pub(super) async fn list_modifier_groups(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    product_id: ProductId,
) -> AppResult<Vec<ModifierGroup>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at \
             FROM modifier_group WHERE organization_id = ? AND product_id = ? ORDER BY sort_order, name",
        )
        .bind(organization_id.to_string())
        .bind(product_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|r| -> AppResult<ModifierGroup> {
            Ok(ModifierGroup {
                id: parse_id(r.get("id"))?,
                organization_id: parse_id(r.get("organization_id"))?,
                product_id: parse_id(r.get("product_id"))?,
                name: r.get("name"),
                min: u32::try_from(r.get::<i64, _>("min_selectable"))
                    .map_err(|_| AppError::Storage)?,
                max: u32::try_from(r.get::<i64, _>("max_selectable"))
                    .map_err(|_| AppError::Storage)?,
                sort_order: r.get("sort_order"),
                active: r.get::<i64, _>("active") != 0,
                created_at: parse_ts(r.get("created_at"))?,
            })
        })
        .collect()
}

pub(super) async fn insert_modifier_option(
    tx: &mut SqliteTx,
    option: &ModifierOption,
) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO modifier_option \
             (id, organization_id, group_id, name, kind, price_delta_minor, sort_order, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(option.id.to_string())
        .bind(option.organization_id.to_string())
        .bind(option.group_id.to_string())
        .bind(&option.name)
        .bind(option.kind.as_str())
        .bind(option.price_delta.minor_units())
        .bind(option.sort_order)
        .bind(i64::from(option.active))
        .bind(format_ts(option.created_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_modifier_option(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    option_id: pos_core::ids::ModifierOptionId,
) -> AppResult<Option<ModifierOption>> {
    let row = sqlx::query(
            "SELECT id, organization_id, group_id, name, kind, price_delta_minor, sort_order, active, created_at \
             FROM modifier_option WHERE id = ? AND organization_id = ?",
        )
        .bind(option_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|r| -> AppResult<ModifierOption> {
        Ok(ModifierOption {
            id: parse_id(r.get("id"))?,
            organization_id: parse_id(r.get("organization_id"))?,
            group_id: parse_id(r.get("group_id"))?,
            name: r.get("name"),
            kind: pos_core::modifiers::OptionKind::from_key(&r.get::<String, _>("kind"))
                .ok_or(AppError::Storage)?,
            price_delta: Money::from_minor_units(r.get("price_delta_minor"))
                .map_err(AppError::Domain)?,
            sort_order: r.get("sort_order"),
            active: r.get::<i64, _>("active") != 0,
            created_at: parse_ts(r.get("created_at"))?,
        })
    })
    .transpose()
}

pub(super) async fn list_modifier_options(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    group_id: pos_core::ids::ModifierGroupId,
) -> AppResult<Vec<ModifierOption>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, group_id, name, kind, price_delta_minor, sort_order, active, created_at \
             FROM modifier_option WHERE organization_id = ? AND group_id = ? ORDER BY sort_order, name",
        )
        .bind(organization_id.to_string())
        .bind(group_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|r| -> AppResult<ModifierOption> {
            Ok(ModifierOption {
                id: parse_id(r.get("id"))?,
                organization_id: parse_id(r.get("organization_id"))?,
                group_id: parse_id(r.get("group_id"))?,
                name: r.get("name"),
                kind: pos_core::modifiers::OptionKind::from_key(&r.get::<String, _>("kind"))
                    .ok_or(AppError::Storage)?,
                price_delta: Money::from_minor_units(r.get("price_delta_minor"))
                    .map_err(AppError::Domain)?,
                sort_order: r.get("sort_order"),
                active: r.get::<i64, _>("active") != 0,
                created_at: parse_ts(r.get("created_at"))?,
            })
        })
        .collect()
}
