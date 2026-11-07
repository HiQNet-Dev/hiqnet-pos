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

## Esquema SQLite

- Las migraciones son incrementales (`0001`…`0010`) y **nunca** se editan una vez
  integradas en `main`.
- Al abrir, la app ejecuta `PRAGMA quick_check` y rechaza bases cuya migración
  máxima sea posterior a la conocida por el ejecutable
  (`StorageError::IncompatibleSchema`).
- La restauración rechaza respaldos de esquema posterior.
- Revertir el binario **no** revierte la base: para volver atrás hay que restaurar
  un respaldo verificado con la utilidad offline.

## Canales de actualización Tauri 2

El plugin `tauri-plugin-updater` está integrado pero **deshabilitado** (endpoints
vacíos y sin clave pública) hasta que exista infraestructura de publicación
firmada. Canales previstos: `alpha`, `beta`, `stable`.

No se publica una release estable ni se distribuye un instalador a clientes sin
aprobar firma, custodia de claves y política de actualización.

## Instalador Windows (prepiloto)

```powershell
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2.12.4 build --debug
```

Genera `target/debug/bundle/nsis/HiQNet POS_0.1.0_x64-setup.exe`.

- **Sin firma** (`NotSigned`): solo prueba interna; Windows puede mostrar avisos.
- No crea servicios ni tareas programadas.
- Instala por usuario y conserva `%LOCALAPPDATA%\HiQNet\POS\data` entre
  actualizaciones y reinstalaciones.
- El actualizador automático no está activo.

## Garantías de actualización

- La instalación no borra SQLite ni reinicia el Trial.
- Las migraciones se aplican de forma incremental y reversible por respaldo.
- No se actualiza durante una operación comercial en curso.
- Antes de actualizar: respaldo verificado y, si es crítico, copia externa.

## Releases

- Etiquetas: `pos-vX.Y.Z` (POS), `platform-vX.Y.Z` (Platform).
- Prerelease si la etiqueta contiene `-`.
- Artefactos con SHA-256 y retención 30 días.
- Publicación bloqueada si falla validación o seguridad.
