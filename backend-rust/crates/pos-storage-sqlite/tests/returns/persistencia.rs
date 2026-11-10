//! Supervivencia al reinicio del proceso.

use super::common::*;

#[tokio::test]
async fn devolucion_y_reportes_persisten_tras_reinicio() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    fixture
        .service
        .create_sale_return(
            &fixture.actor,
            return_draft(&sale, &fixture.shift_id, "persistente", "1", "restock"),
        )
        .await
        .unwrap();
    fixture.pool.close().await;

    let reopened = open(fixture.dir.path().join("returns.db")).await.unwrap();
    let service = build_service(reopened);
    let actor = service
        .login(LoginRequest {
            username: "owner".into(),
            password: "contrasena-segura".into(),
        })
        .await
        .unwrap();
    let returns = service.list_sale_returns(&actor).await.unwrap();
    assert_eq!(returns.len(), 1);
    assert_eq!(returns[0].refund_total, "11.60");
    assert_eq!(
        service
            .current_stock(&actor, fixture.product_id.parse().unwrap(), None)
            .await
            .unwrap(),
        "19"
    );
    let from = OffsetDateTime::now_utc() - time::Duration::days(1);
    let to = OffsetDateTime::now_utc() + time::Duration::days(1);
    let report = service
        .reports_summary(
            &actor,
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
    assert_eq!(report.sales.total, "23.20");
    assert_eq!(report.sales.returned_total, "11.60");
    assert_eq!(report.sales.net_total, "11.60");
}
