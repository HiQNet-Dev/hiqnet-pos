# Matriz de plataformas — 0.1.0

Estados: **VERIFICADO** (ejecutado en esa plataforma), **PARCIAL**,
**PENDIENTE**, **BLOQUEADO**.

No se extrapola un resultado de una plataforma a otra. Un `cargo test` verde en
Linux no valida el WebView de Windows, y una captura de Windows no valida
WebKitGTK.

## Soporte

| Plataforma | Desarrollo | Build | Operación | Nota |
| --- | --- | --- | --- | --- |
| Windows 10/11 x64 (WebView2) | VERIFICADO | VERIFICADO (NSIS) | VERIFICADO | Entorno histórico y de referencia |
| Arch Linux / Omarchy | PARCIAL | PARCIAL (CI verde; falta host Arch) | PENDIENTE | Guía y CI definidos; falta ejecución nativa completa |
| Android ARM64 | PARCIAL | PARCIAL (APK debug) | PARCIAL | SAF y navegación requieren prueba en dispositivo |
| macOS | BLOQUEADO | BLOQUEADO | BLOQUEADO | Sin decisión de producto ni validación |

## Backend y frontend

| Validación | Windows | Linux | Android | CI |
| --- | --- | --- | --- | --- |
| `cargo fmt` / `clippy` / `cargo test --workspace` | VERIFICADO | PENDIENTE (host) | N/A | VERIFICADO (ubuntu) |
| Migraciones SQLite e integridad | VERIFICADO | PENDIENTE (host) | N/A | VERIFICADO (ubuntu) |
| Backend/restore offline | VERIFICADO | PENDIENTE (host) | PARCIAL (SAF) | Cubierto por tests |
| `pnpm lint` / `typecheck` / `vitest` / `build` | VERIFICADO | PENDIENTE (host) | Compartido | VERIFICADO (ubuntu) |
| Playwright (navegador) | VERIFICADO | PENDIENTE (host) | N/A | No ejecutado en Actions |
| E2E nativo Windows | VERIFICADO (manual) | N/A | N/A | No automatizado |
| Compilación Tauri | VERIFICADO | VERIFICADO (CI Ubuntu 24.04) | PARCIAL | `desktop-linux` verde con smoke de CLI |
| APK ARM64 | VERIFICADO (manual) | PENDIENTE (host) | — | Workflow opt-in sin ejecutar |

## Rutas de datos

| Plataforma | Ruta | Estado |
| --- | --- | --- |
| Windows | `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` | Sin cambios; probado |
| Linux | `$XDG_DATA_HOME/hiqnet-pos/data/hiqnet-pos.db` | Resolución con pruebas unitarias |
| Android | `app_data_dir/data/hiqnet-pos.db` | Sin cambios |

## Pendiente para declarar Linux soportado

1. Ejecutar la compilación y el arranque en un host Arch real (la compilación
   equivalente ya está verde en CI sobre Ubuntu 24.04).
2. Arranque nativo en Arch (Wayland y X11) con login, IPC y SQLite.
3. Flujo comercial representativo y backup/restore.
4. Paquete `deb`/AppImage instalado en una máquina limpia.
5. Revisión visual en WebKitGTK, separada de la evidencia de Windows.

Hasta completar esa lista, Linux es un entorno de desarrollo **en preparación**,
no una plataforma aprobada para pilotos.
