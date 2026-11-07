# Desarrollo en Windows

Soportado: Windows 10/11 x64 con WebView2 (incluido en Windows 11).

## Prerrequisitos

1. **Rust** con [rustup](https://rustup.rs). El repositorio fija el canal en
   `rust-toolchain.toml`; no instales `stable` a mano.
2. **Visual Studio Build Tools 2022** con la carga de trabajo *Desarrollo para
   el escritorio con C++* (toolchain MSVC y Windows SDK).
3. **WebView2 Runtime**. En Windows 11 viene instalado; en Windows 10 puede
   requerir el bootstrapper oficial.
4. **Node.js 24** (ver `.node-version`). Con fnm/mise/nvm:
   `fnm use` o `nvm use`.
5. **pnpm 11.24.0**: `corepack enable` o `npm install -g pnpm@11.24.0`.
6. Opcional: **just** (`winget install Casey.Just`).
7. Solo para Android: Android Studio, SDK, NDK `30.0.16248370` y el target de
   Rust `aarch64-linux-android`. Ver [android.md](android.md).

## Puesta en marcha

```powershell
git clone https://github.com/HiQNet-Dev/hiqnet-pos.git
cd hiqnet-pos
just setup
just check
just dev
```

Sin `just`:

```powershell
pnpm --dir frontend install --frozen-lockfile
cargo test --manifest-path backend-rust/Cargo.toml --workspace
cargo clippy --manifest-path backend-rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path backend-rust/Cargo.toml --all -- --check
pnpm --dir frontend run lint
pnpm --dir frontend run typecheck
pnpm --dir frontend run test
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2 dev
```

## Datos locales

`%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db`. Para aislar pruebas:

```powershell
$env:HIQNET_POS_DB_PATH = "$env:TEMP\hiqnet-pos-dev.db"
```

El `.cargo/config.toml` de la app enlaza el runtime C/C++ de forma estática para
que el ejecutable no dependa de `VCRUNTIME140.dll` en imágenes limpias.

## Empaquetado

```powershell
just build-windows
# apps/pos-desktop/src-tauri/target/release/bundle/nsis/
```

El instalador no está firmado y es solo para pruebas internas.

## E2E nativo

Requiere `tauri-driver`, Microsoft Edge WebDriver de la misma versión que
WebView2 y una sesión gráfica. No corre en GitHub Actions:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File tools/windows-native-e2e/run-e2e.ps1 -Script e2e_flow.py
```

Detalles en [`README-TESTING.md`](../../README-TESTING.md) y
[`tools/windows-native-e2e/README.md`](../../tools/windows-native-e2e/README.md).

Windows Sandbox sigue siendo útil para validar el instalador; no aplica a Linux.
