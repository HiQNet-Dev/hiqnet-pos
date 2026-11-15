# Corte de beta local: validación

Este corte configura checks del DevKit para el POS local. No habilita distribución,
firma de código, emisión de licencias ni integración productiva con Platform.

## Entorno y ejecución

Desde la raíz de `hiqnet-pos`, usar Node **24.18.0** (`.node-version`),
pnpm **11.24.0** (`frontend/package.json`) y Rust **1.99.1**
(`rust-toolchain.toml`). Con `mise`, por ejemplo:

```sh
mise exec node@24.18.0 pnpm@11.24.0 -- node tools/check-toolchain.mjs
mise exec node@24.18.0 pnpm@11.24.0 -- devkit doctor
mise exec node@24.18.0 pnpm@11.24.0 -- devkit gates check local --yes
```

Las dependencias de `frontend` deben estar instaladas desde el lockfile antes
de los checks (`pnpm --dir frontend install --frozen-lockfile`, si faltan).
`devkit gates check local --yes` ejecuta verificación de toolchain, formato
Rust, lint, tipos y pruebas unitarias de frontend; el gate `pre-push` declara
solamente toolchain, lint y tipos. El hook **no se instala** por esta
configuración; si se instala por separado, no ejecuta pruebas E2E, nativas ni
dependientes de estado externo. `pre-push` es un hook, no un gate canónico
invocable mediante `devkit gates check pre-push`; para comprobarlo sin instalar
hooks, ejecutar sus tres comandos declarados por separado:

```sh
node tools/check-toolchain.mjs
pnpm --dir frontend run lint
pnpm --dir frontend run typecheck
```

No se declara ningún comando que cargue `.env` o arranque la aplicación.

Para validar antes de un PR, ejecutar manualmente además:

```sh
cargo clippy --manifest-path backend-rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path backend-rust/Cargo.toml --workspace
pnpm --dir frontend run build
```

Estos checks de backend y build quedan fuera del pre-push por tiempo y
precondiciones del host. `pnpm --dir frontend run test:e2e` requiere el entorno
de Playwright; la suite nativa Tauri requiere Windows, WebView2 y el arnés
descrito en `tools/windows-native-e2e/README.md`. No se incluyen en gates
automáticos ni se presumen validados. Consultar
`docs/04-quality/platform-matrix.md` antes de afirmar validación Linux/Android.

La beta local no valida instaladores firmados, actualización en puestos reales
ni emisión productiva de licencias. Ver `internal-pilot-windows.md` y
`operational-validation.md` para evidencia histórica y restricciones.

Los respaldos SQLite no están cifrados. En estas pruebas usa únicamente datos
sintéticos y no compartas los archivos generados; cifrado, custodia y recuperación
de respaldos reales siguen pendientes antes de cualquier piloto con clientes.

## Restauración por CLI: habilitada con política estricta

La CLI exige una base actual verificable (existe, abre y pasa `quick_check`) con
la misma instalación y organización; rechaza material de licencia firmado y
conserva el estado comercial vigente y el respaldo fuente. Mantiene una sola
exclusión desde la validación hasta la reposición comercial y guarda rutas
absolutas para reanudar tras una caída.

**Verificado en Linux:** 13 pruebas de restauración y reanudación, además de dos
pruebas del binario (rechazo de base ausente/corrupta y `--verify-backup`).
**Pendiente:** reejecutar las suites nativas de Windows
(`restore_multiphase_check.py`, `e2e_restore_corruption.py`,
`e2e_restore_guard.py`, `perf_volume.py`) sobre el ejecutable actual. No se
declaran PASS en Windows hasta esa corrida, y la conservación de 27 tablas por
hash sigue pendiente de revalidación.

## Enrolamiento con Platform (beta local)

Configura `HIQNET_PLATFORM_URL` con la URL base de Platform (solo HTTPS; en
loopback se admite `http://`). Sin esa variable la app opera 100 % offline y los
comandos de enrolamiento devuelven `enrollment_unavailable`.

1. La clave privada Ed25519 se genera una vez y se guarda en
   `device-identity.key` junto a la base, con permisos `0600` en Unix. No entra
   en SQLite ni en los respaldos. En Windows hereda las ACL del perfil; DPAPI
   queda pendiente.
2. En *Gestión → Estado comercial → Vincular con HiQNet Platform*, genera el
   código y apruébalo en el Portal con la cuenta propietaria de la organización.
3. Al completar, Platform devuelve la organización comercial y el binding. Esto
   **no concede derechos**: la licencia firmada sigue siendo obligatoria.
4. La etiqueta del dispositivo se deriva del `installation_id`
   (`HiQNet POS <8 caracteres>`): Platform exige unicidad por organización y una
   etiqueta fija impedía enrolar un segundo equipo.
5. La clave se verifica al leer: debe ser un archivo regular, sin symlink y con
   permisos sin bits para grupo/otros en Unix. Si está mal configurada, se
   rechaza en lugar de confiar en ella.
6. La URL rechaza credenciales embebidas (`usuario:clave@host`) y solo persiste
   el host en auditoría. El `device_code` se valida antes de construir la ruta.
7. Evidencia: contrato HTTP local, permisos de clave y E2E real repetido
   `begin → aprobación del owner → proof Ed25519` contra Platform en loopback.

No se declara enrolamiento productivo: falta custodia de clave (DPAPI en
Windows), rotación, revocación, permisos `0600` de la base y despliegue con TLS
real. La revisión independiente quedó en PASS condicionado a esos puntos.
