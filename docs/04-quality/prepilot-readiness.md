# Preparación prepiloto — HiQNet POS 0.1.0

Estado a la fecha de la rama `chore/local-stabilization-prepilot`. La evidencia
proviene de pruebas automatizadas, ejecuciones sobre el ejecutable real y
mediciones locales; lo no ejecutado se declara explícitamente.

## Matriz de preparación

| Capacidad | Estado | Evidencia / límite |
| --- | --- | --- |
| Dominio exacto (`Money`, `Quantity`) | Implementado y probado | 34 pruebas `pos-core`, incluye redondeo acumulativo de devoluciones |
| Bootstrap, identidad y Argon2id | Implementado y probado | Pruebas SQLite + E2E nativo |
| RBAC Owner/Cashier y aislamiento | Implementado y probado | Pruebas y E2E; cajero denegado por IPC |
| Catálogo, variantes y modificadores | Implementado y probado | Inventario por variante **fuera de alcance** |
| Inventario y ledger append-only | Implementado y probado | Triggers + pruebas de concurrencia |
| Caja, turnos y arqueo | Implementado y probado | E2E nativo; justificación obligatoria de diferencias pendiente |
| Ventas en efectivo y checkout | Implementado y probado | E2E nativo con folio, cambio e inventario |
| Compras y recepciones | Implementado y probado | E2E nativo con recepción 2 + 3 |
| Gastos y merma | Implementado y probado | E2E nativo y pruebas de concurrencia |
| Devoluciones | Implementado y probado | E2E nativo; solo efectivo, sin notas de crédito |
| Reportes operativos | Implementado y probado | Agregados SQLite; sin utilidad ni costo de ventas |
| Respaldo verificado | Implementado y probado | `backup_safety.rs`; el respaldo se verifica y se descarta si falla |
| Restauración offline | Implementado y probado | Utilidad `--restore-backup` con copia de recuperación y bloqueo exclusivo de proceso |
| Restauración con la app abierta | Implementado y probado | `e2e_restore_guard.py` sobre el ejecutable: código 2 y base intacta |
| Restauración con conexión SQLite externa | Implementado y probado | `wal_checkpoint(TRUNCATE)` detecta lectores en WAL; probado por unidad y con lector python real |
| Fallos de reemplazo inyectados | Implementado y probado | 9 escenarios en `backup.rs`: copia, validación, promoción, verificación final y reversión |
| Intervalo checkpoint → promoción | Implementado y probado | Bloqueo exclusivo sostenido durante copia y validación; dos pruebas con barrera y conexión externa (rechazo y aborto sin pérdida de datos) |
| Rechazo de doble instancia | Implementado y probado | La app toma `hiqnet-pos.db.lock` al arrancar y avisa si ya está abierta |
| Migraciones y actualización de esquema | Implementado y probado | Actualización 0003→0010, 0007→0010 y 0009→0010 con datos |
| Rechazo de esquema posterior | Implementado y probado | `open` y `verify_backup` devuelven `IncompatibleSchema` |
| Detección de base corrupta | Implementado y probado | `PRAGMA quick_check` al abrir |
| Operación offline | **Verificado en VM sin red** | Adaptadores desactivados en Windows Sandbox; venta en base sintética abierta offline |
| Instalador NSIS | **Instalado en VM limpia** | 4.95 MiB, `NotSigned`, `SHA-256 EA6A90CD…CE01`, modo `currentUser`; PASS en Windows Sandbox |
| Ejecutable de release (contenido del instalador) | Verificado | E2E comercial, devoluciones, guardia de restauración y multifase ejecutados contra `target/release/pos-desktop.exe` |
| Funcionamiento sin redistribuible VC++ | **Corregido y verificado** | Enlace estático del runtime (`+crt-static`); sin `VCRUNTIME140.dll` |
| Actualización sobre versión anterior | **No ejecutado** | Requiere un instalador anterior publicado |
| Desinstalación/reinstalación conservando datos | **Verificado en VM limpia** | Programa eliminado, base intacta y reapertura correcta |
| Instalación sin Internet (WebView2) | **Dependiente de WebView2 preinstalado** | El paquete usa `downloadBootstrapper`: si falta el runtime, descarga |
| Actualizador Tauri | **Deshabilitado** | Sin endpoints, claves ni política de firma |
| Emisión de licencias | Pendiente | Responsabilidad de HiQNet Platform |
| Firma del instalador | **Bloqueado por decisión comercial** | D-09 sin resolver |
| CFDI / facturación | Fuera del alcance prepiloto | Roadmap fiscal posteriores |
| Sincronización Pro / Control Plane | Fuera del alcance prepiloto | Fase G6 |
| Pagos electrónicos / Stripe | Fuera del alcance prepiloto | D-07 sin decidir |
| Multisucursal online | Fuera del alcance prepiloto | Solo sucursal local |
| E2E nativo en CI | **Bloqueado técnicamente** | Runners sin garantía gráfica/WebDriver |
| Rendimiento en jornada de 8 h | **No ejecutado** | Medición puntual: arranque 1.13–1.17 s, checkout 8.66 ms, reportes 10.6 ms con 200 ventas |
| Corte de energía | **No ejecutado** | La durabilidad depende de WAL y transacciones; no se probó hardware |
| Revisión de accesibilidad completa | Parcial | Capturas 1366×768/1024×768/768×1024; sin auditoría formal de lectores de pantalla |

