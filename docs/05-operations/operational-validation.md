# Validación operativa de HiQNet POS

Fecha de la corrida: **2026-11-10**.
Base: `main` @ `3748280`, ejecutable compilado con `tauri build --debug --no-bundle`.

## Entorno

| Elemento | Valor |
|---|---|
| Sistema | Windows, sesión de escritorio interactiva |
| Runtime de WebView2 | `157.0.4341.42` |
| msedgedriver | `157.0.4341.42` (misma versión que el runtime) |
| tauri-driver | instalado desde `cargo` |
| Selenium / Python | 4.52.0 / 3.14.6 |
| Base de datos | temporal aislada por ejecución (`%TEMP%\hiqnet-e2e-<guid>`) |
| Base real del operador | **no se tocó** (`%LOCALAPPDATA%\HiQNet\POS\data`) |

El arnés `run-e2e.ps1` crea una base única por corrida, exporta `HIQNET_POS_DB_PATH`
y termina sólo los procesos descendientes del driver que inició.

## Etapa 1 — Suite nativa existente

| Guion | Resultado | Evidencia principal |
|---|---|---|
| `e2e_smoke` | PASS | ventana real, título `HiQNet POS`, SQLite creada con WAL |
| `e2e_contracts` | PASS | comandos IPC compuestos y aritmética de precios (16.24 con IVA 16 %) |
| `e2e_flow` | PASS | bootstrap → catálogo → variante → modificadores → inventario → caja → venta → reinicio → compras → gastos/merma → arqueo → permisos → 14 capturas |
| `verify_db` | PASS | `failures: []`, venta, stock, compras, gastos, merma y auditoría consistentes |
| `e2e_returns` | PASS | devolución parcial, reportes, reinicio y denegación al cajero |
| `verify_returns_db` | PASS | partida `restock`, `inbound`, reembolso 11.60, stock 19 |
| `e2e_restore_guard` | PASS | restauración rechazada con la app abierta y con lector externo; correcta al cerrar |
| `restore_multiphase_check` | PASS | 27 tablas comparadas por hash, `integrity_check` ok, 0 violaciones FK, canario eliminado |
| `perf_smoke` | PASS | 200 ventas; arranque 1.02 s, checkout medio 7.97 ms |

**9/9 PASS.** No se modificó código productivo para que pasaran.

## Etapa 2 — Resiliencia

### A. Operación sin conectividad externa

`e2e_offline.py` recorre arranque, bootstrap, login local, venta, inventario,
caja, gasto, devolución, respaldo local y reinicio.

- **Conexiones de red del proceso: 0** en todas las etapas muestreadas.
- Tras reiniciar: venta 23.20, devuelto 11.60, gasto 9.00, stock 19, todo desde SQLite.
- **Limitación declarada:** la sesión no es elevada, así que no se aplicó una regla
  de firewall por proceso. La evidencia es la ausencia total de sockets TCP, no un
  corte de red forzado. Un corte real requiere ejecutar el arnés con privilegios.

### B. Cierre inesperado del proceso

`e2e_crash_recovery.py` dispara una operación por IPC y termina el proceso con
`TerminateProcess` (latencia ~1 ms, sin lanzar un proceso hijo).

| Operación | Cortes | Cortes **antes** del commit | Integridad tras el corte |
|---|---|---|---|
| venta | 2 | 1 | ok |
| inventario | 2 | 1 | ok |
| caja | 2 | 1 | ok |
| compra | 2 | 1 | ok |
| venta de 40 líneas | 2 | 2 | ok |

En los 10 cortes: `integrity_check = ok`, `foreign_key_check` vacío, cero ventas sin
partidas o sin pago, cero ventas sin auditoría, cero órdenes o recepciones sin
partidas, cero claves idempotentes duplicadas y ningún turno con saldo negativo.
Tras los cortes la aplicación volvió a abrir y a vender.

**Por qué importa el «antes del commit»:** demuestra que el corte alcanza el interior
de la transacción. Cuando el corte llegó después del commit, la operación quedó
completa; cuando llegó antes, no quedó rastro. Nunca hubo un estado intermedio.

### C. Restauración y corrupción

`e2e_restore_corruption.py` sobre bases y respaldos desechables:

| Caso | `--verify-backup` | `--restore-backup` | Base intacta |
|---|---|---|---|
| respaldo truncado | rechaza (2) | rechaza (2) | sí |
| respaldo vacío | rechaza (2) | rechaza (2) | sí |
| archivo inexistente | rechaza (2) | rechaza (2) | sí |
| archivo que no es SQLite | rechaza (2) | rechaza (2) | sí |
| bytes alterados | rechaza (2) | rechaza (2) | sí |
| sidecar inconsistente | — | aplica y queda `integrity ok` | sí |

**Interrupción real de la restauración:** medida la duración de referencia (57 ms) se
cortó el proceso a 8.6, 17.2, 28.7, 40.1 y 51.6 ms. Los cinco cortes se produjeron
(`cortado: true`) y en todos la base quedó legible (`integrity ok`); la aplicación
volvió a abrir y una restauración válida posterior siguió funcionando.

## Etapa 3 — Rendimiento y estabilidad

