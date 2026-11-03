# ADR-005 (propuesto): catálogo configurable para G2.2

Estado: **Propuesto** — no implementado.

## Contexto

El catálogo actual (Product simple con SKU, nombre, precio e impuesto) no cubre
cafeterías, juguerías o tiendas con tamaños, extras o modificadores. Se necesita
un modelo que soporte ambos sin crear POS separados.

## Modelo conceptual propuesto

- `Category`: agrupación visual y de reportes.
- `Product`: ítem vendible base.
- `ProductVariant`: combinación de opciones que cambia precio/SKU (tamaño, sabor).
- `ModifierGroup`: conjunto de opciones con reglas de selección (mínimo/máximo).
- `ModifierOption`: opción con delta de precio.
- `SelectionRule`: obligatorio, mínimo, máximo y exclusividad.
- `SaleItemSnapshot`: copia inmutable de nombre, precio, impuestos y opciones al
  momento de la venta.
- Futuro: `Recipe`/`Ingredient` para consumo de insumos.

## Invariantes

1. Los precios y cantidades siguen siendo valores exactos (centavos/milésimas).
2. Una venta nunca depende del catálogo vivo para reconstruir su importe.
3. Las reglas de selección se validan en Rust antes de persistir.
4. Los snapshots son inmutables y auditables.

## Fuera de alcance

No se crean tablas ni comandos en esta iteración. Requiere aprobación de
invariantes y contratos antes de implementar.