## Entorno aislado disponible

Preflight ejecutado sin elevación el 2026-11-04:

| Requisito | Valor | Estado |
| --- | --- | --- |
| Edición | Windows 11 Pro `10.0.26300` x64 | Apto |
| Hipervisor | presente; servicios `vmms` y `vmcompute` en ejecución | Apto |
| Memoria | 31.9 GB | Apto |
| Disco libre | 324 GB | Apto |
| Windows Sandbox | habilitado con autorización del titular y reinicio manual | **Apto** |
| `Get-VM` (Hyper-V) | permiso denegado: la sesión no pertenece a *Hyper-V Administrators* | **Bloqueado** |
| Sesión elevada | no | — |

La instalación real no se ejecutó porque **no existe un entorno aislado
habilitado**. El arnés está preparado y validado en
[`tools/prepilot-sandbox/`](../../tools/prepilot-sandbox/README.md): monta solo
una carpeta de artefactos en modo **solo lectura**, deja una carpeta de
evidencias aparte y ejecuta `sandbox-verify.ps1` al iniciar la VM.

### Habilitación (requiere aprobación del titular)

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName "Containers-DisposableClientVM" -All -NoRestart
shutdown /r /t 0
```

Efecto: habilita *Windows Sandbox* (usa Hyper-V), añade `WindowsSandbox.exe` y
requiere reiniciar. No instala ni desinstala aplicaciones del anfitrión y no
modifica sus adaptadores de red. Alternativa con persistencia entre reinicios:
`Add-LocalGroupMember -Group "Hyper-V Administrators" -Member "$env:USERNAME"`,
que además exige una ISO y licencia de Windows.

## Validación en Windows Sandbox (ejecutada)

Sesión real sobre Windows Sandbox (Windows 11 Enterprise `10.0.26100`, x64), con
el instalador `EA6A90CD…CE01` montado en solo lectura y evidencias exportadas a
una carpeta aparte. Resultado global del arnés: **PASS**.

| Escenario | Estado | Detalle |
| --- | --- | --- |
| Instalación limpia (`/S`) | **PASS** | Código 0; `%LOCALAPPDATA%\HiQNet POS\pos-desktop.exe` y entrada de desinstalación `0.1.0` |
| WebView2 en imagen limpia | **INFO** | Windows Sandbox no trae el runtime; se instaló el bootstrapper oficial (`156.0.4314.58`) |
| Primer arranque | **PASS** | Proceso vivo, UI de onboarding, base creada y válida |
| Persistencia SQLite | **PASS** | `--verify-backup` sobre copia con WAL: `resultado=OK` |
| Operación sin red | **PASS** | Adaptadores de la VM desactivados (`activos=0`); arranque y base válidos |
| Datos operativos sin red | **PASS** | Base sintética con venta, caja, compras, gastos, merma y auditoría abierta offline; UI de acceso mostrada |
| Verificación tras operar | **PASS** | Integridad y esquema válidos (SHA-256 del archivo registrado) |
| Restauración sin red | **PASS** | `--restore-backup` código 0 sobre la base instalada |
| Desinstalación | **PASS** | Programa eliminado; la base **se conserva** |
| Reinstalación | **PASS** | Base intacta y aplicación arranca |
| Inicio de sesión y venta por UI | **PENDIENTE** | Requiere interacción; la automatización de teclado no es fiable en Sandbox |
| Actualización desde versión anterior | **NO EJECUTADO** | No existe un instalador anterior publicado |
| SmartScreen interactivo | **NO EJECUTADO** | Instalación silenciosa; la firma sigue pendiente (D-09) |

### Defecto encontrado y corregido

La primera sesión en imagen limpia falló con
«La ejecución de código no puede continuar porque no se encontró
VCRUNTIME140.dll». El ejecutable dependía del redistribuible de Visual C++ y el
instalador no lo advierte. Se corrigió enlazando el runtime de forma estática
(`apps/pos-desktop/src-tauri/.cargo/config.toml`, `+crt-static`), se reconstruyó
el instalador y la segunda sesión completa pasó.

## Correcciones de esta ronda (PR #17)

| Escenario | Estado | Evidencia |
| --- | --- | --- |
| Intervalo entre checkpoint y promoción | **PASS** | Prueba con barrera: el escritor externo es rechazado y la base queda en el estado del respaldo |
| Escritura externa en la ventana de promoción | **PASS** | `DatabaseInUse` y los datos confirmados por el tercero sobreviven |
| Dependencia del redistribuible VC++ | **CORREGIDO** | Hallada en Windows Sandbox; enlace estático del runtime |
| Ejecutable de release | **PASS** | E2E comercial, devoluciones, restauración y multifase |
| Terminación abrupta del proceso | **PASS (parcial)** | Los E2E cierran sobre WAL pendiente y la base se recupera; no es una prueba eléctrica |

## Evidencia ejecutada en esta rama

- `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `cargo audit`.
- Frontend: lint, typecheck, Vitest, build.
- `tauri build --debug --no-bundle` y bundle NSIS interno.
- E2E nativo comercial completo + `verify_db.py`.
- E2E nativo de devoluciones/reportes + `verify_returns_db.py`.
- E2E nativo de guardia de restauración (`e2e_restore_guard.py`).
- Conservación de datos de todas las fases al restaurar
  (`restore_multiphase_check.py`), con hash SHA-256 por tabla, `integrity_check`,
  `foreign_key_check` y deshacer de un cambio posterior.
