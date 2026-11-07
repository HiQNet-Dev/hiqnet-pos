# Internal Test Build 0.1.0 — evidencia Windows

Build interno de pruebas. **No es una release comercial.**

## Artefactos

| Artefacto | Versión | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `HiQNet POS_0.0.9_x64-setup.exe` (referencia) | 0.0.9 | 3 848 960 | `53B35548E76FF9A1CF3E395C46B43A2FEFEE16C6F960D9B7E419F78C35F744AC` |
| `HiQNet POS_0.1.0_x64-setup.exe` (candidato) | 0.1.0 | 3 856 173 | `DC623307954A8D8BB36D57BA6BB5E36AD508B35E96FA09DEFC688D89B2C30DC6` |

- Commit del binario: `5262880` (`chore/integration-android-ux`).
- `pos-desktop.exe` instalado: 13 878 272 bytes, SHA-256 `DC49BFA7F685F6A3F84C367775D00209634AA8FAB4450274FF423AD8028BB279`.
- Directorio instalado (`%LOCALAPPDATA%\HiQNet POS`): 13 957 515 bytes (ejecutable + desinstalador).
- Perfil Cargo: `lto = "thin"`, `codegen-units = 1`, `strip = true`, `opt-level = "s"`, `+crt-static` en `x86_64-pc-windows-msvc`.
- Sin firma de editor. Windows puede advertir con SmartScreen; no se eludió ninguna protección.

La diferencia entre 0.0.9 y 0.1.0 es **solo** el número de versión: el objetivo era
validar el mecanismo de actualización de NSIS y la conservación de datos, no una
migración de esquema (`_sqlx_migrations` idénticas antes y después).

## Instalación limpia, offline y reinstalación (Windows Sandbox)

Dos sesiones independientes en VM desechable con imagen limpia de Windows 11
(`10.0.26100`, cuenta `WDAGUtilityAccount`). **PASS** en ambas, con el mismo
conjunto de comprobaciones.

| Comprobación | Estado |
| --- | --- |
| Instalación silenciosa del candidato | PASS |
| Binario instalado y entrada de desinstalación (`DisplayVersion 0.1.0`) | PASS |
| WebView2 preinstalado por el bootstrapper oficial | PASS |
| Primer arranque crea `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` | PASS |
| Base estructuralmente válida (`--verify-backup`) | PASS |
| Arranque con los adaptadores de red desactivados dentro de la VM | PASS |
| Base operativa (dataset real: venta, compras, gastos, merma) sin red | PASS |
| `--restore-backup` sin red | PASS |
| Desinstalación elimina el programa y **conserva** la base | PASS |
| Reinstalación repone el binario y la aplicación vuelve a arrancar | PASS |
| Base intacta tras reinstalar | PASS |
| Login y venta por interfaz dentro de la VM | NO EJECUTADO: el script no automatiza formularios; quedan capturas para revisión manual |

### Defecto corregido en el arnés

El informe dejó de generarse después de registrar la reinstalación. Causa
demostrada en `install.log`: el transcript terminaba en `[PASS] reinstalacion`
sin llegar a `Stop-Transcript` ni al JSON.

1. El desinstalador de NSIS delega en una copia temporal (`Un_A.exe`) que termina
   **después** de que el proceso original devuelva el control. La reinstalación
   inmediata competía con el borrado del binario recién instalado.
2. `Start-App` hacía `Start-Process` sin comprobar la ruta; con el binario
   ausente, el error terminaba el guion antes de escribir el informe.

Correcciones: espera acotada a que el desinstalador se asiente y a que el binario
reaparezca, cada etapa aislada con su propio `try/catch` (una excepción queda
como `ERROR` y el guion continúa), informe JSON reescrito de forma atómica tras
**cada** comprobación y sin BOM, `progress.log` como latido de la etapa en curso,
y restauración de la red al cerrar para poder revisar la VM.

## Actualización real entre instaladores

Ejecutada en el anfitrión sobre la base aislada `update-test/dataset-A.db`
(nunca la base de trabajo del usuario).

1. Instalación de `0.0.9`; `DisplayVersion = 0.0.9`.
2. E2E completo por la interfaz sobre el **ejecutable instalado**: onboarding,
   categoría, producto, variante, modificadores, inventario, caja, turno, venta,
   reinicio y login, compras y recepción, gastos, merma, arqueo y cierre, RBAC.
3. Verificación de la base: `failures: []`, 1 venta, stock 22 000 milésimas,
   arqueo exacto (diferencia 0).
4. Instalación de `0.1.0` **encima**; `DisplayVersion = 0.1.0`.
5. Verificación de la misma base: **resultado idéntico** (`failures: []`, mismo
   folio `V-40F7ABB431`, mismo stock, mismas migraciones).
6. Continuidad comercial con el binario nuevo (`e2e_post_upgrade.py`): abre la
   base anterior, muestra el acceso, autentica al propietario y conserva catálogo,
   existencias (22), historial de turnos cerrados, gastos (2), merma (1),
   compras (1) y usuarios (2); abre turno y registra la venta `V-AFEF86FF06` con
   saldo de caja 164.96 → 197.44 (+32.48 exacto).
7. SQLite: ventas confirmadas 3 → 4 (exactamente una nueva), un único ingreso de
   caja por esa venta, una auditoría `sales.completed`, `integrity_check = ok`,
   `foreign_key_check` sin referencias inválidas.

## E2E contra el ejecutable instalado (candidato 0.1.0)

