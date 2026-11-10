# Auditoría estructural de HiQNet POS

**Estado:** diagnóstico inicial; no modifica código.
**Versión auditada:** `0.1.1-rc.1` (`main` en `b3def1e`).
**Fecha:** 2026-11-10.
**Alcance:** estructura, fronteras de capas, tamaño y complejidad, pruebas y deuda técnica.
**Relacionado:** [Arquitectura](overview.md) · [ADR-001 Workspace](../01-architecture/decisions/ADR-001-workspace-layout.md) · [Matriz de funcionalidades](../04-quality/feature-matrix.md) · [Matriz de plataformas](../04-quality/platform-matrix.md)

> Este documento es una línea base. No autoriza refactorizaciones ni cambios de comportamiento.

## 1. Método

Medición sobre `git ls-files` con el mismo criterio que usa HiQNet Platform:

- Se cuentan archivos `.rs`, `.ts` y `.tsx` versionados.
- Se excluyen `node_modules/`, `target/`, `**/generated/**` y `*.d.ts`.
- Se mide líneas físicas por archivo.
- Las cifras de pruebas corresponden a declaraciones `#[test]`/`#[tokio::test]` en Rust y a casos declarados en TypeScript.

## 2. Línea base

| Métrica | Valor |
|---|---|
| Archivos medidos | 110 |
| Líneas de código | 34 655 |
| Archivos > 1 000 líneas | **7** |
| Archivos 801–1 000 | 1 |
| Archivos 501–800 | 10 |
| Pruebas Rust | 179 |
| Pruebas frontend | Unitarias por pantalla + 7 especificaciones Playwright |
| Crates del backend | 5 (`pos-core`, `pos-application`, `pos-storage-sqlite`, `pos-security`, `pos-licensing`) |
| Comandos IPC | 63 |

## 3. Arquitectura encontrada

POS **ya aplica monolito modular con puertos y adaptadores**. No requiere migración de estilo.

```text
pos-core                dominio puro por módulo (sales, inventory, purchasing,
                        cash, returns, expenses, catalog, identity, audit, money…)
   ↑
pos-application         casos de uso + puertos + RBAC
   ↑            ↑
pos-security   pos-licensing
   ↑            ↑
pos-storage-sqlite      adaptador SQLx/SQLite (StoreTx, migraciones, respaldo)
   ↑
apps/pos-desktop        Tauri 2: composición + IPC tipado
```

Dependencias reales declaradas en los `Cargo.toml`:

| Crate | Depende de |
|---|---|
| `pos-core` | — |
| `pos-application` | `pos-core` |
| `pos-security` | `pos-application` |
| `pos-licensing` | `pos-core`, `pos-application` |
| `pos-storage-sqlite` | `pos-core`, `pos-application`, `pos-security` |
| `apps/pos-desktop` | los cinco anteriores |

**Dependencias circulares: 0.** `pos-core` no conoce SQLx, Tauri ni Tokio, tal como exige `AGENTS.md`.

Fortalezas confirmadas:

- Dominio dividido por negocio (16 módulos en `pos-core`), no por entidad técnica.
- Puertos explícitos (`pos-application/src/ports.rs`) y una única `StoreTx` por mutación auditable.
- Frontend sin reglas de negocio y sin SQL.
- Documentación de decisiones con 10 ADR y matrices de funcionalidad y plataforma.

## 4. Deuda estructural

| Archivo | Líneas | Responsabilidad mezclada | Riesgo |
|---|---|---|---|
| `backend-rust/crates/pos-application/src/service.rs` | **4 050** | ~60 métodos de servicio + ~900 líneas de DTOs | **Alto**: un solo archivo para usuarios, catálogo, ventas, inventario, compras, gastos, devoluciones y caja (**resuelto**, ver §12) |
| `backend-rust/crates/pos-storage-sqlite/src/uow.rs` | **2 263** | Implementación de `Store`/`StoreTx` para todos los agregados | **Alto**: capa transaccional completa en un archivo (**resuelto**, ver §13) |
| `backend-rust/crates/pos-storage-sqlite/src/backup.rs` | 1 571 | Copia, verificación, promoción y reversión | Medio: procedimiento sensible ya cubierto por pruebas (**resuelto**, ver §14) |
| `backend-rust/crates/pos-storage-sqlite/tests/returns.rs` | 1 348 | Todos los escenarios de devoluciones | Medio: localización de pruebas (**resuelto**, ver §15) |
| `frontend/src/screens/CatalogScreen.tsx` | 1 075 | Pantalla, estado, filtros y acciones | Medio (**resuelto**, ver §16) |
| `apps/pos-desktop/src-tauri/src/lib.rs` | 1 030 | 63 comandos IPC y composición | Medio: **excepción documentada** (§17) |
| `frontend/e2e/visual.spec.ts` | 1 016 | Recorridos visuales | Bajo (**resuelto**, ver §17) |

