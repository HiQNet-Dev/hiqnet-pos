//! Mercancia no reintegrable y efecto en existencias.

use super::common::*;

#[tokio::test]
async fn devolucion_no_reintegrable_conserva_stock() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let created = fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "merma-devolucion", "1", "discard"),
        )
        .await
        .unwrap();
    assert_eq!(created.lines[0].disposition, "discard");
    assert_eq!(stock(&fixture).await, "18");
    let inbound: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'devolución %'",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(inbound, 0);
}