`e2e_flow.py` sobre `%LOCALAPPDATA%\HiQNet POS\pos-desktop.exe` (el que instala
NSIS), con base aislada y automatización WebDriver de Tauri.

- Pasos: `bootstrap, categoria, producto, variante, modificadores, inventario,
  caja_abierta, venta, reinicio_login, persistencia, compras, gastos_merma,
  compras_gastos_reinicio, arqueo_cierre, usuarios, resoluciones` → **PASS**.
- Capturas en 1920×1080, 1440×900, 1366×768, 1024×768 y tablet (horizontal,
  vertical y ticket contraído).
- `verify_db.py`: `result: PASS`, `failures: []`, arqueo exacto, stock 22 000.

## Sin efectos sobre los datos del operador

SHA-256 de la base de trabajo real tomado antes y después de todo el
procedimiento (instalación, actualización, E2E y sandbox):

```text
d1bf5a271907c6d01f57f7bfb20369a24af6aa22b84d4aef52fac89e9584baae  hiqnet-pos.db
37fbbfb36ec7a05f3eb0f74b9dc04ec9c03b6539ce08acff78bbfde75cad9897  hiqnet-pos.db-wal
```

Idéntico en ambos muestreos. El arnés rechaza operar sobre
`%LOCALAPPDATA%\HiQNet\POS\data`.

## Rendimiento en el ejecutable instalado

`perf_smoke.py` sobre `%LOCALAPPDATA%\HiQNet POS\pos-desktop.exe` (release, x64),
catálogo pequeño y 200 cobros por IPC:

| Métrica | Valor |
| --- | --- |
| Arranque hasta onboarding | 0.966 s |
| Reinicio hasta login | 1.761 s |
| 200 checkouts (total) | 1.498 s |
| Checkout medio | 7.49 ms |
| `list_products` | 6.65 ms |
| `list_stock_levels` | 6.46 ms |
| `reports_summary` | 9.20 ms |
| `list_sale_returns` | 6.79 ms |

Medición local de referencia, no un SLA. **No se repitió el escenario de 10 000
productos en esta pasada**; las cifras anteriores de esa carga siguen siendo la
referencia hasta repetirlas sobre este SHA.

## Pendiente

- Login y venta **manualmente** en la sesión de sandbox (las capturas existen).
- Actualización de un esquema con migraciones nuevas: todavía no existe una
  migración posterior a `sale returns`, así que ese caso no se probó.
- Firma de editor para evitar advertencias de SmartScreen.

## Artefactos tras el rediseño de precios y ventas (2026-11-06)

El frontend y el contrato de catálogo cambiaron (`docs/04-quality/price-and-sales-ux-0.1.0.md`),
así que el instalador se regenera desde `f5744d9`. El de `1821d2d` (`B34B30F5…`)
queda como historial.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `HiQNet POS_0.1.0_x64-setup.exe` (NSIS, sin firma de editor) | 3 873 492 | `B871A54873B3F725916E7B6564FD2D86F90A84AF277D0C36B8768E26C9DAA479` |
| `pos-desktop.exe` dentro del paquete | 13 940 736 | `EE37B2126A324082EAB23BA87B2F2E6E84A9C8B759B0A3EDF29F4E8942DC8E84` |

Ubicación: `~/HiQNet-Internal-Test-pricing/windows/instaladores/`.

La verificación E2E nativa (`e2e_layout_drift`, `e2e_flow`, `e2e_returns`) se
ejecutó sobre el **binario release** del mismo árbol, con assets
`index-Bx8ah-xo.js` e `index-CiRAZ_O3.css`, idénticos a los del instalador.
`e2e_flow` comprueba que la tarjeta publica el precio con IVA antes de vender.
No se instaló el NSIS en Windows Sandbox en esta pasada: la evidencia de
instalación y actualización sigue siendo la de `5262880`, cuyo mecanismo no
cambió.

## Artefactos tras la auditoría UX (2026-11-05)

La auditoría UX/UI (`docs/04-quality/ux-audit-0.1.0.md`) cambió el frontend. El
instalador de la tabla inicial corresponde a `5262880` y el de `21bde50`
(`864B2076…`) queda como historial. Artefacto vigente:

- Commit: `1821d2d` (`chore/integration-android-ux`), que añade la corrección del
  Back de Android y la marca unificada.
- Compilado con `cargo tauri build` (perfil release, NSIS).

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `HiQNet POS_0.1.0_x64-setup.exe` (NSIS, sin firma de editor) | 3 854 105 | `B34B30F5C977FE22C1B4F4B22DC849FEE1FC5F0B6895A902B671E85D12F9BFEB` |
| `pos-desktop.exe` dentro del paquete | 13 882 368 | `C8F4AC328CC0AEE1B8FB4F981CB8B20420A0DAE0940835A9F18FB04AC090197` |

Ubicación de la entrega: `~/HiQNet-Internal-Test-backfix2/windows/instaladores/`.

La verificación E2E nativa de esta pasada (`e2e_layout_drift`, `e2e_flow`,
`e2e_returns`) se ejecutó sobre el **binario release** del mismo commit, con
assets `index-BsmKnnR5.js` e `index-9D_khkjU.css`, idénticos a los embebidos en el
instalador. No se instaló el NSIS en Windows Sandbox en esta pasada: la evidencia
de instalación, desinstalación y actualización sigue siendo la de `5262880`, cuyo
mecanismo no cambió.
