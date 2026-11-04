use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(value)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

id_type!(OrganizationId, "Identificador de organización (negocio).");
id_type!(BranchId, "Identificador de sucursal.");
id_type!(UserId, "Identificador de usuario operador local.");
id_type!(ProductId, "Identificador de producto de catálogo.");
id_type!(CategoryId, "Identificador de categoría de catálogo.");
id_type!(ProductVariantId, "Identificador de variante de producto.");
id_type!(ModifierGroupId, "Identificador de grupo de modificadores.");
id_type!(ModifierOptionId, "Identificador de opción de modificador.");
id_type!(CashRegisterId, "Identificador de caja registradora.");
id_type!(CashShiftId, "Identificador de turno de caja.");
id_type!(CashMovementId, "Identificador de movimiento de efectivo.");
id_type!(SaleId, "Identificador de venta.");
id_type!(SaleItemId, "Identificador de partida de venta.");
id_type!(SalePaymentId, "Identificador de pago de venta.");
id_type!(SaleReturnId, "Identificador de devolución de venta.");
id_type!(SaleReturnLineId, "Identificador de partida de devolución.");
id_type!(SupplierId, "Identificador de proveedor.");
id_type!(PurchaseOrderId, "Identificador de orden de compra.");
id_type!(
    PurchaseOrderLineId,
    "Identificador de línea de orden de compra."
);
id_type!(PurchaseReceiptId, "Identificador de recepción de compra.");
id_type!(ExpenseCategoryId, "Identificador de categoría de gasto.");
id_type!(ExpenseId, "Identificador de gasto operativo.");
id_type!(WasteId, "Identificador de registro de merma.");
id_type!(AuditEventId, "Identificador de evento de auditoría.");
id_type!(
    InstallationId,
    "Identificador de instalación local del POS."
);
id_type!(
    InventoryMovementId,
    "Identificador de movimiento de inventario."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip_por_texto() {
        let id = ProductId::new();
        let parsed: ProductId = id.to_string().parse().expect("uuid válido");
        assert_eq!(id, parsed);
    }
}
