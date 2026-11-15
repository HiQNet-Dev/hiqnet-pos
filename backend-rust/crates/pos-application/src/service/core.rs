//! Construccion y estado del servicio.

use super::*;

impl PosService {
    pub fn new(store: SharedStore, hasher: SharedHasher, clock: SharedClock) -> Self {
        Self {
            store,
            hasher,
            clock,
            license: None,
            device_signer: None,
            enrollment_client: None,
            enrollment_url: None,
        }
    }

    /// Instala el verificador de licencias. Sin el, cualquier licencia firmada
    /// se considera no verificable y la instalacion pasa a solo lectura.
    #[must_use]
    pub fn with_license_verifier(mut self, verifier: SharedLicenseVerifier) -> Self {
        self.license = Some(verifier);
        self
    }

    /// Habilita el enrolamiento con Platform. Sin identidad de dispositivo o
    /// cliente, la operación devuelve un error explícito y la app sigue offline.
    #[must_use]
    pub fn with_platform_enrollment(
        mut self,
        signer: SharedDeviceSigner,
        client: SharedPlatformEnrollmentClient,
        platform_url: String,
    ) -> Self {
        self.device_signer = Some(signer);
        self.enrollment_client = Some(client);
        self.enrollment_url = Some(platform_url);
        self
    }

    pub async fn status(&self) -> AppResult<AppStatus> {
        let mut tx = self.store.begin().await?;
        let state = tx.installation_state().await?;
        let user_count = match &state {
            Some(state) => tx.count_users(state.organization_id).await?,
            None => 0,
        };
        tx.rollback().await?;

        let commercial = match state {
            Some(state) => Some(self.commercial_view(&state)?.into()),
            None => None,
        };

        Ok(AppStatus {
            initialized: commercial.is_some(),
            product_version: pos_core::PRODUCT_VERSION,
            commercial,
            user_count,
        })
    }
}
