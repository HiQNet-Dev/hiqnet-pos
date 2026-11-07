# Inventario de ramas y mantenimiento del repositorio

Estado observado: `main` en `0b57cbd` (2026-11-04). 22 ramas remotas contando
`main`.

Este documento es un inventario de trabajo. **No autoriza borrados ni cierres**:
cada operación remota requiere aprobación del propietario.

## Clasificación

### Cadena activa (conservar)

| Rama | PR | Base | Commits sobre `main` | Rol |
| --- | --- | --- | --- | --- |
| `chore/local-stabilization-prepilot` | #17 | `main` | 13 | Estabilización previa al piloto |
| `chore/desktop-ux-polish` | #19 | `chore/local-stabilization-prepilot` | 1 | Ajustes de UX de escritorio |
| `chore/android-local-spike` | #18 | `chore/local-stabilization-prepilot` | 1 | Spike Android |
| `chore/integration-android-ux` | #20 | `chore/desktop-ux-polish` | 22 | Integración Android + UX |
| `feat/android-saf-internal-backup` | #21 | `chore/integration-android-ux` | 10 | Respaldo SAF y firma interna |

Verificación de contenido:

- `chore/desktop-ux-polish` es ancestro de `chore/integration-android-ux`.
- El commit único de `chore/android-local-spike` fue reaplicado en
  `chore/integration-android-ux` (`c860b2b`). La comparación dos a dos solo
  difiere en `apps/pos-desktop/src-tauri/src/lib.rs`, donde la integración es
  superconjunto (añade `preview_product_price`). **#18 está reemplazado por
  #20.**
- `chore/integration-android-ux` es ancestro de
  `feat/android-saf-internal-backup`.

### PR abiertos antiguos

| Rama | PR | Situación | Recomendación |
| --- | --- | --- | --- |
| `docs/tests-readme` | #1 | Un commit que añade `tests/README.md` (7 líneas). El contenido vigente vive en `README-TESTING.md` | Cerrar como obsoleto |
| `fix/ci-security-foundation` | #2 | Un commit (`working-directory` en RustSec y `git grep -e`); ambos cambios ya están en `main` | Cerrar como redundante |

### Fusionadas, candidatas a eliminación

Estas ramas tienen **0 commits por delante de `main`** y sus PR están `MERGED`:

| Rama | PR |
| --- | --- |
| `fix/inventory-integrity` | #3 |
| `feat/catalog-categories-variants` | #4 |
| `feat/catalog-modifier-engine` | #5 |
| `feat/cash-registers-shifts` | #6 |
| `feat/sales-cart-checkout` | #7 |
| `test/g2-5-operational-e2e` | #8 |
| `test/windows-desktop-e2e` | #9 |
| `test/windows-native-e2e` | #10 |
| `feat/desktop-shell-redesign` | #11 |
| `fix/migraciones-crlf-windows` | #12 |
| `feat/local-user-management` | #13 |
| `feat/purchasing-suppliers-receiving` | #14 |
| `feat/local-expenses-waste` | #15 |
| `feat/local-reports-returns` | #16 |

No se usó solo `git branch --merged`: el squash merge no conserva ancestros. La
comprobación fue `git rev-list --count main..<rama>` y `git cherry main <rama>`.

## Orden de integración recomendado

1. Fusionar **#17** en `main` con CI verde.
2. Cerrar **#18** como reemplazado por #20 (contenido verificado como
   superconjunto).
3. Fusionar **#20** en `main` (incluye #18 y #19).
4. Fusionar **#21** en `main`.
5. Cerrar **#1** y **#2** por obsoletos.
6. Eliminar las 14 ramas fusionadas.
7. Eliminar las ramas de #17, #19, #20 y #21 después de fusionar.

Alternativa si se prefiere fusionar cada PR por separado: #17 → #19 → #20 →
#21. **Nunca #18 antes de #20**: duplicaría una implementación divergente.

## Worktrees

| Ruta | Rama |
| --- | --- |
| `…/hiqnet-pos` | `chore/repo-multiplatform-dev` |
| `…/hiqnet-pos-android-local-spike` | `chore/android-local-spike` |

El worktree del spike debe retirarse antes de eliminar esa rama.

## Reglas de higiene

- La evidencia de pruebas vive en `docs/04-quality/`; no en la raíz.
- Capturas sueltas en la raíz (`NN-*.png`), `flow-report.json`,
  `tauri-driver*.log` y similares están ignoradas.
- No se versionan bases, respaldos, APK/AAB, ejecutables, keystores ni `.jks`.
- Las migraciones SQL conservan LF (`backend-rust/migrations/**/*.sql`).
- `.devkit/`, `.serena/`, `.zed/` y las herramientas de diseño locales no se
  versionan.

## Operaciones que requieren aprobación

- Borrar ramas remotas.
- Cerrar o fusionar PR.
- Configurar protección de `main` (hoy **sin protección**: no exige PR, CI ni
  bloqueo de force push).
- Crear o proteger tags de release.
- Configurar secretos de firma.
