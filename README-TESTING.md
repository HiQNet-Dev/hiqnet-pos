# Guía de pruebas — HiQNet POS

Este documento describe qué se ejecuta, con qué comando y qué garantiza cada
nivel de prueba. Para arquitectura y reglas de negocio ver
`docs/01-architecture/overview.md` y `AGENTS.md`.

## 1. Resumen rápido

| Nivel | Comando | Qué comprueba |
| --- | --- | --- |
| Frontend estático | `cd frontend && pnpm lint && pnpm typecheck` | Reglas de ESLint y tipos de TypeScript |
| Unidad y componente | `cd frontend && pnpm test` | Lógica y componentes con Vitest + Testing Library sobre jsdom |
| Navegador (Playwright) | `cd frontend && pnpm exec playwright test` | Shell, navegación, formularios y geometría real del webview con IPC simulado |
| Capturas visuales | `cd frontend && VISUAL_REVIEW=1 pnpm exec playwright test e2e/visual.spec.ts e2e/visual-admin.spec.ts` | Evidencia visual en `.devkit/artifacts/design-review/after/` |
| Rust estático | `cd backend-rust && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` | Formato y lints del dominio, aplicación y persistencia |
| Rust pruebas | `cd backend-rust && cargo test --workspace` | Reglas de negocio, migraciones, transacciones y auditoría con SQLite real |
| E2E nativo Windows | `powershell -File tools/windows-native-e2e/run-e2e.ps1 -Script …` | Ejecutable real con WebView2, base SQLite aislada y capturas |
| Compilación | `cd apps/pos-desktop && cargo tauri build --debug --no-bundle` | Que el paquete de escritorio se construya con el frontend vigente |

## 2. Frontend

```bash
cd frontend
pnpm install
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

- `pnpm test` usa jsdom: **no** comprueba layout. Cualquier afirmación sobre
  alineación, desbordes o tamaños debe venir de Playwright o del E2E nativo.
- `pnpm format:check` no forma parte de la puerta de calidad: el repositorio no
  está formateado íntegramente con Prettier y reformatearlo generaría ruido
  ajeno al cambio.

### Navegador (Playwright)

```bash
cd frontend
pnpm exec playwright test                 # shell, pantallas y responsive
VISUAL_REVIEW=1 pnpm exec playwright test # además, capturas
```

| Archivo | Cobertura |
| --- | --- |
| `e2e/smoke.spec.ts` | Onboarding y login sobre el shell |
| `e2e/shell-nav.spec.ts` | Expandir/colapsar con geometría medida, alternancia repetida, persistencia, riel compacto, tablet, barra superior en 1024→412, teclado, foco del diálogo |
| `e2e/screens-layout.spec.ts` | Densidad del historial de turnos, lectura de existencias al repetir selección, traducción de la auditoría |
| `e2e/android-back.spec.ts` | Un solo registro nativo del Back, prioridad del diálogo sobre la vista de ticket y consumo sin cerrar la vista |
| `e2e/screens-layout.spec.ts` | Precio al público en la tarjeta, importes habituales del cobro, densidad del historial de turnos, lectura de existencias al repetir selección y traducción de la auditoría |
| `e2e/visual.spec.ts` | Capturas de login, ventas, catálogo, inventario, caja, gestión, navegación y autenticación en 1920, 1440, 1366, 1024, 800 y 412 |
| `e2e/visual-admin.spec.ts` | Capturas de reportes, compras, gastos, devoluciones y gestión |

Los fixtures viven en `e2e/visual.spec.ts` (catálogo completo) y
`e2e/mock-shell.ts` (shell mínimo y navegación). Las capturas esperan fuentes,
dos frames de React y un margen corto: sin esa espera el riel podía aparecer con
el resaltado del estado anterior.

## 3. Rust

```bash
cd backend-rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Las pruebas de persistencia usan SQLite real en `pos-storage-sqlite/tests`;
las de dominio, `pos-core`. No hay pruebas que toquen la base de una instalación
personal.

## 4. E2E nativo Windows

Requiere `tauri-driver`, `msedgedriver.exe` de la misma versión que WebView2 y
Selenium. Detalles en `tools/windows-native-e2e/README.md`.

