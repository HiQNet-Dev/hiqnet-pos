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
| Pruebas Rust | 178 |
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
| `backend-rust/crates/pos-storage-sqlite/src/uow.rs` | **2 263** | Implementación de `Store`/`StoreTx` para todos los agregados | **Alto**: capa transaccional completa en un archivo |
| `backend-rust/crates/pos-storage-sqlite/src/backup.rs` | 1 571 | Copia, verificación, promoción y reversión | Medio: procedimiento sensible ya cubierto por pruebas |
| `backend-rust/crates/pos-storage-sqlite/tests/returns.rs` | 1 348 | Todos los escenarios de devoluciones | Medio: localización de pruebas |
| `frontend/src/screens/CatalogScreen.tsx` | 1 075 | Pantalla, estado, filtros y acciones | Medio |
| `apps/pos-desktop/src-tauri/src/lib.rs` | 1 030 | 63 comandos IPC y composición | Medio |
| `frontend/e2e/visual.spec.ts` | 1 016 | Recorridos visuales | Bajo |

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
| P0 | Dividir `pos-storage-sqlite/uow.rs` por agregado | Transacciones y auditoría intactas |
| P1 | Verificador de tamaño con línea base POS | CI falla ante nuevos monolíticos (hecho, ver §12) |
| P1 | Dividir `tests/returns.rs` y `tests/expenses.rs` | Escenarios localizables |
| P2 | `CatalogScreen.tsx` y `lib.rs` de Tauri | Extracción por subcomponente/comando |
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
| Pruebas Rust | 178 | 178 |

### Control preventivo

`tools/check_size_policy.py` se ejecuta en CI (job `supply-chain`). Falla ante un archivo nuevo por encima de 1 000 líneas, ante el crecimiento de un archivo registrado en `tools/maintenance_baseline.json` y advierte entre 500 y 1 000 líneas. La línea base sólo se reduce; subirla exige justificación escrita en el PR.
