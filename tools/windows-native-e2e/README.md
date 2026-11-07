# E2E nativo de Windows

Pruebas ejecutadas **sobre el ejecutable real de HiQNet POS** mediante
`tauri-driver` + Microsoft Edge WebDriver (WebView2). No usan el puente Tauri
simulado de Playwright (`frontend/e2e`), por lo que constituyen la única
verificación de que la interfaz cableada al backend Rust opera de extremo a
extremo.

## Qué se ejecuta

| Guion | Alcance |
| --- | --- |
| `e2e_contracts.py` | Comandos IPC con argumentos compuestos (snake_case) y aritmética del motor de precios |
| `e2e_flow.py` | Flujo comercial completo sobre el shell: bootstrap → catálogo → variante → modificadores → inventario → caja → venta → reinicio → compras → gastos/merma → arqueo → empleados, más capturas en las resoluciones de referencia |
| `verify_db.py` | Verificación de solo lectura de la base del flujo comercial (venta, pago, stock, caja, compras, gastos, merma, auditoría e integridad) |
| `e2e_returns.py` | Devolución parcial operada por la interfaz, reportes por la interfaz, reinicio, persistencia y denegación al cajero |
| `verify_returns_db.py` | Verificación de la devolución: partida `restock`, `inbound`, salida de caja única, stock y auditoría |
| `e2e_restore_guard.py` | Respaldo desde la aplicación, rechazo de restauración con la aplicación abierta y con un lector SQLite externo, restauración correcta tras cerrarla y reapertura con los datos intactos |
| `restore_multiphase_check.py` | Sobre una base ya generada: snapshot `VACUUM INTO`, restauración con la utilidad real, comparación por hash SHA-256 de 27 tablas, `integrity_check`, `foreign_key_check` y deshacer de un cambio posterior |
| `perf_smoke.py` | Medición local: arranque, 200 checkouts por IPC, consultas y reportes |
| `e2e_smoke.py` | Comprobación mínima de que el driver puede abrir la aplicación |
| `e2e_layout_drift.py` | Estrés de la interfaz real: navegación repetida, diálogos, Tab, desplazamiento interno, movimiento y redimensionado de la ventana, ciclos maximizar/restaurar y rueda con Ctrl. Falla si la raíz del documento se desplaza, si `.app` deja de cubrir el viewport o si los contenedores del shell dejan de recortar con `overflow: clip` |
| `e2e_zoom_guard.py` | Verifica que los atajos de zoom no alteran el viewport ni el factor de escala del WebView |

`run-e2e.ps1` orquesta todo: crea un directorio temporal único, arranca
`tauri-driver` con una base SQLite aislada, ejecuta el guion y limpia.

## Requisitos

- Windows con WebView2 instalado.
- Rust y `tauri-driver`:

  ```powershell
  cargo install tauri-driver --locked
  ```

- Microsoft Edge WebDriver **de la misma versión** que el runtime de WebView2:

  ```powershell
  Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' |
      Select-Object pv
  ```

  Descarga `https://msedgedriver.microsoft.com/<version>/edgedriver_win64.zip` y
  deja `msedgedriver.exe` en `%TEMP%\edgedriver\`.

- Python con Selenium:

  ```powershell
  python -m pip install selenium
  ```

## Compilación del ejecutable

El binario debe incluir los recursos del frontend. Compilar con `cargo build` a
secas genera un binario que apunta a `devUrl` y el IPC falla con
`Origin header is not a valid URL`. Usa siempre:

```powershell
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2.12.4 build --debug --no-bundle
```

## Ejecución

```powershell
# Flujo comercial completo
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_flow.py -KeepArtifacts

# Devoluciones y reportes
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_returns.py -KeepArtifacts

# Guardia de restauración (bloqueo de base en uso)
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_restore_guard.py -KeepArtifacts

# Rendimiento local (no es un SLA)
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/perf_smoke.py -KeepArtifacts
```

Verificación de la base, indicando la ruta impresa por el orquestador:

```powershell
python tools/windows-native-e2e/verify_db.py <run_directory>\hiqnet-pos.db
python tools/windows-native-e2e/verify_returns_db.py <run_directory>\hiqnet-pos.db

