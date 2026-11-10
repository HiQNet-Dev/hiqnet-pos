//! Caja: registros, turnos y movimientos.

use super::*;

impl PosService {
    pub async fn create_cash_register(
        &self,
        actor: &Actor,
        draft: CashRegisterDraft,
    ) -> AppResult<CashRegisterView> {
        actor.require(pos_core::identity::Permission::ManageCashRegisters)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        let register = CashRegister::new(actor.organization_id, branch_id, draft.name, draft.code)?;
        tx.insert_cash_register(&register).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashRegisterCreated,
            "cash_register",
            register.id.to_string(),
            serde_json::json!({ "name": register.name, "code": register.code }),
        ))
        .await?;
        tx.commit().await?;
        Ok(CashRegisterView::from(&register))
    }

    pub async fn branch_summary(&self, actor: &Actor) -> AppResult<BranchView> {
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let branch = tx
            .find_branch(actor.organization_id, branch_id)
            .await?
            .ok_or(AppError::NotFound)?;
        tx.rollback().await?;
        Ok(BranchView {
            id: branch.id.to_string(),
            name: branch.name,
            code: branch.code,
        })
    }

    pub async fn list_cash_registers(&self, actor: &Actor) -> AppResult<Vec<CashRegisterView>> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let registers = tx
            .list_cash_registers(actor.organization_id, branch_id)
            .await?;
        tx.rollback().await?;
        Ok(registers.iter().map(CashRegisterView::from).collect())
    }

    pub async fn open_cash_shift(
        &self,
        actor: &Actor,
        draft: CashOpenShiftDraft,
    ) -> AppResult<CashShiftView> {
        actor.require(pos_core::identity::Permission::OpenCashShift)?;
        let register_id: CashRegisterId =
            draft.register_id.parse().map_err(|_| AppError::NotFound)?;
        let opening: Money = draft.opening_amount.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        let registers = tx
            .list_cash_registers(actor.organization_id, branch_id)
            .await?;
        let register = registers
            .iter()
            .find(|item| item.id == register_id && item.active)
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if tx
            .find_open_cash_shift(actor.organization_id, register_id)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict);
        }
        let shift = CashShift::new(
            actor.organization_id,
            branch_id,
            register.id,
            actor.user_id,
            opening,
        );
        tx.insert_cash_shift(&shift).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashShiftOpened,
            "cash_shift",
            shift.id.to_string(),
            serde_json::json!({ "register_id": register_id.to_string(), "opening_amount": opening.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        let expected = shift.opening_amount;
        self.shift_view(actor, shift, Vec::new(), expected, Money::ZERO, 0)
    }

    pub async fn active_cash_shift(
        &self,
        actor: &Actor,
        register_id: CashRegisterId,
    ) -> AppResult<Option<CashShiftView>> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let mut tx = self.store.begin().await?;
        let Some(shift) = tx
            .find_open_cash_shift(actor.organization_id, register_id)
            .await?
        else {
            tx.rollback().await?;
            return Ok(None);
        };
        if shift.branch_id != actor.branch_id.ok_or(AppError::NotFound)? {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.view",
            }
            .into());
        }
        let movements = tx
            .list_cash_movements(actor.organization_id, shift.id)
            .await?;
        let expected = expected_amount(&shift, &movements)?;
        tx.rollback().await?;
        Ok(Some(self.shift_view(
            actor,
            shift,
            movements,
            expected,
            Money::ZERO,
            0,
        )?))
    }

    pub async fn register_cash_movement(
        &self,
        actor: &Actor,
        draft: CashMovementDraft,
    ) -> AppResult<CashMovementView> {
        actor.require(pos_core::identity::Permission::RecordCashMovement)?;
        let shift_id: CashShiftId = draft.shift_id.parse().map_err(|_| AppError::NotFound)?;
        let kind = CashMovementKind::from_key(&draft.kind).ok_or(AppError::NotFound)?;
        let amount: Money = draft.amount.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let open_shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if open_shift.status != CashShiftStatus::Open || open_shift.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        if kind == CashMovementKind::Out {
            let movements = tx
                .list_cash_movements(actor.organization_id, open_shift.id)
                .await?;
            let expected = expected_amount(&open_shift, &movements)?;
            if expected.minor_units() < amount.minor_units() {
                return Err(AppError::Conflict);
            }
        }
        let movement = CashMovement::new(
            actor.organization_id,
            branch_id,
            open_shift.register_id,
            open_shift.id,
            kind,
            amount,
            draft.reason,
            Some(actor.user_id),
        )?;
        tx.insert_cash_movement(&movement).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashMovementRegistered,
            "cash_movement",
            movement.id.to_string(),
            serde_json::json!({ "shift_id": shift_id.to_string(), "kind": kind.as_str(), "amount": amount.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(CashMovementView::from(&movement))
    }

    pub async fn close_cash_shift(
        &self,
        actor: &Actor,
        draft: CashCloseShiftDraft,
    ) -> AppResult<CashShiftView> {
        actor.require(pos_core::identity::Permission::CloseCashShift)?;
        let shift_id: CashShiftId = draft.shift_id.parse().map_err(|_| AppError::NotFound)?;
        let counted: Money = draft.counted_amount.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        let open_shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if open_shift.status != CashShiftStatus::Open || open_shift.branch_id != branch_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.view",
            }
            .into());
        }
        if actor.role != Role::Owner && open_shift.opened_by != actor.user_id {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.close",
            }
            .into());
        }
        let movements = tx
            .list_cash_movements(actor.organization_id, open_shift.id)
            .await?;
        let (entry_total, exit_total) = cash_totals(&movements)?;
        let expected = open_shift
            .opening_amount
            .checked_add(entry_total)?
            .checked_sub(exit_total)?;
        let difference_minor = counted.minor_units() - expected.minor_units();
        tx.close_cash_shift(
            actor.organization_id,
            open_shift.id,
            actor.user_id,
            self.clock.now(),
            expected,
            counted,
            difference_minor,
        )
        .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::CashShiftClosed,
            "cash_shift",
            open_shift.id.to_string(),
            serde_json::json!({ "counted": counted.to_string(), "expected": expected.to_string(), "difference_minor": difference_minor }),
        ))
        .await?;
        tx.commit().await?;
        let expected_string = expected.to_string();
        let counted_string = counted.to_string();
        Ok(CashShiftView {
            id: open_shift.id.to_string(),
            register_id: open_shift.register_id.to_string(),
            status: "closed".into(),
            opened_by: open_shift.opened_by.to_string(),
            opened_at: open_shift.opened_at,
            closed_by: Some(actor.user_id.to_string()),
            closed_at: Some(OffsetDateTime::now_utc()),
            opening_amount: open_shift.opening_amount.to_string(),
            expected_amount: expected_string,
            entry_total: entry_total.to_string(),
            exit_total: exit_total.to_string(),
            counted_amount: Some(counted_string),
            difference_minor: Some(difference_minor),
            movements: movements.iter().map(CashMovementView::from).collect(),
        })
    }

    pub async fn cash_shift_summary(
        &self,
        actor: &Actor,
        shift_id: CashShiftId,
    ) -> AppResult<CashShiftView> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let mut tx = self.store.begin().await?;
        let shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if shift.branch_id != actor.branch_id.ok_or(AppError::NotFound)? {
            return Err(pos_core::DomainError::AccessDenied {
                required: "cash.shift.view",
            }
            .into());
        }
        let movements = tx
            .list_cash_movements(actor.organization_id, shift.id)
            .await?;
        tx.rollback().await?;
        let expected = if shift.status == CashShiftStatus::Open {
            expected_amount(&shift, &movements)?
        } else {
            shift.expected_amount.ok_or(AppError::NotFound)?
        };
        let counted = shift.counted_amount.unwrap_or(Money::ZERO);
        let diff = shift.difference_minor.unwrap_or(0);
        let view = self.shift_view(actor, shift, movements, expected, counted, diff)?;
        Ok(view)
    }

    pub async fn list_cash_shift_history(&self, actor: &Actor) -> AppResult<Vec<CashShiftView>> {
        actor.require(pos_core::identity::Permission::ViewCashShift)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let closed = tx
            .list_closed_cash_shifts(actor.organization_id, branch_id)
            .await?;
        let mut views = Vec::new();
        for shift in closed {
            let movements = tx
                .list_cash_movements(actor.organization_id, shift.id)
                .await?;
            let expected = shift.expected_amount.unwrap_or(shift.opening_amount);
            let counted = shift.counted_amount.unwrap_or(Money::ZERO);
            let diff = shift.difference_minor.unwrap_or(0);
            views.push(self.shift_view(actor, shift, movements, expected, counted, diff)?);
        }
        tx.rollback().await?;
        Ok(views)
    }

    fn shift_view(
        &self,
        _actor: &Actor,
        shift: CashShift,
        movements: Vec<CashMovement>,
        expected: Money,
        counted: Money,
        difference_minor: i64,
    ) -> AppResult<CashShiftView> {
        let (entry_total, exit_total) = cash_totals(&movements)?;
        Ok(CashShiftView {
            id: shift.id.to_string(),
            register_id: shift.register_id.to_string(),
            status: shift.status.as_str().to_owned(),
            opened_by: shift.opened_by.to_string(),
            opened_at: shift.opened_at,
            closed_by: shift.closed_by.map(|id| id.to_string()),
            closed_at: shift.closed_at,
            opening_amount: shift.opening_amount.to_string(),
            expected_amount: expected.to_string(),
            entry_total: entry_total.to_string(),
            exit_total: exit_total.to_string(),
            counted_amount: shift.counted_amount.map(|m| m.to_string()).or_else(|| {
                if shift.status == CashShiftStatus::Closed {
                    Some(counted.to_string())
                } else {
                    None
                }
            }),
            difference_minor: shift.difference_minor.or(Some(difference_minor)),
            movements: movements.iter().map(CashMovementView::from).collect(),
        })
    }
}
