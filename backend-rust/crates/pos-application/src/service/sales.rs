//! Ventas.

use super::*;

impl PosService {
    pub async fn checkout_sale(&self, actor: &Actor, draft: CheckoutDraft) -> AppResult<SaleView> {
        actor.require(pos_core::identity::Permission::Sell)?;
        if draft.items.is_empty() {
            return Err(pos_core::DomainError::InvalidQuantityScale.into());
        }
        let shift_id: CashShiftId = draft.shift_id.parse().map_err(|_| AppError::NotFound)?;
        let cash_received: Money = draft.cash_received.parse()?;
        let fingerprint = serde_json::to_string(&draft).map_err(|_| AppError::Storage)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Sales, self.clock.now())?;
        // La recuperación idempotente se resuelve antes de exigir un turno abierto:
        // un reintento de una venta ya confirmada debe funcionar aunque el turno se
        // haya cerrado después.
        if let Some(existing) = tx
            .find_sale_by_idempotency(actor.organization_id, &draft.idempotency_key)
            .await?
        {
            if existing.branch_id != branch_id {
                return Err(AppError::NotFound);
            }
            if existing.request_fingerprint != fingerprint {
                return Err(AppError::Conflict);
            }
            let items = tx
                .list_sale_items(actor.organization_id, existing.id)
                .await?;
            let payment = tx
                .list_sale_payments(actor.organization_id, existing.id)
                .await?
                .into_iter()
                .next();
            tx.rollback().await?;
            return self.sale_view(existing, items, payment);
        }

