//! Usuarios locales.

use super::*;

pub(super) async fn insert_user(tx: &mut SqliteTx, user: &User) -> AppResult<()> {
    sqlx::query(
            "INSERT INTO app_user \
             (id, organization_id, branch_id, username, display_name, role, password_hash, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user.id.to_string())
        .bind(user.organization_id.to_string())
        .bind(user.branch_id.map(|id| id.to_string()))
        .bind(&user.username)
        .bind(&user.display_name)
        .bind(user.role.to_string())
        .bind(&user.password_hash)
        .bind(i64::from(user.active))
        .bind(format_ts(user.created_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_user_by_username(
    tx: &mut SqliteTx,
    username: &str,
) -> AppResult<Option<User>> {
    let row = sqlx::query(
        "SELECT id, organization_id, branch_id, username, display_name, role, password_hash, \
             active, created_at FROM app_user WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;

    row.map(user_from_row).transpose()
}

pub(super) async fn find_user_by_id(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    user_id: UserId,
) -> AppResult<Option<User>> {
    let row = sqlx::query(
        "SELECT id, organization_id, branch_id, username, display_name, role, password_hash, \
             active, created_at FROM app_user WHERE organization_id = ? AND id = ?",
    )
    .bind(organization_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;

    row.map(user_from_row).transpose()
}

pub(super) async fn list_users(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Vec<User>> {
    let rows = sqlx::query(
        "SELECT id, organization_id, branch_id, username, display_name, role, password_hash, \
             active, created_at FROM app_user \
             WHERE organization_id = ? AND branch_id = ? ORDER BY display_name, username",
    )
    .bind(organization_id.to_string())
    .bind(branch_id.to_string())
    .fetch_all(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;

    rows.into_iter().map(user_from_row).collect()
}

pub(super) async fn update_user_active(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    user_id: UserId,
    active: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE app_user SET active = ? WHERE organization_id = ? AND id = ?")
        .bind(i64::from(active))
        .bind(organization_id.to_string())
        .bind(user_id.to_string())
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn update_user_password(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    user_id: UserId,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE app_user SET password_hash = ? WHERE organization_id = ? AND id = ?")
        .bind(password_hash)
        .bind(organization_id.to_string())
        .bind(user_id.to_string())
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn count_active_owners(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS total FROM app_user \
             WHERE organization_id = ? AND role = 'owner' AND active = 1",
    )
    .bind(organization_id.to_string())
    .fetch_one(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;
    Ok(row.get("total"))
}

pub(super) async fn count_users(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS total FROM app_user WHERE organization_id = ?")
        .bind(organization_id.to_string())
        .fetch_one(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(row.get("total"))
}
