# Precio al público y UX de Ventas — 0.1.0

Informe de la pasada de rediseño sobre `chore/integration-android-ux` (PR #20).
Complementa `docs/04-quality/ux-audit-0.1.0.md`; aquí se documenta el cambio de
semántica de precios, la nueva experiencia de Ventas y los controles unificados.

## A. Auditoría y hallazgos

| # | Pantalla | Hallazgo | Severidad | Causa | Corrección |
| --- | --- | --- | --- | --- | --- |
| 1 | Ventas | La tarjeta mostraba el precio neto y la etiqueta «+ IVA»: el operador no veía el importe que cobra. | **P1** | `product.unit_price` es la base neta del dominio; no existía un precio al público en el contrato. | `public_unit_total` calculado por Rust en `list_products`; la tarjeta publica ese importe. |
| 2 | Ventas | Productos con variantes mostraban el precio base como si fuera el precio de venta. | **P1** | El listado no sabía si el producto tenía variantes. | `active_variants` y `variant_public_from` en lote; la tarjeta muestra «Desde $X». |
| 3 | Productos (alta) | «Precio (MXN)» era ambiguo: podía interpretarse como precio final. | **P1** | Etiqueta sin contexto y sin vista previa. | «Precio base, sin IVA» + vista previa «Precio final al público» por IPC. |
| 4 | Productos (tabla) | La columna «Precio» mostraba solo el neto. | P2 | No existía columna de precio al público. | Columnas «Precio neto», «IVA» y «Al público»; el detalle enseña el precio con IVA. |
| 5 | Ventas | Cada clic en un producto simple hacía tres llamadas IPC (variantes, modificadores, configuración). | P2 | La pantalla no conocía la configuración comercial del producto. | `active_variants`/`has_modifiers` en el listado: un producto simple se agrega en **una** llamada. |
| 6 | Cobro | El campo de efectivo no permitía reemplazar el importe de un golpe y no ofrecía importes habituales. | P2 | Solo `autoFocus`. | Selección al enfocar y chips «Exacto $X», $20, $50, $100; el cambio se recalcula al instante. |
| 7 | Gastos | La casilla «Pagar desde caja» no tenía estilo: `.checkbox-field` existía en el JSX pero no en CSS. | P2 | Estilo ausente. | `.checkbox-field` con 36 px de alto, control de 16 px y `accent-color` del tema. |
| 8 | Reportes | Los campos de fecha usaban el control nativo sin altura, borde ni tipografía del sistema. | P2 | No había reglas para `input[type="date"]`. | Reglas compartidas con el resto de campos; se conserva el selector nativo. |
| 9 | Formularios | Campos deshabilitados, de solo lectura y con error no tenían tratamiento visual. | P3 | Sin reglas `:disabled`, `[readonly]`, `[aria-invalid="true"]`. | Tres estados documentados en `DESIGN.md`. |
| 10 | Ventas | No había forma de agregar productos por teclado ni con un lector de códigos en modo teclado. | P3 | La búsqueda solo filtraba. | `Enter` con una única coincidencia agrega el producto (`aria-keyshortcuts`). |

Todo hallazgo se corrigió en esta pasada; no queda ninguno abierto.

## B. Precios

### Semántica

| Concepto | Dónde vive | Ejemplo |
| --- | --- | --- |
| Precio neto | `product.unit_price` / `product_variant.unit_price` (`Money`, centavos) | `100.00` |
| Impuesto por unidad | `pos_core::pricing::unit_tax`, half-up a centavos | `16.00` |
| Precio al público | `pos_core::pricing::public_unit_total` = neto + impuesto | `116.00` |
| Configuración de venta | `ModifierConfiguration` (base + modificadores + impuesto por separado) | `116.00` |
| Partida vendida | `SaleItem`: `unit_price` neto, `unit_tax`, `line_total` | `100.00 / 16.00 / 232.00` |

`pos_core::pricing` es la única implementación de la política; `catalog` y
`modifiers` la consumen en lugar de repetir la aritmética.

### Redondeo

El impuesto se redondea **por unidad** con half-up sobre centavos, y después se
suma: no se redondea el total agregado. Por eso `0.05` al 16 % publica `0.06`
(impuesto `0.01`) y `100.00` publica `116.00` (impuesto `16.00`). Dos unidades de
`100.00` dan `232.00`, no un importe distinto por redondear el agregado.

Con modificadores, el impuesto se calcula sobre la base y sobre el total de
modificadores por separado —política vigente de ADR-003/ADR-006— y ambos se
suman. El ticket usa ese total; la tarjeta publica el precio base configurado.

### Contratos

`list_products` añade `public_unit_total`, `active_variants`,
`variant_public_from` y `has_modifiers`. `preview_product_price` calcula sin
escribir. Ambas están documentadas en `docs/03-contracts/ipc.md` y validadas con
Zod en `frontend/src/contracts.ts`. No se modificó ninguna regla de negocio
existente: `unit_price` sigue siendo el neto y `SaleItem` conserva sus campos.

### Pruebas

- `pos-core/src/pricing.rs` (7): 16 %, 0 %, centavos con y sin redondeo, límite
  de tasa y monotonía `público ≥ neto`.
- `pos-storage-sqlite/tests/catalog_pricing.rs` (7): catálogo, tasa cero,
  redondeo por unidad, variantes («Desde»), modificadores, vista previa y una
  venta real de 2 × 116.00 = 232.00 con reintento idempotente.
- `SalesScreen.test.tsx`: precio al público en la tarjeta, «Desde» con variantes,
  una sola llamada IPC y `Enter` en la búsqueda.
- `CatalogScreen.test.tsx`: vista previa por IPC y etiquetas nuevas.
- `screens-layout.spec.ts`: precio publicado en navegador real, sin desbordes.
- E2E nativo Windows: la tarjeta muestra «Desde $13.92» (12.00 + 16 %) antes de
  vender y el total del ticket sigue siendo el de Rust.

## C. UX de Ventas

| Flujo | Antes | Ahora |
| --- | --- | --- |
| Venta simple (agregar 1 producto) | 3 llamadas IPC, 1 clic | **1 llamada IPC**, 1 clic |
| Agregar el mismo producto otra vez | 1 clic por unidad | 1 clic (se fusiona la partida) |
| Agregar con teclado o lector | no disponible | escribir y `Enter` con una coincidencia |
| Conocer el precio a cobrar | importe neto + «+ IVA» | importe final, IVA incluido |
| Cobro | escribir el importe a mano | chips con el exacto y redondeos; cambio inmediato |
| Siguiente venta | cerrar el resultado | el ticket se vacía y el aviso confirma el folio |

No se automatizó ninguna operación comercial: el cobro sigue exigiendo
«Confirmar cobro», el cierre de turno y las devoluciones no cambian y no se
añadieron confirmaciones nuevas a acciones reversibles.

Medición reproducible (no se declara una mejora porcentual sin medirla):

- Llamadas IPC para agregar un producto simple: `list_product_variants` +
  `list_modifier_groups` + `configure_product` → `configure_product`. Verificado
  por `SalesScreen.test.tsx › agrega un producto simple con una sola llamada a Rust`.
- Clics hasta el cobro de una venta de un artículo: 3 (producto → Cobrar →
  Confirmar) en escritorio, sin cambios; lo que se elimina es la espera y la
  ambigüedad del precio.

## D. Sistema de controles

Unificados en esta pasada: `:disabled`, `[readonly]`, `[aria-invalid="true"]`,
`input[type="date"]`, `input[type="checkbox"]`, `input[type="radio"]`,
`.checkbox-field`, `.price-preview` y `.chips-amounts`. Las reglas están en
`frontend/src/styles.css` y su uso normativo en `DESIGN.md`.

No se introdujeron componentes nuevos de React: el problema era de consistencia
visual, y la vista previa se resolvió dentro del diálogo existente.

## E. Evidencia visual

| Antes | Después |
| --- | --- |
| Tarjeta con `$10.00 + IVA` (`before/14-ventas-1366.png`) | `after/04-ventas-ticket-1920.png`, `after/30-ventas-800x1280.png` |
| Alta sin vista previa ni IVA explícito | `after/49-catalogo-precio-publico-1440.png` |
| Tabla con una sola columna «Precio» | `after/33-productos-1440.png` |
| Cobro sin importes rápidos | `after/07-checkout-1920.png` |
| — | `after/50-android-precio-publico.png` (emulador, 11.60 y 13.92) |

## F. Calidad

Ver «Regresión» en el informe de la entrega (sección H de este documento).

## G. Rendimiento

- El catálogo añade **dos** consultas por instrucción (variantes activas y
  grupos con modificadores) y elimina dos llamadas IPC por producto al vender.
- La tarjeta no hace ninguna llamada: recibe los importes ya calculados.
- La vista previa escribe con 250 ms de retardo y descarta respuestas obsoletas;
  no se llama por tecla.
- Paquete: JS 457.13 kB → 460.4 kB y CSS 33.28 kB → 35.1 kB (37 módulos nuevos
  no; son las reglas de controles y la vista previa).

## H. Git e instaladores

Los commits, el SHA final, el estado de CI y los hashes de los artefactos se
registran en `docs/06-release/internal-test-build-0.1.0.md` (Windows) y
`docs/06-release/internal-test-build-0.1.0-android.md` (Android).

## I. Bloques que siguen abiertos

- **SAF Android**: no implementado.
- **Hardware ARM64 físico**: solo emulador `x86_64`.
- Scroll Android con desbordamiento real: no forzado en el emulador; verificado en
  Playwright y en el E2E nativo de Windows.
