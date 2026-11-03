# Plan de integración — G2.1 Inventory y G2.2A Catálogo

Estado: **pendiente de autorización expresa**. No se ha fusionado ningún PR.

## PR verificados

| PR | Rama | SHA | Base | CI |
| --- | --- | --- | --- | --- |
| #3 | `fix/inventory-integrity` | `a72ba75` | `main` | 4/4 PASS |
| #4 | `feat/catalog-categories-variants` | `e7bc5c6` | `fix/inventory-integrity` | 4/4 PASS |

## Secuencia segura

1. Obtener autorización para integrar **PR #3**.
2. Fusionar PR #3 en `main` con merge commit (conserva historia) o squash
   (historia lineal). No usar rebase sobre `main` ya publicado.
3. No eliminar `fix/inventory-integrity` hasta actualizar PR #4.
4. En la rama de catálogo, integrar el nuevo `main`:
   - `git fetch origin`
   - `git checkout feat/catalog-categories-variants`
   - `git merge origin/main` (preferido; conserva SHA de catálogo) o
     `git rebase origin/main` si se acepta reescritura de rama no fusionada.
5. Cambiar la base del PR #4 a `main` con GitHub.
6. Verificar que el diff del PR #4 contenga solo catálogo.
7. Ejecutar los cuatro jobs de CI sobre el nuevo SHA.
8. Obtener autorización y fusionar PR #4.

## Cambios por PR

- **PR #3**: inventario, migraciones `0002/0003`, IPC de inventario, UI de
  inventario, correcciones de seguridad/CI (`audit-check`, `supply-chain`).
- **PR #4**: categorías, variantes, migración `0004`, IPC de catálogo, UI de
  catálogo, pruebas de catálogo y ajuste de `ProductDraft.category_id`.
- Los cambios en `tests/inventory.rs` y `persistence.rs` del PR #4 son
  consecuencias del contrato `ProductDraft.category_id` y de las pruebas de
  catálogo, no divergencias de inventario.
- `.github/workflows/ci.yml` no aparece en el diff de PR #4: la corrección de CI
  pertenece exclusivamente a PR #3.

## Riesgos

- `RUSTSEC-2023-0071` se ignora de forma específica en CI y en
  `backend-rust/.cargo/audit.toml`; revisar si cambia la cadena de sqlx/rsa.
- E2E real del ejecutable Tauri Windows sigue pendiente (WebDriver).
- El inventario no distingue existencias por variante; decisión pendiente.