# Conservación de datos de todas las fases al respaldar y restaurar
python tools/windows-native-e2e/restore_multiphase_check.py `
    <run_directory>\hiqnet-pos.db apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe
```

## Aislamiento y limpieza

- Cada ejecución usa `%TEMP%\hiqnet-e2e-<guid>\hiqnet-pos.db` vía
  `HIQNET_POS_DB_PATH`; nunca `%LOCALAPPDATA%\HiQNet\POS\data`.
- La línea `sqlite=` indica `header_ok (main=<bytes>, wal=<bytes>)` o `missing`.
  Con WAL activo el archivo principal puede ser pequeño y los datos vivir en
  `-wal`: esa línea **no** prueba contenido. La verificación de contenido es
  `verify_db.py` / `verify_returns_db.py` sobre el directorio indicado.
- La limpieza termina únicamente el árbol de procesos descendiente del
  `tauri-driver` iniciado por la ejecución. Si el driver murió antes, los hijos
  huérfanos se identifican porque su línea de comandos contiene la base única de
  esta corrida; ninguna instalación personal usa esa ruta. Nunca se selecciona
  por nombre o fecha de inicio.
- Un fallo al detener procesos no impide restaurar `HIQNET_POS_DB_PATH`,
  `HIQNET_E2E_ARTIFACT_DIR` ni `HIQNET_E2E_PORT`; la restauración ocurre siempre
  en el bloque `finally`.
- Si el driver no arranca o termina antes del sondeo, el arnés informa
  `result=FAIL`, código de salida distinto de cero y conserva
  `tauri-driver.err.log` cuando se usa `-KeepArtifacts`.
- Sin `-KeepArtifacts` el directorio temporal se elimina.

## Escenario determinista (flujo comercial)

Producto `CAF-001` a 10.00 netos con IVA 16%, variante `Grande` a 12.00, extra
`Shot extra` +2.00 y omisión `Sin azúcar` sin delta, dos unidades:

| Concepto | Importe |
| --- | --- |
| Neto unitario (variante + extra) | 14.00 |
| Impuesto unitario (16%) | 2.24 |
| Total unitario | 16.24 |
| Total de la venta | 32.48 |
| Efectivo recibido | 50.00 |
| Cambio | 17.52 |
| Fondo inicial | 100.00 |
| Compras (2 + 3 unidades) | stock 18 → 23 |
| Gasto externo / desde caja | 9.00 / 12.00 |
| Merma | 1 unidad → stock 22 |
| Saldo esperado al cierre | 120.48 |

## Escenario determinista (devoluciones)

Producto `Café devolución` 10.00 + IVA 16%, venta de 2 unidades (`23.20`), una
devolución de 1 unidad reintegrable:

| Concepto | Importe / valor |
| --- | --- |
| Reembolso | 11.60 |
| Saldo esperado | 111.60 |
| Stock | 20 → 18 → 19 |
| Reportes | brutas 23.20, devueltas 11.60, netas 11.60 |

## Rendimiento medido (referencia local)

Ejecución `perf_smoke.py` en Windows 11, base aislada, 200 ventas:

| Métrica | Resultado |
| --- | --- |
| Arranque hasta onboarding | 1.17 s |
| Reinicio hasta login | 1.13 s |
| Checkout promedio (IPC) | 8.66 ms |
| `list_products` | 6.8 ms |
| `list_stock_levels` | 6.3 ms |
| `reports_summary` (200 ventas) | 10.6 ms |

No es un SLA ni una prueba de carga; es una referencia reproducible en el equipo
de desarrollo.

## CI

La suite nativa **no se ejecuta en GitHub Actions**: los runners no garantizan
interacción gráfica ni WebDriver/WebView2 estables. Se mantiene como verificación
local obligatoria antes de un piloto, con capturas e informes en el directorio de
artefactos.
