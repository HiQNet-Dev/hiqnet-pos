# ADR-001 — Layout de workspaces Rust y Tauri

- Estado: aceptado
- Fecha: 2026-11-03
- Contexto: fundación greenfield 0.1.0

## Decisión

- Los crates de negocio se agrupan en el workspace `backend-rust/Cargo.toml`.
- La aplicación Tauri (`apps/pos-desktop/src-tauri`) declara **su propio**
  workspace de Cargo y depende de los crates de negocio por ruta.
- El frontend React es un paquete pnpm independiente en `frontend/`.

## Motivo

Cargo no permite que un miembro externo al directorio del workspace herede
`workspace.package` ni las dependencias del workspace, y Tauri necesita su
propio `build.rs` y configuración de bundling. Mantener dos workspaces evita
duplicar metadatos y conserva la estructura solicitada (`apps/pos-desktop`).

## Consecuencias

- `cargo test --workspace` en `backend-rust` no compila Tauri (verificación
  rápida en CI).
- CI compila la app Tauri en un job separado de Windows.
- Los `Cargo.lock` son independientes: `backend-rust/Cargo.lock` y
  `apps/pos-desktop/src-tauri/Cargo.lock`.
- Cualquier crate nuevo de negocio entra en `backend-rust/crates/` y se publica
  al workspace de escritorio mediante dependencias `path`.

## Alternativas descartadas

- Workspace único en la raíz: obliga a mover `apps/` bajo `backend-rust/`.
- Duplicar el código de negocio dentro de `src-tauri`: viola la separación de
  capas.
