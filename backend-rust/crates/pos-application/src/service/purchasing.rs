//! Compras: proveedores y ordenes.

use super::*;

pub(super) fn purchase_order_view(
    order: PurchaseOrder,
    lines: Vec<PurchaseOrderLine>,
) -> PurchaseOrderView {
    PurchaseOrderView {
        id: order.id.to_string(),
        supplier_id: order.supplier_id.to_string(),
        folio: order.folio,
        status: order.status.as_str().to_owned(),
        created_at: order.created_at,
        lines: lines
            .into_iter()
            .map(|line| PurchaseOrderLineView {
                id: line.id.to_string(),
                product_id: line.product_id.to_string(),
                quantity: line.requested_quantity.to_string(),
                received_quantity: line.received_quantity.to_string(),
                unit_cost: line.unit_cost.to_string(),
            })
            .collect(),
    }
}

pub(super) fn purchase_receipt_fingerprint(
    order_id: PurchaseOrderId,
    lines: &[PurchaseReceiptLineDraft],
) -> AppResult<String> {
    let mut values = lines
        .iter()
        .map(|line| {
            let id: PurchaseOrderLineId =
                line.order_line_id.parse().map_err(|_| AppError::NotFound)?;
            let quantity: Quantity = line.quantity.parse()?;
            Ok(format!("{id}:{}", quantity.milli_units()))
        })
        .collect::<AppResult<Vec<_>>>()?;
    values.sort();
    Ok(format!("{}|{}", order_id, values.join("|")))
}

