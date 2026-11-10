//! DTO de usuarios, sesion y bootstrap.

use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapRequest {
    pub organization_name: String,
    pub branch_name: String,
    pub branch_code: String,
    pub owner_username: String,
    pub owner_display_name: String,
    pub owner_password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDraft {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetUserActiveDraft {
    pub user_id: String,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResetUserPasswordDraft {
    pub user_id: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChangeOwnPasswordDraft {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserView {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl UserView {
    pub(in crate::service) fn from_user(user: &User) -> Self {
        Self {
            id: user.id.to_string(),
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            role: user.role.to_string(),
            active: user.active,
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapResponse {
    pub organization_id: OrganizationId,
    pub branch_id: BranchId,
    pub user_id: UserId,
    #[serde(with = "time::serde::rfc3339")]
    pub trial_ends_at: OffsetDateTime,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}
