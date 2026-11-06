# Auditoría administrativa y escala de catálogo — 0.1.0

> Estado: **parcialmente cerrado** en `chore/integration-android-ux`, posterior a
> `7537b4c`. Este informe distingue evidencia ejecutada de pendientes; no declara
> una prueba nativa o Android que no se haya realizado en esta pasada.

## Alcance y evidencia

- Revisión renderizada con fixture determinista de administración: Reportes,
  Compras, Gastos, Devoluciones y Gestión a 1440×900, 1024×700 y 768×1024.
- Revisión de Inventario y Caja contra las capturas renderizadas existentes y el
  código actual. Las capturas previas siguen siendo válidas para sus flujos de
  movimiento, arqueo y panel lateral.
- Benchmark SQLite temporal de 100, 1,000 y 10,000 productos con variantes y
  modificadores representativos.
- Medición Chromium separada para carga/filtrado de 1k y 10k; usa puente IPC
  simulado, por lo que no se atribuye a IPC, SQLite ni Tauri.

Los fixtures no contienen datos de comercios ni bases personales.

## Matriz de auditoría y correcciones

| ID | Módulo | Hallazgo / causa | Severidad | Corrección | Evidencia y estado |
| --- | --- | --- | --- | --- | --- |
| ADM-01 | Inventario | El detalle mostraba precio neto e impuesto, pero no el importe al público que el propietario reconoce en Ventas. | P2 | El resumen del producto muestra `public_unit_total` emitido por Rust como «Al público» y mantiene IVA como dato fiscal. No se calcula nada en React. | `InventoryScreen.test.tsx`; **PASS**. La captura existente `after/09-inventario-1920.png` conserva composición y scroll interno. |
| ADM-02 | Caja y turnos | Riesgo de comprimir el historial lateral con la rejilla de movimientos. | P2, corregido previamente | La composición usa `.ledger-history`, no las cuatro columnas del ledger operativo; estado abierto/cerrado y diferencia siguen visibles. | `screens-layout.spec.ts`; capturas `after/10-caja-1920.png`, `after/10b-arqueo-1920.png`; **PASS anterior, no modificado en esta pasada**. |
| ADM-03 | Compras | `.purchase-orders` imponía 360 px mínimos incluso con dos órdenes, dejando una superficie vacía sin información. | P2 | Se eliminó la altura artificial; proveedor y órdenes mantienen altura natural y se apilan bajo 960 px. | Antes: `after/32-compras-1440.png`. Después: `after/51-compras-flujo-natural-1440.png`; Playwright visual; **PASS**. |
| ADM-04 | Gastos | Categoría, importe, origen y acción se presentan secuencialmente; la casilla sigue dentro del objetivo táctil y explica cuando no hay turno. | P3, sin cambio nuevo | La revisión no encontró un defecto funcional adicional sin rediseñar una operación financiera. Se preservan la selección explícita de origen y la idempotencia. | `after/35-gastos-1024x700.png`; `ExpensesScreen.test.tsx`; **PASS de regresión existente**. |
| ADM-05 | Devoluciones | Un reembolso calculado podía seguir visible después de cambiar cantidad o destino; además una respuesta IPC anterior podía llegar tarde. | **P1** | Se asocia la vista previa Rust a un fingerprint de partidas/destinos y a un identificador de solicitud. Cualquier cambio invalida la vista y toda respuesta obsoleta se ignora; la confirmación queda deshabilitada hasta recalcular. | `ReturnsScreen.test.tsx › invalida…` y `› ignora una respuesta…`; **PASS**. La confirmación, idempotencia y reintegro siguen en Rust. |
| ADM-06 | Reportes | Cambiar una fecha disparaba una consulta al montar y otra por cada edición, mientras el botón «Actualizar» parecía opcional y los datos previos podían parecer actuales. | P2 | Carga inicial única; el período editado se consulta solo con «Actualizar». Mientras haya filtros sin aplicar, se identifica el período de los datos visibles y se solicita actualizar. | `ReportsScreen.test.tsx`; después `after/52-reportes-actualizacion-explicita-1024x700.png`; **PASS**. |
| ADM-07 | Gestión | Usuarios, licencia, respaldo y auditoría ya están separados en superficies coherentes; acciones destructivas son discretas y conservan confirmación. | P3, sin cambio nuevo | No se añadió navegación interna artificial ni controles nuevos. | `after/37-gestion-1024x700.png`, `after/39-gestion-768.png`; `screens-layout.spec.ts`; **PASS de revisión visual**. |

