# Desarrollo en Arch Linux / Omarchy

Objetivo: que un checkout limpio compile, arranque y opere en Arch Linux sin
pasos manuales no documentados.

> **Estado:** los comandos de este documento están definidos y la compilación
> equivalente ya está verde en CI (Ubuntu 24.04 + WebKitGTK 4.1, con smoke de la
> CLI offline). La ejecución nativa completa en un host Arch real (arranque,
> login, IPC, SQLite, venta, backup/restore) **todavía no se ha validado**. No se
> declara Linux aprobado hasta completar esa lista.

## Prerrequisitos

Paquetes oficiales de Tauri 2:

```bash
sudo pacman -Syu
sudo pacman -S --needed \
  base-devel \
  webkit2gtk-4.1 \
  gtk3 \
  librsvg \
  libappindicator-gtk3 \
  appmenu-gtk-module \
  openssl \
  curl \
  wget \
  file \
  xdotool \
  git
```

- `webkit2gtk-4.1` es el motor del WebView. No es WebView2: el comportamiento
  de CSS, fuentes y captura de teclado puede diferir.
- `libappindicator-gtk3` y `appmenu-gtk-module` solo son necesarios si se usan
  indicadores de bandeja.
- Wayland y X11 están soportados por GTK/WebKitGTK. Si aparecen problemas de
  escalado, prueba `GDK_BACKEND=x11` para descartar el compositor antes de
  atribuirlo a la aplicación.

## Rust, Node y pnpm

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup show          # instala el canal de rust-toolchain.toml
```

Node 24.18.0 (elige un gestor de versiones; no dependas del Node del sistema):

```bash
# fnm
fnm install 24.18.0 && fnm use
# o mise
mise install node@24.18.0 && mise use node@24.18.0
```

pnpm:

```bash
corepack enable
corepack prepare pnpm@11.24.0 --activate
# alternativa: npm install -g pnpm@11.24.0
```

Opcional: `sudo pacman -S just`.

## Puesta en marcha

```bash
git clone https://github.com/HiQNet-Dev/hiqnet-pos.git
cd hiqnet-pos
just setup
just check
just dev
```

`just dev` compila la app Tauri y abre la ventana con el WebView del sistema.

## Datos locales

La base se crea en:

```text
${XDG_DATA_HOME:-$HOME/.local/share}/hiqnet-pos/data/hiqnet-pos.db
```

`XDG_DATA_HOME` debe ser absoluto; si es relativo, la especificación XDG lo
ignora y se usa `~/.local/share`. Para aislar pruebas:

```bash
HIQNET_POS_DB_PATH="$PWD/.dev-data/qa.db" just dev
```

`.dev-data/` está excluido de Git. Nunca apuntes a una base con datos reales
durante pruebas.

## Empaquetado

```bash
just build-linux
# target/release/bundle/deb/ y target/release/bundle/appimage/
```

Los targets por plataforma viven en `tauri.linux.conf.json`. No se generan
paquetes Windows desde Linux: no hay estrategia de build cruzado validada.

## Diferencias relevantes frente a WebView2

- WebKitGTK no expone Chrome DevTools Protocol; `tauri-driver` y las herramientas
  basadas en CDP no aplican igual.
- El renderizado de `backdrop-filter`, sombras y fuentes puede variar. La
  evidencia visual de Windows no sustituye una revisión en Linux.
- Atajos y gestos dependen del compositor (Wayland) además del WebView.
- El zoom y el escalado fraccional se comportan distinto.

## Pendiente de validación en Linux

Lista mínima antes de declarar soporte:

- [ ] `cargo tauri build --debug --no-bundle` en Arch.
- [ ] Arranque y renderizado en Wayland y X11.
- [ ] Login, IPC y lectura/escritura SQLite.
- [ ] Inventario, venta, modales, sidebar, teclado y scroll.
- [ ] Backup y restauración con la app cerrada.
- [ ] Paquete `deb`/AppImage instalado en una máquina limpia.
