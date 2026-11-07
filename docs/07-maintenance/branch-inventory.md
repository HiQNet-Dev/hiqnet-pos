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
| `chore/repo-multiplatform-dev` | #22 | `feat/android-saf-internal-backup` | 9 | Saneamiento y soporte Linux |

Total: **23 ramas remotas** contando `main`.

Verificación de contenido:

- `chore/desktop-ux-polish` es ancestro de `chore/integration-android-ux`.
- El commit único de `chore/android-local-spike` (`8babf94`) fue reaplicado en
  `chore/integration-android-ux` (`c860b2b`). Verificación por `patch-id`
  excluyendo `apps/pos-desktop/src-tauri/src/lib.rs`: **parches idénticos**
  (`69807f05…`). En ese archivo la integración es superconjunto (añade
  `preview_product_price`). **#18 está reemplazado por #20.**
- `chore/integration-android-ux` es ancestro de
  `feat/android-saf-internal-backup`.
- `feat/android-saf-internal-backup` es ancestro de
  `chore/repo-multiplatform-dev`.

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

## Simulación de integración (local, no publicada)

Rama temporal `integration-sim-local` creada desde `main` (`0b57cbd`) y fusionada
en este orden:

| Paso | Rama | Resultado |
| --- | --- | --- |
| 1 | `chore/local-stabilization-prepilot` (#17) | Fast-forward |
| 2 | `chore/integration-android-ux` (#20) | Fast-forward (incluye #19) |
| 3 | `feat/android-saf-internal-backup` (#21) | Fast-forward |
| 4 | `chore/repo-multiplatform-dev` (#22) | Fast-forward |

Sin conflictos. El árbol resultante es **idéntico** a
`chore/repo-multiplatform-dev` (`git diff --stat` vacío): 11 migraciones,
precios públicos, ventas/devoluciones, SAF, firma externa, rutas Windows/Linux,
CI Linux y documentación presentes. Validaciones sobre el árbol integrado:
`cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `pnpm lint`,
`typecheck`, `vitest` (120/120) y `build`: todo verde.

La rama temporal no se publica. `git branch -D integration-sim-local` es seguro
cuando el propietario lo autorice.

## Orden de integración recomendado

Cada flecha representa un merge con CI verde; no usar squash para conservar la
cadena verificada.

1. **#17** → `main`.
2. **#20** → `main` (incluye #18 y #19).
3. **#21** → `main`.
4. **#22** → `main` (retargetear al fusionar #21).
5. Cerrar **#1**, **#2**, **#18** y **#19** como obsoletos/reemplazados.
6. Eliminar ramas según la lista de abajo.

Alternativa si se prefiere fusionar cada PR por separado: #17 → #19 → #20 →
#21 → #22. **Nunca #18 antes de #20**: duplicaría una implementación divergente.

Si se usa squash en cada PR, la simulación sigue siendo válida porque se
validaron los árboles, no solo los ancestros.

## Plan de eliminación (requiere autorización)

Borrar solo después de fusionar #17–#22 y cerrar los PR obsoletos:

```bash
# Tras el merge de #22 y con CI verde
git push origin --delete \
  fix/inventory-integrity feat/catalog-categories-variants \
  feat/catalog-modifier-engine feat/cash-registers-shifts \
  feat/sales-cart-checkout test/g2-5-operational-e2e \
  test/windows-desktop-e2e test/windows-native-e2e \
  feat/desktop-shell-redesign fix/migraciones-crlf-windows \
  feat/local-user-management feat/purchasing-suppliers-receiving \
  feat/local-expenses-waste feat/local-reports-returns \
  docs/tests-readme fix/ci-security-foundation \
  chore/android-local-spike chore/desktop-ux-polish \
  chore/local-stabilization-prepilot chore/integration-android-ux \
  feat/android-saf-internal-backup chore/repo-multiplatform-dev
```

Antes de ejecutarlo: retirar el worktree
`…/hiqnet-pos-android-local-spike` y confirmar que no tiene cambios locales
(`git -C …/hiqnet-pos-android-local-spike status`). No borrar ninguna rama de
#17–#22 antes de completar los merges.

## Worktrees

| Ruta | Rama |
| --- | --- |
| `…/hiqnet-pos` | `chore/repo-multiplatform-dev` |
| `…/hiqnet-pos-android-local-spike` | `chore/android-local-spike` |

El worktree del spike debe retirarse antes de eliminar esa rama. La rama local
`integration-sim-local` (sin remoto) solo contiene la simulación y puede
borrarse tras la entrega.

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
  bloqueo de force push). Propuesta lista en
  [`branch-protection.md`](branch-protection.md).
- Crear o proteger tags de release (`ruleset-tags.json`).
- Configurar secretos de firma.
