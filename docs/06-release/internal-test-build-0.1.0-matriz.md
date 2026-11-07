# Matriz de aceptación — Internal Test Build 0.1.0

Estados admitidos: **PASS**, **FAIL**, **BLOCKED**, **NO EJECUTADO**, **NO APLICA**.
Ningún PASS se apoya en inferencias: cada fila tiene evidencia ejecutada.

## Windows

| ID | Requisito | Estado | Evidencia |
| --- | --- | --- | --- |
| W-01 | Instalador NSIS generado | PASS | `windows/instaladores/HiQNet POS_0.1.0_x64-setup.exe`, 3 856 173 B, SHA `DC62…DC6` |
| W-02 | Instalación limpia en imagen sin dependencias | PASS | sandbox-run2 `instalacion`, `binario_instalado`, `webview2_tras_instalar` |
| W-03 | Arranque sin dependencias faltantes (`+crt-static`) | PASS | sandbox-run2 `arranque_primer_arranque` |
| W-04 | Primera ejecución crea SQLite | PASS | `base_tras_primer_arranque` |
| W-05 | Onboarding real | PASS | `e2e_flow` `bootstrap` (ejecutable instalado) |
| W-06 | Login real | PASS | `e2e_flow` `reinicio_login` |
| W-07 | Venta real desde la interfaz instalada | PASS | `fase3-instalado/dataset.db`, folio `V-A913F32703` |
| W-08 | Persistencia SQLite tras reinicio | PASS | pasos `reinicio_login` y `persistencia` |
| W-09 | Inventario consistente | PASS | `verify_db.py` → `stock_milli 22000` |
| W-10 | Caja y turno consistentes | PASS | `verify_db.py` → esperado/contado 12 048, diferencia 0 |
| W-11 | Devolución válida | PASS | `e2e_returns.py` + `verify_returns_db.py` (`failures: []`) |
| W-12 | Compras, gastos y merma | PASS | `verify_db.py` → `recepciones 2`, `gastos 2`, `merma 1` |
| W-13 | RBAC en interfaz y Rust | PASS | `e2e_flow` `usuarios`: catálogo/gestión/compras denegados al cajero |
| W-14 | Respaldo y restauración | PASS | `e2e_restore_guard.py`: respaldo, rechazo con base en uso, restauración y reapertura |
| W-15 | Operación sin red | PASS | sandbox-run2 `red_desconectada`, `arranque_sin_red`, `base_sin_red`, `restauracion_sin_red` |
| W-16 | Desinstalación conserva datos | PASS | `desinstalacion`, `base_tras_desinstalar` |
| W-17 | Reinstalación con datos intactos | PASS | `reinstalacion`, `arranque_arranque_reinstalado`, `base_tras_reinstalar` |
| W-18 | Actualización entre dos versiones | PASS | `verificacion-antes/tras-actualizar.json`, `e2e_post_upgrade.py` |
| W-19 | Continuidad comercial tras actualizar | PASS | venta `V-AFEF86FF06`, caja 164.96 → 197.44 |
| W-20 | Integridad SQLite tras actualizar | PASS | `integrity_check = ok`, `foreign_key_check` vacío |
| W-21 | E2E del ejecutable instalado | PASS | `evidence/windows/fase3-instalado/` |
| W-22 | Regresión visual Windows (1024…1920 y tablet) | PASS | 14 capturas de `e2e_flow` |
| W-23 | Ausencia de P0 | PASS | sin P0 abiertos |
| W-24 | Arranque en frío y caliente medidos | PASS | `am start -W` / `perf_smoke.py` (ver informes) |
| W-25 | Firmado de editor | NO EJECUTADO | sin certificado; SmartScreen puede advertir |
| W-26 | Login y venta manuales dentro del sandbox | NO EJECUTADO | el arnés no automatiza formularios |
| W-27 | Catálogo de 10 000 productos sobre este SHA | NO EJECUTADO | no repetido en esta pasada |

## Android

| ID | Requisito | Estado | Evidencia |
| --- | --- | --- | --- |
| A-01 | APK ARM64 generada desde el SHA actual | PASS | `hiqnet-pos-0.1.0-internal-arm64.apk`, 11 917 504 B, SHA `E459…94A` |
| A-02 | Firma de pruebas con clave fuera de Git | PASS | `apksigner verify`: v2 + v3, certificado `CN=HiQNet Internal Test` |
| A-03 | Instalación en emulador | PASS | AVD `x86_64` con APK `x86_64` (el emulador no ejecuta `arm64-v8a`) |
| A-04 | Arranque sin Vite | PASS | assets embebidos en `libpos_desktop_lib.so`; onboarding renderizado |
| A-05 | SQLite privada y migraciones | PASS | `/data/user/0/com.hiqnet.pos/data/`, 10/10 migraciones, `integrity_check ok` |
| A-06 | Onboarding, login, venta y persistencia | NO EJECUTADO | requiere interacción táctil |
| A-07 | Operación offline | PASS (arranque) | wifi y datos desactivados + `force-stop` + reapertura |
| A-08 | Respaldo/restauración SAF | **BLOCKED** | no implementado: el respaldo queda en almacenamiento privado |
| A-09 | Ciclo de vida (segundo plano, cierre forzado) | NO EJECUTADO | solo se probó `force-stop` + reapertura |
| A-10 | Tablet ARM64 física | **BLOCKED** | no hay hardware disponible |

La falta de tablet física impide declarar `ANDROID HARDWARE VALIDADO`. La ausencia
de SAF impide considerar Android apto para pruebas prolongadas con recuperación
externa demostrada.
