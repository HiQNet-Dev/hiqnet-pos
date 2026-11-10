//! Reintentos con la misma clave.

use super::common::*;

#[tokio::test]
async fn devolucion_idempotente_y_conflicto_por_contenido() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let draft = return_draft(&sale, &fixture.shift_id, "misma-clave", "1", "restock");
    let first = fixture
        .service
        .create_sale_return(&fixture.actor, draft.clone())
        .await
        .unwrap();
    let replay = fixture
        .service
        .create_sale_return(&fixture.actor, draft)
        .await
        .unwrap();
    assert_eq!(first.id, replay.id);
    assert_eq!(stock(&fixture).await, "19");

    let conflict = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "misma-clave", "1", "discard"),
        )
        .await
        .unwrap_err();
    assert_eq!(conflict.code(), "conflict");

    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM sale_return) AS returns, (SELECT COUNT(*) FROM sale_return_line) AS lines, (SELECT COUNT(*) FROM cash_movement WHERE kind = 'out') AS exits, (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'devolución %') AS movements",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("returns"), 1);
    assert_eq!(row.get::<i64, _>("lines"), 1);
    assert_eq!(row.get::<i64, _>("exits"), 1);
    assert_eq!(row.get::<i64, _>("movements"), 1);
}
