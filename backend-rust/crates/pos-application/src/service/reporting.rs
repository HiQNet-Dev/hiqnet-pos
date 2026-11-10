//! Reportes y estado comercial.

use super::*;

impl PosService {
    pub async fn reports_summary(
        &self,
        actor: &Actor,
        query: ReportsQuery,
    ) -> AppResult<ReportsView> {
        actor.require(pos_core::identity::Permission::ViewReports)?;
        let branch_id = actor.branch_id.ok_or(AppError::NotFound)?;
        let from = parse_rfc3339(&query.from)?;
        let to = parse_rfc3339(&query.to)?;
        if from >= to {
            return Err(AppError::InvalidRequest);
        }
        let mut tx = self.store.begin().await?;
        let sales = tx
            .sales_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let by_product = tx
            .sales_report_by_product(actor.organization_id, branch_id, from, to)
            .await?;
        let returns = tx
            .returns_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let cash = tx
            .cash_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let purchases = tx
            .purchases_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let expenses = tx
            .expenses_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        let inventory = tx
            .inventory_report_totals(actor.organization_id, branch_id, from, to)
            .await?;
        tx.rollback().await?;

        let returned_total =
            Money::from_minor_units(returns.total_minor).map_err(AppError::Domain)?;
        let net_minor = sales
            .total_minor
            .checked_sub(returns.total_minor)
            .ok_or(AppError::Storage)?;
        Ok(ReportsView {
            from,
            to,
            sales: ReportsSalesView {
                count: sales.count,
                subtotal: money_minor(sales.subtotal_minor)?,
                tax: money_minor(sales.tax_minor)?,
                total: money_minor(sales.total_minor)?,
                returned_count: returns.count,
                returned_total: returned_total.to_string(),
                net_total: signed_money_minor(net_minor),
                products: by_product
                    .into_iter()
                    .map(|row| {
                        Ok(ReportsProductView {
                            product_id: row.product_id.to_string(),
                            product_name: row.product_name,
                            quantity: Quantity::from_milli_units(row.quantity_milli)?.to_string(),
                            net: money_minor(row.net_minor)?,
                            tax: money_minor(row.tax_minor)?,
                            total: money_minor(row.total_minor)?,
                        })
                    })
                    .collect::<AppResult<Vec<_>>>()?,
            },
            cash: ReportsCashView {
                shifts_opened: cash.shifts_opened,
                shifts_closed: cash.shifts_closed,
                entries: money_minor(cash.entries_minor)?,
                exits: money_minor(cash.exits_minor)?,
                expenses_from_cash: money_minor(cash.expenses_from_cash_minor)?,
                refunds: money_minor(cash.refunds_minor)?,
                difference: signed_money_minor(cash.difference_minor),
            },
            purchases: ReportsPurchasesView {
                orders_created: purchases.orders_created,
                pending_orders: purchases.pending_orders,
                received_quantity: Quantity::from_milli_units(purchases.received_quantity_milli)?
                    .to_string(),
                received_cost: money_minor(purchases.received_cost_minor)?,
            },
            expenses: ReportsExpensesView {
                total: money_minor(expenses.total_minor)?,
                from_cash: money_minor(expenses.from_cash_minor)?,
                external: money_minor(expenses.external_minor)?,
                categories: expenses
                    .by_category
                    .into_iter()
                    .map(|row| {
                        Ok(ReportsExpenseCategoryView {
                            name: row.name,
                            total: money_minor(row.total_minor)?,
                        })
                    })
                    .collect::<AppResult<Vec<_>>>()?,
            },
            inventory: ReportsInventoryView {
                stock_quantity: Quantity::from_milli_units(inventory.stock_quantity_milli)?
                    .to_string(),
                inbound: Quantity::from_milli_units(inventory.inbound_milli)?.to_string(),
                outbound: Quantity::from_milli_units(inventory.outbound_milli)?.to_string(),
                waste: Quantity::from_milli_units(inventory.waste_milli)?.to_string(),
                adjustment_in: Quantity::from_milli_units(inventory.adjustment_in_milli)?
                    .to_string(),
                adjustment_out: Quantity::from_milli_units(inventory.adjustment_out_milli)?
                    .to_string(),
            },
        })
    }

    pub async fn commercial_state(&self) -> AppResult<CommercialStateView> {
        let mut tx = self.store.begin().await?;
        let state = tx.installation_state().await?.ok_or(AppError::NotFound)?;
        tx.rollback().await?;
        commercial_view(&state, self.clock.now())
    }
}
