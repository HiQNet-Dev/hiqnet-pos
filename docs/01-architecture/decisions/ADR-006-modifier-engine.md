# ADR-006 (propuesto): motor de modificadores y personalización

Estado: **Propuesto** — no implementado.

## Alcance de G2.2B

Permitir que un producto simple o una variante tenga grupos de opciones:

- `ModifierGroup`: nombre, mínimo, máximo, obligatorio, orden.
- `ModifierOption`: nombre, delta de precio exacto, activo, orden.
- Reglas: selección obligatoria, mínimo/máximo, exclusividad.
- Ingredientes omitibles: opción de tipo exclusión sin cambio de precio.
- Extras: opción con delta de precio positivo.

## Invariantes propuestas

1. El precio final se calcula en Rust: precio base + deltas exactos.
2. Una configuración incompleta o fuera de reglas se rechaza antes de persistir.
3. La selección se representa como snapshot inmutable para la futura venta.
4. Los modificadores no descuentan inventario por sí mismos.
5. Recetas e insumos son un concepto separado y requieren política explícita.

## Ejemplo funcional

Licuado de fresa:
- Tamaño: chico, mediano, grande (grupo obligatorio, uno).
- Leche: normal o deslactosada (grupo obligatorio, uno).
- Extras: miel, avena, proteína (grupo opcional, hasta 3).
- Observación: sin azúcar.

## Fuera de alcance

Checkout, ventas persistidas, recetas, descuentos automáticos de insumos y
sincronización. No se crean tablas hasta ratificar invariantes y contratos.
