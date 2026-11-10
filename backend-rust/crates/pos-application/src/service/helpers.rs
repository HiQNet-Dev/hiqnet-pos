//! Utilidades compartidas entre dominios.

use super::*;

pub(super) fn parse_rfc3339(value: &str) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(value.trim(), &time::format_description::well_known::Rfc3339)
        .map_err(|_| AppError::InvalidRequest)
}

pub(super) fn money_minor(value: i64) -> AppResult<String> {
    Ok(Money::from_minor_units(value)
        .map_err(AppError::Domain)?
        .to_string())
}

/// Métrica firmada del período (ventas netas, diferencias de arqueo).
/// `Money` sigue siendo no negativo para cobros y reembolsos.
pub(super) fn signed_money_minor(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.unsigned_abs();
    format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
}

pub(super) fn cash_totals(movements: &[CashMovement]) -> pos_core::DomainResult<(Money, Money)> {
    let mut entries = Money::ZERO;
    let mut exits = Money::ZERO;
    for movement in movements {
        match movement.kind {
            CashMovementKind::In => entries = entries.checked_add(movement.amount)?,
            CashMovementKind::Out => exits = exits.checked_add(movement.amount)?,
        }
    }
    Ok((entries, exits))
}

pub(super) fn expected_amount(
    shift: &CashShift,
    movements: &[CashMovement],
) -> pos_core::DomainResult<Money> {
    let (entries, exits) = cash_totals(movements)?;
    shift
        .opening_amount
        .checked_add(entries)?
        .checked_sub(exits)
}

pub(super) fn validate_password(password: &str) -> AppResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(pos_core::DomainError::WeakPassword {
            min: MIN_PASSWORD_LEN,
        }
        .into());
    }
    if password.trim().is_empty() {
        return Err(pos_core::DomainError::WeakPassword {
            min: MIN_PASSWORD_LEN,
        }
        .into());
    }
    Ok(())
}

pub(super) async fn require_commercial(
    tx: &mut dyn crate::ports::StoreTx,
) -> AppResult<CommercialState> {
    let state = tx
        .installation_state()
        .await?
        .ok_or(pos_core::DomainError::AlreadyInitialized)?;
    commercial_from_installation(&state)
}

pub(super) fn commercial_from_installation(
    state: &InstallationState,
) -> AppResult<CommercialState> {
    let plan = match state.plan.as_str() {
        "trial" => PlanName::Trial,
        "local" => PlanName::Local,
        "pro" => PlanName::Pro,
        "enterprise" => PlanName::Enterprise,
        _ => PlanName::Trial,
    };
    let status = match state.license_status.as_str() {
        "active" => LicenseStatus::Active,
        "grace" => LicenseStatus::Grace,
        "read_only" => LicenseStatus::ReadOnly,
        "expired" => LicenseStatus::Expired,
        _ => LicenseStatus::Trial,
    };
    let features: BTreeSet<FeatureKey> = state
        .features
        .iter()
        .filter_map(|key| FeatureKey::from_key(key))
        .collect();

    // La verificación de la firma del grant pertenece a `pos-licensing`; este
    // estado refleja únicamente lo observado por la aplicación.
    Ok(CommercialState {
        installation_id: state.installation_id,
        organization_id: state.organization_id,
        plan,
        status,
        features,
        valid_until: state.valid_until,
    })
}

pub(super) fn commercial_view(
    state: &InstallationState,
    now: OffsetDateTime,
) -> AppResult<CommercialStateView> {
    let commercial = commercial_from_installation(state)?;
    let effective = commercial.effective_status(now);
    Ok(CommercialStateView {
        plan: commercial.plan.as_str(),
        status: match effective {
            LicenseStatus::Trial => "trial",
            LicenseStatus::Active => "active",
            LicenseStatus::Grace => "grace",
            LicenseStatus::ReadOnly => "read_only",
            LicenseStatus::Expired => "expired",
        },
        valid_until: commercial.valid_until,
        features: commercial
            .features
            .iter()
            .map(|feature| feature.as_str())
            .collect(),
        read_only: matches!(effective, LicenseStatus::ReadOnly | LicenseStatus::Expired),
    })
}

pub(super) fn installation_from_commercial(commercial: &CommercialState) -> InstallationState {
    InstallationState {
        installation_id: commercial.installation_id,
        organization_id: commercial.organization_id,
        trial_started_at: commercial.valid_until - time::Duration::days(30),
        plan: commercial.plan.as_str().to_owned(),
        license_status: LicenseStatus::Trial.as_str().to_owned(),
        features: local_features()
            .iter()
            .map(|feature| feature.as_str().to_owned())
            .collect(),
        valid_until: commercial.valid_until,
        license_grant_json: None,
        license_signature_b64: None,
    }
}