### Patrón observado

Es el mismo que tenía HiQNet Platform antes de su refactorización, pero **de menor magnitud** y sobre una arquitectura de capas ya correcta. El problema no es el diseño, es la granularidad de archivos.

### Duplicación y acoplamiento

1. DTOs de aplicación mezclados con la lógica del servicio en `service.rs`.
2. `uow.rs` agrupa operaciones de agregados independientes: cualquier cambio en uno obliga a tocar el archivo que usan todos.
3. Pantallas de React con estado, consultas y presentación en el mismo archivo (catálogo y administración).
4. Pruebas de persistencia con escenarios de varios dominios en un solo archivo (`returns.rs`, `expenses.rs`).

## 5. Hallazgo de seguridad (bloqueante de CI)

Detectado al ejecutar el pipeline de esta auditoría el 2026-11-10. **No lo introduce este cambio**: el aviso apareció en la base de datos de RustSec después del último CI verde en `main` (2026-11-07).

```text
Crate:     libsqlite3-sys
Version:   0.30.1
ID:        RUSTSEC-2026-0459
Severity:  6.8 (medium)
Solution:  Upgrade to >=0.39.0
```

Ruta de dependencia real, verificada con `cargo tree`:

```text
libsqlite3-sys v0.30.1
└── sqlx-sqlite v0.8.6
    └── sqlx v0.8.6
        └── pos-storage-sqlite v0.1.1-rc.1
```

### Por qué no puede ignorarse como en Platform

En HiQNet Platform este aviso era inerte: aparecía en `Cargo.lock` como dependencia opcional de `sqlx` sin ruta de compilación. **En POS ocurre lo contrario**: `sqlx` está compilado con la feature `sqlite` y `pos-storage-sqlite` es la autoridad de datos. El C SQLite vulnerable está en el binario que gestiona toda la base local, incluidos respaldos y restauración.

### Opciones evaluadas

| Opción | Viable | Motivo |
|---|---|---|
| Actualizar a `sqlx 0.9` | **No corrige** | `sqlx-sqlite 0.9.0` exige `libsqlite3-sys >=0.30.1, <0.38.0`. El aviso sólo está parcheado en `>=0.39.0`, así que la actualización no cambia el SQLite embebido |
| `[patch.crates-io]` a `libsqlite3-sys 0.39` | **No** | Cargo exige que el parche satisfaga el requisito del dependiente; `0.39` queda fuera de `<0.38` y el parche se ignora |
| Enlazar SQLite del sistema (`sqlite-unbundled`) | **No para escritorio** | Obligaría a distribuir `sqlite3.dll` y a confiar en la versión del sistema operativo; peor postura de despliegue |
| Sustituir el driver (`rusqlite 0.41`, SQLite 3.54) | **Desproporcionado** | Reescribir `uow.rs` (2 263 líneas), migraciones y macro de consultas; riesgo muy superior al del aviso |

**Conclusión:** hoy no existe corrección por dependencia. Cualquier cambio de versión sería trabajo sin beneficio de seguridad para este CVE.

### Tratamiento aplicado

Se acepta el riesgo de forma acotada y verificable, con la misma disciplina que la excepción ya existente para `RUSTSEC-2023-0071`:

- Excepción en `backend-rust/.cargo/audit.toml` (auditorías locales del workspace) y en `.cargo/audit.toml` de la raíz, que es el que consume `cargo audit` desde la raíz —la invocación que hace `rustsec/audit-check` en CI—.
- `cargo audit` vuelve a pasar en ambas invocaciones, por lo que el job de seguridad de CI recupera su valor como señal.

