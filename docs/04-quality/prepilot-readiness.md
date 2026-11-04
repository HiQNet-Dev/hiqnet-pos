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
| Rechazo de doble instancia | Implementado y probado | La app toma `hiqnet-pos.db.lock` al arrancar y avisa si ya está abierta |
| Migraciones y actualización de esquema | Implementado y probado | Actualización 0003→0010, 0007→0010 y 0009→0010 con datos |
| Rechazo de esquema posterior | Implementado y probado | `open` y `verify_backup` devuelven `IncompatibleSchema` |
| Detección de base corrupta | Implementado y probado | `PRAGMA quick_check` al abrir |
| Operación offline | Implementado; validación por diseño | No hay llamadas de red en los flujos; E2E sin dependencias externas |
| Instalador NSIS | Implementado; **validación incompleta** | Artefacto construido y sin firma; **no se instaló en VM limpia** |
| Actualización sobre versión anterior | **No ejecutado** | Requiere VM limpia y build previo |
| Desinstalación/reinstalación conservando datos | **No ejecutado** | Requiere VM limpia |
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

## Evidencia ejecutada en esta rama

- `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `cargo audit`.
- Frontend: lint, typecheck, Vitest, build.
- `tauri build --debug --no-bundle` y bundle NSIS interno.
- E2E nativo comercial completo + `verify_db.py`.
- E2E nativo de devoluciones/reportes + `verify_returns_db.py`.
- E2E nativo de guardia de restauración (`e2e_restore_guard.py`).
- Conservación de datos de todas las fases al restaurar
  (`restore_multiphase_check.py`), incluido el deshacer de un cambio posterior.
- `perf_smoke.py` con 200 ventas.
- Utilidad offline: verificación, restauración y rechazo de archivo corrupto.

## Criterios para declarar el piloto apto

1. Instalación/desinstalación/actualización verificadas en VM Windows limpia.
2. Respaldo y restauración ejecutados con datos sintéticos de varias fases.
3. Operación offline comprobada con adaptador de red deshabilitado.
4. Defectos P0/P1 cerrados y CI de `main` en verde.
5. Decisión documentada sobre firma y actualizador (aunque sea "deshabilitado").
6. Procedimiento de soporte y recuperación entregado al operador.

## Riesgos abiertos

- Sin firma de código: los avisos de SmartScreen pueden confundir al operador.
- Sin instalación probada en equipo limpio; WebView2 puede faltar en Windows 10
  sin actualizaciones.
- Emisión de licencias productiva aún no existe; el Trial es el modo verificable.
- Sin prueba de corte de energía ni de jornada prolongada.
- El bloqueo de la base es cooperativo: otra herramienta que ignore
  `hiqnet-pos.db.lock` podría reemplazar el archivo. El procedimiento soportado
  es cerrar HiQNet POS y usar la utilidad incluida.
- El respaldo no sustituye a una copia externa: si el disco falla, respaldo y
  base se pierden juntos.
