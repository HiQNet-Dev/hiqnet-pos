# Alcance mínimo — G2.7 Proveedores, compras y recepción

- Estado: propuesto (no implementado)
- Fecha: 2026-11-04
- Depende de: G2.6 (usuarios) y del ledger de inventario vigente

## Objetivo

Registrar compras a proveedores y **recibir mercancía** de forma que el inventario
sólo aumente cuando la recepción se confirma, nunca al crear una orden de compra.

## Incluido

1. **Proveedor**: nombre, código o RFC opcional, contacto, activo. Alta y
   listado; sin borrado destructivo (desactivación).
2. **Orden de compra** en estado `draft` o `ordered`: proveedor, líneas con
   producto y cantidad pedida. **No afecta existencias.**
3. **Recepción** parcial o total de una orden: registra cantidades recibidas y
   crea movimientos de inventario `inbound` con motivo `compra {folio}` en la
   **misma transacción** que actualiza la orden.
4. **Costo de compra** por línea, almacenado como importe exacto (`Money`). No
   recalcula precios de venta automáticamente.
5. **Auditoría**: `purchasing.supplier_created`, `purchasing.order_created`,
   `purchasing.received`.
6. **IPC**: `create_supplier`, `list_suppliers`, `create_purchase_order`,
   `list_purchase_orders`, `receive_purchase_order`.
7. **UI**: sección *Compras* en el shell (tabla de órdenes + recepción en
   diálogo). No se apilan formularios ni se duplica el ledger.

## Reglas

- Una recepción no puede exceder lo pendiente de la orden.
- Una orden `received` no admite nuevas recepciones.
- El inventario sólo cambia por recepción confirmada, nunca por la orden.
- Autorización: `purchasing.manage` (nuevo permiso) o `inventory.manage` para
  recibir; se decide en el ADR de G2.7.
- Aislamiento por organización y sucursal; una orden pertenece a una sucursal.
- Migración incremental `0008_purchasing.sql`; nunca se editan migraciones
  publicadas.

## Fuera de alcance

Cuentas por pagar, pagos a proveedores, CFDI de compras, costeo promedio o PEPS,
órdenes automáticas por stock mínimo y sincronización entre sucursales.

## Criterio de terminado

Crear proveedor → crear orden (sin movimiento de inventario) → recibir
parcialmente → verificar existencia incrementada y auditoría → recibir el resto →
la orden queda cerrada y el ledger cuadra con `verify_db` y el E2E nativo.
