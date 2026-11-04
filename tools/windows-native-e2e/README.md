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
| `e2e_flow.py` | Flujo comercial completo sobre el shell de escritorio: bootstrap → productos → variante → modificadores → inventario → caja → venta → reinicio → persistencia → arqueo, más capturas en las resoluciones de referencia |
| `verify_db.py` | Verificación de solo lectura de la base resultante (venta, partida, pago, stock, caja, auditoría) |
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
  deja `msedgedriver.exe` en `%TEMP%\edgedriver\`. El host `msedgedriver.azureedge.net`
  no resuelve en todas las redes.

- Python con Selenium:

  ```powershell
  python -m pip install selenium
  ```

Versiones verificadas en la última ejecución local: rustc/cargo 1.99.1,
tauri-driver 2.1.0, WebView2 156.0.4314.58, msedgedriver 156.0.4314.58,
Selenium 4.52.0.

## Compilación del ejecutable

El binario debe incluir los recursos del frontend (feature `custom-protocol`).
Compilar con `cargo build` a secas genera un binario que apunta a `devUrl` y el
IPC falla con `Origin header is not a valid URL`. Usa siempre:

```powershell
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2 build --debug --no-bundle
```

## Ejecución

```powershell
# Prueba de contrato IPC (rápida)
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_contracts.py

# Flujo comercial completo, conservando capturas e informe
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_flow.py -KeepArtifacts
```

Con `-KeepArtifacts` se conserva el directorio temporal, que contiene
`flow-report.json`, capturas reales de cada pantalla y los logs del driver. El
guion captura:

- `ventas-1920`, `ventas-1440`, `ventas-1366`, `ventas-1024` (catálogo y ticket
  simultáneos), `ventas-tablet-catalogo` y `ventas-tablet-ticket` (768×1024),
- `productos`, `inventario`, `caja` y `gestion` a 1366×768,
- estados intermedios: login, configurador de la partida, ticket, checkout,
  venta confirmada, turno abierto y arqueo cerrado.

El guion navega con el menú lateral (`data-section`), así que un cambio de
estructura obliga a actualizar los selectores de navegación, no el flujo.

La verificación de la base se ejecuta aparte, indicando la ruta impresa por el
orquestador:

```powershell
python tools/windows-native-e2e/verify_db.py <run_directory>\hiqnet-pos.db
```

## Aislamiento

- Cada ejecución usa `%TEMP%\hiqnet-e2e-<guid>\hiqnet-pos.db` vía
  `HIQNET_POS_DB_PATH`; nunca `%LOCALAPPDATA%\HiQNet\POS\data`.
- `run-e2e.ps1` valida la cabecera SQLite de la base y restaura las variables de
  entorno al terminar.
- Solo se detienen el `tauri-driver` y los procesos `pos-desktop` iniciados
  durante la ejecución (filtrados por hora de arranque).
- Sin `-KeepArtifacts` el directorio temporal se elimina.

## Escenario determinista

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
| Saldo esperado al cierre | 132.48 |
| Stock 20 → | 18 |

Los importes del fixture son ilustrativos; no representan tarifas comerciales.

## Limitaciones conocidas

- La administración de grupos y opciones de modificadores existe en la pantalla
  Productos, pero este guion los crea por IPC (casos de uso auditados) para
  mantener el flujo corto; la creación por interfaz está cubierta por Vitest.
- El usuario cajero no puede crearse desde la interfaz (no existe caso de uso de
  alta de usuarios), así que el RBAC por rol se cubre con pruebas de dominio y de
  servicio, no en este E2E.
- El flujo se ejecutó en un equipo local con WebView2. **No** está automatizado en
  GitHub Actions: los runners no garantizan sesión gráfica ni el runtime de
  WebView2 requerido. No debe interpretarse como prueba de distribución.
