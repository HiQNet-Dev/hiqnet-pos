//! Devoluciones.

use super::*;

pub(super) struct ResolvedReturnLine {
    item: SaleItem,
    quantity: Quantity,
    disposition: ReturnDisposition,
    net: Money,
    tax: Money,
    total: Money,
}

/// Valida pertenencia, cantidades pendientes y calcula el reembolso exacto a
/// partir de los snapshots de la venta original.
pub(super) async fn resolve_return_lines(
    tx: &mut dyn StoreTx,
    organization_id: OrganizationId,
    items: &[SaleItem],
    drafts: &[SaleReturnLineDraft],
) -> AppResult<Vec<ResolvedReturnLine>> {
    if drafts.is_empty() {
        return Err(AppError::InvalidRequest);
    }
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::new();
    for draft in drafts {
        let item_id: SaleItemId = draft.sale_item_id.parse().map_err(|_| AppError::NotFound)?;
        if !seen.insert(item_id) {
            return Err(AppError::InvalidRequest);
        }
        let quantity: Quantity = draft.quantity.parse()?;
        if quantity.is_zero() {
            return Err(pos_core::DomainError::NegativeQuantity.into());
        }
        let disposition = ReturnDisposition::from_key(draft.disposition.trim())
            .ok_or(AppError::InvalidRequest)?;
        let item = items
            .iter()
            .find(|item| item.id == item_id)
            .ok_or(AppError::NotFound)?;
        let totals = tx.returned_totals(organization_id, item_id).await?;
        let remaining = item.quantity.milli_units() - totals.quantity_milli;
        if remaining < 0 || quantity.milli_units() > remaining {
            return Err(AppError::Conflict);
        }
        let basis = RefundBasis {
            returned_quantity: Quantity::from_milli_units(totals.quantity_milli)?,
            refunded_net: Money::from_minor_units(totals.net_minor).map_err(AppError::Domain)?,
            refunded_tax: Money::from_minor_units(totals.tax_minor).map_err(AppError::Domain)?,
        };
        let refund = refund_increment(item, &basis, quantity)?;
        resolved.push(ResolvedReturnLine {
            item: item.clone(),
            quantity,
            disposition,
            net: refund.net,
            tax: refund.tax,
            total: refund.total,
        });
    }
    Ok(resolved)
}

pub(super) fn sale_return_fingerprint(draft: &SaleReturnDraft, reason: &str) -> AppResult<String> {
    let mut rows = Vec::new();
    for line in &draft.lines {
        let item_id: SaleItemId = line.sale_item_id.parse().map_err(|_| AppError::NotFound)?;
        let quantity: Quantity = line.quantity.parse()?;
        let disposition =
            ReturnDisposition::from_key(line.disposition.trim()).ok_or(AppError::InvalidRequest)?;
        rows.push(format!(
            "{item_id}:{}:{}",
            quantity.milli_units(),
            disposition.as_str()
        ));
    }
    rows.sort();
    Ok(format!(
        "{}|{}|{reason}|{}",
        draft.sale_id.trim(),
        draft.shift_id.as_deref().unwrap_or("").trim(),
        rows.join(",")
    ))
}

pub(super) fn sale_return_view(
    value: SaleReturn,
    sale: &Sale,
    lines: &[SaleReturnLine],
    items: &[SaleItem],
) -> SaleReturnView {
    SaleReturnView {
        id: value.id.to_string(),
        sale_id: sale.id.to_string(),
        sale_folio: sale.folio.clone(),
        reason: value.reason,
        refund_total: value.refund_total.to_string(),
        occurred_at: value.occurred_at,
        lines: lines
            .iter()
            .map(|line| SaleReturnLineView {
                sale_item_id: line.sale_item_id.to_string(),
                product_name: items
                    .iter()
                    .find(|item| item.id == line.sale_item_id)
                    .map(|item| item.product_name.clone())
                    .unwrap_or_default(),
                quantity: line.quantity.to_string(),
                disposition: line.disposition.as_str().to_owned(),
                refund_net: line.refund_net.to_string(),
                refund_tax: line.refund_tax.to_string(),
                refund_total: line.refund_total.to_string(),
            })
            .collect(),
    }
}