### Responsive y accesibilidad observados

| Vista | Resultado |
| --- | --- |
| Reportes 1440×900 | La tabla de ventas ocupa la fila completa; importes se alinean y los resúmenes secundarios no fuerzan overflow. |
| Reportes 1024×700 / 768×1024 | Las superficies se apilan; fechas y acción siguen visibles. |
| Compras 1440×900 | Proveedor y órdenes conservan columnas legibles; no hay panel de órdenes vacío interno tras el cambio. |
| Gastos / Devoluciones / Gestión 1024×700 | Campos, tabla y acciones permanecen dentro del scroll de pantalla; no se observó overflow horizontal en Playwright. |
| Administración 768×1024 | Usuarios, estado comercial, respaldo y auditoría siguen una secuencia vertical y los botones conservan etiquetas. |

No se repitió una instalación Android en esta pasada: safe areas, IME, Back y
hardware ARM64 físico siguen con el estado ya documentado en los informes
anteriores. No se declara SAF Android terminado.

## Eficiencia operativa

| Flujo | Antes | Ahora | Medición verificable |
| --- | --- | --- | --- |
| Editar fechas de Reportes | Consulta inicial y consultas automáticas por edición; el botón era redundante. | Una carga inicial y una consulta explícita al actualizar. | `ReportsScreen.test.tsx` verifica 1 llamada antes de editar y 2 tras confirmar «Actualizar». |
| Devolución | El operador podía cambiar partidas después del cálculo y conservar un importe mostrado que ya no correspondía. | Cambio de cantidad/destino exige recálculo antes de habilitar confirmación. | `ReturnsScreen.test.tsx` verifica invalidez y ausencia de llamada de devolución. |
| Examinar catálogo muy grande | Hasta 10,000 tarjetas/botones podían llegar a montarse. | Bloques incrementales de 200 tarjetas; búsqueda/filtros conservan todos los resultados, muestran el total y permiten continuar con «Mostrar N más». | Vitest 10k + Playwright 1k/10k. |

No se afirma una reducción porcentual de clics sin una prueba cronometrada de
operador. No se automatizaron gastos, devoluciones, recepciones ni cierres.

## Benchmark de catálogo

### Método SQLite/Rust

Comando reproducible:

```sh
cd backend-rust
cargo test --release -p pos-storage-sqlite --test catalog_scale_benchmark -- --ignored --nocapture
```

- 25 muestras warm por tamaño, SQLite local temporal, una conexión.
- Distribución: 10 % con variante activa, 5 % con grupo de modificadores,
  productos activos/inactivos, tasas 0/8/16 % y datos UUID válidos.
- `list_products` ejecuta exactamente tres consultas: productos, variantes
  activas y productos con grupos activos. No hay N+1.
- `0011_catalog_list_index.sql` añade `idx_product_org_name` para la consulta
  ordenada por nombre. `EXPLAIN QUERY PLAN` verificó:
  `SEARCH product USING INDEX idx_product_org_name (organization_id=?)`.

