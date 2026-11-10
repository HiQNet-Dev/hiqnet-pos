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

/// Resultado de resolver el estado comercial desde lo persistido.
pub(super) enum LicenseResolution {
    /// No hay documento firmado: se conserva el trial local.
    NoGrant,
    /// Documento validado criptograficamente.
    Verified(VerifiedLicense),
    /// Hay documento pero no autoriza.
    Rejected(&'static str),
}

/// Estado comercial resultante de una licencia que no puede acreditarse.
///
/// Sin funciones y en solo lectura: ninguna columna editable de SQLite puede
/// reactivar la operacion comercial por si misma.
fn revoked_state(state: &InstallationState) -> CommercialState {
    CommercialState {
        installation_id: state.installation_id,
        organization_id: state.organization_id,
        plan: PlanName::Trial,
        status: LicenseStatus::ReadOnly,
        features: BTreeSet::new(),
        valid_until: state.valid_until,
    }
}

/// Resuelve el estado comercial.
///
/// Regla: si existe un documento firmado, **solo** autoriza cuando la firma se
/// verifica y sus identidades coinciden con la instalacion. Las columnas
/// `plan`, `features` y `valid_until` nunca autorizan por si solas cuando hay
/// un grant presente; sin grant se conserva el trial local.
pub(super) fn resolve_commercial(
    state: &InstallationState,
    verifier: Option<&dyn LicenseVerifier>,
) -> (CommercialState, LicenseResolution) {
    let grant_json = state
        .license_grant_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let signature = state
        .license_signature_b64
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (grant_json, signature) {
        (None, None) => (commercial_from_trial_row(state), LicenseResolution::NoGrant),
        (Some(json), Some(signature)) => {
            let Some(verifier) = verifier else {
                return (
                    revoked_state(state),
                    LicenseResolution::Rejected("public_key_unavailable"),
                );
            };
            let license = match verifier.verify(json, signature) {
                Ok(license) => license,
                Err(_) => {
                    return (
                        revoked_state(state),
                        LicenseResolution::Rejected("signature_rejected"),
                    );
                }
            };
            if license.installation_id != state.installation_id {
                return (
                    revoked_state(state),
                    LicenseResolution::Rejected("installation_mismatch"),
                );
            }
            if license.organization_id != state.organization_id {
                return (
                    revoked_state(state),
                    LicenseResolution::Rejected("organization_mismatch"),
                );
            }
            let status = if license.plan == PlanName::Trial {
                LicenseStatus::Trial
            } else {
                LicenseStatus::Active
            };
            (
                CommercialState {
                    installation_id: license.installation_id,
                    organization_id: license.organization_id,
                    plan: license.plan,
                    status,
                    features: license.features.clone(),
                    valid_until: license.valid_until,
                },
                LicenseResolution::Verified(license),
            )
        }
        // Documento incompleto (grant sin firma o firma sin grant): no autoriza
        // y no cae al trial.
        _ => (
            revoked_state(state),
            LicenseResolution::Rejected("incomplete_document"),
        ),
    }
}

/// Estado comercial de una instalacion sin licencia firmada (trial local).
pub(super) fn commercial_from_trial_row(state: &InstallationState) -> CommercialState {
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

    CommercialState {
        installation_id: state.installation_id,
        organization_id: state.organization_id,
        plan,
        status,
        features,
        valid_until: state.valid_until,
    }
}

impl PosService {
    /// Estado comercial exigido antes de cualquier mutacion protegida.
    ///
    /// Es el unico camino: ningun caso de uso lee `plan`, `features` ni
    /// `valid_until` directamente de la fila de instalacion.
    pub(super) async fn require_commercial(
        &self,
        tx: &mut dyn crate::ports::StoreTx,
    ) -> AppResult<CommercialState> {
        let state = tx
            .installation_state()
            .await?
            .ok_or(pos_core::DomainError::AlreadyInitialized)?;
        let (commercial, resolution) = resolve_commercial(&state, self.license.as_deref());
        match resolution {
            LicenseResolution::Rejected(reason) => tracing::warn!(
                installation_id = %state.installation_id,
                reason,
                "licencia no verificable: la instalacion queda en solo lectura"
            ),
            LicenseResolution::Verified(license) => tracing::debug!(
                plan = license.plan.as_str(),
                valid_until = %license.valid_until,
                "licencia verificada"
            ),
            LicenseResolution::NoGrant => {}
        }
        Ok(commercial)
    }

    /// Vista comercial para la interfaz, derivada del mismo camino verificado.
    pub(super) fn commercial_view(
        &self,
        state: &InstallationState,
    ) -> AppResult<CommercialStateView> {
        let (commercial, resolution) = resolve_commercial(state, self.license.as_deref());
        match resolution {
            LicenseResolution::Rejected(reason) => tracing::warn!(
                installation_id = %state.installation_id,
                reason,
                "licencia no verificable: se muestra en solo lectura"
            ),
            LicenseResolution::Verified(license) => tracing::debug!(
                plan = license.plan.as_str(),
                valid_until = %license.valid_until,
                "licencia verificada"
            ),
            LicenseResolution::NoGrant => {}
        }
        let effective = commercial.effective_status(self.clock.now());
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
