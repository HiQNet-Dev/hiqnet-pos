//! Rechazos y casos limite del contrato de devolucion.

use super::common::*;

#[tokio::test]
async fn devolucion_rechaza_turno_cerrado_y_saldo_insuficiente() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .register_cash_movement(
            &fixture.actor,
            CashMovementDraft {
                shift_id: fixture.shift_id.clone(),
                kind: "out".into(),
                amount: "120.00".into(),
                reason: "retiro".into(),
            },
        )
        .await
        .unwrap();
    let error = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "sin-saldo", "1", "restock"),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "validation_error");
    assert_eq!(
        stock(&fixture).await,
        "18",
        "no debe revertir sin reembolso"
    );

    fixture
        .service
        .close_cash_shift(
            &fixture.actor,
            CashCloseShiftDraft {
                shift_id: fixture.shift_id.clone(),
                counted_amount: "3.20".into(),
            },
        )
        .await
        .unwrap();
    let closed = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "turno-cerrado", "1", "restock"),
        )
        .await
        .unwrap_err();
    assert_eq!(closed.code(), "conflict");
    assert_eq!(stock(&fixture).await, "18");
    assert!(
        fixture
            .service
            .list_sale_returns(&fixture.actor)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn devolucion_de_cero_centavos_no_exige_turno_ni_mueve_caja() {
    let fixture = fixture().await;
    let product = new_product(&fixture, "MIL-1", "0.01", 0).await;
    add_stock(&fixture, &product, "10").await;
    let sale = sell_product(
        &fixture.service,
        &fixture.actor,
        &fixture.shift_id,
        &product,
        "1",
        "1.00",
        "zero-sale",
    )
    .await;
    fixture
        .service
        .close_cash_shift(
            &fixture.actor,
            CashCloseShiftDraft {
                shift_id: fixture.shift_id.clone(),
                counted_amount: "100.01".into(),
            },
        )
        .await
        .unwrap();

    let created = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            SaleReturnDraft {
                sale_id: sale.id.clone(),
                shift_id: None,
                reason: "reintegro sin importe".into(),
                idempotency_key: "zero-refund".into(),
                lines: vec![SaleReturnLineDraft {
                    sale_item_id: sale.items[0].id.clone(),
                    quantity: "0.001".into(),
                    disposition: "restock".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(created.refund_total, "0.00");
    assert_eq!(created.lines[0].refund_total, "0.00");

    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM cash_movement WHERE kind = 'out') AS exits, (SELECT shift_id IS NULL AND cash_movement_id IS NULL FROM sale_return WHERE id = ?) AS sin_caja",
    )
    .bind(&created.id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(
        row.get::<i64, _>("exits"),
        0,
        "sin movimiento de caja ficticio"
    );
    assert_eq!(row.get::<i64, _>("sin_caja"), 1);
    assert_eq!(
        fixture
            .service
            .current_stock(&fixture.actor, product.parse().unwrap(), None)
            .await
            .unwrap(),
        "9.001",
        "la restitución física sí ocurre"
    );
}
