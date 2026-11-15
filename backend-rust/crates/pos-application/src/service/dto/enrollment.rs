//! DTO del enrolamiento con HiQNet Platform.

use super::*;

/// Estado visible del enrolamiento. Nunca incluye `device_code`.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformEnrollmentView {
    pub status: &'static str,
    pub user_code: Option<String>,
    pub verification_uri: Option<String>,
    pub expires_at: Option<String>,
    pub organization_id: Option<String>,
    pub installation_id: InstallationId,
    pub completed: bool,
}
