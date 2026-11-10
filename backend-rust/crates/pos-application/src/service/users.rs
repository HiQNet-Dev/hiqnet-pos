//! Usuarios, sesion y bootstrap.

use super::*;

impl PosService {
    pub async fn bootstrap(&self, request: BootstrapRequest) -> AppResult<BootstrapResponse> {
        validate_password(&request.owner_password)?;

        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if tx.installation_state().await?.is_some() {
            return Err(pos_core::DomainError::AlreadyInitialized.into());
        }

        let organization = Organization::new(OrganizationId::new(), request.organization_name)?;
        let branch = Branch::new(
            BranchId::new(),
            organization.id,
            request.branch_name,
            request.branch_code,
        )?;

        let password_hash = self.hasher.hash(&request.owner_password)?;
        let user = User::new(
            UserId::new(),
            organization.id,
            Some(branch.id),
            request.owner_username,
            request.owner_display_name,
            Role::Owner,
            password_hash,
        )?;

        tx.insert_organization(&organization).await?;
        tx.insert_branch(&branch).await?;
        tx.insert_user(&user).await?;

        let trial = CommercialState::new_trial(InstallationId::new(), organization.id, now);
        let state = installation_from_commercial(&trial);
        tx.save_installation_state(&state).await?;

        tx.append_audit(&AuditEvent::new(
            organization.id,
            Some(branch.id),
            Some(user.id),
            AuditAction::InstallationBootstrapped,
            "organization",
            organization.id.to_string(),
            serde_json::json!({ "branch_code": branch.code }),
        ))
        .await?;

        tx.commit().await?;

        Ok(BootstrapResponse {
            organization_id: organization.id,
            branch_id: branch.id,
            user_id: user.id,
            trial_ends_at: trial.valid_until,
        })
    }

    pub async fn login(&self, request: LoginRequest) -> AppResult<Actor> {
        let username = normalize_username(&request.username)?;
        let mut tx = self.store.begin().await?;
        let user = match tx.find_user_by_username(&username).await? {
            Some(user) if user.active => user,
            _ => return Err(pos_core::DomainError::InvalidCredentials.into()),
        };

        if !self.hasher.verify(&request.password, &user.password_hash)? {
            return Err(pos_core::DomainError::InvalidCredentials.into());
        }

        tx.append_audit(&AuditEvent::new(
            user.organization_id,
            user.branch_id,
            Some(user.id),
            AuditAction::SessionLoggedIn,
            "user",
            user.id.to_string(),
            serde_json::json!({ "role": user.role.to_string() }),
        ))
        .await?;
        tx.commit().await?;

        Ok(Actor::from_user(&user))
    }

    pub async fn create_user(&self, actor: &Actor, draft: UserDraft) -> AppResult<UserView> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let role = Role::from_key(&draft.role).ok_or(pos_core::DomainError::UnknownRole)?;
        let username = normalize_username(&draft.username)?;
        validate_password(&draft.password)?;

        let mut tx = self.store.begin().await?;
        let commercial = self.require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        if tx.find_user_by_username(&username).await?.is_some() {
            return Err(pos_core::DomainError::DuplicateUsername.into());
        }

        let password_hash = self.hasher.hash(&draft.password)?;
        let user = User::new(
            UserId::new(),
            actor.organization_id,
            Some(branch_id),
            username,
            draft.display_name,
            role,
            password_hash,
        )?;
        tx.insert_user(&user).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::UserCreated,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username, "role": user.role.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(UserView::from_user(&user))
    }

    pub async fn list_users(&self, actor: &Actor) -> AppResult<Vec<UserView>> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let users = tx.list_users(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(users.iter().map(UserView::from_user).collect())
    }

    pub async fn set_user_active(
        &self,
        actor: &Actor,
        draft: SetUserActiveDraft,
    ) -> AppResult<UserView> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let user_id: UserId = draft.user_id.parse().map_err(|_| AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = self.require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let mut user = tx
            .find_user_by_id(actor.organization_id, user_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if user.branch_id != Some(branch_id) {
            return Err(AppError::NotFound);
        }

        if !draft.active {
            // Defensa en profundidad: ninguna operación puede dejar la
            // instalación sin un propietario activo. Cubre también la
            // desactivación de la cuenta propia.
            if user.active && user.is_owner() {
                let restantes = tx.count_active_owners(actor.organization_id).await? - 1;
                if restantes < 1 {
                    return Err(pos_core::DomainError::LastActiveOwner.into());
                }
            }
            user.deactivate();
        } else {
            user.activate();
        }

        tx.update_user_active(actor.organization_id, user.id, user.active)
            .await?;
        let action = if user.active {
            AuditAction::UserReactivated
        } else {
            AuditAction::UserDeactivated
        };
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            action,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username, "role": user.role.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(UserView::from_user(&user))
    }

    pub async fn reset_user_password(
        &self,
        actor: &Actor,
        draft: ResetUserPasswordDraft,
    ) -> AppResult<()> {
        actor.require(pos_core::identity::Permission::ManageUsers)?;
        validate_password(&draft.password)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let user_id: UserId = draft.user_id.parse().map_err(|_| AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = self.require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let user = tx
            .find_user_by_id(actor.organization_id, user_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if user.branch_id != Some(branch_id) {
            return Err(AppError::NotFound);
        }

        let password_hash = self.hasher.hash(&draft.password)?;
        tx.update_user_password(actor.organization_id, user.id, &password_hash)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::UserPasswordReset,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username }),
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn change_own_password(
        &self,
        actor: &Actor,
        draft: ChangeOwnPasswordDraft,
    ) -> AppResult<()> {
        validate_password(&draft.new_password)?;
        let branch_id = actor.branch_id;

        let mut tx = self.store.begin().await?;
        let commercial = self.require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let user = tx
            .find_user_by_id(actor.organization_id, actor.user_id)
            .await?
            .filter(|user| user.active)
            .ok_or(AppError::Unauthenticated)?;
        if !self
            .hasher
            .verify(&draft.current_password, &user.password_hash)?
        {
            return Err(pos_core::DomainError::InvalidCredentials.into());
        }

        let password_hash = self.hasher.hash(&draft.new_password)?;
        tx.update_user_password(actor.organization_id, user.id, &password_hash)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            branch_id,
            Some(actor.user_id),
            AuditAction::UserPasswordChanged,
            "user",
            user.id.to_string(),
            serde_json::json!({ "username": user.username }),
        ))
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn refresh_actor(&self, actor: &Actor) -> AppResult<Actor> {
        let mut tx = self.store.begin().await?;
        let user = tx
            .find_user_by_id(actor.organization_id, actor.user_id)
            .await?;
        tx.rollback().await?;
        match user {
            Some(user) if user.active => Ok(Actor::from_user(&user)),
            _ => Err(AppError::Unauthenticated),
        }
    }
}
