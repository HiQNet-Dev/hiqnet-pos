# HiQNet POS — comandos de desarrollo multiplataforma.
#
# `just` es opcional: cada receta equivale a los comandos documentados en
# README.md y docs/02-development/. No oculta fallos ni sustituye a Cargo o pnpm.
#
# Instalación: `winget install Casey.Just` (Windows),
#              `sudo pacman -S just` (Arch/Omarchy),
#              `brew install just` (macOS).

default:
    @just --list

# Verifica el toolchain y prepara las dependencias del frontend.
setup:
    rustup show active-toolchain
    cargo --version
    node --version
    pnpm --version
    pnpm --dir frontend install --frozen-lockfile
    @echo "Entorno listo. Siguiente paso: just dev"

# Aplica formato al backend.
fmt:
    cargo fmt --manifest-path backend-rust/Cargo.toml --all

# Verifica formato y lints del backend.
lint-rust:
    cargo fmt --manifest-path backend-rust/Cargo.toml --all -- --check
    cargo clippy --manifest-path backend-rust/Cargo.toml --workspace --all-targets -- -D warnings

# Lints y tipos del frontend.
lint-frontend:
    pnpm --dir frontend run lint
    pnpm --dir frontend run typecheck

# Pruebas del backend (SQLite real para persistencia y transacciones).
test-rust:
    cargo test --manifest-path backend-rust/Cargo.toml --workspace

# Pruebas unitarias del frontend.
test-frontend:
    pnpm --dir frontend run test

# Pruebas E2E del frontend con Playwright.
test-e2e:
    pnpm --dir frontend run test:e2e

test: test-rust test-frontend

# Validación completa previa a un PR.
check: lint-rust lint-frontend test
    pnpm --dir frontend run build

# Ejecuta el POS en modo desarrollo.
dev:
    cd apps/pos-desktop/src-tauri && pnpm dlx @tauri-apps/cli@2 dev

# Compila el escritorio sin empaquetar, útil como verificación rápida.
build-desktop:
    cd apps/pos-desktop/src-tauri && pnpm dlx @tauri-apps/cli@2 build --debug --no-bundle

# Construye el instalador NSIS. Requiere un host Windows.
build-windows:
    @echo "{{ if os() == "windows" { "Host Windows detectado." } else { error("build-windows requiere un host Windows; host actual: " + os()) } }}"
    cd apps/pos-desktop/src-tauri && pnpm dlx @tauri-apps/cli@2 build

# Construye deb y AppImage. Requiere un host Linux con WebKitGTK 4.1.
build-linux:
    @echo "{{ if os() == "linux" { "Host Linux detectado." } else { error("build-linux requiere un host Linux; host actual: " + os()) } }}"
    cd apps/pos-desktop/src-tauri && pnpm dlx @tauri-apps/cli@2 build

# Construye la APK ARM64. Requiere SDK/NDK y el target de Rust.
build-android:
    @echo "{{ if env_var_or_default("ANDROID_HOME", env_var_or_default("ANDROID_SDK_ROOT", "")) == "" { error("Define ANDROID_HOME (o ANDROID_SDK_ROOT) antes de build-android.") } else { "SDK Android detectado." } }}"
    cd apps/pos-desktop/src-tauri && pnpm dlx @tauri-apps/cli@2 android build --target aarch64 --apk

# Verifica que el árbol no contiene bases, secretos ni artefactos de firma.
audit-repo:
    bash tools/check-no-secrets.sh
