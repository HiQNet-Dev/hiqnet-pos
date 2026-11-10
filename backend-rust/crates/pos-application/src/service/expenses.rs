//! Gastos y merma.

use super::*;

pub(super) fn expense_view(expense: Expense) -> ExpenseView {
    ExpenseView {
        id: expense.id.to_string(),
        category_id: expense.category_id.to_string(),
        amount: expense.amount.to_string(),
        concept: expense.concept,
        paid_from_cash: expense.cash_movement_id.is_some(),
        occurred_at: expense.occurred_at,
    }
}

pub(super) fn waste_view(waste: WasteRecord) -> WasteView {
    WasteView {
        id: waste.id.to_string(),
        product_id: waste.product_id.to_string(),
        quantity: waste.quantity.to_string(),
        reason: waste.reason,
        occurred_at: waste.occurred_at,
    }
}

impl PosService {
    pub async fn create_expense_category(
        &self,
        actor: &Actor,
        draft: ExpenseCategoryDraft,
    ) -> AppResult<ExpenseCategoryView> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let category = ExpenseCategory::new(actor.organization_id, draft.name)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Expenses, self.clock.now())?;
        tx.insert_expense_category(&category).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ExpenseCategoryCreated,
            "expense_category",
            category.id.to_string(),
            serde_json::json!({ "name": category.name }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ExpenseCategoryView {
            id: category.id.to_string(),
            name: category.name,
            active: category.active,
        })
    }

    pub async fn list_expense_categories(
        &self,
        actor: &Actor,
    ) -> AppResult<Vec<ExpenseCategoryView>> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let mut tx = self.store.begin().await?;
        let categories = tx.list_expense_categories(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(categories
            .into_iter()
            .map(|item| ExpenseCategoryView {
                id: item.id.to_string(),
                name: item.name,
                active: item.active,
            })
            .collect())
    }

    pub async fn list_expenses(&self, actor: &Actor) -> AppResult<Vec<ExpenseView>> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let expenses = tx.list_expenses(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(expenses.into_iter().map(expense_view).collect())
    }

    pub async fn record_expense(
        &self,
        actor: &Actor,
        draft: ExpenseDraft,
    ) -> AppResult<ExpenseView> {
        // Dos procesos locales pueden alcanzar la escritura del movimiento físico
        // antes de observar la clave idempotente. Reintentamos sólo la misma
        // solicitud inmutable para recuperar el hecho confirmado por SQLite.
        for delay_ms in [20_u64, 50_u64] {
            match self.record_expense_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.record_expense_once(actor, draft).await
    }

    async fn record_expense_once(
        &self,
        actor: &Actor,
        draft: ExpenseDraft,
    ) -> AppResult<ExpenseView> {
        actor.require(pos_core::identity::Permission::ManageExpenses)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let category_id: ExpenseCategoryId =
            draft.category_id.parse().map_err(|_| AppError::NotFound)?;
        let amount: Money = draft.amount.parse()?;
        let concept = draft.concept.trim().to_owned();
        if concept.is_empty()
            || draft.idempotency_key.trim().is_empty()
            || (!draft.paid_from_cash && draft.shift_id.is_some())
        {
            return Err(AppError::Conflict);
        }
        let fingerprint = format!(
            "{category_id}:{amount}:{}:{}:{}",
            concept,
            draft.paid_from_cash,
            draft.shift_id.as_deref().unwrap_or("")
        );
        let mut tx = self.store.begin().await?;
        if let Some(existing) = tx
            .find_expense_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if existing.request_fingerprint != fingerprint || existing.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(expense_view(existing));
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Expenses, self.clock.now())?;
        let category = tx
            .find_expense_category(actor.organization_id, category_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if !category.active {
            return Err(AppError::Conflict);
        }
        let (cash_shift_id, cash_movement_id) = if draft.paid_from_cash {
            let shift_id: CashShiftId = draft
                .shift_id
                .as_deref()
                .ok_or(AppError::NotFound)?
                .parse()
                .map_err(|_| AppError::NotFound)?;
            let shift = tx
                .find_cash_shift(actor.organization_id, shift_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if shift.status != CashShiftStatus::Open || shift.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let expected = expected_amount(
                &shift,
                &tx.list_cash_movements(actor.organization_id, shift.id)
                    .await?,
            )?;
            if expected.minor_units() < amount.minor_units() {
                return Err(AppError::Conflict);
            }
            let movement = CashMovement::new(
                actor.organization_id,
                branch_id,
                shift.register_id,
                shift.id,
                CashMovementKind::Out,
                amount,
                format!("gasto: {concept}"),
                Some(actor.user_id),
            )?;
            tx.insert_cash_movement(&movement).await?;
            (Some(shift.id), Some(movement.id))
        } else {
            (None, None)
        };
        let expense = Expense {
            id: ExpenseId::new(),
            organization_id: actor.organization_id,
            branch_id,
            category_id,
            amount,
            concept,
            cash_shift_id,
            cash_movement_id,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint,
            created_by: actor.user_id,
            occurred_at: self.clock.now(),
        };
        if !tx.insert_expense(&expense).await? {
            let existing = tx
                .find_expense_by_idempotency(actor.organization_id, &expense.idempotency_key)
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != expense.request_fingerprint {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(expense_view(existing));
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::ExpenseRecorded, "expense", expense.id.to_string(), serde_json::json!({ "amount": amount.to_string(), "paid_from_cash": draft.paid_from_cash }))).await?;
        tx.commit().await?;
        Ok(expense_view(expense))
    }

    pub async fn list_waste(&self, actor: &Actor) -> AppResult<Vec<WasteView>> {
        actor.require(pos_core::identity::Permission::ManageWaste)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let rows = tx.list_waste(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(rows.into_iter().map(waste_view).collect())
    }

    pub async fn record_waste(&self, actor: &Actor, draft: WasteDraft) -> AppResult<WasteView> {
        // Igual que los cobros y las recepciones, una contención SQLite no debe
        // convertir un reintento idéntico en una segunda merma.
        for delay_ms in [20_u64, 50_u64] {
            match self.record_waste_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.record_waste_once(actor, draft).await
    }

    async fn record_waste_once(&self, actor: &Actor, draft: WasteDraft) -> AppResult<WasteView> {
        actor.require(pos_core::identity::Permission::ManageWaste)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let product_id: ProductId = draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let quantity: Quantity = draft.quantity.parse()?;
        let reason = draft.reason.trim().to_owned();
        if reason.is_empty() || draft.idempotency_key.trim().is_empty() {
            return Err(AppError::Conflict);
        }
        let fingerprint = format!("{product_id}:{}:{reason}", quantity.milli_units());
        let mut tx = self.store.begin().await?;
        if let Some(existing) = tx
            .find_waste_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if existing.request_fingerprint != fingerprint || existing.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(waste_view(existing));
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Inventory, self.clock.now())?;
        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if !product.active {
            return Err(pos_core::DomainError::InactiveProduct.into());
        }
        if tx
            .current_stock(actor.organization_id, branch_id, product_id)
            .await?
            .milli_units()
            < quantity.milli_units()
        {
            return Err(pos_core::DomainError::InsufficientStock.into());
        }
        let movement = InventoryMovement::new(
            actor.organization_id,
            branch_id,
            product_id,
            MovementKind::AdjustmentOut,
            quantity,
            Some(format!("merma: {reason}")),
            Some(actor.user_id),
        )?;
        tx.insert_inventory_movement(&movement).await?;
        let waste = WasteRecord {
            id: WasteId::new(),
            organization_id: actor.organization_id,
            branch_id,
            product_id,
            quantity,
            reason,
            inventory_movement_id: movement.id,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint,
            created_by: actor.user_id,
            occurred_at: self.clock.now(),
        };
        if !tx.insert_waste(&waste).await? {
            let existing = tx
                .find_waste_by_idempotency(actor.organization_id, &waste.idempotency_key)
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != waste.request_fingerprint
                || existing.branch_id != branch_id
            {
                return Err(AppError::Conflict);
            }
            tx.rollback().await?;
            return Ok(waste_view(existing));
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::WasteRecorded, "waste_record", waste.id.to_string(), serde_json::json!({ "product_id": product_id.to_string(), "quantity": quantity.to_string() }))).await?;
        tx.commit().await?;
        Ok(waste_view(waste))
    }
}
