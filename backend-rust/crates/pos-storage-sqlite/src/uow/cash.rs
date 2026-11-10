//! Cajas, turnos y movimientos.

use super::*;

pub(super) async fn insert_cash_register(
    tx: &mut SqliteTx,
    register: &CashRegister,
) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO cash_register (id, organization_id, branch_id, name, code, active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(register.id.to_string())
        .bind(register.organization_id.to_string())
        .bind(register.branch_id.to_string())
        .bind(&register.name)
        .bind(&register.code)
        .bind(i64::from(register.active))
        .bind(format_ts(register.created_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_cash_registers(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<CashRegister>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, name, code, active, created_at FROM cash_register WHERE organization_id = ? AND branch_id = ? ORDER BY name",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| -> AppResult<CashRegister> {
            Ok(CashRegister {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_id(row.get("branch_id"))?,
                name: row.get("name"),
                code: row.get("code"),
                active: row.get::<i64, _>("active") != 0,
                created_at: parse_ts(row.get("created_at"))?,
            })
        })
        .collect()
}

pub(super) async fn insert_cash_shift(tx: &mut SqliteTx, shift: &CashShift) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO cash_shift (id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(shift.id.to_string())
        .bind(shift.organization_id.to_string())
        .bind(shift.branch_id.to_string())
        .bind(shift.register_id.to_string())
        .bind(shift.opened_by.to_string())
        .bind(format_ts(shift.opened_at))
        .bind(shift.opening_amount.minor_units())
        .bind(shift.status.as_str())
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_open_cash_shift(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    register_id: pos_core::ids::CashRegisterId,
) -> AppResult<Option<CashShift>> {
    let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status, closed_by, closed_at, expected_amount_minor, counted_amount_minor, difference_minor FROM cash_shift WHERE organization_id = ? AND register_id = ? AND status = 'open'",
        )
        .bind(organization_id.to_string())
        .bind(register_id.to_string())
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|row| -> AppResult<CashShift> {
        let status =
            CashShiftStatus::from_key(&row.get::<String, _>("status")).ok_or(AppError::Storage)?;
        Ok(CashShift {
            id: parse_id(row.get("id"))?,
            organization_id: parse_id(row.get("organization_id"))?,
            branch_id: parse_id(row.get("branch_id"))?,
            register_id: parse_id(row.get("register_id"))?,
            opened_by: parse_id(row.get("opened_by"))?,
            opened_at: parse_ts(row.get("opened_at"))?,
            opening_amount: Money::from_minor_units(row.get("opening_amount_minor"))
                .map_err(AppError::Domain)?,
            status,
            closed_by: parse_optional_id(row.get("closed_by"))?,
            closed_at: row
                .get::<Option<String>, _>("closed_at")
                .map(parse_ts)
                .transpose()?,
            expected_amount: row
                .get::<Option<i64>, _>("expected_amount_minor")
                .map(Money::from_minor_units)
                .transpose()
                .map_err(AppError::Domain)?,
            counted_amount: row
                .get::<Option<i64>, _>("counted_amount_minor")
                .map(Money::from_minor_units)
                .transpose()
                .map_err(AppError::Domain)?,
            difference_minor: row.get("difference_minor"),
        })
    })
    .transpose()
}

