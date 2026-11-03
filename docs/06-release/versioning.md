# Versionado y releases

## SemVer

HiQNet POS usa Semantic Versioning: `MAJOR.MINOR.PATCH` con pre-releases
`-alpha.N`, `-beta.N`, `-rc.N`.

- Una sola aplicación POS cubre Trial/Local/Pro/Enterprise mediante entitlements.
  No hay releases distintos por plan.
- `hiqnet-pos` y `hiqnet-platform` mantienen numeración independiente.

## Fuente de versión

- Rust: `backend-rust/Cargo.toml` (`workspace.package.version`).
- Tauri: `apps/pos-desktop/src-tauri/Cargo.toml` (debe coincidir).
- Frontend: `frontend/package.json`.
- `tauri.conf.json` (`version`).

La sincronización automatizada se añadirá cuando se introduzca el primer release
candidate; por ahora se verifica manualmente en CI.

## Canales de actualización Tauri 2

El plugin `tauri-plugin-updater` está integrado pero deshabilitado (endpoints
vacíos) hasta que exista infraestructura de publicación firmada. Canales
previstos: `alpha`, `beta`, `stable`.

## Garantías de actualización

- La instalación no borra SQLite ni reinicia Trial.
- Las migraciones se aplican de forma incremental y reversible por respaldo.
- No se actualiza durante una operación comercial en curso.
- Revertir binario no revierte la base de datos.

## Releases

- Etiquetas: `pos-vX.Y.Z` (POS), `platform-vX.Y.Z` (Platform).
- Prerelease si la etiqueta contiene `-`.
- Artefactos con SHA-256 y retención 30 días.
- Publicación bloqueada si falla validación o seguridad.
EOF