| Productos | SQL productos mediana / p95 ms | SQL config mediana / p95 ms | `list_products` Rust mediana / p95 ms | JSON mediana / p95 ms | Cold ms | JSON bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 0.23 / 0.28 | 0.16 / 0.18 | 0.50 / 0.59 | 0.03 / 0.05 | 0.76 | 26,589 |
| 1,000 | 1.99 / 2.47 | 0.47 / 0.63 | 4.14 / 4.56 | 0.32 / 0.38 | 4.21 | 266,179 |
| 10,000 | 20.80 / 21.84 | 3.40 / 3.71 | 45.06 / 45.92 | 3.27 / 3.55 | 46.35 | 2,679,108 |

p99 registrado para 10k: SQL productos **22.65 ms**, configuración **3.77 ms**,
listado **45.95 ms** y serialización **3.78 ms**.

La columna `list_products` incluye apertura/rollback de lectura, mapeo SQLite,
agrupación, cálculo monetario autoritativo y construcción de `ProductView`; no
se descompone artificialmente como si todo fuera SQL. CPU, RAM e IPC no se
midieron en este test y por ello no se reportan como números.

### Método de navegador / React

Comando reproducible:

```sh
cd frontend
pnpm build
pnpm exec playwright test e2e/catalog-scale.spec.ts
```

| Catálogo | Primer render observado ms | Filtrado exacto ms | Tarjetas tras filtrar | Nodos DOM tras filtrar |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 542.41 | 20.46 | 1 | 189 |
| 10,000 | 431.67 | 16.99 | 1 | 189 |

Estas son mediciones Chromium con puente Tauri simulado; incluyen arranque y
render de la SPA, pero **no** miden transferencia IPC real, SQLite ni hardware
Android. La corrección proporcional es representar el catálogo en bloques
incrementales de 200 tarjetas, sin introducir un contrato nuevo ni cargar miles
de nodos simultáneos. La prueba unitaria 10k cubre posiciones 1, 199, 201,
5,000 y 10,000, filtro de categoría, búsqueda por nombre/SKU y Enter con una
coincidencia única.

## Integridad comercial

- `public_unit_total` sigue calculándose en Rust y se presenta sin aritmética
  monetaria en React.
- La migración solo crea un índice; no modifica precios, ventas, devoluciones,
  inventario ni datos históricos.
- La vista previa de devolución continúa siendo de Rust y su payload de
  confirmación conserva idempotency key, turno y líneas ordenadas.
- No se tocaron permisos, auditoría, transacciones ni contratos IPC.

## Validación ejecutada en esta pasada

| Comprobación | Estado |
| --- | --- |
| Vitest completo | **PASS — 118 pruebas** |
| `pnpm typecheck` | **PASS** |
| `pnpm lint` | **PASS** |
| `pnpm build` | **PASS** |
| Playwright catálogo 1k/10k | **PASS — 2 pruebas** |
| Playwright visual administrativo | **PASS — 1 prueba** |
| Benchmark SQLite/Rust release 100/1k/10k | **PASS — 1 prueba ignorada ejecutada manualmente** |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `cargo test --workspace` | **PASS** — benchmark manual permanece ignorado en la suite normal |
| Build Tauri Windows debug (`cargo tauri build --debug --no-bundle`) | **PASS** |
| E2E nativo Windows: flujo operativo | **PASS** — inventario, caja, venta, compras, gastos, merma, permisos y resoluciones tablet sobre SQLite temporal |
| E2E nativo Windows: devoluciones | **PASS** — preview, reembolso, caja, reportes, persistencia y RBAC sobre SQLite temporal |
| E2E nativo Windows: layout drift | **PASS** — 8 rondas, resize/maximizar, scroll interno, modal, foco y sin clipping detectado |
| APK/emulador Android de esta pasada | **NOT RUN todavía** |

## Pendientes y decisión de entrega

- Falta la regresión Android (APK/emulador) sobre el commit resultante.
- ARM64 físico y SAF Android siguen bloqueados/no implementados conforme a los
  informes anteriores.
- Al haber cambiado frontend y SQLite, los instaladores Windows/APK existentes
  dejan de corresponder al SHA final: se deben regenerar y volver a firmar antes
  de presentar artefactos de prueba.
