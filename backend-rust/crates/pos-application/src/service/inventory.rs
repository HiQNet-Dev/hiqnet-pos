//! Inventario y diagnostico de integridad.

use super::*;

impl PosService {
    pub async fn diagnose_inventory_integrity(
        &self,
        actor: &Actor,
    ) -> AppResult<IntegrityDiagnosticReport> {
        actor.require(pos_core::identity::Permission::ViewInventory)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let movements = tx
            .list_inventory_movements(actor.organization_id, branch_id, None, 100_000)
            .await?;
        let stock_levels = tx.stock_levels(actor.organization_id, branch_id).await?;
        let waste_records = tx.list_waste(actor.organization_id, branch_id).await?;
        tx.rollback().await?;

        let mut findings = Vec::new();
        let mut by_id = std::collections::BTreeMap::new();

        for (product_id, stock) in &stock_levels {
            if stock.milli_units() < 0 {
                findings.push(IntegrityFinding {
                    scope: "inventory.stock".to_string(),
                    entity_id: Some(product_id.to_string()),
                    status: IntegrityStatus::Mismatch,
                    message: "saldo negativo detectado".to_string(),
                });
            }
        }

        for movement in &movements {
            if movement.organization_id != actor.organization_id || movement.branch_id != branch_id
            {
                findings.push(IntegrityFinding {
                    scope: "inventory.movement_scope".to_string(),
                    entity_id: Some(movement.id.to_string()),
                    status: IntegrityStatus::Mismatch,
                    message: "movimiento fuera del alcance del diagnostico".to_string(),
                });
            }
            if movement.quantity.milli_units() == 0 {
                findings.push(IntegrityFinding {
                    scope: "inventory.movement_quantity".to_string(),
                    entity_id: Some(movement.id.to_string()),
                    status: IntegrityStatus::Mismatch,
                    message: "movimiento con cantidad cero".to_string(),
                });
            }
            if by_id.contains_key(&movement.id) {
                findings.push(IntegrityFinding {
                    scope: "inventory.movement_duplicate".to_string(),
                    entity_id: Some(movement.id.to_string()),
                    status: IntegrityStatus::Mismatch,
                    message: "movimiento duplicado en el diagnostico".to_string(),
                });
            }
            by_id.insert(movement.id, movement.clone());
        }

        let mut seen_waste_keys = std::collections::BTreeSet::new();
        for waste in &waste_records {
            if !seen_waste_keys.insert(waste.idempotency_key.clone()) {
                findings.push(IntegrityFinding {
                    scope: "waste.idempotency".to_string(),
                    entity_id: Some(waste.id.to_string()),
                    status: IntegrityStatus::Mismatch,
                    message: "idempotencia de merma duplicada".to_string(),
                });
            }
        }

        for waste in &waste_records {
            match by_id.get(&waste.inventory_movement_id) {
                None => {
                    let status = if movements.len() >= 100_000 {
                        IntegrityStatus::Unverifiable
                    } else {
                        IntegrityStatus::Mismatch
                    };
                    let message = if movements.len() >= 100_000 {
                        "movimiento asociado no cargado por limite de paginacion".to_string()
                    } else {
                        "merma sin movimiento asociado".to_string()
                    };
                    findings.push(IntegrityFinding {
                        scope: "waste.reference".to_string(),
                        entity_id: Some(waste.id.to_string()),
                        status,
                        message,
                    })
                }
                Some(movement) => {
                    if movement.product_id != waste.product_id
                        || movement.quantity != waste.quantity
                        || movement.organization_id != waste.organization_id
                        || movement.branch_id != waste.branch_id
                        || movement.kind != pos_core::inventory::MovementKind::Outbound
                    {
                        findings.push(IntegrityFinding {
                            scope: "waste.reference".to_string(),
                            entity_id: Some(waste.id.to_string()),
                            status: IntegrityStatus::Mismatch,
                            message: "merma y movimiento asociado no coinciden".to_string(),
                        });
                    }
                }
            }
        }

        findings.push(IntegrityFinding {
            scope: "inventory.sales".to_string(),
            entity_id: None,
            status: IntegrityStatus::Unverifiable,
            message: "la correspondencia venta-movimiento no es referencial en el esquema actual"
                .to_string(),
        });

        let complete = movements.len() < 100_000;
        if !complete {
            findings.push(IntegrityFinding {
                scope: "inventory.pagination".to_string(),
                entity_id: None,
                status: IntegrityStatus::Unverifiable,
                message: "diagnostico incompleto: se alcanzo el limite de 100000 movimientos"
                    .to_string(),
            });
        }

        let status = if findings
            .iter()
            .any(|f| f.status == IntegrityStatus::Mismatch)
        {
            IntegrityStatus::Mismatch
        } else if findings.iter().any(|f| f.status == IntegrityStatus::Error) {
            IntegrityStatus::Error
        } else if findings
            .iter()
            .any(|f| f.status == IntegrityStatus::Unverifiable)
        {
            IntegrityStatus::Unverifiable
        } else {
            IntegrityStatus::Consistent
        };

        Ok(IntegrityDiagnosticReport {
            organization_id: actor.organization_id.to_string(),
            branch_id: branch_id.to_string(),
            status,
            movement_count: movements.len(),
            complete,
            findings,
        })
    }

