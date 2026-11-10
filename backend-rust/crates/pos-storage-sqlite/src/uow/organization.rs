//! Instalacion, organizacion y sucursal.

use super::*;

pub(super) async fn installation_state(tx: &mut SqliteTx) -> AppResult<Option<InstallationState>> {
    let tx = tx.tx()?;
    let row = sqlx::query(
        "SELECT installation_id, organization_id, plan, license_status, features_json, \
             trial_started_at, valid_until, license_grant_json, license_signature_b64 \
             FROM local_installation_state WHERE id = 1",
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;

    row.map(|row| -> AppResult<InstallationState> {
        let features: Vec<String> = serde_json::from_str(&row.get::<String, _>("features_json"))
            .map_err(|_| AppError::Storage)?;
        Ok(InstallationState {
            installation_id: parse_id(row.get("installation_id"))?,
            organization_id: parse_id(row.get("organization_id"))?,
            trial_started_at: parse_ts(row.get("trial_started_at"))?,
            plan: row.get("plan"),
            license_status: row.get("license_status"),
            features,
            valid_until: parse_ts(row.get("valid_until"))?,
            license_grant_json: row.get("license_grant_json"),
            license_signature_b64: row.get("license_signature_b64"),
        })
    })
    .transpose()
}

pub(super) async fn save_installation_state(
    tx: &mut SqliteTx,
    state: &InstallationState,
) -> AppResult<()> {
    let features = serde_json::to_string(&state.features).map_err(|_| AppError::Storage)?;
    sqlx::query(
            "INSERT INTO local_installation_state \
             (id, installation_id, organization_id, plan, license_status, features_json, \
              trial_started_at, valid_until, license_grant_json, license_signature_b64, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (id) DO UPDATE SET \
              installation_id = excluded.installation_id, \
              organization_id = excluded.organization_id, \
              plan = excluded.plan, \
              license_status = excluded.license_status, \
              features_json = excluded.features_json, \
              trial_started_at = excluded.trial_started_at, \
              valid_until = excluded.valid_until, \
              license_grant_json = excluded.license_grant_json, \
              license_signature_b64 = excluded.license_signature_b64, \
              updated_at = excluded.updated_at",
        )
        .bind(state.installation_id.to_string())
        .bind(state.organization_id.to_string())
        .bind(&state.plan)
        .bind(&state.license_status)
        .bind(features)
        .bind(format_ts(state.trial_started_at))
        .bind(format_ts(state.valid_until))
        .bind(&state.license_grant_json)
        .bind(&state.license_signature_b64)
        .bind(format_ts(OffsetDateTime::now_utc()))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn insert_organization(
    tx: &mut SqliteTx,
    organization: &Organization,
) -> AppResult<()> {
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES (?, ?, ?)")
        .bind(organization.id.to_string())
        .bind(&organization.name)
        .bind(format_ts(organization.created_at))
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn insert_branch(tx: &mut SqliteTx, branch: &Branch) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO branch (id, organization_id, name, code, created_at) \
             VALUES (?, ?, ?, ?, ?)",
    )
    .bind(branch.id.to_string())
    .bind(branch.organization_id.to_string())
    .bind(&branch.name)
    .bind(&branch.code)
    .bind(format_ts(branch.created_at))
    .execute(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_branch(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    branch_id: BranchId,
) -> AppResult<Option<Branch>> {
    let row = sqlx::query(
        "SELECT id, organization_id, name, code, created_at FROM branch \
             WHERE organization_id = ? AND id = ?",
    )
    .bind(organization_id.to_string())
    .bind(branch_id.to_string())
    .fetch_optional(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;
    row.map(|row| -> AppResult<Branch> {
        Ok(Branch {
            id: parse_id(row.get("id"))?,
            organization_id: parse_id(row.get("organization_id"))?,
            name: row.get("name"),
            code: row.get("code"),
            created_at: parse_ts(row.get("created_at"))?,
        })
    })
    .transpose()
}
