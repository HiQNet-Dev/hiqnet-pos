//! Devoluciones simultaneas sobre la misma venta.

use super::common::*;

#[tokio::test]
async fn devoluciones_concurrentes_no_duplican_ni_exceden() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let first = Arc::new(build_service(
        open(fixture.dir.path().join("returns.db")).await.unwrap(),
    ));
    let second = Arc::new(build_service(
        open(fixture.dir.path().join("returns.db")).await.unwrap(),
    ));
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let draft = return_draft(&sale, &fixture.shift_id, "concurrente-1", "1", "restock");
    let left = {
        let service = first.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        let draft = draft.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let right = {
        let service = second.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert!(
        results.iter().all(Result::is_ok),
        "el mismo intento concurrente debe recuperar el hecho: {results:?}"
    );
    assert_eq!(stock(&fixture).await, "19");

    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let left = {
        let service = first.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        let draft = return_draft(&sale, &fixture.shift_id, "concurrente-a", "2", "restock");
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let right = {
        let service = second.clone();
        let actor = fixture.actor.clone();
        let gate = gate.clone();
        let draft = return_draft(&sale, &fixture.shift_id, "concurrente-b", "2", "restock");
        tokio::spawn(async move {
            gate.wait().await;
            service.create_sale_return(&actor, draft).await
        })
    };
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        0,
        "solo queda una unidad devolvible"
    );
    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM sale_return) AS returns, (SELECT COALESCE(SUM(quantity_milli), 0) FROM sale_return_line) AS quantity",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("returns"), 1);
    assert_eq!(row.get::<i64, _>("quantity"), 1000);
}
