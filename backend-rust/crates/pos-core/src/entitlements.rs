use crate::error::{DomainError, DomainResult};
use crate::ids::{InstallationId, OrganizationId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Duración aprobada del Trial: 30 días desde la activación registrada.
pub const TRIAL_DURATION_DAYS: i64 = 30;

/// Edición comercial contratada. El plan no debe confundirse con el estado de
/// cobro ni con los permisos RBAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanName {
    Trial,
    Local,
    Pro,
    Enterprise,
}

impl PlanName {
    pub fn as_str(self) -> &'static str {
        match self {
            PlanName::Trial => "trial",
            PlanName::Local => "local",
            PlanName::Pro => "pro",
            PlanName::Enterprise => "enterprise",
        }
    }
}

/// Capacidades comerciales con claves estables. Un entitlement nunca sustituye
/// a un permiso RBAC: ambos deben cumplirse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKey {
    Catalog,
    Sales,
    Inventory,
    Purchasing,
    Expenses,
    Reports,
    LocalBackups,
    CloudSync,
    RemoteAdmin,
    EnterpriseIntegrations,
}

impl FeatureKey {
    pub const ALL: [FeatureKey; 10] = [
        FeatureKey::Catalog,
        FeatureKey::Sales,
        FeatureKey::Inventory,
        FeatureKey::Purchasing,
        FeatureKey::Expenses,
        FeatureKey::Reports,
        FeatureKey::LocalBackups,
        FeatureKey::CloudSync,
        FeatureKey::RemoteAdmin,
        FeatureKey::EnterpriseIntegrations,
    ];

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.as_str() == key)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FeatureKey::Catalog => "catalog",
            FeatureKey::Sales => "sales",
            FeatureKey::Inventory => "inventory",
            FeatureKey::Purchasing => "purchasing",
            FeatureKey::Expenses => "expenses",
            FeatureKey::Reports => "reports",
            FeatureKey::LocalBackups => "backups.local",
            FeatureKey::CloudSync => "sync.cloud",
            FeatureKey::RemoteAdmin => "admin.remote",
            FeatureKey::EnterpriseIntegrations => "enterprise.integrations",
        }
    }
}

/// Estado comercial observable. `Trial` y `Active` habilitan nuevas
/// operaciones; `Grace` es un plazo pendiente de decisión (D-04) y
/// `ReadOnly` conserva lectura, exportación y respaldo sin nuevas ventas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Trial,
    Active,
    Grace,
    ReadOnly,
    Expired,
}

impl LicenseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            LicenseStatus::Trial => "trial",
            LicenseStatus::Active => "active",
            LicenseStatus::Grace => "grace",
            LicenseStatus::ReadOnly => "read_only",
            LicenseStatus::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommercialState {
    pub installation_id: InstallationId,
    pub organization_id: OrganizationId,
    pub plan: PlanName,
    pub status: LicenseStatus,
    pub features: BTreeSet<FeatureKey>,
    pub valid_until: time::OffsetDateTime,
}

impl CommercialState {
    pub fn new_trial(
        installation_id: InstallationId,
        organization_id: OrganizationId,
        now: time::OffsetDateTime,
    ) -> Self {
        Self {
            installation_id,
            organization_id,
            plan: PlanName::Trial,
            status: LicenseStatus::Trial,
            features: local_features(),
            valid_until: now + time::Duration::days(TRIAL_DURATION_DAYS),
        }
    }

    /// Estado efectivo considerando el reloj. La lógica de gracia/lease sigue
    /// pendiente (D-04); por ahora el vencimiento pasa a solo lectura.
    pub fn effective_status(&self, now: time::OffsetDateTime) -> LicenseStatus {
        if matches!(
            self.status,
            LicenseStatus::Expired | LicenseStatus::ReadOnly
        ) {
            return self.status;
        }
        if now > self.valid_until {
            return LicenseStatus::ReadOnly;
        }
        self.status
    }

    pub fn allows(&self, feature: FeatureKey, now: time::OffsetDateTime) -> bool {
        !matches!(
            self.effective_status(now),
            LicenseStatus::Expired | LicenseStatus::ReadOnly
        ) && self.features.contains(&feature)
    }

    pub fn require_feature(
        &self,
        feature: FeatureKey,
        now: time::OffsetDateTime,
    ) -> DomainResult<()> {
        if self.allows(feature, now) {
            Ok(())
        } else {
            Err(DomainError::FeatureNotEntitled {
                feature: feature.as_str(),
            })
        }
    }

    /// Bloquea nuevas operaciones comerciales cuando la licencia venció, sin
    /// impedir lectura, exportación ni respaldo.
    pub fn require_writes_allowed(&self, now: time::OffsetDateTime) -> DomainResult<()> {
        match self.effective_status(now) {
            LicenseStatus::Trial | LicenseStatus::Active | LicenseStatus::Grace => Ok(()),
            LicenseStatus::ReadOnly | LicenseStatus::Expired => {
                Err(DomainError::CommercialStateBlocked)
            }
        }
    }
}

pub fn local_features() -> BTreeSet<FeatureKey> {
    [
        FeatureKey::Catalog,
        FeatureKey::Sales,
        FeatureKey::Inventory,
        FeatureKey::Purchasing,
        FeatureKey::Expenses,
        FeatureKey::Reports,
        FeatureKey::LocalBackups,
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(now: time::OffsetDateTime) -> CommercialState {
        CommercialState::new_trial(InstallationId::new(), OrganizationId::new(), now)
    }

    #[test]
    fn trial_dura_treinta_dias_y_luego_es_solo_lectura() {
        let start = time::OffsetDateTime::now_utc();
        let state = trial(start);
        assert_eq!(
            state.valid_until,
            start + time::Duration::days(TRIAL_DURATION_DAYS)
        );
        assert_eq!(state.effective_status(start), LicenseStatus::Trial);
        assert!(
            state
                .require_writes_allowed(start + time::Duration::days(29))
                .is_ok()
        );
        assert_eq!(
            state.effective_status(start + time::Duration::days(31)),
            LicenseStatus::ReadOnly
        );
        assert_eq!(
            state
                .require_writes_allowed(start + time::Duration::days(31))
                .unwrap_err(),
            DomainError::CommercialStateBlocked
        );
    }

    #[test]
    fn local_no_incluye_sincronizacion() {
        let now = time::OffsetDateTime::now_utc();
        let mut state = trial(now);
        state.plan = PlanName::Local;
        assert!(state.allows(FeatureKey::Sales, now));
        assert!(!state.allows(FeatureKey::CloudSync, now));
        assert_eq!(
            state
                .require_feature(FeatureKey::CloudSync, now)
                .unwrap_err(),
            DomainError::FeatureNotEntitled {
                feature: "sync.cloud"
            }
        );
    }
}
