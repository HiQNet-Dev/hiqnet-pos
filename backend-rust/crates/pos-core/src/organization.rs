use crate::error::{DomainError, DomainResult};
use crate::ids::{BranchId, OrganizationId};
use serde::{Deserialize, Serialize};

/// Organización (negocio) propietaria de los datos locales.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    pub created_at: time::OffsetDateTime,
}

impl Organization {
    pub fn new(id: OrganizationId, name: impl Into<String>) -> DomainResult<Self> {
        let name = name.into();
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyName);
        }
        Ok(Self {
            id,
            name: trimmed.to_owned(),
            created_at: time::OffsetDateTime::now_utc(),
        })
    }
}

/// Sucursal operativa. En el MVP Local existe una sucursal inicial; el modelo
/// ya incluye el vínculo a organización para no bloquear multisucursal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub id: BranchId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub code: String,
    pub created_at: time::OffsetDateTime,
}

impl Branch {
    pub fn new(
        id: BranchId,
        organization_id: OrganizationId,
        name: impl Into<String>,
        code: impl Into<String>,
    ) -> DomainResult<Self> {
        let name = name.into();
        let code = code.into();
        if name.trim().is_empty() {
            return Err(DomainError::EmptyName);
        }
        let code = code.trim().to_ascii_uppercase();
        if code.is_empty() || code.len() > 16 {
            return Err(DomainError::InvalidSku(format!(
                "código de sucursal '{code}'"
            )));
        }
        Ok(Self {
            id,
            organization_id,
            name: name.trim().to_owned(),
            code,
            created_at: time::OffsetDateTime::now_utc(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_nombres_y_codigos() {
        let org = Organization::new(OrganizationId::new(), "  Abarrotes La Esquina  ").unwrap();
        assert_eq!(org.name, "Abarrotes La Esquina");

        let branch = Branch::new(BranchId::new(), org.id, "Matriz", "  mx-01 ").unwrap();
        assert_eq!(branch.code, "MX-01");
    }

    #[test]
    fn rechaza_nombres_vacios() {
        assert_eq!(
            Organization::new(OrganizationId::new(), "   ").unwrap_err(),
            DomainError::EmptyName
        );
    }
}
