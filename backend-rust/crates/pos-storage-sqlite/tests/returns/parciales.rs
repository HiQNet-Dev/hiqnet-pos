//! Devoluciones parciales, sucesivas y reembolsos fraccionarios.

use super::common::*;

#[tokio::test]
async fn devolucion_parcial_cuadra_reembolso_inventario_y_caja() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let preview = fixture
        .service
        .preview_sale_return(
            &fixture.actor,
            SaleReturnPreviewDraft {
                sale_id: sale.id.clone(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "1".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(preview.refund_total, "11.60");

    let created = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-1", "1", "restock"),
        )
        .await
        .unwrap();
    assert_eq!(created.refund_total, "11.60");
    assert_eq!(created.sale_folio, sale.folio);
    assert_eq!(created.lines[0].refund_net, "10.00");
    assert_eq!(created.lines[0].refund_tax, "1.60");
    assert_eq!(stock(&fixture).await, "19");

    let active = fixture
        .service
        .active_cash_shift(&fixture.actor, fixture.register_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "111.60");
    assert_eq!(active.exit_total, "11.60");
    assert_eq!(active.entry_total, "23.20");

    let audit = fixture
        .service
        .list_audit(&fixture.actor, 100)
        .await
        .unwrap();
    assert_eq!(
        audit
            .iter()
            .filter(|event| event.action == "sales.returned")
            .count(),
        1
    );
}

#[tokio::test]
async fn devoluciones_sucesivas_nunca_exceden_y_la_completa_cuadra() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-a", "1", "restock"),
        )
        .await
        .unwrap();
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-b", "1", "restock"),
        )
        .await
        .unwrap();
    assert_eq!(stock(&fixture).await, "20");
    let active = fixture
        .service
        .active_cash_shift(&fixture.actor, fixture.register_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "100.00", "todo el efectivo regresó");
    assert_eq!(active.exit_total, "23.20");

    let excess = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "ret-c", "1", "restock"),
        )
        .await
        .unwrap_err();
    assert_eq!(excess.code(), "conflict");
    assert_eq!(stock(&fixture).await, "20");
}

#[tokio::test]
async fn reembolso_fraccionario_acumulado_no_excede_la_linea() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "CENT-1", "0.01", 0).await;
    add_stock(&fixture, &product, "10").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "3",
        "1.00",
        "frac-sale",
    )
    .await;
    assert_eq!(sale.total, "0.03", "cinco centavos de línea");

    for index in 0..6 {
        fixture
            .service
            .create_sale_return(
                &fixture.actor,
                return_draft(
                    &sale,
                    &fixture.shift_id,
                    &format!("frac-{index}"),
                    "0.5",
                    "restock",
                ),
            )
            .await
            .unwrap();
    }

    let row = sqlx::query(
        "SELECT COALESCE(SUM(refund_total_minor), 0) AS refunded, COUNT(*) AS returns FROM sale_return",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("refunded"), 3, "nunca más que la línea");
    assert_eq!(row.get::<i64, _>("returns"), 6);

    // Los incrementos de cero centavos no crean movimientos de caja ficticios.
    let cash = sqlx::query(
        "SELECT COALESCE(SUM(amount_minor), 0) AS total, COUNT(*) AS movements FROM cash_movement WHERE kind = 'out'",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(cash.get::<i64, _>("total"), 3);
    assert_eq!(cash.get::<i64, _>("movements"), 3);

    let active = fixture
        .service
        .active_cash_shift(&fixture.actor, fixture.register_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.expected_amount, "100.00");
    assert_eq!(
        fixture
            .service
            .current_stock(&fixture.actor, product.parse().unwrap(), None)
            .await
            .unwrap(),
        "10"
    );
}

#[tokio::test]
async fn preview_coincide_con_la_transaccion_en_cantidades_fraccionarias() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "TRE-3", "0.03", 0).await;
    add_stock(&fixture, &product, "2").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "0.5",
        "1.00",
        "frac-preview-sale",
    )
    .await;
    assert_eq!(sale.total, "0.02");

    let preview = fixture
        .service
        .preview_sale_return(
            &fixture.actor,
            SaleReturnPreviewDraft {
                sale_id: sale.id.clone(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "0.25".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(preview.refund_total, "0.01");
    let first = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(
                &sale,
                &fixture.shift_id,
                "frac-preview-a",
                "0.25",
                "restock",
            ),
        )
        .await
        .unwrap();
    assert_eq!(first.refund_total, preview.refund_total);

    let second_preview = fixture
        .service
        .preview_sale_return(
            &fixture.actor,
            SaleReturnPreviewDraft {
                sale_id: sale.id.clone(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "0.25".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(second_preview.refund_total, "0.01");
    let second = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(
                &sale,
                &fixture.shift_id,
                "frac-preview-b",
                "0.25",
                "restock",
            ),
        )
        .await
        .unwrap();
    assert_eq!(second.refund_total, second_preview.refund_total);
    assert_eq!(
        first.refund_total.parse::<MoneyView>().unwrap().0
            + second.refund_total.parse::<MoneyView>().unwrap().0,
        2,
        "la devolución completa cuadra con el total vendido"
    );
}