- Fallos de reemplazo y reversión simulados en `backup.rs` (puntos de fallo solo
  en pruebas) y fallos reales: temporal bloqueado, lector externo activo y
  residuos `-wal`/`-shm`.
- `perf_smoke.py` con 200 ventas.
- Utilidad offline: verificación, restauración y rechazo de archivo corrupto.
- Instalador NSIS `0.1.0` (4.89 MiB, sin firma) con SHA-256 registrado en
  [`docs/05-operations/windows-installation.md`](../05-operations/windows-installation.md).
- Batería completa repetida contra el **ejecutable de release**.

## Criterios para declarar el piloto apto

1. Instalación/desinstalación/actualización verificadas en VM Windows limpia.
2. Respaldo y restauración ejecutados con datos sintéticos de varias fases.
3. Operación offline comprobada con adaptador de red deshabilitado.
4. Defectos P0/P1 cerrados y CI de `main` en verde.
5. Decisión documentada sobre firma y actualizador (aunque sea "deshabilitado").
6. Procedimiento de soporte y recuperación entregado al operador.

## Decisión

**`PREPILOTO TÉCNICAMENTE APTO` para una beta privada en Windows 11**, con dos
salvedades explícitas:

- la **actualización** desde una versión anterior no puede probarse hasta que
exista un instalador posterior a `0.1.0`;
- la **firma de código** sigue pendiente de decisión comercial (D-09): el
instalador es `NotSigned` y SmartScreen advertirá al operador.

Esto es aptitud técnica para pruebas privadas, **no** autorización para
distribuir públicamente ni para etiquetar una release.

## Riesgos abiertos

- Instalador sin firma: SmartScreen mostrará advertencias.
- WebView2 puede requerir descarga durante la instalación si falta el runtime:
  la operación offline está probada, la **instalación offline** no.
- Sin prueba de actualización entre versiones ni de Windows 10.
- Inicio de sesión y venta por UI en la VM: requieren interacción manual.
- Sin prueba de corte de energía ni de jornada prolongada.
- Emisión de licencias productiva aún no existe; el Trial es el modo verificable.
- El bloqueo de la base es cooperativo: otra herramienta que ignore
  `hiqnet-pos.db.lock` podría reemplazar el archivo. El procedimiento soportado
  es cerrar HiQNet POS y usar la utilidad incluida.
- El respaldo no sustituye a una copia externa: si el disco falla, respaldo y
  base se pierden juntos.
- En Windows no es posible reemplazar un archivo que otra conexión mantiene
  abierto. La guardia exclusiva cubre la copia y la validación; el reemplazo deja
  una ventana de milisegundos en la que una herramienta externa podría escribir.
  Esa escritura se detecta (WAL con datos) y aborta la restauración conservando
  los datos confirmados, pero no puede impedirse por completo.
