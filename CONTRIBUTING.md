# Contribuir a HiQNet POS

## Ramas

Trunk-based con ramas breves y PR hacia `main`:

- `feat/*`, `fix/*`, `docs/*`, `refactor/*`, `test/*`, `chore/*`, `security/*`.

No se crean ramas permanentes por edición (Local/Pro/Enterprise) ni una rama
`dev` de larga duración sin justificación.

## Flujo

1. Actualiza `main` sin sobrescribir trabajo ajeno.
2. Crea una rama de alcance pequeño.
3. Implementa y añade pruebas.
4. Ejecuta las validaciones locales.
5. Revisa el diff y confirma que no hay secretos ni bases de datos.
6. Commit con Conventional Commits en español.
7. Push y Pull Request hacia `main`.
8. Squash merge cuando CI esté verde.

## Commits

```text
feat: registrar productos en el catálogo local
fix: impedir mutaciones con licencia vencida
test: cubrir restauración de respaldo
docs: documentar contratos IPC
ci: validar formato del frontend
```

## Definición de terminado

Una funcionalidad requiere: invariantes identificadas, prueba de dominio,
prueba de integración SQLite, autorización, auditoría cuando aplique, contrato
IPC actualizado, UI usable, errores en español y documentación.

## Entorno

El procedimiento soportado está en
[`docs/02-development/`](docs/02-development/README.md):

- Rust según `rust-toolchain.toml` (no instales `stable` a mano).
- Node según `.node-version` y pnpm según `packageManager`.
- Windows requiere WebView2 y Build Tools C++; Arch Linux requiere
  `webkit2gtk-4.1` y GTK 3.

`just setup` deja el checkout listo. El inventario de ramas y las operaciones de
mantenimiento viven en
[`docs/07-maintenance/branch-inventory.md`](docs/07-maintenance/branch-inventory.md).

## Validaciones locales

```bash
just check          # formato, lints, tipos, pruebas y build
just test-rust      # cargo test --workspace
just test-frontend  # vitest
```

Equivalente manual:

```bash
cargo fmt --manifest-path backend-rust/Cargo.toml --all -- --check
cargo clippy --manifest-path backend-rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path backend-rust/Cargo.toml --workspace

pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend run lint
pnpm --dir frontend run typecheck
pnpm --dir frontend run test
pnpm --dir frontend run build
```

Para la app de escritorio:

```bash
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2.12.4 build --debug --no-bundle
```

## Portabilidad

- El código de negocio no conoce rutas del sistema operativo. La base se
  resuelve en `pos-storage-sqlite::paths`.
- Un script nuevo debe funcionar en Windows y Linux o declarar su host y
  abortar con un diagnóstico (ver `justfile`).
- No fijes `stable` en CI: la versión vive en `rust-toolchain.toml`.
- No versiones APK/AAB, keystores, `.jks`, bases ni respaldos.
- Las migraciones SQL conservan LF; no cambies sus finales de línea.

## Datos

Nunca agregues bases reales, `.env`, respaldos ni fixtures con datos de clientes.
Usa datos sintéticos.

## Licencia

La licencia del código está pendiente. Al contribuir, no asumas permisos de
reutilización que el repositorio no concede.
