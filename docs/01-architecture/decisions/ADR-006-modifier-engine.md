# ADR-006 (propuesto): motor de modificadores y personalización

Estado: **Propuesto** — implementado en PR #5 y pendiente de ratificación.

Aprobación técnica explícita: **pendiente**.

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

## Política fiscal provisional de G2.2B

- `unit_price` es un importe neto.
- Los impuestos se calculan y suman después, con redondeo half-up a centavos.
- Variantes y modificadores se consideran incrementos netos y usan la tasa del
  producto padre.
- El total configurable representa importe neto + impuesto.
- La política fiscal mexicana final y el redondeo por venta corresponden a
  G2.4/G4 y siguen en análisis legal.

## Estado actual de implementación (2026-11-03)

Implementado en `main` mediante PR #5:
- Tablas `modifier_group` y `modifier_option` en migración `0005`.
- Casos de uso e IPC tipados para grupos, opciones y configuración.
- Motor Rust con validación y cálculo autoritativo.
- UI de configuración y desglose.

Todavía se mantienen como propuestos, aunque el código exista:
- Fusión arquitectónica definitiva de modificadores y ventas.
- Relación con recetas e insumos.
- Política fiscal final.

## Distinción de precios

- Cuando se selecciona una variante, su precio neto propio **sustituye** al precio
  base del producto para esa configuración.
- `ModifierOption.price_delta` es un incremento neto independiente que se suma
  después.
- El impuesto resultante es la suma de ambos, calculada con la tasa del producto.
- En el ejemplo de licuado, el tamaño se representa mediante variantes, no como
  un grupo de modificadores duplicado.

## Fuera de alcance

Checkout, ventas persistidas, recetas, descuentos automáticos de insumos y
sincronización. No se crean tablas hasta ratificar invariantes y contratos.