```powershell
# Compilar primero el binario con el frontend empaquetado
cd apps/pos-desktop
cargo tauri build --debug --no-bundle

# Guardia de layout: navegación, diálogos, Tab, scroll, redimensionado y zoom
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_layout_drift.py

# Flujo comercial completo
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_flow.py

# Devoluciones y reportes por interfaz
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows-native-e2e/run-e2e.ps1 `
    -Script tools/windows-native-e2e/e2e_returns.py
```

Cada ejecución usa `%TEMP%\hiqnet-e2e-<guid>\hiqnet-pos.db` vía
`HIQNET_POS_DB_PATH` y nunca la base real. Un `result=PASS` al final es la señal
de éxito; el guion devuelve código distinto de cero en caso contrario.

## 5. Android

```bash
cd apps/pos-desktop
cargo tauri android build --apk --target aarch64
```

La APK resultante queda **sin firmar** en
`src-tauri/gen/android/app/build/outputs/apk/universal/release/`. La firma se
aplica con la clave interna (fuera del repositorio) y se verifica con
`apksigner`; los comandos están en `docs/06-release/internal-test-build-0.1.0-android.md`.

Verificado en emulador `x86_64` (Android 17, 2560×1600) el 2026-11-06, sobre la
APK compilada desde `1821d2d`: instalación limpia, arranque en frío, onboarding,
inicio de sesión, navegación por las nueve secciones, modal de alta, alta real de
producto, orientación vertical con riel compacto y teclado virtual (el viewport
se reduce, el campo sigue visible).

El Back de Android tiene su propia matriz de 33 comprobaciones
(`.devkit/back-matrix.py`, evidencia en
`docs/06-release/internal-test-build-0.1.0-android.md`): diálogo, doble pulsación,
20 ciclos modal → Back, teclado abierto, vista de ticket, checkout, venta
confirmada, gesto del sistema, resume, force-stop, login y persistencia tras
reinicio. Se ejecuta leyendo el árbol de accesibilidad, sin coordenadas fijas, y
cada paso comprueba que el proceso sigue vivo y que `logcat` no registra
`FORTIFY` ni `Fatal signal`.

Pendiente y **no declarado superado**:

- Scroll con desbordamiento real en el dispositivo: no forzado en el emulador.
- Hardware ARM64 físico (solo emulador `x86_64`) y SAF (respaldo/restauración
  externa).

## 6. Resultados de la última pasada

Ejecutada el 2026-11-06 sobre `chore/integration-android-ux` (`f5744d9`):

| Comprobación | Resultado |
| --- | --- |
| `pnpm lint` / `pnpm typecheck` | Pasan |
| `pnpm test` | 115 pruebas, 16 archivos |
| `pnpm build` | Pasa (`index-Bx8ah-xo.js`, `index-CiRAZ_O3.css`) |
| `playwright test` | 20 pruebas (8 capturas omitidas sin `VISUAL_REVIEW`) |
| `VISUAL_REVIEW=1 playwright test` | 8 capturas pasan |
| `cargo fmt` / `clippy -D warnings` / `cargo test --workspace` | Pasan (26 binarios) |
| `cargo tauri build` (release, NSIS) | Pasa |
| `cargo tauri android build --apk --target aarch64` | Pasa |
| `e2e_layout_drift.py` | PASS (8 rondas, binario release) |
| `e2e_flow.py` | PASS (verifica el precio al público antes de vender) |
| `e2e_returns.py` | PASS |
| Matriz Back Android | 33/33 PASS, 0 abortos nativos |

Pruebas añadidas en esta pasada:

- `pos-core/src/pricing.rs`: redondeo half-up, tasa cero, centavos y límites.
- `pos-storage-sqlite/tests/catalog_pricing.rs`: precio al público en el
  catálogo, variantes, modificadores, vista previa y venta real de dos unidades.
- `SalesScreen.test.tsx`: precio publicado, «Desde», una sola llamada IPC y
  `Enter` en la búsqueda.
- `CatalogScreen.test.tsx`: vista previa del precio final por IPC.
- `screens-layout.spec.ts`: precio en navegador real e importes habituales del
  cobro.

## 7. Qué no cubre esta suite

- No hay E2E nativo en CI: los runners no garantizan WebView2 ni WebDriver
  estables. Es una verificación local previa a un piloto.
- No hay pruebas de carga. `perf_smoke.py` es una referencia local, no un SLA.
- No hay modo oscuro implementado.
- La regresión visual no compara píxeles: combina geometría medida, estilos
  computados y aserciones funcionales.
