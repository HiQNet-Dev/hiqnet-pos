use crate::error::{DomainError, DomainResult};
use crate::ids::{BranchId, OrganizationId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Permisos RBAC locales con claves estables (no traducibles). El texto en
/// español se resuelve en la capa de presentación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ManageOrganization,
    ManageUsers,
    ManageCatalog,
    Sell,
    ViewReports,
    ManageBackups,
    ViewAudit,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::ManageOrganization => "organization.manage",
            Permission::ManageUsers => "users.manage",
            Permission::ManageCatalog => "catalog.manage",
            Permission::Sell => "sales.sell",
            Permission::ViewReports => "reports.view",
            Permission::ManageBackups => "backups.manage",
            Permission::ViewAudit => "audit.view",
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Roles locales del MVP. El propietario concentra la administración inicial;
/// el cajero solo opera ventas y consulta catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Cashier,
}

impl Role {
    pub fn permissions(self) -> BTreeSet<Permission> {
        let permissions: &[Permission] = match self {
            Role::Owner => &[
                Permission::ManageOrganization,
                Permission::ManageUsers,
                Permission::ManageCatalog,
                Permission::Sell,
                Permission::ViewReports,
                Permission::ManageBackups,
                Permission::ViewAudit,
            ],
            Role::Cashier => &[Permission::Sell],
        };
        permissions.iter().copied().collect()
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Owner => "owner",
            Role::Cashier => "cashier",
        })
    }
}

/// Usuario operador local. El hash de contraseña nunca sale del backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub organization_id: OrganizationId,
    pub branch_id: Option<BranchId>,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub password_hash: String,
    pub active: bool,
    pub created_at: time::OffsetDateTime,
}

impl User {
    pub fn new(
        id: UserId,
        organization_id: OrganizationId,
        branch_id: Option<BranchId>,
        username: impl Into<String>,
        display_name: impl Into<String>,
        role: Role,
        password_hash: String,
    ) -> DomainResult<Self> {
        let username = normalize_username(&username.into())?;
        let display_name = display_name.into();
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(DomainError::EmptyName);
        }
        Ok(Self {
            id,
            organization_id,
            branch_id,
            username,
            display_name: display_name.to_owned(),
            role,
            password_hash,
            active: true,
            created_at: time::OffsetDateTime::now_utc(),
        })
    }

    pub fn permissions(&self) -> BTreeSet<Permission> {
        self.role.permissions()
    }
}

pub fn normalize_username(value: &str) -> DomainResult<String> {
    let username = value.trim().to_lowercase();
    if username.len() < 3 || username.len() > 40 {
        return Err(DomainError::InvalidSku(format!(
            "nombre de usuario '{username}'"
        )));
    }
    let valid = username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(DomainError::InvalidSku(format!(
            "nombre de usuario '{username}'"
        )));
    }
    Ok(username)
}

/// Actor autenticado. Se construye exclusivamente a partir del principal
/// verificado en Rust; nunca desde datos enviados por el frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub user_id: UserId,
    pub organization_id: OrganizationId,
    pub branch_id: Option<BranchId>,
    pub display_name: String,
    pub role: Role,
    pub permissions: BTreeSet<Permission>,
}

impl Actor {
    pub fn from_user(user: &User) -> Self {
        Self {
            user_id: user.id,
            organization_id: user.organization_id,
            branch_id: user.branch_id,
            display_name: user.display_name.clone(),
            role: user.role,
            permissions: user.permissions(),
        }
    }

    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Autorización deny-by-default para operaciones sensibles.
    pub fn require(&self, permission: Permission) -> DomainResult<()> {
        if self.has(permission) {
            Ok(())
        } else {
            Err(DomainError::AccessDenied {
                required: permission.as_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role) -> User {
        User::new(
            UserId::new(),
            OrganizationId::new(),
            None,
            " Owner ",
            "Propietario",
            role,
            "hash".into(),
        )
        .unwrap()
    }

    #[test]
    fn owner_puede_administrar_y_cajero_no() {
        let owner = user(Role::Owner);
        let cashier = user(Role::Cashier);

        assert!(owner.permissions().contains(&Permission::ManageCatalog));
        assert!(!cashier.permissions().contains(&Permission::ManageCatalog));
        assert!(Actor::from_user(&cashier).require(Permission::Sell).is_ok());
        assert_eq!(
            Actor::from_user(&cashier)
                .require(Permission::ManageCatalog)
                .unwrap_err(),
            DomainError::AccessDenied {
                required: "catalog.manage"
            }
        );
    }

    #[test]
    fn normaliza_usuario_y_rechaza_invalidos() {
        assert_eq!(normalize_username(" Admin.User ").unwrap(), "admin.user");
        assert!(normalize_username("ab").is_err());
        assert!(normalize_username("bad user").is_err());
    }
}