pub(super) async fn find_cash_shift(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    shift_id: pos_core::ids::CashShiftId,
) -> AppResult<Option<CashShift>> {
    let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status, closed_by, closed_at, expected_amount_minor, counted_amount_minor, difference_minor FROM cash_shift WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.to_string())
        .bind(shift_id.to_string())
        .fetch_optional(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    row.map(|row| -> AppResult<CashShift> {
        let status =
            CashShiftStatus::from_key(&row.get::<String, _>("status")).ok_or(AppError::Storage)?;
        Ok(CashShift {
            id: parse_id(row.get("id"))?,
            organization_id: parse_id(row.get("organization_id"))?,
            branch_id: parse_id(row.get("branch_id"))?,
            register_id: parse_id(row.get("register_id"))?,
            opened_by: parse_id(row.get("opened_by"))?,
            opened_at: parse_ts(row.get("opened_at"))?,
            opening_amount: Money::from_minor_units(row.get("opening_amount_minor"))
                .map_err(AppError::Domain)?,
            status,
            closed_by: parse_optional_id(row.get("closed_by"))?,
            closed_at: row
                .get::<Option<String>, _>("closed_at")
                .map(parse_ts)
                .transpose()?,
            expected_amount: row
                .get::<Option<i64>, _>("expected_amount_minor")
                .map(Money::from_minor_units)
                .transpose()
                .map_err(AppError::Domain)?,
            counted_amount: row
                .get::<Option<i64>, _>("counted_amount_minor")
                .map(Money::from_minor_units)
                .transpose()
                .map_err(AppError::Domain)?,
            difference_minor: row.get("difference_minor"),
        })
    })
    .transpose()
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn close_cash_shift(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    shift_id: pos_core::ids::CashShiftId,
    closed_by: UserId,
    closed_at: time::OffsetDateTime,
    expected_amount: Money,
    counted_amount: Money,
    difference_minor: i64,
) -> AppResult<()> {
    let result = sqlx::query(
            "UPDATE cash_shift SET status = 'closed', closed_by = ?, closed_at = ?, expected_amount_minor = ?, counted_amount_minor = ?, difference_minor = ? WHERE id = ? AND organization_id = ? AND status = 'open'",
        )
        .bind(closed_by.to_string())
        .bind(format_ts(closed_at))
        .bind(expected_amount.minor_units())
        .bind(counted_amount.minor_units())
        .bind(difference_minor)
        .bind(shift_id.to_string())
        .bind(organization_id.to_string())
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(super) async fn insert_cash_movement(
    tx: &mut SqliteTx,
    movement: &CashMovement,
) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO cash_movement (id, organization_id, branch_id, register_id, shift_id, kind, amount_minor, reason, actor_user_id, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(movement.id.to_string())
        .bind(movement.organization_id.to_string())
        .bind(movement.branch_id.to_string())
        .bind(movement.register_id.to_string())
        .bind(movement.shift_id.to_string())
        .bind(movement.kind.as_str())
        .bind(movement.amount.minor_units())
        .bind(&movement.reason)
        .bind(movement.actor_user_id.map(|id| id.to_string()))
        .bind(format_ts(movement.occurred_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_cash_movements(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    shift_id: pos_core::ids::CashShiftId,
) -> AppResult<Vec<CashMovement>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, shift_id, kind, amount_minor, reason, actor_user_id, occurred_at FROM cash_movement WHERE organization_id = ? AND shift_id = ? ORDER BY occurred_at",
        )
        .bind(organization_id.to_string())
        .bind(shift_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| -> AppResult<CashMovement> {
            let kind = pos_core::cash::CashMovementKind::from_key(&row.get::<String, _>("kind"))
                .ok_or(AppError::Storage)?;
            Ok(CashMovement {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_id(row.get("branch_id"))?,
                register_id: parse_id(row.get("register_id"))?,
                shift_id: parse_id(row.get("shift_id"))?,
                kind,
                amount: Money::from_minor_units(row.get("amount_minor"))
                    .map_err(AppError::Domain)?,
                reason: row.get("reason"),
                actor_user_id: parse_optional_id(row.get("actor_user_id"))?,
                occurred_at: parse_ts(row.get("occurred_at"))?,
            })
        })
        .collect()
}

pub(super) async fn list_closed_cash_shifts(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<CashShift>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status, closed_by, closed_at, expected_amount_minor, counted_amount_minor, difference_minor FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND status = 'closed' ORDER BY closed_at DESC",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    rows.into_iter()
        .map(|row| -> AppResult<CashShift> {
            Ok(CashShift {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_id(row.get("branch_id"))?,
                register_id: parse_id(row.get("register_id"))?,
                opened_by: parse_id(row.get("opened_by"))?,
                opened_at: parse_ts(row.get("opened_at"))?,
                opening_amount: Money::from_minor_units(row.get("opening_amount_minor"))
                    .map_err(AppError::Domain)?,
                status: CashShiftStatus::Closed,
                closed_by: parse_optional_id(row.get("closed_by"))?,
                closed_at: row
                    .get::<Option<String>, _>("closed_at")
                    .map(parse_ts)
                    .transpose()?,
                expected_amount: row
                    .get::<Option<i64>, _>("expected_amount_minor")
                    .map(Money::from_minor_units)
                    .transpose()
                    .map_err(AppError::Domain)?,
                counted_amount: row
                    .get::<Option<i64>, _>("counted_amount_minor")
                    .map(Money::from_minor_units)
                    .transpose()
                    .map_err(AppError::Domain)?,
                difference_minor: row.get("difference_minor"),
            })
        })
        .collect()
}
