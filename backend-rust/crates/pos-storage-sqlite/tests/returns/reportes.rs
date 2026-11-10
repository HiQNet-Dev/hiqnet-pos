//! Agregados de reportes y su conciliacion.

use super::common::*;

#[tokio::test]
async fn reportes_agregan_cada_fuente_sin_mezclar_ingresos() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "reporte-ret", "1", "restock"),
        )
        .await
        .unwrap();
    let category = fixture
        .service
        .create_expense_category(
            &fixture.actor,
            ExpenseCategoryDraft {
                name: "Operación".into(),
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .record_expense(
            &fixture.actor,
            ExpenseDraft {
                category_id: category.id.clone(),
                amount: "9.00".into(),
                concept: "Externo".into(),
                paid_from_cash: false,
                shift_id: None,
                idempotency_key: "rep-exp-a".into(),
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .record_expense(
            &fixture.actor,
            ExpenseDraft {
                category_id: category.id,
                amount: "12.00".into(),
                concept: "Caja".into(),
                paid_from_cash: true,
                shift_id: Some(fixture.shift_id.clone()),
                idempotency_key: "rep-exp-b".into(),
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .record_waste(
            &fixture.actor,
            WasteDraft {
                product_id: fixture.product_id.clone(),
                quantity: "1".into(),
                reason: "caducidad".into(),
                idempotency_key: "rep-waste".into(),
            },
        )
        .await
        .unwrap();
    let supplier = fixture
        .service
        .create_supplier(
            &fixture.actor,
            SupplierDraft {
                code: Some("SUP-1".into()),
                name: "Proveedor".into(),
                contact: None,
                rfc: None,
            },
        )
        .await
        .unwrap();
    let order = fixture
        .service
        .create_purchase_order(
            &fixture.actor,
            PurchaseOrderDraft {
                supplier_id: supplier.id,
                lines: vec![PurchaseOrderLineDraft {
                    product_id: fixture.product_id.clone(),
                    quantity: "5".into(),
                    unit_cost: "8.00".into(),
                }],
            },
        )
        .await
        .unwrap();
    fixture
        .service
        .confirm_purchase_order(&fixture.actor, order.id.parse().unwrap())
        .await
        .unwrap();
    fixture
        .service
        .receive_purchase_order(
            &fixture.actor,
            PurchaseReceiptDraft {
                order_id: order.id.clone(),
                idempotency_key: "rep-receipt".into(),
                lines: vec![PurchaseReceiptLineDraft {
                    order_line_id: order.lines[0].id.clone(),
                    quantity: "2".into(),
                }],
            },
        )
        .await
        .unwrap();

    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let report = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();

    assert_eq!(report.sales.count, 1);
    assert_eq!(report.sales.subtotal, "20.00");
    assert_eq!(report.sales.tax, "3.20");
    assert_eq!(report.sales.total, "23.20");
    assert_eq!(report.sales.returned_count, 1);
    assert_eq!(report.sales.returned_total, "11.60");
    assert_eq!(report.sales.net_total, "11.60");
    assert_eq!(report.sales.products[0].quantity, "2");
    assert_eq!(report.sales.products[0].net, "20.00");

    assert_eq!(report.cash.shifts_opened, 1);
    assert_eq!(report.cash.entries, "23.20");
    assert_eq!(report.cash.exits, "23.60");
    assert_eq!(report.cash.expenses_from_cash, "12.00");
    assert_eq!(report.cash.refunds, "11.60");

    assert_eq!(report.purchases.orders_created, 1);
    assert_eq!(report.purchases.pending_orders, 1);
    assert_eq!(report.purchases.received_quantity, "2");
    assert_eq!(report.purchases.received_cost, "16.00");

    assert_eq!(report.expenses.total, "21.00");
    assert_eq!(report.expenses.from_cash, "12.00");
    assert_eq!(report.expenses.external, "9.00");
    assert_eq!(report.expenses.categories.len(), 1);

    assert_eq!(report.inventory.stock_quantity, "20");
    assert_eq!(
        report.inventory.inbound, "23",
        "entradas: 20 iniciales + 2 de compra + 1 devuelta"
    );
    assert_eq!(report.inventory.outbound, "2");
    assert_eq!(report.inventory.waste, "1");
    assert_eq!(report.inventory.adjustment_out, "0");
}

#[tokio::test]
async fn reportes_sin_datos_y_filtro_de_fechas() {
    let fixture = fixture().await;
    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let empty = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(empty.sales.count, 0);
    assert_eq!(empty.sales.total, "0.00");
    assert!(empty.sales.products.is_empty());

    sell(&fixture, "2").await;
    let future_from = OffsetDateTime::now_utc() + time::Duration::days(2);
    let future_to = OffsetDateTime::now_utc() + time::Duration::days(3);
    let outside = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: future_from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: future_to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(outside.sales.count, 0, "el intervalo es semiabierto");

    let invalid = fixture
        .service
        .reports_summary(
            &fixture.actor,
            ReportsQuery {
                from: future_to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: future_from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(invalid.code(), "validation_error");
}

#[tokio::test]
async fn reportes_requieren_permiso_de_consulta() {
    let fixture = fixture().await;
    let cashier = Actor {
        role: Role::Cashier,
        permissions: Role::Cashier.permissions(),
        ..fixture.actor.clone()
    };
    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let error = fixture
        .service
        .reports_summary(
            &cashier,
            ReportsQuery {
                from: from
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
                to: to
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "access_denied");
}

#[tokio::test]
async fn reportes_con_devolucion_total_dejan_neto_cero() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "neto-cero", "2", "restock"),
        )
        .await
        .unwrap();

    let report = report_between(
        &fixture.service,
        &fixture.actor,
        OffsetDateTime::now_utc() - time::Duration::days(1),
        OffsetDateTime::now_utc() + time::Duration::days(1),
    )
    .await;
    assert_eq!(report.sales.total, "23.20");
    assert_eq!(report.sales.returned_total, "23.20");
    assert_eq!(report.sales.net_total, "0.00");
}

#[tokio::test]
async fn reportes_neto_negativo_cuando_la_venta_cae_fuera_del_periodo() {
    let fixture = fixture().await;
    let past = build_service_with_clock(
        open(fixture.dir.path().join("returns.db")).await.unwrap(),
        OffsetDateTime::now_utc() - time::Duration::days(2),
    );
    let past_actor = past
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let sale = sell_product(
        &past,
        &past_actor,
        &fixture.shift_id,
        &fixture.product_id,
        "2",
        "50.00",
        "past-sale",
    )
    .await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "neto-fuera", "2", "restock"),
        )
        .await
        .unwrap();

    let report = report_between(
        &fixture.service,
        &fixture.actor,
        OffsetDateTime::now_utc() - time::Duration::days(1),
        OffsetDateTime::now_utc() + time::Duration::days(1),
    )
    .await;
    assert_eq!(report.sales.count, 0, "la venta quedó fuera del período");
    assert_eq!(report.sales.total, "0.00");
    assert_eq!(report.sales.returned_total, "23.20");
    assert_eq!(report.sales.net_total, "-23.20");
}

#[tokio::test]
async fn reportes_soportan_faltantes_sobrantes_y_arqueo_exacto() {
    for (delta_minor, expected) in [(0_i64, "0.00"), (500, "5.00"), (-1000, "-10.00")] {
        let fixture = fixture().await;
        sell(&fixture, "2").await;
        let counted = money_text(12_320 + delta_minor);
        fixture
            .service
            .close_cash_shift(
                &fixture.actor,
                CashCloseShiftDraft {
                    shift_id: fixture.shift_id.clone(),
                    counted_amount: counted,
                },
            )
            .await
            .unwrap();
        let report = report_between(
            &fixture.service,
            &fixture.actor,
            OffsetDateTime::now_utc() - time::Duration::days(1),
            OffsetDateTime::now_utc() + time::Duration::days(1),
        )
        .await;
        assert_eq!(report.cash.difference, expected, "diferencia {delta_minor}");
        assert_eq!(report.cash.shifts_closed, 1);
        assert_eq!(report.cash.entries, "23.20");
    }
}

#[tokio::test]
async fn reportes_por_producto_reconcilian_con_cantidades_fraccionarias() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "REC-3", "0.03", 0).await;
    add_stock(&fixture, &product, "2").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "0.5",
        "1.00",
        "reconcile-sale",
    )
    .await;
    assert_eq!(sale.subtotal, "0.02");
    assert_eq!(sale.total, "0.02");

    let report = report_between(
        &fixture.service,
        &fixture.actor,
        OffsetDateTime::now_utc() - time::Duration::days(1),
        OffsetDateTime::now_utc() + time::Duration::days(1),
    )
    .await;
    assert_eq!(report.sales.subtotal, "0.02");
    assert_eq!(report.sales.total, "0.02");
    assert_eq!(report.sales.products.len(), 1);
    let row = &report.sales.products[0];
    assert_eq!(row.quantity, "0.5");
    assert_eq!(row.net, "0.02", "half-up, no truncamiento");
    assert_eq!(row.tax, "0.00");
    assert_eq!(row.total, "0.02");
}
