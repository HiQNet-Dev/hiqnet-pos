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
| SQLite local con WAL y migraciones SQLx (ruta por plataforma) | Implementado y verificado |
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
docs/02-development/               bootstrap Windows, Linux y Android
docs/                              arquitectura, contratos, ADR y operación
tools/                             scripts de desarrollo y E2E
.github/                           CI, plantillas y controles
```

La app Tauri vive en su propio workspace de Cargo porque Tauri compila con su
propio `build.rs`; los crates de negocio comparten un workspace distinto. Ver
`docs/01-architecture/decisions/ADR-001-workspace-layout.md`.

## Requisitos

Comunes:

- Rust 1.99.1 (fijado en `rust-toolchain.toml`).
- Node.js 24 (`.node-version`; mínimo soportado 22) y pnpm 11.24.0.
- Opcional: `just` para los comandos de `justfile`.

Windows 10/11:

- WebView2 (incluido en Windows 11).
- Visual Studio Build Tools con la carga de trabajo C++ para el toolchain MSVC.

Arch Linux / Omarchy:

- `webkit2gtk-4.1`, `gtk3`, `librsvg`, `libappindicator-gtk3`,
  `appmenu-gtk-module`, `openssl` y `base-devel`.

Android (opcional): SDK, NDK `30.0.16248370`, JDK 17 y el target
`aarch64-linux-android`.

Guías completas: [`docs/02-development/`](docs/02-development/README.md).

## Puesta en marcha

Con `just`:

```bash
just setup   # toolchain y dependencias del frontend
just check   # formato, lints, tipos, pruebas y build
just dev     # aplicación de escritorio en desarrollo
```

Equivalente sin `just`:

```bash
pnpm --dir frontend install --frozen-lockfile

cargo fmt --manifest-path backend-rust/Cargo.toml --all -- --check
cargo clippy --manifest-path backend-rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path backend-rust/Cargo.toml --workspace

pnpm --dir frontend run lint
pnpm --dir frontend run typecheck
pnpm --dir frontend run test
pnpm --dir frontend run build

cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2 dev
```

La base local se crea fuera del repositorio:

| Plataforma | Ruta |
| --- | --- |
| Windows | `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` |
| Linux | `$XDG_DATA_HOME/hiqnet-pos/data/hiqnet-pos.db` o `~/.local/share/hiqnet-pos/data/hiqnet-pos.db` |
| Android | almacenamiento privado de la aplicación |

`HIQNET_POS_DB_PATH` permite aislar la base en desarrollo y pruebas; la
aplicación y la CLI de respaldo usan la misma resolución. **Nunca** se versiona
el archivo de base de datos.

## Instalación piloto (interna)

Windows:

```powershell
just build-windows
# Artefacto: target/release/bundle/nsis/HiQNet POS_0.1.0_x64-setup.exe
```

Linux (host Linux con WebKitGTK 4.1):

```bash
just build-linux
# Artefactos: target/release/bundle/deb/ y appimage/
```

El instalador Windows generado **no está firmado** y no debe distribuirse
públicamente. El actualizador permanece deshabilitado. Consulta la lista de
comprobación en
[`docs/05-operations/pilot-checklist.md`](docs/05-operations/pilot-checklist.md) y
la guía de instalación, rutas y actualización en
[`docs/05-operations/windows-installation.md`](docs/05-operations/windows-installation.md).

## Respaldo y restauración offline

Desde la aplicación (Owner): crear respaldo genera un archivo autocontenido con
`VACUUM INTO` y lo verifica antes de ofrecerlo. La restauración se ejecuta con la
aplicación **cerrada**, mediante la utilidad incluida en el mismo ejecutable:

```bash
# Verificar sin abrir la interfaz
pos-desktop --verify-backup <respaldo.db>

# Restaurar (la base destino se resuelve por plataforma)
pos-desktop --restore-backup <respaldo.db>

# Forzar una base concreta
pos-desktop --restore-backup <respaldo.db> --database <hiqnet-pos.db>
```

En Windows el binario es `pos-desktop.exe`; en Linux,
`target/release/pos-desktop`. `--help-backup` muestra la ruta efectiva.

La restauración verifica integridad, esquema y claves foráneas, rechaza bases de
una versión posterior y cualquier conexión abierta (aplicación o herramienta
externa), y crea una copia de recuperación `*.pre-restore-<epoch>.db` antes de
reemplazar. Si la promoción o su verificación fallan, revierte a esa copia; si la
reversión también falla, informa un error crítico conservando los artefactos. Ver
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
