# Auditoría UX/UI 0.1.0 — escritorio y tablet

- Fecha: 2026-11-05 (segunda pasada de cierre)
- Rama: `chore/integration-android-ux` (PR #20)
- Alcance: shell completo (`frontend/src`), estados responsive de 1920×1080 a
  412×915, navegación lateral, barra superior, ventas y cobro, pantallas
  administrativas y autenticación.
- Método: capturas del webview real con el arnés de Playwright, medición de
  geometría renderizada, pruebas E2E de navegación y E2E nativo sobre el
  ejecutable.

> Las capturas se generan con un puente IPC simulado y datos de fixture
> deterministas. El E2E nativo se ejecuta sobre `pos-desktop.exe` con base
> SQLite aislada. Ninguna prueba se ejecutó sobre datos personales.
>
> Esta pasada revisó **todas** las capturas de `before/` y `after/` contra el
> comportamiento renderizado, y refrescó la carpeta `after/` completa: las
> capturas de la revisión anterior ya no representaban el código vigente.

## A. Auditoría visual

| # | Pantalla / zona | Hallazgo | Sev. | Evidencia | Causa | Corrección |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Navegación lateral, escritorio colapsado | Al contraer, las etiquetas seguían visibles dentro de una columna de 72 px y se desbordaban sobre el área de trabajo (“Ven”, “Prod”, “Inve” encima del catálogo). | **P0** | `before/21-nav-colapsada-1920.png` → `after/21-nav-colapsada-1920.png` | `.app-body[data-collapsed]` solo cambiaba el ancho de la columna; no existía ninguna regla que retirara la etiqueta, centrara el icono ni reposicionara el contador. | Estado colapsado propio en `@media (min-width: 1081px)`: etiquetas fuera del layout, iconos centrados, contador sobre el icono. |
| 2 | Barra superior ≤ 600 px | “Cerrar sesión” quedaba fuera del viewport e inalcanzable (el shell recorta y no hay scroll horizontal). | **P1** | `before/23-topbar-600.png` → `after/26-topbar-412.png` | Los controles del extremo tenían `flex: none` y el chip de caja no cedía; la marca tampoco truncaba. | Chip de caja truncable, `min-width: 0` en marca y meta, sello del plan oculto ≤ 980 px, chip como icono ≤ 720 px, nombre de marca con elipsis. |
| 3 | Reportes 1440×900 | La tabla del reporte de ventas se cortaba: la columna **Total** quedaba fuera de la tarjeta y los nombres de producto se partían en tres líneas. | **P1** | `before/24-reportes-1440.png` → `after/31-reportes-1440.png` | La rejilla dejaba la tarjeta en 383 px; la tabla de cinco columnas no cabe y no había pista de desplazamiento. | La primera tarjeta (reporte de ventas) ocupa la fila completa. |
| 4 | Cualquier diálogo | Al cerrar, el foco no volvía al control que lo abrió: caía en `body`. | **P1** | `shell-nav.spec.ts › el diálogo no desplaza la raíz y devuelve el foco` | El elemento de origen se leía dentro del `useEffect`; un campo con `autoFocus` ya había movido el foco durante el montaje. | El origen se captura en el primer render de `Modal`. |
| 5 | Autenticación con poca altura / teclado virtual | Con la altura reducida, la cabecera del formulario quedaba fuera del área desplazable. | **P1** | `after/41-login-412-teclado.png` | `justify-content: center` con desbordamiento deja la parte superior inalcanzable. | Márgenes automáticos en el primer y último hijo, `100dvh` con respaldo y `interactive-widget=resizes-content`. |
| 6 | Caja, historial de turnos cerrados | Las filas del historial se comprimían en una columna de 92 px: el nombre de la caja y la fecha se partían en dos líneas y el arqueo quedaba descolgado. | **P1** | `before/25-caja-historial-comprimido-1920.png` → `after/10-caja-1920.png` | La lista reutilizaba `.ledger`, cuya regla `.ledger li` impone la rejilla de cuatro columnas del libro de movimientos. | Clase propia `.ledger-history`; la fila vuelve a ser texto + arqueo a todo el ancho. |
| 7 | Inventario, selector de producto | Volver a elegir el mismo producto dejaba las existencias en “…” y el historial en “Consultando inventario…” de forma indefinida. | **P1** | `before/26-inventario-bloqueado-1920.png` → `after/09-inventario-1920.png` | El `onChange` marcaba la lectura en curso aunque el valor no cambiara; sin cambio de `productId` el efecto no se disparaba. | La selección repetida es una salida temprana: no se resetea el panel ni se marca carga. |
| 8 | Gestión, últimos eventos | Ocho acciones reales de Rust (`purchasing.*`, `expenses.*`, `waste.recorded`, `sales.returned`) se mostraban como claves técnicas. | **P2** | `after/11-gestion-1920.png`, `after/39-gestion-768.png` | El mapa de traducción no cubría todo el enum `AuditAction`. | Mapa completo + fallback en español; la clave vive en `data-action`, nunca en pantalla. |
| 9 | Ventas, fila de filtros 1366/1440 | El chip “Combos” aparecía cortado a media palabra. | P2 | `after/14-ventas-1366.png` | Búsqueda y chips se repartían el ancho al 50 % (`flex: 1` con base 0). | La búsqueda cede (`flex: 1 1 220px`); los chips conservan su ancho natural y pasan a fila propia cuando no caben. |
| 10 | Navegación lateral ≤ 1080 px | El control de colapso se ocultaba pero el estado seguía existiendo: tres selectores CSS peleaban por el ancho. | P2 | `after/24-nav-riel-1024.png` | Se resolvía sobrescribiendo reglas en lugar de definir un estado. | Un solo estado “riel compacto” documentado; `data-collapsed` solo aplica por encima de 1081 px. |
| 11 | Gestión, tabla de usuarios | “Desactivar” (destructiva) era visualmente idéntica a “Contraseña” (neutra) en la misma fila. | P2 | `after/37-gestion-1024x700.png` | Ambas usaban `button-quiet`. | Variante `.button-quiet-danger`: discreta, en color de peligro, con confirmación en diálogo. |
| 12 | Navegación lateral colapsada | Sin forma de conocer el nombre completo más allá del `title` nativo (invisible al enfocar con teclado). | P2 | `after/23-nav-colapsada-tooltip-1440.png` | No existía tooltip. | Tooltip en hover y `:focus-visible` con el nombre completo y el contador. |
| 13 | Reportes, tarjeta Gastos | “Insumos ·” dejaba un separador colgando antes del importe. | P3 | `after/31-reportes-1440.png` | Nombre e importe compartían un nodo de texto con un `·` fijo. | Nombre e importe como columnas separadas. |
| 14 | Gestión, meta de cada evento | “Venta sale-128 · fecha” repetía el sustantivo que ya llevaba la acción. | P3 | `after/11-gestion-1920.png` | La meta combinaba etiqueta de entidad, identificador y fecha. | Fecha primero y referencia corta en monoespaciado. |
| 15 | Navegación lateral | El contador del ticket podía crecer a tres dígitos y desbordar la píldora del riel. | P3 | Código + `shell-nav.spec.ts` | Sin límite de dígitos. | Abreviatura `99+`; el número completo va en la etiqueta accesible. |
| 16 | Shell Android | `100vh` no sigue las barras dinámicas del sistema. | P3 | Código | Altura fija al viewport grande. | `100dvh` con respaldo en `100vh`. |
| 17 | Android, gesto Back con un diálogo abierto | Cerraba el diálogo **y** finalizaba la Activity: el proceso moría y `libc` registraba `FORTIFY: pthread_mutex_lock called on a destroyed mutex` durante el desmontaje. | **P1 corregido** | `after/46-android-back-modal-cerrado.png`, `after/47-android-back-ticket.png`, `after/48-android-back-persistencia.png` | Tauri registra un `OnBackPressedCallback` y, sin listener en la interfaz, delega en `Activity.onBackPressed()`: `finish()` sobre una SPA de una sola entrada. | Un único listener con `onBackButtonPress` y una pila de prioridad en `frontend/src/lib/backNavigation.ts` (diálogo → vista de ticket → raíz). Matriz Android 33/33 PASS, 20/20 ciclos, 0 abortos nativos. Detalle en `docs/06-release/internal-test-build-0.1.0-android.md`. |
| 18 | Marca de la aplicación | El favicon con el texto «HQ» era ilegible a 16 px y el icono del instalador no se parecía al de la cabecera. | P3 | `apps/pos-desktop/src-tauri/app-icon.svg`, `frontend/public/favicon.svg` | El favicon era un `<text>` y la cabecera un distintivo aparte. | Un único dibujo (cuadrado de acento con H geométrica) como fuente en `app-icon.svg`, iconos del paquete regenerados y `BrandMark` en la interfaz. Documentado en `DESIGN.md`. |

Hallazgo de proceso: el arnés podía fotografiar estados intermedios (inventario
en “Consultando…”, riel con el resaltado del frame anterior). El capturador
ahora espera fuentes, dos frames y un margen corto, y las pruebas de inventario
esperan a que la lectura esté resuelta. Una captura en carga ya no se considera
evidencia.

## B. Navegación lateral

| Aspecto | Comportamiento implementado |
| --- | --- |
| Ancho expandido | 216 px (`--sidenav-width`) |
| Ancho colapsado | 72 px (`--sidenav-width-collapsed`) |
| Riel compacto | 88 px (`--sidenav-width-rail`) |
| Breakpoint | `min-width: 1081px` define expandida/colapsada; `max-width: 1080px` define el riel compacto |
| Transición | `grid-template-columns 160 ms ease`; desactivada por `prefers-reduced-motion` |
| Etiquetas | Expandida: nombre completo. Colapsada: ninguna (tooltip). Riel: etiqueta corta bajo el icono |
| Contador | En línea al final de la fila (expandida); sobre el icono (colapsada y riel); `99+` por encima de 99 |
| Control de colapso | Visible solo en escritorio; `aria-expanded` refleja el estado; en el riel no se muestra porque no tendría efecto |
| Tooltip | Visible al hover y con `:focus-visible` en escritorio colapsado; `pointer-events: none`; fuera del flujo del botón |
| Persistencia | `localStorage["hiqnet.nav.collapsed"]`, por instalación, sin datos personales; se restaura al iniciar sesión |
| Redimensionado | Escritorio → compacto: el riel sustituye al colapso y el control desaparece. Compacto → escritorio: vuelve la preferencia guardada. No se pierde la sección activa ni el ticket |
| Teclado | Botones nativos; orden barra superior → navegación → contenido; el tooltip aparece al enfocar |
| Accesibilidad | `aria-label` explícito por entrada (incluye el contador con singular/plural), `aria-current="page"`, `aria-expanded`, foco visible con `:focus-visible` |
| Táctil | `min-height: 48px` por entrada en `pointer: coarse`; objetivos de 44 px en chips |

Decisión de responsive: se mantiene el riel compacto de 88 px en lugar de un
cajón temporal. Conserva la navegación visible en tablet vertical (768×1024), no
oculta el catálogo ni el ticket y evita un segundo mecanismo superpuesto sobre
los modales. El control de colapso se retira en ese rango para no dejar un botón
sin efecto.

Geometría verificada (`shell-nav.spec.ts`, no solo estilos): ancho de la barra
en cada estado, centro del icono respecto al centro del riel (±1 px), contador
dentro del ancho del riel, visibilidad de etiquetas y del control, y
`scrollWidth ≤ clientWidth` del documento tras cada cambio.

## C. Responsive

| Viewport | Navegación | Resultado |
| --- | --- | --- |
| 1920×1080 | 216 / 72 | Expandida y colapsada; contador dentro del riel; sin desbordes |
| 1440×900 | 216 / 72 | Chips completos en una fila; “Cobrar” visible |
| 1366×768 | 216 | Chips completos (`scrollWidth = clientWidth = 484 px`); “Cobrar” y “Cerrar sesión” en viewport |
| 1024×700 (ventana mínima Tauri) | 88 | Riel compacto; chips en fila propia; “Cobrar” y modal de cobro dentro del alto |
| 1280×800 (ventana por omisión, E2E nativo) | 216 | Flujo comercial completo |
| 800×1280 (tablet vertical) | 88 | Catálogo en rejilla; barra inferior con total y cobro |
| 768×1024 (tablet) | 88 | Etiquetas cortas sin recorte; carrito y cobro accesibles |
| 600×900 | 88 | Barra superior completa, incluidas las acciones de cuenta |
| 412×915 (teléfono) | 88 | Chip de caja como icono; acciones de cuenta visibles |

Estados probados en cada resolución aplicable: navegación expandida y colapsada,
sección activa, modal abierto, tabla con datos, formulario con errores (cobro y
contraseña), estado vacío, texto largo, teclado/táctil y redimensionado dinámico
en ambos sentidos.

Sin scroll raíz en ninguna captura: `document.scrollingElement.scrollTop = 0` y
`scrollWidth ≤ clientWidth`. El guardia nativo (`e2e_layout_drift.py`) confirma
que `.app`, `.app-body`, `.app-content` y `body` mantienen `overflow: clip` y que
el contenido interno (`.product-scroll`) sí se desplaza.

## D. Componentes unificados

| Componente | Cambio |
| --- | --- |
| Botón secundario discreto | `.button-quiet-danger` para acciones destructivas de fila |
| Chip de la barra superior | `.topbar-cash` con etiqueta truncable y nombre accesible completo; icono solo ≤ 720 px |
| Campo de búsqueda / chips | Búsqueda flexible, filtros con su ancho natural |
| Navegación (`nav-item`) | `aria-label` explícito, tooltip, contador abreviado, icono `flex: none` |
| Marca (`brand-mark`) | `BrandMark` con el mismo dibujo que el icono del sistema, sin texto |
| Barra superior | Prioridad de colapso: identidad → contexto operativo → caja/turno → usuario → cuenta |
| Diálogo (`Modal`) | Foco devuelto al control de origen |
| Autenticación | Centrado seguro con desplazamiento utilizable |
| Tarjeta de reporte | La tarjeta con tabla ocupa la fila completa |
| Historial lateral | `.ledger-history`: lista de texto y arqueo, sin heredar la rejilla del libro de movimientos |
| Auditoría | Acción traducida, `data-action` oculto, meta con fecha y referencia |

No se añadieron librerías ni un segundo sistema de componentes. Tokens, radios,
tipografía y estados semánticos siguen siendo los de `DESIGN.md`, actualizado
para describir el comportamiento real.

## E. Regresión

| Comprobación | Resultado |
| --- | --- |
| `pnpm lint` | Pasa |
| `pnpm typecheck` | Pasa |
| `pnpm test` | 109 pruebas, 16 archivos |
| `pnpm build` | Pasa (`index-9D_khkjU.css`, `index-BsmKnnR5.js`) |
| `playwright test` | 18 pruebas; 8 capturas omitidas sin `VISUAL_REVIEW` |
| `VISUAL_REVIEW=1 playwright test` | 8 capturas pasan |
| `cargo fmt --all --check` | Pasa |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pasa |
| `cargo test --workspace` | 25 binarios en verde |
| `cargo tauri build` (release, NSIS) | Pasa |
| `cargo tauri android build --apk --target aarch64` | Pasa |
| E2E nativo `e2e_layout_drift.py` | PASS (8 rondas, binario release) |
| E2E nativo `e2e_flow.py` | PASS (bootstrap → venta → reinicio → compras → gastos/merma → arqueo → usuarios → resoluciones) |
| E2E nativo `e2e_returns.py` | PASS (devolución → reportes → reinicio → permisos) |
| Matriz Back Android (`.devkit/back-matrix.py`) | 33/33 PASS, 0 abortos nativos |

Pruebas añadidas en esta pasada:

- `screens-layout.spec.ts` (3): ancho real de la fila del historial de turnos,
  lectura de existencias al repetir la selección, y traducción completa de la
  auditoría sin claves técnicas ni sustantivos repetidos.
- `InventoryScreen.test.tsx`: caso de selección repetida.
- `visual.spec.ts`: `waitForInventory`, que impide capturar inventario en carga.
- `backNavigation.test.ts` (10), `Modal.test.tsx` (+2), `SalesScreen.test.tsx`
  (+1) y `android-back.spec.ts` (5): prioridad del Back, un solo registro
  nativo, cierre único del diálogo y conservación del ticket.

Ver `README-TESTING.md` para comandos, prerrequisitos y cobertura por nivel.

## F. Rendimiento

- No se añadieron intervalos, observadores ni escuchas globales. El tooltip es
  CSS puro; el estado de colapso es un booleano que se lee una vez al montar y se
  escribe en `localStorage` en un efecto.
- La transición de layout dura 160 ms y solo se dispara al alternar la barra.
- Paquete final: CSS 33.28 kB y JS 457.13 kB (antes de la corrección del Back
  y de la marca: 33.42 kB y 453.80 kB). El incremento es el componente de marca
  y la pila de Back; no hay dependencias nuevas.
- `perf_smoke.py` no se volvió a ejecutar; los cambios no tocan el camino de
  cobro ni las consultas. El flujo comercial nativo completo se ejecutó sobre el
  binario final.

## G. Git

- Rama: `chore/integration-android-ux`, base `84caf76`.
- Commits de esta corrección: `a9b980d` (Back de Android) y `1821d2d` (marca
  unificada); la documentación va en el commit siguiente. Sin merge, sin rebase
  destructivo sobre `main` y sin publicar releases.
- Archivos de código: `frontend/index.html`, `styles.css`, `shell/SideNav.tsx`,
  `shell/TopBar.tsx`, `shell/AppShell.tsx`, `shell/icons.tsx`,
  `components/Modal.tsx`, `lib/backNavigation.ts`, `screens/AdminScreen.tsx`,
  `screens/CashScreen.tsx`, `screens/InventoryScreen.tsx`,
  `screens/ReportsScreen.tsx`, `screens/SalesScreen.tsx`.
- Pruebas: `shell/SideNav.test.tsx`, `screens/InventoryScreen.test.tsx`,
  `lib/backNavigation.test.ts`, `components/Modal.test.tsx`,
  `screens/SalesScreen.test.tsx`, `e2e/mock-shell.ts`, `e2e/shell-nav.spec.ts`,
  `e2e/screens-layout.spec.ts`, `e2e/android-back.spec.ts`,
  `e2e/visual-admin.spec.ts`, `e2e/visual.spec.ts`.
- Documentación: `DESIGN.md`, `README-TESTING.md`, este informe y las capturas
  de `docs/04-quality/design-review/`.
- No se modificaron contratos IPC, migraciones ni reglas financieras.

## H. Entregable

El frontend cambió, así que los instaladores internos de 0.1.0 se regeneran
desde `1821d2d`. Los artefactos de `5262880` (`DC623307…` en Windows y
`E45914FC…` en Android) y los de `21bde50` (`864B2076…` y `ACEA7BED…`) quedan
como historial.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `HiQNet POS_0.1.0_x64-setup.exe` (NSIS, sin firma de editor) | 3 854 105 | `B34B30F5C977FE22C1B4F4B22DC849FEE1FC5F0B6895A902B671E85D12F9BFEB` |
| `hiqnet-pos-0.1.0-internal-arm64.apk` (firmada, v2 + v3) | 11 860 301 | `FC80389DE88B92B687F983CFB6A4EA1BF61D4F8BF684CE15A196C59B6EC84206` |

Ubicación: `~/HiQNet-Internal-Test-backfix2/{windows,android}/`.

Android verificado en emulador `x86_64` (Android 17, 2560×1600): instalación
limpia, arranque en frío, onboarding, inicio de sesión, navegación por las nueve
secciones, modal de alta, alta real de producto y teclado virtual. Evidencia en
`after/42-android-ventas-1280x800.png`, `after/43-android-vertical-800x1280.png`,
`after/44-android-teclado.png` y `after/45-android-modal.png`. La corrección del
Back se verificó con la matriz 33/33 (`after/46`–`after/48`).

**Bloqueos abiertos** (no se declaran superados):

- SAF para respaldo/restauración externa: no implementado.
- Tablet ARM64 física: sin dispositivo; sólo emulador `x86_64`.
- Scroll Android con desbordamiento real: no forzado en el emulador; sí
  verificado en el arnés de Playwright y en el E2E nativo de Windows.
- El NSIS se compiló, pero no se reinstaló en Windows Sandbox en esta pasada: la
  evidencia de instalación, desinstalación y actualización sigue siendo la de
  `5262880`, cuyo mecanismo no cambió.

## Limitaciones declaradas

- El E2E nativo se ejecutó en un solo equipo Windows; no cubre hardware ARM64.
- La comprobación del teclado virtual Android se hizo por reducción del viewport
  (412×360), no con un teclado real.
- No hay modo oscuro ni se añadió.
- Compras, gastos y devoluciones se revisaron con datos de fixture y con el flujo
  nativo; no se revisó el caso de catálogos de miles de productos.
- Las comparaciones por píxel no se usan como criterio PASS: la verificación
  combina geometría medida, estilos computados y aserciones funcionales.