impl PosService {
    pub async fn create_supplier(
        &self,
        actor: &Actor,
        draft: SupplierDraft,
    ) -> AppResult<SupplierView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let supplier = Supplier::new(
            actor.organization_id,
            draft.code,
            draft.name,
            draft.contact,
            draft.rfc,
        )?;
        tx.insert_supplier(&supplier).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::SupplierCreated,
            "supplier",
            supplier.id.to_string(),
            serde_json::json!({ "name": supplier.name, "code": supplier.code }),
        ))
        .await?;
        tx.commit().await?;
        Ok(SupplierView::from(&supplier))
    }

    pub async fn list_suppliers(&self, actor: &Actor) -> AppResult<Vec<SupplierView>> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let mut tx = self.store.begin().await?;
        let suppliers = tx.list_suppliers(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(suppliers.iter().map(SupplierView::from).collect())
    }

    pub async fn create_purchase_order(
        &self,
        actor: &Actor,
        draft: PurchaseOrderDraft,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let supplier_id: SupplierId = draft.supplier_id.parse().map_err(|_| AppError::NotFound)?;
        if draft.lines.is_empty() {
            return Err(pos_core::DomainError::EmptyName.into());
        }
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let supplier = tx
            .find_supplier(actor.organization_id, supplier_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if !supplier.active {
            return Err(AppError::Conflict);
        }
        let order_id = PurchaseOrderId::new();
        let folio = format!(
            "OC-{}-{}",
            self.clock.now().unix_timestamp(),
            &order_id.to_string()[..8]
        );
        let order = PurchaseOrder {
            id: order_id,
            organization_id: actor.organization_id,
            branch_id,
            supplier_id,
            folio,
            status: PurchaseOrderStatus::Draft,
            created_by: actor.user_id,
            created_at: self.clock.now(),
        };
        let mut lines = Vec::with_capacity(draft.lines.len());
        let mut seen = BTreeSet::new();
        for line in draft.lines {
            let product_id: ProductId = line.product_id.parse().map_err(|_| AppError::NotFound)?;
            if !seen.insert(product_id) {
                return Err(AppError::Conflict);
            }
            let product = tx
                .find_product(actor.organization_id, product_id)
                .await?
                .ok_or(pos_core::DomainError::ProductNotFound)?;
            if !product.active {
                return Err(pos_core::DomainError::InactiveProduct.into());
            }
            let quantity: Quantity = line.quantity.parse()?;
            let unit_cost: Money = line.unit_cost.parse()?;
            lines.push(PurchaseOrderLine::new(
                order.id, product_id, quantity, unit_cost,
            )?);
        }
        tx.insert_purchase_order(&order).await?;
        for line in &lines {
            tx.insert_purchase_order_line(line).await?;
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::PurchaseOrderCreated, "purchase_order", order.id.to_string(), serde_json::json!({ "folio": order.folio, "supplier_id": supplier_id.to_string(), "lines": lines.len() }))).await?;
        tx.commit().await?;
        Ok(purchase_order_view(order, lines))
    }

    pub async fn list_purchase_orders(&self, actor: &Actor) -> AppResult<Vec<PurchaseOrderView>> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let orders = tx
            .list_purchase_orders(actor.organization_id, branch_id)
            .await?;
        let mut views = Vec::with_capacity(orders.len());
        for order in orders {
            let lines = tx
                .list_purchase_order_lines(actor.organization_id, order.id)
                .await?;
            views.push(purchase_order_view(order, lines));
        }
        tx.rollback().await?;
        Ok(views)
    }

    pub async fn get_purchase_order(
        &self,
        actor: &Actor,
        order_id: PurchaseOrderId,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let order = tx
            .find_purchase_order(actor.organization_id, order_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if order.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        let lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        tx.rollback().await?;
        Ok(purchase_order_view(order, lines))
    }

    pub async fn confirm_purchase_order(
        &self,
        actor: &Actor,
        order_id: PurchaseOrderId,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let mut order = tx
            .find_purchase_order(actor.organization_id, order_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if order.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        if order.status != PurchaseOrderStatus::Draft {
            return Err(AppError::Conflict);
        }
        tx.confirm_purchase_order(actor.organization_id, order_id)
            .await?;
        order.status = PurchaseOrderStatus::Ordered;
        let lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::PurchaseOrderConfirmed,
            "purchase_order",
            order.id.to_string(),
            serde_json::json!({ "folio": order.folio }),
        ))
        .await?;
        tx.commit().await?;
        Ok(purchase_order_view(order, lines))
    }

    pub async fn receive_purchase_order(
        &self,
        actor: &Actor,
        draft: PurchaseReceiptDraft,
    ) -> AppResult<PurchaseOrderView> {
        // Dos procesos locales pueden intentar la misma clave simultáneamente.
        // SQLite puede devolver una contención transitoria antes de que la segunda
        // transacción alcance la restricción única; se reintenta el mismo payload
        // inmutable para recuperar el resultado idempotente confirmado.
        for delay_ms in [20_u64, 50_u64] {
            match self.receive_purchase_order_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.receive_purchase_order_once(actor, draft).await
    }

    async fn receive_purchase_order_once(
        &self,
        actor: &Actor,
        draft: PurchaseReceiptDraft,
    ) -> AppResult<PurchaseOrderView> {
        actor.require(pos_core::identity::Permission::ManagePurchasing)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let order_id: PurchaseOrderId = draft.order_id.parse().map_err(|_| AppError::NotFound)?;
        if draft.idempotency_key.trim().is_empty() || draft.lines.is_empty() {
            return Err(AppError::Conflict);
        }
        let fingerprint = purchase_receipt_fingerprint(order_id, &draft.lines)?;
        let mut tx = self.store.begin().await?;
        if let Some(receipt) = tx
            .find_purchase_receipt_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if receipt.request_fingerprint != fingerprint {
                return Err(AppError::Conflict);
            }
            let order = tx
                .find_purchase_order(actor.organization_id, receipt.order_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if order.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let lines = tx
                .list_purchase_order_lines(actor.organization_id, order.id)
                .await?;
            tx.rollback().await?;
            return Ok(purchase_order_view(order, lines));
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Purchasing, self.clock.now())?;
        let order = tx
            .find_purchase_order(actor.organization_id, order_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if order.branch_id != branch_id
            || !matches!(
                order.status,
                PurchaseOrderStatus::Ordered | PurchaseOrderStatus::PartiallyReceived
            )
        {
            return Err(AppError::Conflict);
        }
        let lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        let mut receipt_lines = Vec::new();
        let mut seen = BTreeSet::new();
        for input in &draft.lines {
            let line_id: PurchaseOrderLineId = input
                .order_line_id
                .parse()
                .map_err(|_| AppError::NotFound)?;
            if !seen.insert(line_id) {
                return Err(AppError::Conflict);
            }
            let quantity: Quantity = input.quantity.parse()?;
            if quantity.is_zero() {
                return Err(pos_core::DomainError::NegativeQuantity.into());
            }
            let line = lines
                .iter()
                .find(|line| line.id == line_id)
                .ok_or(AppError::NotFound)?;
            if quantity.milli_units() > line.remaining()?.milli_units() {
                return Err(AppError::Conflict);
            }
            receipt_lines.push((line.clone(), quantity));
        }
        let receipt = PurchaseReceipt {
            id: pos_core::ids::PurchaseReceiptId::new(),
            organization_id: actor.organization_id,
            branch_id,
            order_id,
            received_by: actor.user_id,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint,
            received_at: self.clock.now(),
        };
        if !tx.insert_purchase_receipt(&receipt).await? {
            let existing = tx
                .find_purchase_receipt_by_idempotency(
                    actor.organization_id,
                    &receipt.idempotency_key,
                )
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != receipt.request_fingerprint
                || existing.order_id != order_id
            {
                return Err(AppError::Conflict);
            }
            let replay_order = tx
                .find_purchase_order(actor.organization_id, existing.order_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if replay_order.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let replay_lines = tx
                .list_purchase_order_lines(actor.organization_id, replay_order.id)
                .await?;
            tx.rollback().await?;
            return Ok(purchase_order_view(replay_order, replay_lines));
        }
        for (line, quantity) in &receipt_lines {
            tx.insert_purchase_receipt_line(receipt.id, line.id, *quantity)
                .await?;
        }
        // Each conditional update verifies the pending quantity again at write time.
        tx.apply_purchase_receipt(
            actor.organization_id,
            order_id,
            &receipt_lines
                .iter()
                .map(|(line, quantity)| (line.id, *quantity))
                .collect::<Vec<_>>(),
            PurchaseOrderStatus::PartiallyReceived,
        )
        .await?;
        let updated_lines = tx
            .list_purchase_order_lines(actor.organization_id, order_id)
            .await?;
        let complete = updated_lines.iter().all(|line| {
            line.remaining()
                .map(|value| value.is_zero())
                .unwrap_or(false)
        });
        let status = if complete {
            PurchaseOrderStatus::Received
        } else {
            PurchaseOrderStatus::PartiallyReceived
        };
        if status == PurchaseOrderStatus::Received {
            tx.apply_purchase_receipt(actor.organization_id, order_id, &[], status)
                .await?;
        }
        for (line, quantity) in &receipt_lines {
            let movement = InventoryMovement::new(
                actor.organization_id,
                branch_id,
                line.product_id,
                MovementKind::Inbound,
                *quantity,
                Some(format!("compra {}", order.folio)),
                Some(actor.user_id),
            )?;
            tx.insert_inventory_movement(&movement).await?;
        }
        tx.append_audit(&AuditEvent::new(actor.organization_id, Some(branch_id), Some(actor.user_id), AuditAction::PurchaseReceived, "purchase_receipt", receipt.id.to_string(), serde_json::json!({ "folio": order.folio, "order_id": order.id.to_string(), "lines": receipt_lines.len() }))).await?;
        tx.commit().await?;
        let mut view_order = order;
        view_order.status = status;
        Ok(purchase_order_view(view_order, updated_lines))
    }
}