    pub async fn record_inventory_movement(
        &self,
        actor: &Actor,
        draft: InventoryMovementDraft,
    ) -> AppResult<InventoryMovementView> {
        actor.require(pos_core::identity::Permission::ManageInventory)?;
        let product_id: ProductId = draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let kind = MovementKind::from_key(&draft.kind).ok_or(AppError::NotFound)?;
        let quantity: Quantity = draft.quantity.parse()?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = self.require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Inventory, self.clock.now())?;

        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if !product.active {
            return Err(pos_core::DomainError::InactiveProduct.into());
        }

        if matches!(kind, MovementKind::Outbound | MovementKind::AdjustmentOut) {
            let current = tx
                .current_stock(actor.organization_id, branch_id, product_id)
                .await?;
            if current.milli_units() < quantity.milli_units() {
                return Err(pos_core::DomainError::InsufficientStock.into());
            }
        }

        let movement = InventoryMovement::new(
            actor.organization_id,
            branch_id,
            product_id,
            kind,
            quantity,
            draft.reason,
            Some(actor.user_id),
        )?;

        tx.insert_inventory_movement(&movement).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::InventoryMovementRegistered,
            "inventory_movement",
            movement.id.to_string(),
            serde_json::json!({ "product_id": product_id.to_string(), "kind": kind.as_str(), "quantity": quantity.to_string() }),
        ))
        .await?;
        tx.commit().await?;

        Ok(InventoryMovementView::from(&movement))
    }

    pub async fn current_stock(
        &self,
        actor: &Actor,
        product_id: ProductId,
        branch_id: Option<BranchId>,
    ) -> AppResult<String> {
        actor.require(pos_core::identity::Permission::ViewInventory)?;
        if let Some(requested_branch) = branch_id
            && Some(requested_branch) != actor.branch_id
        {
            return Err(pos_core::DomainError::AccessDenied {
                required: "inventory.view",
            }
            .into());
        }
        let branch_id = branch_id.or(actor.branch_id).ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let stock = tx
            .current_stock(actor.organization_id, branch_id, product_id)
            .await?;
        tx.rollback().await?;
        Ok(stock.to_string())
    }

    pub async fn list_stock_levels(&self, actor: &Actor) -> AppResult<Vec<StockLevelView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let levels = tx.stock_levels(actor.organization_id, branch_id).await?;
        tx.rollback().await?;
        Ok(levels
            .into_iter()
            .map(|(product_id, quantity)| StockLevelView {
                product_id: product_id.to_string(),
                quantity: quantity.to_string(),
            })
            .collect())
    }

    pub async fn list_inventory_movements(
        &self,
        actor: &Actor,
        product_id: Option<ProductId>,
        limit: u32,
    ) -> AppResult<Vec<InventoryMovementView>> {
        actor.require(pos_core::identity::Permission::ViewInventory)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let limit = i64::from(limit).clamp(1, MAX_AUDIT_PAGE);
        let mut tx = self.store.begin().await?;
        let movements = tx
            .list_inventory_movements(actor.organization_id, branch_id, product_id, limit)
            .await?;
        tx.rollback().await?;
        Ok(movements.iter().map(InventoryMovementView::from).collect())
    }
}