Controles compensatorios, verificados en código:

| Control | Evidencia |
|---|---|
| El vector es local (`CVSS AV:L`) y exige que un tercero escriba la base y su `-wal` | Aviso RUSTSEC-2026-0459 |
| POS sólo abre su propia base en una ruta fija, nunca rutas del usuario | `pos-storage-sqlite::paths` |
| La base viva y su WAL los escribe únicamente la aplicación en el directorio de datos del usuario | `backup.rs` usa `wal_checkpoint(TRUNCATE)` sobre la base propia |
| Los respaldos, único archivo que puede venir de fuera, se generan con `VACUUM INTO`: archivo único sin `-wal` acompañante | `backup.rs:86`, `backup.rs:696` |
| El impacto es divulgación de memoria en resultados, no ejecución de código | Categoría `memory-exposure` del aviso |

La precondición del CVE —una base y un `-wal` hermano con tamaños de página distintos— **no se produce en el flujo de datos de POS**: o los escribe la propia aplicación, o llegan como un archivo único sin WAL.

### Condición de salida

Retirar la excepción cuando exista una versión de `sqlx` que admita `libsqlite3-sys >=0.39`, o al sustituir el driver SQLite. Revisar obligatoriamente antes del piloto comercial.

## 6. Riesgos

| Riesgo | Impacto | Nota |
|---|---|---|
| `service.rs` monolítico | Alto | Conflictos de merge y dificultad para añadir dominios |
| `uow.rs` monolítico | Alto | Toca la capa transaccional; cualquier división debe preservar `StoreTx` y el orden de bloqueos |
| RUSTSEC-2026-0459 en `libsqlite3-sys` | Medio | Aceptado con excepción justificada y controles; revisar antes del piloto (§5) |
| Restauración desde UI | Medio | La matriz la marca PENDIENTE; el camino seguro es la utilidad offline |
| Emisión de licencias | Medio | PENDIENTE y es responsabilidad de Platform |
| Validación Linux/Android | Medio | Requiere ejecutar `docs/04-quality/platform-matrix.md` en cada host |
| Integración con Platform | Medio | Contratos existentes; falta custodia de clave para emisión productiva |

## 7. Pruebas

- **178 pruebas Rust** en `pos-core`, `pos-application` y `pos-storage-sqlite`, incluidas migraciones, fallos de migración, restauración, concurrencia y aislamiento.
- Pruebas de frontend por pantalla con Testing Library.
- 7 especificaciones Playwright: `smoke`, `shell-nav`, `screens-layout`, `visual`, `visual-admin`, `catalog-scale`, `android-back`.
- CI cubre Rust (fmt, clippy, tests), staging de release y `android.yml`/`release-pos.yml` como flujos separados.

No se detectan pruebas desactivadas que oculten funcionalidad bloqueante en lo revisado.

## 8. Recomendación arquitectónica

**Aplicar la técnica de Platform, no su estructura.**

1. Dividir `service.rs` por dominio con implementaciones inherentes en archivos separados, conservando firmas y comportamiento.
2. Dividir `uow.rs` por agregado sin alterar la semántica de `StoreTx` ni el orden de bloqueos y auditoría.
3. Adoptar un verificador de tamaño con línea base propia de POS, con las mismas reglas que Platform: falla por encima de 1 000 líneas sin excepción y ante crecimiento de un archivo existente.
4. Dividir las pruebas de persistencia por escenario cuando se toque el dominio correspondiente.
5. **No** introducir crates nuevos por entidad, capas intermedias ni microservicios. POS es local-first y Tauri ya es el borde.

## 9. Backlog inicial

