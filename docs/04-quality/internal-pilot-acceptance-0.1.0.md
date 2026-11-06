# Aceptación operativa interna — HiQNet POS Local 0.1.0

- Fecha: 2026-11-06
- Rama: `feat/android-saf-internal-backup` (PR #21)
- Base de integración: `chore/integration-android-ux` (PR #20)
- Versión probada: `0.1.0` (`com.hiqnet.pos`)
- Alcance: validación de aceptación con datos sintéticos. **No** es una release comercial.

## 1. Congelación funcional (baseline)

| Elemento | Valor |
| --- | --- |
| Commit funcional | `0fa656c59f8a7f5a9f8555dfb1c1824cc496ec47` (el informe añade solo documentación y herramienta de firma) |
| Versión | `0.1.0` |
| Migraciones | 11/11 (`max = 11`) |
| Comandos IPC documentados | 63 en `apps/pos-desktop/src-tauri/src/lib.rs` |
| `backend-rust/Cargo.lock` | `6e1ee1cb51f0c41844d5be5c519fe4138bf733deede47f9d99c5b944e66f33c6` |
| `apps/pos-desktop/src-tauri/Cargo.lock` | `1be274261ef5d6e723d552650f2a8c5c5987653bdf3b16435be5c66b5ec1068a` |
| Frontend | `index-BDaRMYmU.js` (`084801cd…`), `index-ABHdBvv2.css` (`fc0f3d5a…`) |
| Estado Git | Solo capturas y `flow-report.json` no versionados |
| CI de PR #21 | 4/4 en verde sobre el HEAD publicado |

No se añadieron funcionalidades nuevas. Los cambios de esta pasada son la rotación de la
credencial de firma, el endurecimiento del transporte SAF, pruebas y documentación.

## 2. Seguridad del keystore

- La contraseña estaba en texto plano en `LEEME.txt`, fuera del repositorio, y se consideró
  **expuesta**.
- Se creó una copia del keystore en `C:\Users\gaelg\HiQNet-Internal-Signing-Backup\` con su
  credencial anterior protegida por DPAPI (solo el usuario actual).
- Se rotaron **storepass y keypass** con valores aleatorios y **distintos**. El par de claves y
  el certificado **no cambiaron**.
- El nuevo material se guarda en
  `%LOCALAPPDATA%\HiQNet\credentials\internal-keystore.clixml` (DPAPI, ACL del usuario).
- `LEEME.txt` quedó saneado (0 ocurrencias de contraseñas).
- `tools/android/sign-internal-apk.ps1` admite credenciales DPAPI o solicitud interactiva y
  **no asume** que storepass y keypass sean iguales.
- Firma de comprobación: v2/v3 correctos y certificado
  `3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072`, idéntico al histórico.
- No se detectaron secretos en el repositorio (`tools/check-no-secrets.sh`: OK) ni en la APK
  (sin `.db`, keystore, `.env` ni respaldos).

> La contraseña anterior debe considerarse comprometida; se rotó. El archivo de respaldo conserva
> la credencial antigua cifrada por DPAPI y puede eliminarse cuando ya no se necesite.

## 3. Paquetes

| Paquete | Bytes | SHA-256 |
| --- | ---: | --- |
| NSIS Windows `HiQNet POS_0.1.0_x64-setup.exe` | 3 918 937 | `81989E2133E7821F121EBE9AAD7D8F0E53778A2885AB687D848A37159A8B5BCC` |
| APK ARM64 firmada `hiqnet-pos-0.1.0-internal-arm64.apk` | 12 122 640 | `82AD7A9163594B0FEB1E55A1C7C9503DD9112C463D16052A087DD4C7ED08E32F` |
| APK x86_64 firmada (emulador/actualización) | 12 970 509 | `04EDE9A6C21E1B4BDC600E3B55E14C49611DA851DA2BE2E5E513243DDFBF9F0B` |

- NSIS: **sin firma de editor** (`Get-AuthenticodeSignature` → `NotSigned`). Se declara.
- APK: `arm64-v8a`, `targetSdk 37`, v2/v3, `zipalign -c -p 4` correcto.
- La reconstrucción desde el árbol congelado con la nueva contraseña produjo **exactamente los
  mismos SHA-256** que la pasada anterior, lo que confirma un build reproducible.

## 4. Windows

Prueba aislada en Windows Sandbox (red activa solo para que el instalador obtenga WebView2):

| Comprobación | Resultado |
| --- | --- |
| Instalación silenciosa desde cero | PASS (`install_exit=0`) |
| WebView2 obtenido por el instalador | PASS |
| Archivos y versión instalados | PASS (`pos-desktop.exe`, `0.1.0`, desinstalador) |
| Red desactivada tras instalar | PASS |
| Arranque offline | PASS (proceso vivo, base creada) |
| Base creada | PASS (11 migraciones, `integrity_check=ok`, 0 FK) |
| Desinstalación | PASS (`uninstall_exit=0`; programa eliminado) |
| Datos tras desinstalar | PASS (base conservada) |
| Reinstalación | PASS (`reinstall_exit=0`) |
| Datos tras reinstalar | PASS (base conservada) |

No se tocó la instalación real del host (`%LOCALAPPDATA%\HiQNet POS`).

Regresión funcional: `e2e_flow.py`, `e2e_returns.py` y `e2e_layout_drift.py` → **PASS**;
`cargo fmt`, Clippy, workspace tests, 120 Vitest y build → **PASS**.

## 5. Android release

AVD desechable `hiqnet_saf_test` (Android 17, `x86_64`, 2560×1600). La APK ARM64 no se pudo
ejecutar: no hay dispositivo ARM64 físico.

| Comprobación | Resultado |
| --- | --- |
| Instalación/actualización con la misma clave | PASS |
| Arranque | PASS (COLD 534 ms) |
| Login y datos previos | PASS (`UPD-001` conservado) |
| Exportación SAF | PASS (442 368 bytes) |
| Respaldo verificado con Rust | PASS (`resultado=OK`) |
| Importación + confirmación + reinicio | PASS |
| Datos posteriores al respaldo revertidos | PASS (`SAF-POST-F6` desaparece) |
| Datos previos conservados | PASS (`SAF-F6`, `UPD-001`) |
| Trial | PASS (mismo plan y vencimiento) |
| Integridad y migraciones | PASS (11, `ok`, 0 FK) |
| Back, IME, scroll real, suspensión, offline | PASS (pasada anterior sobre la misma build) |
| Abortos nativos | PASS (0 `FORTIFY`/`Fatal signal`/`panic`) |

## 6. Jornada operativa simulada

La jornada se cubrió con los E2E nativos de Windows sobre bases sintéticas aisladas y con el E2E
release de Android. Cada ejecución usa su propia base; las cifras no se suman entre corridas.

| # | Paso | Estado | Evidencia |
| ---: | --- | --- | --- |
| 1 | Login propietario | PASS | `e2e_flow` / Android |
| 2 | Revisar existencias | PASS | `e2e_flow` (stock 18) |
| 3 | Abrir caja | PASS | `e2e_flow` |
| 4 | Turno con fondo $100 | PASS | `e2e_flow` |
| 5 | Estado operativo | PASS | saldo esperado visible |
| 6 | Venta simple | PASS | Android: $11.60 |
| 7 | Venta de varios productos/modificadores | PASS | `e2e_flow` |
| 8 | Venta con IVA | PASS | $10.00 + $1.60 = $11.60 |
| 9 | Venta con variantes | PASS | `e2e_flow` |
| 10 | Venta con modificadores | PASS | `e2e_flow` |
| 11 | Búsqueda/SKU | PARTIAL | SKU usado al crear; búsqueda cubierta en Playwright |
| 12 | Venta solo con teclado | NOT RUN | no se automatizó un cobro exclusivamente por teclado |
| 13 | Cambio en efectivo | PASS | Android: $20.00 → $8.40 |
| 14 | Stock insuficiente | NOT RUN | no se intentó una venta sin existencias |
| 15 | Folios y totales | PASS | folio `V-D4C85A24F4`, total $11.60 |
| 16 | Recepción de compra | PASS | `e2e_flow` (stock 23) |
| 17 | Aumento de existencias | PASS | 18 → 23 |
| 18 | Merma | PASS | stock 22 |
| 19 | Disminución de existencias | PASS | 23 → 22 |
| 20 | Historial de inventario | PASS | movimientos visibles |
| 21 | Gasto (externo y de caja) | PASS | $9.00 y $12.00 |
| 22 | Compra | PASS | folio `OC-…` |
| 23 | Revisar caja | PASS | saldo $120.48 |
| 24 | Crear cajero | PASS | `e2e_flow` |
| 25 | RBAC | PASS | cajero sin catálogo/compras/gestion |
| 26 | Auditoría | PASS | eventos visibles |
| 27 | Reportes | PASS | `e2e_returns` |
| 28 | Localizar venta | PASS | `e2e_returns` |
| 29 | Devolución parcial (preview) | PASS | `e2e_returns` |
| 30 | Cambiar cantidad y actualizar | PASS | pruebas de `ReturnsScreen` (preview obsoleto) |
| 31 | Confirmar devolución | PASS | reembolso $11.60 |
| 32 | Importe, caja e inventario | PASS | caja $111.60, stock 19 |
| 33 | Resumen de ventas | PASS | bruto $23.20 |
| 34 | Arqueo | PASS | `e2e_flow` |
| 35 | Diferencias | PASS | mostradas en el cierre |
| 36 | Cerrar turno | PASS | `e2e_flow` |
| 37 | Cerrar sesión | PASS | Android |
| 38 | Reiniciar aplicación | PASS | Android y Windows |
| 39 | Persistencia | PASS | venta y datos tras reinicio |

Pasos no ejecutados (12, 14) y parcial (11) se declaran como tales; no se consideran PASS.

## 7. Conciliación financiera observada

| Concepto | Valor |
| --- | --- |
| Venta simple con IVA | $10.00 neto + $1.60 IVA = $11.60 |
| Cambio con $20.00 | $8.40 |
| Venta multi-línea (`e2e_flow`) | $32.48 en caja |
| Devolución parcial | $11.60 |
| Ventas brutas tras devolución | $23.20 |
| Saldo esperado de caja tras devolución | $111.60 |
| Existencias tras devolución | 19 |
| Compras recibidas | stock final 23 |
| Merma | stock 22 |
| Gastos (externo + caja) | $9.00 + $12.00 = $21.00 |
| Saldo esperado final (`e2e_flow`) | $120.48 |

No se observaron ventas duplicadas, doble IVA ni alteraciones indebidas de caja o inventario.
La idempotencia y los reintentos están cubiertos por pruebas de dominio y E2E.

## 8. Respaldo y recuperación (Android)

- Exportación mediante SAF y verificación del documento con `--verify-backup`: **PASS**.
- Restauración de la misma instalación en el siguiente arranque, antes de abrir SQLite: **PASS**.
- El estado comercial vigente se conserva; un Trial antiguo no se revive: **PASS** (pruebas).
- Instalación u organización distinta: **rechazo**.
- Respaldo corrupto o alterado: **rechazo** y base intacta.
- El respaldo exportado **no está cifrado**. Debe tratarse como archivo sensible.
- Desinstalar la aplicación o perder el dispositivo puede impedir recuperar los datos en 0.1.0,
  aunque exista un archivo exportado: no hay migración entre dispositivos.

## 9. Incidencias UX observadas en el piloto

| ID | Severidad | Observación |
| --- | --- | --- |
| UX-01 | P2 | Con el teclado virtual abierto, el pie del diálogo («Agregar producto») queda fuera del área visible; hay que ocultar el teclado o desplazar el cuerpo. El campo enfocado sí permanece visible. |
| UX-02 | P3 | El desplegable nativo de producto en Inventario no expone sus opciones al árbol de accesibilidad de `uiautomator`; la interacción táctil funciona. |
| UX-03 | P3 | El aviso de restauración correcta solo se muestra al abrir Gestión; quien permanezca en Ventas no lo ve. |
| UX-04 | P3 | Los errores de almacenamiento usan un mensaje genérico, sin detalle accionable (observado solo con una base sintética inválida). |
| UX-05 | P3 | La creación de caja coloca Nombre y Código en la misma fila; en pantallas estrechas exige cuidado, aunque Tab funciona. |

No se identificaron incidencias **P0** ni **P1**. No se inició un rediseño.

## 10. Revisión de seguridad

| Comprobación | Resultado |
| --- | --- |
| Ventas duplicadas | PASS (idempotencia y reintentos) |
| Doble IVA | PASS ($10.00 + $1.60) |
| Devolución no altera caja/inventario de forma indebida | PASS |
| Trial no se prolonga con respaldos | PASS (pruebas) |
| Licencia no se transfiere entre instalaciones | PASS (identidad) |
| Respaldos corruptos rechazados | PASS (pruebas + dispositivo) |
| Datos privados en paquetes | PASS (sin `.db`, keystore, `.env`) |
| Respaldos exportados | **Sin cifrar**; tratarlos como sensibles |
| Secretos en Git/logs | PASS en repositorio; credencial rotada |

## 11. Cadena de PRs y propuesta de integración

Estado actual:

- **#17** `chore/local-stabilization-prepilot` → `main`: abierto, no draft, `MERGEABLE`.
- **#19** `chore/desktop-ux-polish` → `#17`: draft. **Ya contenido** en #20 (ancestro verificado).
- **#18** `chore/android-local-spike` → `#17`: draft. **No es ancestro** de #20. Su contenido de
  `startup.ts`, `startup.test.ts`, `navigation.ts` y `Cargo.toml` es **idéntico** al presente en
  #20; los conflictos aparecen solo en `index.html`, `App.tsx`, `styles.css` y
  `tools/android/build-apk.ps1`, que #20 evolucionó después.
- **#20** `chore/integration-android-ux` → `#19`: draft, `MERGEABLE`, CI verde.
- **#21** `feat/android-saf-internal-backup` → `#20`: draft, `MERGEABLE`, CI verde.

Propuesta (sin ejecutar merges):

1. Revisar y fusionar **#17** en `main` (base de todo).
2. **#19** puede cerrarse como superado: #20 ya lo contiene.
3. **#18** puede cerrarse como superado: su contenido está en #20 y fusionarlo reintroduciría
   conflictos en archivos que #20 ya evolucionó. Requiere autorización.
4. Fusionar **#20** en `main` (o retarget tras #17).
5. Fusionar **#21** después de #20.
6. No cerrar #18/#19 sin autorización explícita.

## 12. Decisión

- **APTO PARA PRUEBAS INTERNAS CON DATOS SINTÉTICOS.** Windows (instalación aislada, arranque
  offline, persistencia) y Android release x86_64 (operación, SAF, actualización) están
  verificados; no hay incidencias P0/P1.
- **PENDIENTE DE VALIDACIÓN EN TABLET ARM64 FÍSICA.** La APK ARM64 está compilada, firmada y
  verificada, pero no se instaló ni ejecutó en hardware real.
- **NO APTO TODAVÍA PARA DISTRIBUCIÓN COMERCIAL.** Faltan firma de editor Windows, validación en
  hardware ARM64, política de recuperación entre dispositivos y, si se requiere, cifrado de
  respaldos.
