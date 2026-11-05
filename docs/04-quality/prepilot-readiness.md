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
| Operación offline | Implementado; **no ejecutado con red desconectada** | Sin llamadas de red en los flujos; falta la prueba física en VM |
| Instalador NSIS | Construido; **instalación no ejecutada** | 4.89 MiB, `NotSigned`, `SHA-256 228AFD0F…3EBC`, modo `currentUser`; sin entorno aislado habilitado |
| Ejecutable de release (contenido del instalador) | Verificado | E2E comercial, devoluciones, guardia de restauración y multifase ejecutados contra `target/release/pos-desktop.exe` |
| Actualización sobre versión anterior | **No ejecutado** | Requiere VM limpia y build previo |
| Desinstalación/reinstalación conservando datos | **No ejecutado** | Requiere VM limpia |
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
| Windows Sandbox | `WindowsSandbox.exe` **ausente** (característica deshabilitada) | **Bloqueado** |
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

## Validación de esta ronda (PR #17)

Entorno: Windows 11 Pro 24H2 (`10.0.26300`), x64, 12 hilos. Hyper-V presente
pero **sin acceso a máquinas virtuales** desde la sesión (el comando `Get-VM`
requiere elevación y no hay VM creada). Docker Desktop solo ejecuta contenedores
Linux, insuficientes para una aplicación Tauri con WebView2.

| Escenario | Estado | Evidencia |
| --- | --- | --- |
| Intervalo entre checkpoint y promoción | **PASS** | `otra_conexion_no_puede_escribir_entre_checkpoint_y_promocion`: el escritor externo es rechazado y la base queda en el estado del respaldo |
| Escritura externa en la ventana de promoción | **PASS** | `escritura_externa_en_la_ventana_aborta_sin_perder_datos`: rechazo `DatabaseInUse` y los datos confirmados por el tercero sobreviven |
| Instalación inicial en VM limpia | **NO EJECUTADO** | Sin VM accesible |
| SmartScreen con instalador sin firma | **NO EJECUTADO** | Sin VM accesible; el artefacto es `NotSigned` |
| Dependencia de WebView2 | **VERIFICADO ESTÁTICAMENTE** | `INSTALLWEBVIEW2MODE=downloadBootstrapper` en `installer.nsi` |
| Apertura de la aplicación instalada | **NO EJECUTADO** | Sin instalación |
| Ejecutable de release (mismo binario del instalador) | **PASS** | E2E comercial, devoluciones y restauración sobre `target/release/pos-desktop.exe` |
| Actualización / desinstalación / reinstalación | **NO EJECUTADO** | Sin VM accesible |
| Operación sin red | **NO EJECUTADO** | No se modifica la red del equipo anfitrión |
| Terminación abrupta del proceso | **PASS (parcial)** | Cada E2E termina con `driver.quit` sobre WAL pendiente y la base se recupera; no es una prueba eléctrica |
| Entorno aislado (Windows Sandbox) | **NO EJECUTADO** | Característica deshabilitada; requiere administrador y reinicio |
| Arnés de sandbox (artefactos, `.wsb`, script) | **PREPARADO Y VALIDADO** | XML válido, montaje de artefactos `ReadOnly=true`, SHA-256 registrado; sin ejecución |

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

`PREPILOTO BLOQUEADO`: la instalación, actualización y operación sin red no se
han ejecutado porque no existe un entorno aislado habilitado en el equipo de
desarrollo. El arnés está listo y el único paso pendiente es una acción de
administrador (habilitar Windows Sandbox y reiniciar). La aptitud técnica del
binario está probada; eso no sustituye la validación de instalación ni autoriza
distribuir el producto.

## Riesgos abiertos

- Instalador sin firma: SmartScreen mostrará advertencias.
- WebView2 puede requerir descarga durante la instalación si falta el runtime.
- Sin instalación probada en equipo limpio.
- Sin prueba de operación con red desconectada ni de corte de energía.
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
