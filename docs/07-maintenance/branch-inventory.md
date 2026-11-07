# Inventario de ramas y mantenimiento del repositorio

Estado observado: `main` en `455718f` (merge de #22, 2026-11-07). Tras la
limpieza quedan **9 ramas remotas** contando `main`; la rama de este PR de
documentación es transitoria.

Este documento es un inventario de trabajo. **No autoriza borrados**: cada
operación remota requiere aprobación del propietario. La integración de #17–#22
y el cierre de #1, #2, #18 y #19 ya fueron ejecutados por el propietario.

## Integración completada

| PR | Rama | Estado | Resultado |
| --- | --- | --- | --- |
| #17 | `chore/local-stabilization-prepilot` | MERGED | `ef9f71d` |
| #20 | `chore/integration-android-ux` | MERGED | `4c196fa` (incluye #18 y #19) |
| #21 | `feat/android-saf-internal-backup` | MERGED | `92a75b8` |
| #22 | `chore/repo-multiplatform-dev` | MERGED | `455718f` |
| #18 | `chore/android-local-spike` | CLOSED | Reemplazado por #20 |
| #19 | `chore/desktop-ux-polish` | CLOSED | Incluido en #20 |
| #1 | `docs/tests-readme` | CLOSED | Obsoleto (`README-TESTING.md`) |
| #2 | `fix/ci-security-foundation` | CLOSED | Redundante (ya en `main`) |

Verificación de contenido:

- `chore/desktop-ux-polish` es ancestro de `chore/integration-android-ux`.
- El commit único de `chore/android-local-spike` (`8babf94`) fue reaplicado en
  `chore/integration-android-ux` (`c860b2b`). Verificación por `patch-id`
  excluyendo `apps/pos-desktop/src-tauri/src/lib.rs`: **parches idénticos**
  (`69807f05…`). En ese archivo la integración es superconjunto (añade
  `preview_product_price`). Por eso #18 se cerró sin fusionar.
- `chore/integration-android-ux` es ancestro de
  `feat/android-saf-internal-backup`, que es ancestro de
  `chore/repo-multiplatform-dev`.

## Simulación de integración

Antes de la integración real se creó la rama local `integration-sim-local` desde
`main` (`0b57cbd`) y se fusionó en este orden:

| Paso | Rama | Resultado |
| --- | --- | --- |
| 1 | `chore/local-stabilization-prepilot` (#17) | Fast-forward |
| 2 | `chore/integration-android-ux` (#20) | Fast-forward (incluye #19) |
| 3 | `feat/android-saf-internal-backup` (#21) | Fast-forward |
| 4 | `chore/repo-multiplatform-dev` (#22) | Fast-forward |

Sin conflictos. El árbol resultante fue **idéntico** al HEAD de #22
(`git diff --stat` vacío): 11 migraciones, precios públicos,
ventas/devoluciones, SAF, firma externa, rutas Windows/Linux, CI Linux y
documentación. Validaciones sobre el árbol integrado: `cargo fmt`,
`clippy -D warnings`, `cargo test --workspace`, `pnpm lint`, `typecheck`,
`vitest` (120/120) y `build`: todo verde. La rama local puede borrarse.

El orden real ejecutado coincidió: #17 → #20 → #21 → #22.

## Ramas remotas actuales

| Rama | PR | Estado | Acción propuesta |
| --- | --- | --- | --- |
| `main` | — | activa | Conservar |
| `chore/repo-multiplatform-dev` | #22 | MERGED | Eliminar tras CI verde en `main` |
| `feat/android-saf-internal-backup` | #21 | MERGED | Eliminar |
| `chore/integration-android-ux` | #20 | MERGED | Eliminar |
| `chore/local-stabilization-prepilot` | #17 | MERGED | Eliminar |
| `chore/android-local-spike` | #18 | CLOSED | Eliminar tras retirar su worktree |
| `chore/desktop-ux-polish` | #19 | CLOSED | Eliminar |
| `docs/tests-readme` | #1 | CLOSED | Eliminar |
| `fix/ci-security-foundation` | #2 | CLOSED | Eliminar |

Las 14 ramas históricas de #3–#16 ya fueron eliminadas por el propietario
(`fix/inventory-integrity`, `feat/catalog-categories-variants`,
`feat/catalog-modifier-engine`, `feat/cash-registers-shifts`,
`feat/sales-cart-checkout`, `test/g2-5-operational-e2e`,
`test/windows-desktop-e2e`, `test/windows-native-e2e`,
`feat/desktop-shell-redesign`, `fix/migraciones-crlf-windows`,
`feat/local-user-management`, `feat/purchasing-suppliers-receiving`,
`feat/local-expenses-waste`, `feat/local-reports-returns`).

No se usó solo `git branch --merged`: el squash merge no conserva ancestros. La
comprobación fue `git rev-list --count main..<rama>` y `git cherry main <rama>`.

## Plan de eliminación (requiere autorización)

```bash
# Tras confirmar CI verde en main y retirar el worktree del spike
git push origin --delete \
  chore/repo-multiplatform-dev \
  feat/android-saf-internal-backup \
  chore/integration-android-ux \
  chore/local-stabilization-prepilot \
  chore/android-local-spike \
  chore/desktop-ux-polish \
  docs/tests-readme \
  fix/ci-security-foundation
```

Antes de ejecutarlo:

1. `git -C …/hiqnet-pos-android-local-spike status` sin cambios locales.
2. `git worktree remove …/hiqnet-pos-android-local-spike`.
3. CI de `main` en verde para `455718f` o posterior.

## Worktrees

| Ruta | Rama |
| --- | --- |
| `…/hiqnet-pos` | `docs/post-merge-state` (o la rama de trabajo activa) |
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

- Borrar ramas remotas (lista de arriba).
- Configurar protección de `main` (hoy **sin protección**). Propuesta en
  [`branch-protection.md`](branch-protection.md).
- Crear o proteger tags de release (`ruleset-tags.json`).
- Configurar secretos de firma.