        let shift = tx
            .find_cash_shift(actor.organization_id, shift_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if shift.status != CashShiftStatus::Open || shift.branch_id != branch_id {
            return Err(AppError::NotFound);
        }

        let sale_id = pos_core::ids::SaleId::new();
        let folio = format!(
            "V-{}",
            sale_id
                .to_string()
                .replace('-', "")
                .chars()
                .take(10)
                .collect::<String>()
                .to_uppercase()
        );
        let mut subtotal = Money::ZERO;
        let mut tax_total = Money::ZERO;
        let mut sale_items = Vec::new();
        let mut stock_needed: BTreeMap<pos_core::ids::ProductId, Quantity> = BTreeMap::new();

        for draft_item in &draft.items {
            let product_id: pos_core::ids::ProductId = draft_item
                .product_id
                .parse()
                .map_err(|_| AppError::NotFound)?;
            let variant_id = draft_item
                .variant_id
                .as_deref()
                .map(str::parse::<pos_core::ids::ProductVariantId>)
                .transpose()
                .map_err(|_| AppError::NotFound)?;
            let quantity: Quantity = draft_item.quantity.parse()?;
            if quantity.is_zero() {
                return Err(pos_core::DomainError::NegativeQuantity.into());
            }
            let product = tx
                .find_product(actor.organization_id, product_id)
                .await?
                .ok_or(pos_core::DomainError::ProductNotFound)?;
            if !product.active {
                return Err(pos_core::DomainError::InactiveProduct.into());
            }
            let variant = if let Some(variant_id) = variant_id {
                Some(
                    tx.list_product_variants(actor.organization_id, product_id)
                        .await?
                        .into_iter()
                        .find(|item| item.id == variant_id)
                        .ok_or(AppError::NotFound)?,
                )
            } else {
                None
            };
            let groups = tx
                .list_modifier_groups(actor.organization_id, product_id)
                .await?;
            let mut options = Vec::new();
            for group in &groups {
                options.extend(
                    tx.list_modifier_options(actor.organization_id, group.id)
                        .await?,
                );
            }
            let selected_ids = draft_item
                .selected_option_ids
                .iter()
                .map(|value| value.parse())
                .collect::<Result<Vec<pos_core::ids::ModifierOptionId>, _>>()
                .map_err(|_| AppError::NotFound)?;
            let configuration = pos_core::modifiers::configure(
                &product,
                variant.as_ref(),
                &groups,
                &options,
                &selected_ids,
            )?;

            let unit_net = Money::from_minor_units(configuration.base_price_minor)
                .map_err(AppError::Domain)?
                .checked_add(
                    Money::from_minor_units(configuration.modifiers_total_minor)
                        .map_err(AppError::Domain)?,
                )?;
            let unit_tax =
                Money::from_minor_units(configuration.tax_minor).map_err(AppError::Domain)?;
            let line_net = unit_net.multiply_by(quantity)?;
            let line_tax = unit_tax.multiply_by(quantity)?;
            let line_total = line_net.checked_add(line_tax)?;
            subtotal = subtotal.checked_add(line_net)?;
            tax_total = tax_total.checked_add(line_tax)?;
            let required = stock_needed.entry(product_id).or_insert(Quantity::ZERO);
            *required = required.checked_add(quantity)?;
            let modifiers_summary = configuration
                .selected_options
                .iter()
                .map(|option| option.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            sale_items.push(SaleItem::new(
                sale_id,
                product_id,
                variant.as_ref().map(|item| item.id),
                product.name.clone(),
                variant.as_ref().map(|item| item.name.clone()),
                quantity,
                unit_net,
                unit_tax,
                line_total,
                modifiers_summary,
            ));
        }

        let total = subtotal.checked_add(tax_total)?;
        let change = validate_payment(total, cash_received)?;
        for (product_id, quantity) in &stock_needed {
            let available = tx
                .current_stock(actor.organization_id, branch_id, *product_id)
                .await?;
            if available.milli_units() < quantity.milli_units() {
                return Err(pos_core::DomainError::InsufficientStock.into());
            }
        }

        let occurred_at = self.clock.now();
        let sale = Sale {
            id: sale_id,
            organization_id: actor.organization_id,
            branch_id,
            register_id: shift.register_id,
            shift_id: shift.id,
            operator_id: actor.user_id,
            folio: folio.clone(),
            occurred_at,
            status: SaleStatus::Completed,
            subtotal,
            tax: tax_total,
            total,
            idempotency_key: draft.idempotency_key.clone(),
            request_fingerprint: fingerprint,
        };
        tx.insert_sale(&sale).await?;
        for item in &sale_items {
            tx.insert_sale_item(item).await?;
        }
        let payment = SalePayment {
            id: pos_core::ids::SalePaymentId::new(),
            sale_id,
            method: "cash".into(),
            amount: cash_received,
            change_given: change,
            occurred_at,
        };
        tx.insert_sale_payment(&payment).await?;

        for (product_id, quantity) in &stock_needed {
            let movement = InventoryMovement::new(
                actor.organization_id,
                branch_id,
                *product_id,
                MovementKind::Outbound,
                *quantity,
                Some(format!("venta {folio}")),
                Some(actor.user_id),
            )?;
            tx.insert_inventory_movement(&movement).await?;
        }
        let cash_movement = CashMovement::new(
            actor.organization_id,
            branch_id,
            shift.register_id,
            shift.id,
            CashMovementKind::In,
            total,
            format!("venta {folio}"),
            Some(actor.user_id),
        )?;
        tx.insert_cash_movement(&cash_movement).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            Some(branch_id),
            Some(actor.user_id),
            AuditAction::SaleCompleted,
            "sale",
            sale_id.to_string(),
            serde_json::json!({ "folio": folio, "total": total.to_string(), "shift_id": shift_id.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        self.sale_view(sale, sale_items, Some(payment))
    }

    pub async fn get_sale(
        &self,
        actor: &Actor,
        sale_id: pos_core::ids::SaleId,
    ) -> AppResult<SaleView> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let sale = tx
            .find_sale(actor.organization_id, sale_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if sale.branch_id != actor.branch_id.ok_or(AppError::NotFound)? {
            return Err(AppError::NotFound);
        }
        let items = tx.list_sale_items(actor.organization_id, sale_id).await?;
        let payment = tx
            .list_sale_payments(actor.organization_id, sale_id)
            .await?
            .into_iter()
            .next();
        tx.rollback().await?;
        self.sale_view(sale, items, payment)
    }

    fn sale_view(
        &self,
        sale: Sale,
        items: Vec<SaleItem>,
        payment: Option<SalePayment>,
    ) -> AppResult<SaleView> {
        Ok(SaleView {
            id: sale.id.to_string(),
            folio: sale.folio,
            status: sale.status.as_str().to_owned(),
            subtotal: sale.subtotal.to_string(),
            tax: sale.tax.to_string(),
            total: sale.total.to_string(),
            items: items
                .into_iter()
                .map(|item| SaleItemView {
                    id: item.id.to_string(),
                    product_id: item.product_id.to_string(),
                    product_name: item.product_name,
                    variant_name: item.variant_name,
                    quantity: item.quantity.to_string(),
                    unit_price: item.unit_price.to_string(),
                    unit_tax: item.unit_tax.to_string(),
                    line_total: item.line_total.to_string(),
                    modifiers_summary: item.modifiers_summary,
                })
                .collect(),
            payment: payment.map(|payment| SalePaymentView {
                method: payment.method,
                amount: payment.amount.to_string(),
                change: payment.change_given.to_string(),
            }),
        })
    }
}
