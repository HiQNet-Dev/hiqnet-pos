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
- Hardware ARM64 físico (solo emulador `x86_64`).
- SAF en una APK instalada: el adaptador y las pruebas de persistencia se implementan en
  `feat/android-saf-internal-backup`, pero aún requiere AVD conectado, selector real y firma
  interna antes de declararse aprobado.

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

## 7. Cierre técnico más reciente — catálogo administrativo (2026-11-06)

El cierre posterior se ejecutó sobre el commit funcional `28a95f8`, que corrige
la invalidación de compilación de migraciones embebidas. Las tablas anteriores
son evidencia histórica de sus commits indicados; no sustituyen estos resultados.

| Comprobación | Resultado |
| --- | --- |
| Frontend | **PASS** — `pnpm lint`, `pnpm typecheck`, 118 Vitest, build, 22 Playwright (8 visuales omitidas), revisión visual administrativa y escala 1k/10k |
| Rust | **PASS** — `cargo fmt --all -- --check`, Clippy con `-D warnings` y `cargo test --workspace` |
| Tauri Windows release | **PASS** — NSIS reconstruido tras recompilar `pos-storage-sqlite` |
| Actualización SQLite con ejecutable release | **PASS** — base sintética `0010 → 0011`: 11 migraciones, índice `idx_product_org_name`, `integrity_check=ok` y 0 claves foráneas inválidas |
| Flujo operativo después de actualizar | **PASS** — `e2e_post_upgrade.py` conserva catálogo, inventario, caja, gastos, merma, compras, usuarios, ventas y auditoría |
| Instalación aislada NSIS | **PASS** — Windows Sandbox verifica instalación, arranque, operación sin red, desinstalación, reinstalación y persistencia SQLite |
| Instalador candidato | `apps/pos-desktop/src-tauri/target/release/bundle/nsis/HiQNet POS_0.1.0_x64-setup.exe` — 3 872 471 bytes — SHA-256 `9D35261FC65FDBEA816DCDA5FC2242F8C760D9A3F8086AA8091F309231A07E0E` |
| Android de este cierre | **BLOCKED** — se reconstruyó `app-universal-release-unsigned.apk` (ARM64, 11 879 361 bytes, SHA-256 `85A50959348996A308B21C613E1CCB4CF5F8FAD54D0C7FF39D0CD605D3CB62EB`), pero `apksigner` confirma que no verifica por falta de firma. No hay ADB conectado; falta firmar v2/v3 con la clave externa e instalar/probar. No es una entrega. |

La crate `pos-storage-sqlite` contiene un `build.rs` que incorpora una huella de
el directorio de migraciones a la compilación. Así, agregar, quitar o cambiar una
migración vuelve a compilar la crate aun cuando el build Tauri use su propio
target; la prueba de actualización anterior es parte obligatoria de cualquier
nuevo instalador con migraciones.

## 8. Qué no cubre esta suite

- No hay E2E nativo en CI: los runners no garantizan WebView2 ni WebDriver
  estables. Es una verificación local previa a un piloto.
- No hay pruebas de carga. `perf_smoke.py` es una referencia local, no un SLA.
- No hay modo oscuro implementado.
- La regresión visual no compara píxeles: combina geometría medida, estilos
  computados y aserciones funcionales.

## 9. SAF Android — pruebas de la rama aislada

La rama `feat/android-saf-internal-backup` incorpora un plugin Tauri local versionado, no una
edición de `src-tauri/gen/android`. El adaptador usa `ACTION_CREATE_DOCUMENT`,
`ACTION_OPEN_DOCUMENT` y `ContentResolver`; solo recibe rutas temporales privadas generadas por
Rust. No solicita permisos de almacenamiento amplios.

```bash
cd backend-rust
cargo test -p pos-storage-sqlite --test pending_restore

cd ../frontend
pnpm exec vitest run src/screens/AdminScreen.test.tsx

# Pruebas JVM del plugin (sin dispositivo)
cd ../apps/pos-desktop/src-tauri/gen/android
./gradlew.bat :tauri-plugin-saf-document:testDebugUnitTest

# APK release para un AVD desechable
cd ../..
cargo tauri android build --apk --target x86_64

# Firma interna (pide la contraseña de forma segura, sin registrarla)
powershell -NoProfile -File ../../../tools/android/sign-internal-apk.ps1 \
    -Apk <ruta>/app-universal-release-unsigned.apk
```