`perf_volume.py`, volumen sintético documentado: **2 000 productos, 1 000 ventas,
3 turnos, 5 010 eventos de auditoría**.

| Métrica | Valor |
|---|---|
| Arranque hasta onboarding | 1.006 s |
| Alta de 2 000 productos | 13.8 s (≈6.9 ms por producto) |
| Stock inicial de 2 000 productos | 14.2 s |
| 1 000 ventas (total / media) | 8.25 s / **8.25 ms** |
| `list_products` (2 000 filas) | 98 ms |
| `list_stock_levels` | 40 ms |
| `reports_summary` | 49 ms |
| `list_audit_events` (200) | 17 ms |
| Cierre de turno | 22 ms |
| Memoria residente | 41.9 MB |
| Respaldo (6.4 MB) | 0.10 s |
| Restauración | 0.27 s |
| Base tras 1 000 ventas | 7.0 MB, `integrity ok`, 0 violaciones FK |
| Estabilidad (3 reinicios) | arranque 1.14 / 1.11 / 1.10 s; memoria 39.1 / 39.2 / 39.1 MB |

Sin degradación entre ciclos ni crecimiento de memoria. Las guardas del guion
(8 ms de checkout medio, 100 ms de media, 3 s por consulta, 30 s de respaldo) son
topes anchos contra regresiones patológicas, no un SLA.

**No medido:** tamaño del instalador y tamaño instalado. No se generó un bundle
(`--no-bundle`); corresponde a la etapa de empaquetado.

## Etapa 4 — Automatización en CI

Se añadió `.github/workflows/native-e2e.yml`:

- `workflow_dispatch` (con selección opcional de guiones) y ejecución semanal (lunes 04:00 UTC).
- Runner `windows-latest`, toolchain fijado por `rust-toolchain.toml`, `tauri-driver`
  instalado con `cargo install --locked`, driver de WebView2 descargado según la
  versión real del runtime del runner.
- Artefactos: resumen JSON y capturas, retención de 14 días.
- Orquestador nuevo: `run-all.ps1`, que encadena la suite, ejecuta `verify_db`,
  `verify_returns_db` y `restore_multiphase_check`, y agrega `summary.json`.

**Estado: no validado.** El workflow aún no se ha ejecutado en GitHub Actions; no se
declara verde. Las dudas a resolver en la primera ejecución son la disponibilidad de
sesión interactiva en el runner hospedado y la estabilidad de `tauri-driver` allí.

## Clasificación para piloto

| Criterio | Estado |
|---|---|
| Arranque, login local y sesión | **Aprobado** |
| Catálogo, inventario, ventas, caja y arqueo | **Aprobado** |
| Compras, gastos, merma y devoluciones | **Aprobado** |
| Persistencia tras reinicio | **Aprobado** |
| Respaldo y restauración (incluida la rechazada) | **Aprobado** |
| Atomicidad ante cierre inesperado | **Aprobado** |
| Restauración ante respaldos corruptos | **Aprobado** |
| Operación sin red | **Aprobado con restricciones** (sin corte de red forzado) |
| Rendimiento con volumen de 2 000 productos / 1 000 ventas | **Aprobado** |
| Interfaz nativa en Windows | **Aprobado** (1920/1440/1366/1024/tablet) |
| Linux y Android | **No validado** (requiere ejecutar `docs/04-quality/platform-matrix.md` en cada host) |
| Instalador, firma y actualización | **No validado** (no se generó bundle) |
| Integración productiva con Platform (licencia firmada) | **Bloqueado** (custodia de clave y elegibilidad de emisión) |

## Riesgos residuales

| Riesgo | Impacto | Nota |
|---|---|---|
| Aislamiento de red no forzado | Medio | La app no abre sockets; falta la prueba con firewall activo |
| Suite nativa fuera de CI | Medio | El workflow existe pero no se ha ejecutado |
| RUSTSEC-2026-0459 en `libsqlite3-sys` | Medio | Excepción justificada; revisar antes del piloto comercial |
| Restauración desde la UI | Bajo | El camino validado es la utilidad offline |
| Linux/Android sin validar | Medio | Requiere hosts dedicados |
| Instrumentación de memoria en CI | Bajo | Medición local; en runner puede variar |

## Integración con HiQNet Platform

Contratos existentes y estado:

| Aspecto | Estado |
|---|---|
| Identidad de organización e instalación | Definida; la instalación guarda `installation_id`, `organization_id`, plan y estado de licencia |
| Enrolamiento | Protocolo Ed25519 implementado y probado en Platform; el POS lo consume, no lo emite |
| Licencia offline (`LicenseGrant` v1) | Verificación local implementada; **emisión productiva bloqueada** por custodia de clave |
| Elegibilidad comercial y entitlements | Resueltos en Platform; el POS aplica el resultado localmente |
| Renovación y expiración | El trial y el vencimiento degradan a sólo lectura; la renovación depende de Platform |
| Recuperación | Respaldo/restauración validados; la reinstalación con re-enrolamiento no se probó |
| Independencia sin internet | **Aprobada en local**; sin red el POS opera con la licencia vigente |

Pendiente para producción: custodia de la clave de firma, elegibilidad de emisión,
auditoría de emisión y un procedimiento de renovación sin conexión.
