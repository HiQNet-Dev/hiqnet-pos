# Entorno de desarrollo

Procedimiento soportado para preparar un checkout limpio y trabajar en HiQNet
POS desde Windows o Arch Linux/Omarchy sin reproducir configuraciones locales.

- [Windows](windows.md)
- [Arch Linux / Omarchy](linux-arch.md)
- [Android](android.md)

## Versiones fijadas

| Herramienta | Fuente de verdad | Valor actual |
| --- | --- | --- |
| Rust | `rust-toolchain.toml` | 1.99.1 (rustfmt y clippy) |
| Node.js | `.node-version` | 24.18.0 (mínimo declarado: 22) |
| pnpm | `frontend/package.json` → `packageManager` | 11.24.0 |
| Tauri CLI | `justfile` y CI | 2.12.4 (fijado, sin `@2`) |

CI instala exactamente el canal de `rust-toolchain.toml`; no usa `stable` a
secas. Si cambias una versión, actualiza su fuente de verdad y no un workflow.

## Comandos

`just` es opcional. Cada receta equivale a los comandos manuales de esta tabla.

| Objetivo | Receta | Equivalente manual |
| --- | --- | --- |
| Preparar dependencias | `just setup` | `pnpm --dir frontend install --frozen-lockfile` |
| Ejecutar el POS | `just dev` | `cd apps/pos-desktop/src-tauri && pnpm dlx @tauri-apps/cli@2.12.4 dev` |
| Validar todo | `just check` | `just lint-rust lint-frontend test` + build del frontend |
| Pruebas backend | `just test-rust` | `cargo test --manifest-path backend-rust/Cargo.toml --workspace` |
| Pruebas frontend | `just test-frontend` | `pnpm --dir frontend run test` |
| E2E del frontend | `just test-e2e` | `pnpm --dir frontend run test:e2e` |
| Build de verificación | `just build-desktop` | `pnpm dlx @tauri-apps/cli@2.12.4 build --debug --no-bundle` |
| Instalador Windows | `just build-windows` | `pnpm dlx @tauri-apps/cli@2.12.4 build` en un host Windows |
| Paquetes Linux | `just build-linux` | `pnpm dlx @tauri-apps/cli@2.12.4 build` en un host Linux |
| APK ARM64 | `just build-android` | `pnpm dlx @tauri-apps/cli@2.12.4 android build --target aarch64 --apk` |
| Auditoría de artefactos | `just audit-repo` | `bash tools/check-no-secrets.sh` |

`just build-windows` en Linux y `just build-linux` en Windows abortan con un
diagnóstico explícito: no se generan paquetes cruzados sin una estrategia
probada.

## Rutas de la base

| Plataforma | Ruta por defecto |
| --- | --- |
| Windows | `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` (sin cambios) |
| Linux | `$XDG_DATA_HOME/hiqnet-pos/data/hiqnet-pos.db` o `~/.local/share/hiqnet-pos/data/hiqnet-pos.db` |
| macOS (no soportado aún) | `~/Library/Application Support/hiqnet-pos/data/hiqnet-pos.db` |
| Android | almacenamiento privado de la app (`app_data_dir`) |

`HIQNET_POS_DB_PATH` aísla la base en desarrollo y QA. La aplicación y la CLI de
respaldo usan la misma resolución. Si no se puede determinar el directorio
persistente, el arranque falla con un diagnóstico en lugar de escribir en el
temporal. No muevas una base real sin respaldo previo.

## Artefactos

No se versionan bases SQLite, respaldos, APK/AAB, ejecutables, keystores,
`.jks`, `.keystore` ni credenciales de firma. `tools/check-no-secrets.sh` lo
verifica en local y en CI.
