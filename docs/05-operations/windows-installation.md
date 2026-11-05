# Instalación en Windows (piloto interno)

Procedimiento para instalar HiQNet POS en un equipo Windows 10/11 de un piloto
privado. El instalador actual **no está firmado** y es de uso interno: no debe
distribuirse como versión comercial.

## Artefacto

| Dato | Valor |
| --- | --- |
| Nombre | `HiQNet POS_0.1.0_x64-setup.exe` |
| Versión | `0.1.0` |
| Identificador | `com.hiqnet.pos` |
| Arquitectura | x64 |
| Formato | NSIS |
| Modo de instalación | `currentUser` (sin privilegios administrativos) |
| Firma Authenticode | **NotSigned** |
| Tamaño | 5 124 009 bytes (4.89 MiB) |
| SHA-256 | `228AFD0F34D24C6F29FE9D78E9DAC0B0486151A8F90C30AAFE77919411FC3EBC` |

Se construye desde la raíz del repositorio con:

```powershell
cd apps/pos-desktop/src-tauri
pnpm dlx @tauri-apps/cli@2 build --bundles nsis
```

El resultado queda en `target/release/bundle/nsis/`.

## Requisitos

- Windows 10/11 x64.
- Microsoft Edge WebView2 Runtime.
- Espacio libre en el perfil del usuario.

### WebView2 y el mito del instalador offline

El paquete generado usa `INSTALLWEBVIEW2MODE = "downloadBootstrapper"`:

- si WebView2 **ya está instalado**, el instalador no descarga nada;
- si **falta**, descarga el bootstrapper desde
  `https://go.microsoft.com/fwlink/p/?LinkId=2124703` y la instalación **requiere
  Internet**.

Windows 11 incluye WebView2 de forma habitual, pero eso no es una garantía
verificable desde el repositorio. Para un piloto que deba instalarse sin red hay
que cambiar la configuración a `embedBootstrapper` (el bootstrapper se incluye,
pero la descarga del runtime sigue necesitando red) o a `offlineInstaller` (el
runtime completo se incluye en el paquete, ~130 MB). Es una decisión de
distribución pendiente; no se ha modificado en este repositorio.

## Instalación

1. Copia el instalador al equipo del piloto.
2. Ejecuta el archivo. Si Windows muestra SmartScreen, elige
   **Más información → Ejecutar de todas formas**; el binario no está firmado.
3. Acepta el destino propuesto por el instalador (perfil del usuario).
4. Si la aplicación ya está abierta, el instalador se detiene: ciérrala.

Rutas habituales:

| Dato | Ruta |
| --- | --- |
| Programa | `%LOCALAPPDATA%\HiQNet POS\` (`pos-desktop.exe`, `uninstall.exe`) |
| Datos de WebView2 | `%LOCALAPPDATA%\com.hiqnet.pos\` |
| Base de datos | `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` |
| Respaldos | `%LOCALAPPDATA%\HiQNet\POS\data\backups\` |
| Bloqueo de base | `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db.lock` |

La base **no** se guarda en la carpeta del programa: desinstalar no la elimina.

## Primera ejecución

1. Abre **HiQNet POS**.
2. Completa negocio, sucursal y propietario (el asistente crea la base).
3. Cierra y vuelve a abrir: debe pedir inicio de sesión.
4. Crea un producto, abre caja y registra una venta de prueba.

## Actualización

1. Cierra la aplicación.
2. Crea un respaldo verificado (`--verify-backup`).
3. Ejecuta el instalador de la versión nueva sobre la existente.
4. Abre la aplicación: SQLx aplica las migraciones pendientes.
5. Verifica catálogo, última venta y auditoría.

Una versión **anterior** del ejecutable no puede abrir una base migrada a un
esquema más nuevo: la aplicación lo rechaza con
`la base de datos pertenece a una versión más reciente`.

## Desinstalación

La desinstalación elimina el programa, no la base. Antes de desinstalar:

1. Crea un respaldo y guárdalo fuera del equipo.
2. Desinstala desde **Aplicaciones instaladas**.

Para reinstalar conservando datos, deja
`%LOCALAPPDATA%\HiQNet\POS\data\` intacto.

## Estado de verificación

La instalación real en una VM Windows limpia **no se ha ejecutado**: Windows
Sandbox no está habilitado en el equipo de desarrollo y la sesión no tiene
privilegios para habilitarlo. El arnés está preparado en
[`tools/prepilot-sandbox/`](../../tools/prepilot-sandbox/README.md), con el
comando exacto de habilitación. Consulta
[`docs/04-quality/prepilot-readiness.md`](../04-quality/prepilot-readiness.md)
para la tabla de PASS/FAIL/NO EJECUTADO.
