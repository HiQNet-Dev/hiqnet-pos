//! Fallo a mitad del procedimiento y atomicidad.

use super::common::*;

#[tokio::test]
async fn rollback_ante_fallo_de_caja_inventario_o_auditoria() {
    for (name, trigger) in [
        (
            "cash",
            "CREATE TRIGGER fail_return_cash BEFORE INSERT ON cash_movement WHEN NEW.reason LIKE 'devolución %' BEGIN SELECT RAISE(ABORT, 'cash failure'); END",
        ),
        (
            "inventory",
            "CREATE TRIGGER fail_return_inventory BEFORE INSERT ON inventory_movement WHEN NEW.reason LIKE 'devolución %' BEGIN SELECT RAISE(ABORT, 'inventory failure'); END",
        ),
        (
            "audit",
            "CREATE TRIGGER fail_return_audit BEFORE INSERT ON audit_event WHEN NEW.action = 'sales.returned' BEGIN SELECT RAISE(ABORT, 'audit failure'); END",
        ),
    ] {
        let fixture = fixture().await;
        let sale = sell(&fixture, "2").await;
        sqlx::query(trigger).execute(&fixture.pool).await.unwrap();
        let error = fixture
            .service
            .create_sale_return(
                &fixture.actor,
                return_draft(&sale, &fixture.shift_id, name, "1", "restock"),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), "storage_error", "{name}");
        let row = sqlx::query(
            "SELECT (SELECT COUNT(*) FROM sale_return) AS returns, (SELECT COUNT(*) FROM inventory_movement WHERE reason LIKE 'devolución %') AS movements, (SELECT COUNT(*) FROM audit_event WHERE action = 'sales.returned') AS audits",
        )
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(row.get::<i64, _>("returns"), 0, "{name}");
        assert_eq!(row.get::<i64, _>("movements"), 0, "{name}");
        assert_eq!(row.get::<i64, _>("audits"), 0, "{name}");
        assert_eq!(stock(&fixture).await, "18", "{name}");
    }
}