| Prioridad | Incremento | Criterio de aceptación |
|---|---|---|
| P0 | Dividir `pos-application/service.rs` por dominio | Sin cambios de comportamiento; `cargo test --workspace` idéntico (hecho, ver §12) |
| P0 | Dividir `pos-storage-sqlite/uow.rs` por agregado | Transacciones y auditoría intactas (hecho, ver §13) |
| P1 | Verificador de tamaño con línea base POS | CI falla ante nuevos monolíticos (hecho, ver §12) |
| P1 | Dividir `tests/returns.rs` y `tests/expenses.rs` | Escenarios localizables (returns hecho, ver §15) |
| P1 | Dividir `backup.rs` por responsabilidad | Secuencia de restauración intacta (hecho, ver §14) |
| P2 | `CatalogScreen.tsx` y `lib.rs` de Tauri | Extracción por subcomponente/comando (catálogo hecho, ver §16) |
| P2 | Restauración desde UI | Procedimiento seguro o decisión documentada |
| P3 | Integración de licencia y enrolamiento con Platform | Contrato productivo con custodia de clave |

## 10. Primer incremento recomendado

**Dividir `pos-application/service.rs`.**

Es el mayor riesgo de mantenibilidad, no toca SQLite ni Tauri, y está cubierto por 178 pruebas. Enfoque propuesto: extraer los DTOs a un módulo propio y repartir el `impl PosService` en módulos por dominio (`users`, `catalog`, `sales`, `inventory`, `purchasing`, `expenses`, `returns`, `cash`, `backup`), sin cambiar firmas, orden de operaciones ni reglas de RBAC.

## 11. Dependencias con HiQNet Platform

Platform Beta `0.1.0-beta.1` queda cerrada y en mantenimiento. POS consume:

- **Enrolamiento** de instalaciones con proof Ed25519.
- **Entitlements** efectivos por organización y producto.
- **Licencia** firmada (`LicenseGrant` v1) y su verificación local.
- **Organización y producto** asociados a la instalación.

