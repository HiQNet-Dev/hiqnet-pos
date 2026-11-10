//! Aislamiento por organizacion y permisos.

use super::common::*;

#[tokio::test]
async fn devolucion_aislada_y_denegada_a_cajero() {
    let fixture = fixture().await;
    let sale = sell(&fixture, "2").await;
    let cashier = Actor {
        role: Role::Cashier,
        permissions: Role::Cashier.permissions(),
        ..fixture.actor.clone()
    };
    assert_eq!(
        fixture
            .service
            .create_sale_return(
                &cashier,
                return_draft(&sale, &fixture.shift_id, "cashier", "1", "restock"),
            )
            .await
            .unwrap_err()
            .code(),
        "access_denied"
    );
    let foreign_branch = Actor {
        branch_id: Some(BranchId::new()),
        ..fixture.actor.clone()
    };
    assert_eq!(
        fixture
            .service
            .create_sale_return(
                &foreign_branch,
                return_draft(&sale, &fixture.shift_id, "foreign", "1", "restock"),
            )
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    let foreign_organization = Actor {
        organization_id: OrganizationId::new(),
        ..fixture.actor.clone()
    };
    assert_eq!(
        fixture
            .service
            .find_sale_for_return(&foreign_organization, sale.folio.clone())
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
}
