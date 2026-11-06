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

Estado real de la verificación Android:

- Compilación ARM64: verificada.
- Instalación y arranque en emulador `x86_64`: verificables con `adb`; requiere
  compilar también para `x86_64`.
- Interacción táctil completa, teclado virtual real y hardware ARM64 físico:
  **pendientes**. No se declaran superados por una reducción de viewport.
- SAF (respaldo/restauración externa): **no implementado**.

## 6. Resultados de la última pasada

Ejecutada el 2026-11-05 sobre `chore/integration-android-ux`:

| Comprobación | Resultado |
| --- | --- |
| `pnpm lint` / `pnpm typecheck` | Pasan |
| `pnpm test` | 96 pruebas, 15 archivos |
| `pnpm build` | Pasa |
| `playwright test` | 13 pruebas (8 capturas omitidas sin `VISUAL_REVIEW`) |
| `VISUAL_REVIEW=1 playwright test` | 8 capturas pasan |
| `cargo fmt` / `clippy -D warnings` / `cargo test --workspace` | Pasan (25 binarios) |
| `cargo tauri build --debug --no-bundle` | Pasa |
| `e2e_layout_drift.py` | PASS |
| `e2e_flow.py` | PASS |
| `e2e_returns.py` | PASS |

## 7. Qué no cubre esta suite

- No hay E2E nativo en CI: los runners no garantizan WebView2 ni WebDriver
  estables. Es una verificación local previa a un piloto.
- No hay pruebas de carga. `perf_smoke.py` es una referencia local, no un SLA.
- No hay modo oscuro implementado.
- La regresión visual no compara píxeles: combina geometría medida, estilos
  computados y aserciones funcionales.