impl PosService {
    pub async fn find_sale_for_return(
        &self,
        actor: &Actor,
        folio: String,
    ) -> AppResult<ReturnableSaleView> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let folio = folio.trim().to_owned();
        if folio.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        let mut tx = self.store.begin().await?;
        let sale = tx
            .find_sale_by_folio(actor.organization_id, branch_id, &folio)
            .await?
            .ok_or(AppError::NotFound)?;
        let items = tx.list_sale_items(actor.organization_id, sale.id).await?;
        let mut views = Vec::new();
        for item in &items {
            let totals = tx.returned_totals(actor.organization_id, item.id).await?;
            let returnable_milli = (item.quantity.milli_units() - totals.quantity_milli).max(0);
            views.push(ReturnableSaleItemView {
                id: item.id.to_string(),
                product_name: item.product_name.clone(),
                variant_name: item.variant_name.clone(),
                modifiers_summary: item.modifiers_summary.clone(),
                quantity: item.quantity.to_string(),
                returned_quantity: Quantity::from_milli_units(totals.quantity_milli)?.to_string(),
                returnable_quantity: Quantity::from_milli_units(returnable_milli)?.to_string(),
                unit_refund: item.unit_price.checked_add(item.unit_tax)?.to_string(),
            });
        }
        tx.rollback().await?;
        Ok(ReturnableSaleView {
            id: sale.id.to_string(),
            folio: sale.folio,
            occurred_at: sale.occurred_at,
            total: sale.total.to_string(),
            items: views,
        })
    }

    pub async fn preview_sale_return(
        &self,
        actor: &Actor,
        draft: SaleReturnPreviewDraft,
    ) -> AppResult<SaleReturnPreviewView> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let sale_id = draft.sale_id.parse().map_err(|_| AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let sale = tx
            .find_sale(actor.organization_id, sale_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if sale.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        let items = tx.list_sale_items(actor.organization_id, sale_id).await?;
        let resolved =
            resolve_return_lines(tx.as_mut(), actor.organization_id, &items, &draft.lines).await?;
        let mut total = Money::ZERO;
        let mut lines = Vec::new();
        for line in &resolved {
            total = total.checked_add(line.total)?;
            lines.push(SaleReturnPreviewLineView {
                sale_item_id: line.item.id.to_string(),
                quantity: line.quantity.to_string(),
                refund_total: line.total.to_string(),
            });
        }
        tx.rollback().await?;
        Ok(SaleReturnPreviewView {
            refund_total: total.to_string(),
            lines,
        })
    }

    pub async fn create_sale_return(
        &self,
        actor: &Actor,
        draft: SaleReturnDraft,
    ) -> AppResult<SaleReturnView> {
        // Misma garantía que compras y gastos: una contención de SQLite se
        // reintenta con el payload inmutable para recuperar el hecho confirmado.
        for delay_ms in [20_u64, 50_u64] {
            match self.create_sale_return_once(actor, draft.clone()).await {
                Err(error) if error.is_storage() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                result => return result,
            }
        }
        self.create_sale_return_once(actor, draft).await
    }

    async fn create_sale_return_once(
        &self,
        actor: &Actor,
        draft: SaleReturnDraft,
    ) -> AppResult<SaleReturnView> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let reason = draft.reason.trim().to_owned();
        if reason.is_empty() || draft.idempotency_key.trim().is_empty() || draft.lines.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        let sale_id: pos_core::ids::SaleId =
            draft.sale_id.parse().map_err(|_| AppError::NotFound)?;
        let fingerprint = sale_return_fingerprint(&draft, &reason)?;
        let mut tx = self.store.begin().await?;
        if let Some(existing) = tx
            .find_sale_return_by_idempotency(actor.organization_id, draft.idempotency_key.trim())
            .await?
        {
            if existing.request_fingerprint != fingerprint {
                return Err(AppError::Conflict);
            }
            if existing.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let sale = tx
                .find_sale(actor.organization_id, existing.sale_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if sale.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            let lines = tx
                .list_sale_return_lines(actor.organization_id, existing.id)
                .await?;
            let items = tx.list_sale_items(actor.organization_id, sale.id).await?;
            let view = sale_return_view(existing, &sale, &lines, &items);
            tx.rollback().await?;
            return Ok(view);
        }
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        let sale = tx
            .find_sale(actor.organization_id, sale_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if sale.branch_id != branch_id {
            return Err(AppError::NotFound);
        }
        let items = tx.list_sale_items(actor.organization_id, sale_id).await?;
        let resolved =
            resolve_return_lines(tx.as_mut(), actor.organization_id, &items, &draft.lines).await?;
        let mut refund_total = Money::ZERO;
        for line in &resolved {
            refund_total = refund_total.checked_add(line.total)?;
        }
        // Un reembolso de cero centavos solo restituye inventario: no exige turno
        // abierto ni crea un movimiento de caja ficticio.
        let (shift_id, cash_movement_id) = if refund_total.minor_units() > 0 {
            let raw_shift = draft
                .shift_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(AppError::InvalidRequest)?;
            let shift_id: CashShiftId = raw_shift.parse().map_err(|_| AppError::NotFound)?;
            let shift = tx
                .find_cash_shift(actor.organization_id, shift_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if shift.status != CashShiftStatus::Open || shift.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            let expected = expected_amount(
                &shift,
                &tx.list_cash_movements(actor.organization_id, shift.id)
                    .await?,
            )?;
            if expected.minor_units() < refund_total.minor_units() {
                return Err(pos_core::DomainError::InsufficientCash.into());
            }
            let movement = CashMovement::new(
                actor.organization_id,
                branch_id,
                shift.register_id,
                shift.id,
                CashMovementKind::Out,
                refund_total,
                format!("devolución {}", sale.folio),
                Some(actor.user_id),
            )?;
            tx.insert_cash_movement(&movement).await?;
            (Some(shift.id), Some(movement.id))
        } else {
            (None, None)
        };
        let return_entity = SaleReturn {
            id: pos_core::ids::SaleReturnId::new(),
            organization_id: actor.organization_id,
            branch_id,
            sale_id,
            shift_id,
            cash_movement_id,
            operator_id: actor.user_id,
            reason: reason.clone(),
            refund_total,
            idempotency_key: draft.idempotency_key.trim().to_owned(),
            request_fingerprint: fingerprint.clone(),
            occurred_at: self.clock.now(),
        };
        if !tx.insert_sale_return(&return_entity).await? {
            let existing = tx
                .find_sale_return_by_idempotency(
                    actor.organization_id,
                    &return_entity.idempotency_key,
                )
                .await?
                .ok_or(AppError::Conflict)?;
            if existing.request_fingerprint != fingerprint || existing.branch_id != branch_id {
                return Err(AppError::Conflict);
            }
            let replay_lines = tx
                .list_sale_return_lines(actor.organization_id, existing.id)
                .await?;
            let view = sale_return_view(existing, &sale, &replay_lines, &items);
            tx.rollback().await?;
            return Ok(view);
        }
        let mut stored_lines = Vec::new();
        for line in &resolved {
            let inbound_movement_id = if line.disposition == ReturnDisposition::Restock {
                let inbound = InventoryMovement::new(
                    actor.organization_id,
                    branch_id,
                    line.item.product_id,
                    MovementKind::Inbound,
                    line.quantity,
                    Some(format!("devolución {}", sale.folio)),
                    Some(actor.user_id),
                )?;
                tx.insert_inventory_movement(&inbound).await?;
                Some(inbound.id)
            } else {
                None
            };
            let stored = SaleReturnLine {
                id: pos_core::ids::SaleReturnLineId::new(),
                return_id: return_entity.id,
                sale_item_id: line.item.id,
                quantity: line.quantity,
                disposition: line.disposition,
                refund_net: line.net,
                refund_tax: line.tax,
                refund_total: line.total,
                inbound_movement_id,
            };
            tx.insert_sale_return_line(&stored).await?;
            stored_lines.push(stored);
        }
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::SaleReturned,
            "sale_return",
            return_entity.id.to_string(),
            serde_json::json!({
                "sale_folio": sale.folio,
                "refund": refund_total.to_string(),
                "lines": resolved.len(),
                "shift_id": shift_id.map(|id| id.to_string()),
            }),
        ))
        .await?;
        tx.commit().await?;
        Ok(sale_return_view(
            return_entity,
            &sale,
            &stored_lines,
            &items,
        ))
    }

    pub async fn list_sale_returns(&self, actor: &Actor) -> AppResult<Vec<SaleReturnView>> {
        actor.require(pos_core::identity::Permission::ReturnSale)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let returns = tx
            .list_sale_returns(actor.organization_id, branch_id, 50)
            .await?;
        let mut views = Vec::new();
        for item in returns {
            let sale = tx
                .find_sale(actor.organization_id, item.sale_id)
                .await?
                .ok_or(AppError::NotFound)?;
            let lines = tx
                .list_sale_return_lines(actor.organization_id, item.id)
                .await?;
            let sale_items = tx
                .list_sale_items(actor.organization_id, item.sale_id)
                .await?;
            views.push(sale_return_view(item, &sale, &lines, &sale_items));
        }
        tx.rollback().await?;
        Ok(views)
    }
}
