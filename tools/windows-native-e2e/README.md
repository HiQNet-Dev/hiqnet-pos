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
| `perf_smoke.py` | Medición local: arranque, 200 checkouts por IPC, consultas y reportes |
| `e2e_smoke.py` | Comprobación mínima de que el driver puede abrir la aplicación |

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
pnpm dlx @tauri-apps/cli@2 build --debug --no-bundle
```

## Ejecución

```powershell
# Flujo comercial completo
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_flow.py -KeepArtifacts

# Devoluciones y reportes
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_returns.py -KeepArtifacts

# Rendimiento local (no es un SLA)
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/perf_smoke.py -KeepArtifacts
```

Verificación de la base, indicando la ruta impresa por el orquestador:

```powershell
python tools/windows-native-e2e/verify_db.py <run_directory>\hiqnet-pos.db
python tools/windows-native-e2e/verify_returns_db.py <run_directory>\hiqnet-pos.db
```

## Aislamiento

- Cada ejecución usa `%TEMP%\hiqnet-e2e-<guid>\hiqnet-pos.db` vía
  `HIQNET_POS_DB_PATH`; nunca `%LOCALAPPDATA%\HiQNet\POS\data`.
- `run-e2e.ps1` valida la cabecera SQLite y restaura las variables de entorno.
- La limpieza identifica únicamente el árbol de procesos descendiente del
  `tauri-driver` iniciado por la ejecución (por PID), nunca por nombre ni fecha.
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
