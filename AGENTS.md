# AGENTS.md — HiQNet POS

Reglas para agentes automatizados que trabajan en este repositorio. Complementan
`README.md`, `CONTRIBUTING.md` y `docs/01-architecture/overview.md`.

## Reglas no negociables

1. Rust es la autoridad de negocio. No implementes reglas en React ni en los
   comandos IPC.
2. No coloques SQL en `frontend/`, `apps/pos-desktop/` ni en `pos-application`.
3. Toda mutación auditable usa **una** `StoreTx` con auditoría en la misma
   transacción.
4. No aceptes `organization_id`, `branch_id` ni permisos desde el frontend: se
   derivan del `Actor` autenticado.
5. No publiques bases SQLite, `.env`, respaldos, dumps ni claves.
6. No commitees ni hagas push a `main` sin PR y CI verde.
7. No inventes precios, límites de plan, plazos de gracia ni resultados de
   pruebas.

## Capas

```text
pos-core <- pos-application <- pos-storage-sqlite
                 ^                ^
           pos-security      apps/pos-desktop
           pos-licensing
```

- `pos-core`: dominio puro; sin SQLx, Tauri, Tokio ni configuración.
- `pos-application`: casos de uso, puertos y RBAC.
- `pos-storage-sqlite`: adaptador SQLite, migraciones y respaldo.
- `pos-security`: Argon2id.
- `pos-licensing`: verificación Ed25519 y contrato de licencia.
- `apps/pos-desktop`: composición e IPC.

## Flujo de trabajo

1. Inspecciona el código y las pruebas relevantes.
2. Elige el cambio más pequeño coherente.
3. Añade o ajusta pruebas en el nivel más bajo suficiente.
4. Ejecuta: `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`,
   `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`.
5. Si tocas la app de escritorio, compila con
   `cargo tauri build --debug --no-bundle`.
6. Revisa el diff y los archivos prohibidos antes de commitear.

## Contratos

- Comandos IPC: `docs/03-contracts/ipc.md` + `frontend/src/contracts.ts` deben
  cambiar en el mismo PR que Rust.
- Migraciones: incrementales, versionadas y reproducibles; nunca edites una
  migración ya publicada. Añade una nueva.
- Errores de usuario en español; identificadores técnicos en inglés.

## Pruebas

- Dominio: unitarias en `pos-core`.
- Persistencia y transacciones: integración con SQLite real en
  `pos-storage-sqlite/tests`.
- UI: Vitest + Testing Library.
- E2E Tauri: pendiente de infraestructura WebDriver; no lo declares ejecutado.

## Seguridad

- Deny-by-default en RBAC y entitlements.
- Consultas parametrizadas.
- No registres contraseñas, tokens ni datos personales.
- Mantén las capabilities de Tauri al mínimo.

## Documentación

- ADR en español para decisiones arquitectónicas significativas.
- No copies documentación interna de Drive al repositorio público.
- Distingue siempre entre verificado, parcial, pendiente y bloqueado.
