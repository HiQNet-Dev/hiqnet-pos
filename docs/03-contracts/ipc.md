# Contratos IPC (Tauri 2)

Los comandos se invocan desde React y son validados en Rust. Las respuestas se
validan en el frontend con Zod (`frontend/src/contracts.ts`).

## Errores

Todo rechazo tiene la forma:

```json
{ "code": "access_denied", "message": "acceso denegado: falta el permiso ..." }
```

Códigos estables: `access_denied`, `invalid_credentials`, `unauthenticated`,
`already_initialized`, `feature_not_entitled`, `commercial_state_blocked`,
`duplicate_username`, `validation_error`, `not_found`, `conflict`,
`storage_error`, `crypto_error`.

## `app_status`

- Entrada: ninguna.
- Salida:

```json
{
  "initialized": true,
  "product_version": "0.1.0",
  "commercial": {
    "plan": "trial",
    "status": "trial",
    "valid_until": "2026-12-03T18:00:00Z",
    "features": ["catalog", "sales"],
    "read_only": false
  },
  "user_count": 1
}
```

`commercial` es `null` antes del bootstrap.

## `bootstrap`

Crea organización, sucursal, propietario y Trial de 30 días en una transacción.

Entrada:

```json
{
  "request": {
    "organization_name": "Abarrotes La Esquina",
    "branch_name": "Matriz",
    "branch_code": "MX-01",
    "owner_username": "propietario",
    "owner_display_name": "Ana Propietaria",
    "owner_password": "mínimo 10 caracteres"
  }
}
```

Salida: `organization_id`, `branch_id`, `user_id`, `trial_ends_at`.
Falla con `already_initialized` si ya existe estado de instalación.

## `login`

Entrada: `{ "request": { "username": "...", "password": "..." } }`.

Salida: `Actor` (identidad, rol y permisos) y deja la sesión en memoria de Rust.
La contraseña nunca se devuelve ni se registra.

## `logout` / `current_session`

`logout` limpia la sesión (salida `null`). `current_session` devuelve el `Actor`
o `null`.

## `create_product`

Requiere permiso `catalog.manage`, entitlement `catalog` y licencia que permita
escrituras.

Entrada:

```json
{ "draft": { "sku": "CAF-001", "name": "Café americano", "price": "12.50", "tax_rate_bp": 1600 } }
```

Salida: producto con `unit_price` como cadena decimal. Un SKU duplicado en la
organización devuelve `conflict`.

## `list_products`

Requiere permiso `sales.sell`. Devuelve los productos de la organización del
actor, ordenados por nombre.

## `create_category` / `list_categories`

`create_category` requiere `catalog.manage` y entitlement `catalog`. Entrada:
`{ "draft": { "name": "Bebidas", "sort_order": 1 } }`.
`list_categories` requiere `sales.sell` y devuelve las categorías de la organización
ordenadas por `sort_order`.

## `assign_product_category`

Requiere `catalog.manage` y entitlement `catalog`. Entrada:
`{ "product_id": "...", "category_id": "... | null" }`. Devuelve el producto
actualizado. La categoría debe pertenecer a la organización del actor y estar
activa; `null` desasocia el producto. Se audita como
`catalog.product_category_changed`.

## `create_product_variant` / `list_product_variants`

`create_product_variant` requiere `catalog.manage` y entitlement `catalog`.
Entrada: `{ "draft": { "product_id": "...", "name": "Grande", "sku": "CAF-001-G", "price": "15.00", "sort_order": 1 } }`.
El SKU es opcional y único por organización. `list_product_variants` requiere
`sales.sell` y recibe `{ "product_id": "..." }`.

## Ventas

`checkout_sale` requiere `sales.sell` y un turno de caja abierto. Entrada:

```json
{
  "draft": {
    "shift_id": "...",
    "idempotency_key": "...",
    "items": [{ "product_id": "...", "variant_id": null, "quantity": "1", "selected_option_ids": [] }],
    "cash_received": "50.00"
  }
}
```

Devuelve folio, subtotal, impuesto, total, partidas y cambio. La misma clave de
idempotencia devuelve la misma venta; contenido distinto produce `conflict`.
La operación descuenta inventario y registra el efectivo en caja en una sola
transacción.

`get_sale` requiere `sales.sell` y devuelve una venta confirmada de la organización.

## Modificadores de catálogo

Comandos: `create_modifier_group`, `list_modifier_groups`,
`create_modifier_option`, `list_modifier_options` y `configure_product`.

`configure_product` requiere `sales.sell`; calcula y devuelve una configuración
validada con desglose autoritativo. Rechaza selecciones fuera de límites,
opciones inactivas y grupos inexistentes.

## `current_stock`

Requiere permiso `inventory.view`. Entrada: `{ "product_id": "...", "branch_id": "... (opcional)" }`.
Devuelve la cantidad exacta como cadena decimal. La sucursal debe coincidir con la
sucursal del actor; en esta versión no hay acceso multisucursal.

## `record_inventory_movement`

Requiere permiso `inventory.manage`, entitlement `inventory` y licencia con
escrituras. Entrada:

```json
{
  "draft": {
    "product_id": "...",
    "kind": "inbound | outbound | adjustment_in | adjustment_out",
    "quantity": "2.5",
    "reason": "motivo obligatorio en ajustes"
  }
}
```

El movimiento y su evento de auditoría se escriben en la misma transacción.
El ledger es append-only. Una salida mayor que el stock devuelve
`validation_error`.

## `list_inventory_movements`

Requiere permiso `inventory.view`. Entrada opcional `{ "product_id": "...", "limit": 50 }`.
Devuelve los movimientos de la sucursal del actor en orden descendente.

## `list_audit_events`

Requiere permiso `audit.view`. Entrada opcional `{ "limit": 50 }` (máximo 200).
Devuelve eventos en orden descendente.

## `commercial_state`

Devuelve el estado comercial efectivo (con reloj actual). Requiere instalación
inicializada.

## `backup_database`

Requiere permiso `backups.manage`. Crea un respaldo consistente con
`VACUUM INTO` en `.../data/backups/` y registra el evento `backup.created`.
Devuelve la ruta del respaldo.

> La restauración no se expone todavía por IPC: requiere cerrar la conexión. El
> procedimiento probado está en `docs/05-operations/backup-and-restore.md`.

## Compatibilidad

Los nombres de campos son parte del contrato. Cualquier cambio requiere
actualizar Rust, `contracts.ts` y este documento en el mismo PR.
