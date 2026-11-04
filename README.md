# HiQNet POS

Aplicación de punto de venta **local-first** para pequeños y medianos negocios de
México. Rust es la autoridad de negocio, React presenta la interfaz y SQLite
guarda los datos operativos en el equipo del comercio.

> Estado: **fundación técnica 0.1.0**. Este repositorio es un inicio greenfield;
> no continúa el código ni las migraciones del repositorio histórico
> [`HiQNet-Dev/point-of-sale`](https://github.com/HiQNet-Dev/point-of-sale).

## Qué funciona hoy

| Área | Estado |
| --- | --- |
| Workspace Rust (`pos-core`, `pos-application`, `pos-storage-sqlite`, `pos-licensing`, `pos-security`) | Implementado y verificado con pruebas |
| Dominio `Money` y `Quantity` exactos | Implementado y verificado |
| Bootstrap de organización, sucursal y propietario | Implementado y verificado |
| Identidad local con Argon2id y RBAC deny-by-default | Implementado y verificado |
| Catálogo, variantes y modificadores | Implementado y verificado (inventario por variante pendiente) |
| Inventario con ledger append-only | Implementado y verificado |
| Caja, turnos, arqueo y cierre | Implementado y verificado |
| Ventas en efectivo, ticket y checkout | Implementado y verificado en el ejecutable |
| Compras, proveedores y recepciones | Implementado y verificado |
| Gastos operativos y merma | Implementado y verificado |
| Devoluciones de venta y reportes operativos | Implementado y verificado |
| Auditoría append-only en la misma transacción | Implementado y verificado |
| SQLite en `%LOCALAPPDATA%` con WAL y migraciones SQLx | Implementado y verificado |
| Respaldo verificado y restauración offline segura | Implementado y verificado (rechaza restaurar con la app abierta) |
| Trial de 30 días y modo solo lectura al vencer | Implementado y verificado |
| Verificación de licencias Ed25519 | Contrato y verificador implementados; emisión productiva **pendiente** |
| Instalador NSIS | Construible; **sin firma**, solo prueba interna |
| Actualizador Tauri | **Deshabilitado** (sin endpoints ni firma aprobados) |

La matriz completa, con lo que está propuesto y lo bloqueado, vive en
[`docs/04-quality/feature-matrix.md`](docs/04-quality/feature-matrix.md). El estado
prepiloto y sus límites se detallan en
[`docs/04-quality/prepilot-readiness.md`](docs/04-quality/prepilot-readiness.md).

## Modelo comercial

Una sola aplicación soporta las ediciones **Trial**, **Local**, **Pro** y
**Enterprise**. El plan contratado se verifica en Rust mediante entitlements
firmados; no esconder botones en React no autoriza una operación.

- Trial: 30 días con las capacidades de Local.
- Local: operación local-first sin Internet permanente.
- Pro: Local más sincronización cloud y administración remota (fase posterior).
- Enterprise: Pro más capacidades contractuales (fase posterior).

Los precios, límites numéricos, plazos de gracia (D-04) y proveedor de cobro
(D-07) siguen **pendientes**. No hay un plan gratuito permanente.

## Estructura

```text
backend-rust/
  Cargo.toml                       workspace Rust
  crates/pos-core/                 dominio puro (sin SQLx, sin Tauri)
  crates/pos-application/          casos de uso, puertos, RBAC
  crates/pos-storage-sqlite/       adaptador SQLite, migraciones, respaldo
  crates/pos-licensing/            contrato y verificación Ed25519
  crates/pos-security/             Argon2id
  migrations/local/                migraciones SQLite versionadas
apps/pos-desktop/src-tauri/        entrypoint Tauri 2 e IPC tipado
frontend/                          React + TypeScript + Vite
tests/                             documentación de pruebas E2E
docs/                              arquitectura, contratos, ADR y operación
tools/                             scripts de desarrollo
.github/                           CI, plantillas y controles
```

La app Tauri vive en su propio workspace de Cargo porque Tauri compila con su
propio `build.rs`; los crates de negocio comparten un workspace distinto. Ver
`docs/01-architecture/decisions/ADR-001-workspace-layout.md`.

## Requisitos

- Windows 10/11 con WebView2 (incluido en Windows 11).
- Rust 1.99.1 (fijado en `rust-toolchain.toml`).
- Node.js 22 o superior y pnpm 11.
- Visual Studio Build Tools con la carga de trabajo C++ para el toolchain MSVC.
- Opcional: Docker, solo para HiQNet Platform.

## Puesta en marcha

```bash
# Dependencias del frontend
pnpm --dir frontend install --frozen-lockfile

# Pruebas y validaciones del backend
cargo test --manifest-path backend-rust/Cargo.toml --workspace
cargo clippy --manifest-path backend-rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path backend-rust/Cargo.toml --all -- --check

# Validaciones del frontend
pnpm --dir frontend run lint
pnpm --dir frontend run typecheck
pnpm --dir frontend run test
pnpm --dir frontend run build

# Aplicación de escritorio en desarrollo
cd apps/pos-desktop/src-tauri
cargo tauri dev
```

La base local se crea en `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db`.
Puedes usar otra ruta con `HIQNET_POS_DB_PATH` (útil en desarrollo y pruebas).
**Nunca** se versiona el archivo de base de datos.

## Instalación piloto (interna)

```powershell
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2 build --debug
# Artefacto: target/debug/bundle/nsis/HiQNet POS_0.1.0_x64-setup.exe
```

El instalador generado **no está firmado** y no debe distribuirse públicamente.
El actualizador permanece deshabilitado. Consulta la lista de comprobación en
[`docs/05-operations/pilot-checklist.md`](docs/05-operations/pilot-checklist.md).

## Respaldo y restauración offline

Desde la aplicación (Owner): crear respaldo genera un archivo autocontenido con
`VACUUM INTO` y lo verifica antes de ofrecerlo. La restauración se ejecuta con la
aplicación **cerrada**, mediante la utilidad incluida en el mismo ejecutable:

```powershell
pos-desktop.exe --verify-backup <respaldo.db>
pos-desktop.exe --restore-backup <respaldo.db> --database "%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db"
```

La restauración verifica integridad, esquema y claves foráneas, rechaza bases de
una versión posterior, exige que no haya escrituras activas y crea una copia de
recuperación `*.pre-restore-<epoch>.db` antes de reemplazar. Ver
[`docs/05-operations/backup-and-restore.md`](docs/05-operations/backup-and-restore.md).

La operación es **local y offline**: no se realiza ninguna petición de red para
vender, cobrar, consultar reportes ni respaldar.

## Comandos IPC

Los comandos tipados y sus contratos se documentan en
[`docs/03-contracts/ipc.md`](docs/03-contracts/ipc.md). El frontend valida las
respuestas con Zod para detectar desviaciones entre Rust y React.

## Seguridad

Consulta [`SECURITY.md`](SECURITY.md). Resumen: autorización en Rust, permisos
mínimos de Tauri (`core:default`), CSP restrictiva, contraseñas Argon2id,
consultas parametrizadas y prohibición de publicar bases reales, `.env` o claves.

## Licencia

La licencia del código **no está decidida**. Este repositorio es público, pero
eso no concede permiso de reutilización. Ver
[`docs/00-product/licensing-decision.md`](docs/00-product/licensing-decision.md).

## Documentos rectores

Las decisiones vigentes provienen de los documentos internos 00C, 08-01, 08-02 y
08-03. Este repositorio solo publica resúmenes sanitizados, sin datos
comerciales sensibles.