Pendiente para producción: custodia de clave de firma, elegibilidad de emisión y auditoría de emisión, según [Transición a POS](https://github.com/HiQNet-Dev/hiqnet-platform/blob/main/docs/02-development/platform-transition-pos.md).

## 12. Estado tras el incremento P0 (división de `service.rs`)

`backend-rust/crates/pos-application/src/service.rs` pasó de un archivo único de 4 050 líneas a un módulo `service/` con un archivo por dominio, sin cambios de comportamiento ni de contratos.

### Estructura resultante

```text
crates/pos-application/src/service/
├── mod.rs          # imports, constantes, mods, reexports y struct PosService
├── core.rs         # new, status
├── helpers.rs      # utilidades compartidas (fechas, dinero, comercial, contrasena)
├── users.rs        # sesion, usuarios y bootstrap
├── catalog.rs      # productos, categorias, variantes y modificadores
├── inventory.rs    # movimientos, stock y diagnostico de integridad
├── cash.rs         # registros, turnos y movimientos de caja
├── sales.rs        # ventas
├── purchasing.rs   # proveedores y ordenes de compra
├── expenses.rs     # gastos y merma
├── returns.rs      # devoluciones
├── reporting.rs    # reportes
├── audit.rs        # auditoria y respaldos
├── tests.rs        # pruebas unitarias del servicio
└── dto/            # un archivo de DTO por dominio
```

Cada archivo de dominio contiene su propio `impl PosService`; los DTO viven en `service/dto/` y se reexportan desde `mod.rs`, de modo que los nombres públicos de `pos_application` no cambian.

### Decisiones de la división

1. **Una implementación inherente por dominio**: los `impl` no se pueden repartir entre módulos, así que cada dominio declara el suyo. Es la misma técnica empleada en Platform.
2. **Utilidades transversales en `helpers.rs`**: fechas, importes, política de contraseña y resolución comercial se usan desde varios dominios; se centralizan allí con visibilidad `pub(super)` para no duplicar validaciones críticas.
3. **DTO separados de los casos de uso**, uno por dominio, con los constructores inherentes (`UserView::from_user`, `ProductView::of`) junto a su tipo.
4. **Sin abstracciones nuevas**: no se crearon traits, crates ni capas intermedias; la división es exclusivamente organizativa.

### Métricas antes/después

| Métrica | Antes | Después |
|---|---|---|
| Archivos de código > 1 000 líneas | 8 | 7 |
| Archivo manual más largo | 4 050 (`service.rs`) | 2 263 (`uow.rs`) |
| `service.rs` | 4 050 líneas | eliminado |
| Mayor archivo de `pos-application` | 4 050 | 539 (`ports.rs`) |
| Métodos de `PosService` | 66 | 66 |
| Tipos de aplicación | 78 | 78 |
| Pruebas Rust | 179 | 179 |

### Control preventivo

`tools/check_size_policy.py` se ejecuta en CI (job `supply-chain`). Falla ante un archivo nuevo por encima de 1 000 líneas, ante el crecimiento de un archivo registrado en `tools/maintenance_baseline.json` y advierte entre 500 y 1 000 líneas. La línea base sólo se reduce; subirla exige justificación escrita en el PR.

## 13. Estado tras el incremento P0 (división de `uow.rs`)

`backend-rust/crates/pos-storage-sqlite/src/uow.rs` pasó de un archivo único de 2 263 líneas a un módulo `uow/` con un archivo por agregado, sin cambios en el comportamiento transaccional.

### Estructura resultante

```text
crates/pos-storage-sqlite/src/uow/
├── mod.rs           # SqliteStore, impl Store, SqliteTx y su handle de transaccion
├── store_tx.rs      # impl StoreTx: fachada que delega (89 metodos)
├── rows.rs          # formato de valores y mapeo de filas
├── organization.rs  # instalacion, organizacion, sucursal
├── users.rs         # usuarios locales
├── catalog.rs       # productos, categorias, variantes
├── modifiers.rs     # modificadores
├── inventory.rs     # inventario y stock
├── sales.rs         # ventas
├── cash.rs          # cajas, turnos y movimientos
├── purchasing.rs    # compras y recepciones
├── expenses.rs      # gastos y merma
├── returns.rs       # devoluciones
├── reporting.rs     # consultas agregadas
└── audit.rs         # auditoria
```

### Decisiones de la división

1. **Rust no permite repartir un `impl` de trait entre módulos**. `store_tx.rs` conserva el contrato completo con una delegación por método; el resto vive en módulos por agregado como funciones libres `async fn(tx: &mut SqliteTx, ...)`.
2. **Sin duplicar lógica en la fachada**: ninguna operación se implementa dos veces. El despacho es de una línea por método y no crece con la lógica del dominio.
3. **El cuerpo de cada operación se movió literalmente**: la única transformación mecánica fue `self.tx()` → `tx.tx()`. La transacción, el orden de escrituras, los `?` y los puntos de commit son los mismos.
4. **`commit` y `rollback` permanecen en `store_tx.rs`** porque consumen `Box<Self>` y definen el cierre de la unidad de trabajo.
5. **Sin macros ni `include!`**: la división es real, no cosmética.

### Garantías conservadas

- Una sola `Transaction<'static, Sqlite>` por unidad de trabajo y un único punto de commit.
- El orden y el contenido de las sentencias SQL no cambió: la comparación automática de los 87 cuerpos sólo detectó reajustes de línea de `rustfmt`.
- `BEGIN` diferido, `WAL`, `busy_timeout`, `synchronous` y `foreign_keys` siguen en `open()` de `lib.rs`, sin tocar.
- La auditoría se sigue escribiendo dentro de la misma transacción (`append_audit`).
- `Store`/`StoreTx` no cambian de firma: `pos-application` no requirió modificaciones.

### Métricas antes/después

| Métrica | Antes | Después |
|---|---|---|
| `uow.rs` | 2 263 líneas | eliminado |
| Mayor archivo del backend | 2 263 (`uow.rs`) | 1 571 (`backup.rs`) |
| Archivos de código > 1 000 líneas | 6 | 5 |
| Archivos de `uow/` | 1 | 15 |
| Mayor archivo de `uow/` | 2 263 | 631 (`store_tx.rs`, fachada) |
| Métodos de `StoreTx` | 89 | 89 |
| Pruebas Rust | 179 | 179 |

`store_tx.rs` (631 líneas) se mantiene por encima del umbral informativo de 500 porque replica las 89 firmas del puerto y Rust no permite repartir el `impl`. Su contenido es una delegación por método, no lógica.

## 14. Estado tras el incremento P1 (división de `backup.rs`)

`backend-rust/crates/pos-storage-sqlite/src/backup.rs` pasó de 1 571 líneas a un módulo `backup/` con un archivo por responsabilidad. **La secuencia crítica de restauración no se fragmentó.**

### Responsabilidades identificadas y destino

| Responsabilidad | Archivo | Líneas |
|---|---|---|
| Copia de respaldo y hash | `file.rs` | 49 |
| Verificación (estructura, migraciones, snapshot) y lectura | `verify.rs` | 177 |
| Política, previsualización y pertenencia | `policy.rs` | 157 |
| Restauración diferida y workspace SAF | `pending.rs` | 151 |
| Preparación, promoción y reversión atómicas | `restore.rs` | 472 |
| Puntos de fallo | `hooks.rs` | 24 |
| Inyector de fallos (solo pruebas) | `fault_injector.rs` | 43 |
| Gancho de la ventana checkpoint/promoción (solo pruebas) | `race.rs` | 33 |
| Pruebas de fallo y concurrencia | `failure_tests.rs` | 458 |
| API pública, constantes y reexportes | `mod.rs` | 40 |

### Por qué no se dividió más `restore.rs`

`prepare_destination → promote_and_verify → rollback_to` forman una sola unidad: la atomicidad depende del orden, de mantener el bloqueo exclusivo, de apartar los sidecars WAL/SHM y de la ventana mínima entre el checkpoint y el reemplazo del archivo. Separarlas en módulos distintos sólo ocultaría esa secuencia. `StagedFile` y su `impl Drop` viven en el mismo archivo por la misma razón: la limpieza de artefactos temporales es parte del procedimiento.

### Garantías conservadas

- Copia de recuperación antes de reemplazar la base y reversión si falla la verificación final.
- Cuarentena de sidecars con guarda `wal_has_data` y rechazo de escrituras externas en la ventana.
- `sync_file`/`sync_parent` (fsync) tras escribir; `StagedFile::drop` limpia temporales.
- Marcador de restauración pendiente con SHA-256 y política de pertenencia (instalación/organización).
- `RestoreCritical` cuando ni la base ni la copia se pueden verificar: no se borra nada.
- Bloqueo exclusivo para impedir restaurar con la base en uso.
- El inyector de fallos y el gancho de carrera siguen existiendo **sólo** en compilaciones de prueba.

### Métricas antes/después

| Métrica | Antes | Después |
|---|---|---|
| `backup.rs` | 1 571 líneas | eliminado |
| Mayor archivo del backend | 1 571 (`backup.rs`) | 1 348 (`tests/returns.rs`) |
| Mayor archivo de producción del backend | 1 571 (`backup.rs`) | 631 (`uow/store_tx.rs`) |
| Archivos de código > 1 000 líneas | 5 | 4 |
| Mayor archivo de `backup/` | 1 571 | 472 (`restore.rs`) |
| Pruebas de restauración | 13 | 13 |
| Pruebas Rust | 179 | 179 |

## 15. Estado tras R4 (división de `tests/returns.rs`)

`backend-rust/crates/pos-storage-sqlite/tests/returns.rs` (1 348 líneas, 19 pruebas) pasa a ser un objetivo de pruebas en formato directorio, con un archivo por escenario y los fixtures compartidos en un solo lugar.

```text
crates/pos-storage-sqlite/tests/returns/
├── main.rs           # declara los modulos
├── common.rs         # fixtures: servicio, venta, stock, borradores, MoneyView
├── parciales.rs      # devoluciones parciales, sucesivas y fraccionarias
├── validaciones.rs   # rechazos y casos limite
├── idempotencia.rs   # reintentos con la misma clave
├── integridad.rs     # mercancia no reintegrable
├── concurrencia.rs   # devoluciones simultaneas
├── rollback.rs       # fallo a mitad y atomicidad
├── permisos.rs       # aislamiento y permisos
├── persistencia.rs   # reinicio del proceso
└── reportes.rs       # agregados de reportes
```

### Decisiones

1. **Formato directorio con `main.rs`**: Cargo descubre `tests/<nombre>/main.rs` como objetivo `returns`, de modo que `mod common;` resuelve a `tests/returns/common.rs`. Un `tests/returns.rs` con submódulos no lo permite: el root de un objetivo de pruebas resuelve sus módulos en `tests/`.
2. **Los fixtures se publican** (`pub`) porque cruzan módulos; los tipos de la aplicación se reexportan desde `common`, así cada escenario solo importa `use super::common::*;`.
3. **Los cuerpos de las pruebas no se tocaron**: la comparación textual de los 19 bloques contra el original los reporta idénticos.
4. **Separación de dominios**: la mitad del archivo probaba reportes, no devoluciones. Ahora `reportes.rs` agrupa esos siete escenarios sin mezclarlos con el ciclo de devolución.

### Métricas antes/después

| Métrica | Antes | Después |
|---|---|---|
| `tests/returns.rs` | 1 348 líneas | eliminado |
| Archivos del objetivo `returns` | 1 | 11 |
| Mayor archivo del objetivo | 1 348 | 397 (`reportes.rs`) |
| Archivos de código > 1 000 líneas | 4 | 3 |
| Pruebas del objetivo | 19 | 19 |
| Pruebas Rust del workspace | 179 | 179 |

### Nota sobre el recuento

El total real de pruebas del workspace es **179**. Los informes anteriores de esta auditoría indicaban 178 por un error aritmético al sumar los recuentos por objetivo; la cifra correcta se ha fijado en las tablas de este documento.

## 16. Estado tras R5 (división de `CatalogScreen.tsx`)

`frontend/src/screens/CatalogScreen.tsx` (1 075 líneas) mezclaba la pantalla, seis subcomponentes, dos diálogos y utilidades. La pantalla queda como entrada pública y sus partes pasan a la carpeta de la funcionalidad.

### Estructura resultante

```text
frontend/src/screens/
├── CatalogScreen.tsx          # pantalla: estado, filtros, detalle y acciones
└── catalog/
    ├── ProductCreateDialog.tsx   # alta de producto
    ├── CategoryCreateDialog.tsx  # alta de categoría
    ├── VariantEditor.tsx         # alta de variante
    ├── GroupEditor.tsx           # grupo de modificadores
    ├── OptionEditor.tsx          # opción de modificador
    ├── ConfigurationPreview.tsx  # previsualización de precio
    ├── messages.ts               # mensaje de error de IPC
    └── types.ts                  # GroupDraft y OptionDraft
```

### Decisiones

1. **La pantalla conserva su ruta pública** (`screens/CatalogScreen.tsx`), así `AppShell` y las pruebas no cambian de importación y el diff se mantiene pequeño.
2. **Cada componente arrastra sólo los imports que usa**: se calcularon por uso real del identificador, y `tsc` con `noUnusedLocals` lo verifica.
3. **Las props propias viajan con su componente**; sólo se extraen a `types.ts` los borradores que cruzan módulos (`GroupDraft`, `OptionDraft`).
4. **Sin cambios de contrato**: no se toca `api.ts`, ni los tipos de `contracts.ts`, ni ninguna llamada al backend. La autoridad de negocio sigue en Rust.

### Métricas antes/después

| Métrica | Antes | Después |
|---|---|---|
| `CatalogScreen.tsx` | 1 075 líneas | 478 |
| Archivos de la funcionalidad | 1 | 9 |
| Mayor archivo de la funcionalidad | 1 075 | 478 (pantalla) |
| Archivos de código > 1 000 líneas | 3 | 2 |
| Pruebas frontend | 120 | 120 |

### Deuda registrada

`messageOf` sigue duplicado en seis archivos (`AdminScreen`, `ExpensesScreen`, `PurchasingScreen`, `ReturnsScreen`, `AppShell` y el catálogo). Extraerlo a `src/lib` exige tocar cinco pantallas fuera del alcance de este incremento; queda anotado para un incremento transversal.

## 17. Estado tras R6 (evaluación de `src-tauri` y pruebas visuales)

### `frontend/e2e/visual.spec.ts` — dividido

El archivo mezclaba datos de demostración, el puente Tauri simulado y los siete escenarios. Queda:

```text
frontend/e2e/
├── visual.spec.ts          # los 7 escenarios (314 líneas)
└── support/
    ├── fixtures.ts         # datos de demostración (342)
    └── visual.ts           # mockBridge y utilidades de captura (386)
```

Los cuerpos de las pruebas no se modificaron; `playwright test --list` sigue enumerando los 7 escenarios y la suite completa pasa en Chromium real (ver §18).

### `apps/pos-desktop/src-tauri/src/lib.rs` — excepción documentada

**No se divide.** Diagnóstico:

- Es un **adaptador de entrada**, no un módulo de dominio: cada uno de los ~60 comandos ocupa entre 3 y 8 líneas y se limita a `authenticated(&state)` → `state.service.<caso de uso>` → mapeo de error.
- No contiene reglas de negocio, SQL ni estado propio más allá de `AppState` (servicio + sesión) y `IpcError`.
- Su tamaño **crece con el contrato IPC**, no con la complejidad: dividirlo por dominio produciría ocho módulos de envoltorios finos y obligaría a repartir `AppState`, `IpcError` y el registro `generate_handler!`, que debe enumerar todos los comandos en un solo sitio.
- La cobertura de esta capa es la que aporta el E2E nativo (`tools/windows-native-e2e`), no pruebas unitarias: refactorizarla sin esa red sería más arriesgado que el beneficio de mantenibilidad.

Se mantiene, por tanto, como excepción razonada en la línea base.

### Métricas antes/después

| Métrica | Antes | Después |
|---|---|---|
| `visual.spec.ts` | 1 016 líneas | 314 |
| Mayor archivo de `e2e/` | 1 016 | 386 (`support/visual.ts`) |
| Archivos de código > 1 000 líneas | 2 | **1** (`src-tauri/src/lib.rs`, excepción) |

## 18. Cierre de la deuda estructural prioritaria

### Evidencia gráfica

La suite visual se ejecutó en Chromium real sobre el build de producción, con el puente Tauri simulado:

```text
VISUAL_REVIEW=1 pnpm exec playwright test e2e/visual.spec.ts
7 passed (8.7s)
```

Cada escenario verifica composición, ausencia de desbordamiento horizontal, contraste de texto y bordes, foco visible y ausencia de errores de consola en 1920, 1440, 1366, 1024 y tablet. Las capturas quedan en `.devkit/artifacts/design-review/after/`.

**Alcance de esta evidencia:** es el webview real renderizando el build real, pero con IPC simulado. La validación contra el ejecutable nativo corresponde a `tools/windows-native-e2e` (Selenium + `tauri-driver`), que no se ejecutó en este entorno.

### Balance global de la refactorización

| Métrica | Inicio | Cierre |
|---|---|---|
| Archivo de producción más largo | 4 050 (`service.rs`) | 1 030 (`src-tauri/src/lib.rs`, excepción) |
| Archivos de código > 1 000 líneas | 8 | 1 |
| Pruebas Rust | 179 | 179 |
| Pruebas frontend | 120 | 120 |
| Endpoints, esquema, migraciones o contratos modificados | — | 0 |

### Deuda que se conserva, con motivo

| Archivo | Líneas | Motivo |
|---|---|---|
| `apps/pos-desktop/src-tauri/src/lib.rs` | 1 030 | Fachada IPC fina; su tamaño sigue al contrato, no a la lógica (§17) |
| `frontend/src/screens/AdminScreen.tsx` | 843 | Cohesiva y cubierta por pruebas de pantalla; sólo requiere revisión |
| `frontend/src/screens/SalesScreen.tsx` | 604 | Flujo de venta cohesionado, con pruebas |
| `backend-rust/crates/pos-application/src/ports.rs` | 539 | Un puerto por agregado en un solo trait `StoreTx`; dividirlo cambiaría la API interna |
| `backend-rust/crates/pos-storage-sqlite/src/uow/store_tx.rs` | 631 | Repite las firmas del puerto; Rust no permite repartir el `impl` |
| `messageOf` duplicado en 6 archivos | — | Extracción transversal fuera del alcance de estos incrementos |

**Conclusión:** no quedan archivos monolíticos de producción sin justificación. La política de tamaño en CI impide que reaparezcan.