| Comprobación | Estado | Alcance |
| --- | --- | --- |
| Restauración misma instalación | PASS (SQLite) | snapshot, datos y Trial vigente preservado |
| Reanudación tras interrupción antes/después de reponer el estado comercial | PASS (SQLite) | dos escenarios; no bloquea el arranque |
| Reabrir varias veces | PASS (SQLite) | sin marcador no hay segunda pasada |
| Trial vencido no se revive | PASS (SQLite) | se conserva `read_only` y el vencimiento vigente |
| Fallo de promoción tras confirmar | PASS (SQLite) | marcador y base previa se conservan; el reintento completa |
| Instalación/organización distinta | PASS (SQLite + dispositivo) | rechazo antes de reemplazar la base |
| Licencia firmada sin verificador temprano | PASS (SQLite) | rechazo seguro, sin copiar derechos |
| Archivo alterado después de confirmar | PASS (SQLite) | SHA-256 rechaza la operación; base intacta |
| Limpieza de temporales SAF | PASS (SQLite) | residuos eliminados sin tocar la base |
| Concurrencia del plugin | PASS (JVM, 7 pruebas) | una sola operación hasta terminar la copia |
| Límite de tamaño | PASS (JVM + Rust) | streaming con tope; no se carga en memoria |
| Exportación SAF real en release | PASS (emulador) | 442 368 bytes; verificado con Rust |
| Importación + reinicio en release | PASS (emulador) | aviso correcto, datos revertidos, sesión cerrada |
| Archivo corrupto / instalación ajena | PASS (emulador) | mensajes en español; app viva |
| Actualización de APK sin desinstalar | PASS (emulador) | misma clave interna; 11 migraciones; datos preservados |
| Back, IME, suspensión, offline y scroll real | PASS (emulador release) | 61 productos; último visible y seleccionable |
| Abortos nativos | PASS | 0 `FORTIFY`, `Fatal signal`, `panic` |
| Firma interna v2/v3 | PASS | certificado `3a522cd0…`, idéntico al histórico |
| Hardware ARM64 físico | BLOCKED | no hay dispositivo |

El AVD usado fue `hiqnet_saf_test` (Android 17, `x86_64`, 2560×1600), creado para esta pasada.
La instalación firmada preexistente de `hiqnet_tablet` **no** se desinstaló ni se sobrescribió.
La APK release de E2E se firmó con una identidad aislada `CN=HiQNet AVD Test`; la actualización
se probó con la clave interna sobre el mismo AVD, previa desinstalación del build de pruebas.

Evidencia local (`.devkit/` está fuera de Git): capturas `01`–`50` y árboles `ui-*.xml` en
`.devkit/artifacts/android-saf/`.

### Artefactos vigentes

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `HiQNet POS_0.1.0_x64-setup.exe` (NSIS) | 3 918 937 | `81989E2133E7821F121EBE9AAD7D8F0E53778A2885AB687D848A37159A8B5BCC` |
| `hiqnet-pos-0.1.0-internal-arm64.apk` (firmada) | 12 122 640 | `82AD7A9163594B0FEB1E55A1C7C9503DD9112C463D16052A087DD4C7ED08E32F` |
| `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` (firmada, solo actualización en AVD) | 12 970 509 | `04EDE9A6C21E1B4BDC600E3B55E14C49611DA851DA2BE2E5E513243DDFBF9F0B` |
| `hiqnet-pos-0.1.0-avd-test-x86_64.apk` (clave aislada, no entregable) | 12 970 511 | `6275C09B05D98DE39F838FBB5B8942321B83DAD18E3F28F03DA0F140C1570E03` |

Ubicación de los paquetes Android: `~/HiQNet-Internal-Test-saf/android/`.

El procedimiento interactivo de firma es `tools/android/sign-internal-apk.ps1`; solicita la
contraseña con `SecureString`, la pasa por variables de entorno del proceso y compara el SHA-256
del certificado con la huella histórica. No escribe secretos en disco ni en la salida.

La política y los límites de 0.1.0 están en
`docs/01-architecture/decisions/ADR-009-android-saf-same-installation-restore.md`.
