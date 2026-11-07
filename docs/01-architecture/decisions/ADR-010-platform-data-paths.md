# ADR-010 — Rutas de datos por plataforma

- Estado: aceptado
- Fecha: 2026-11-06

## Contexto

La aplicación nació como producto Windows y resolvía la base con
`%LOCALAPPDATA%`. El desarrollo pasa a ser multiplataforma (Windows y Arch
Linux/Omarchy) y Android ya usa el almacenamiento privado de la aplicación. La
ruta debe ser la misma para la interfaz Tauri, la CLI de respaldo, la
restauración y las pruebas; de lo contrario una copia creada por una vía no se
encuentra desde otra.

## Decisión

`pos-storage-sqlite::paths` es la única autoridad para resolver la ruta:

| Plataforma | Base |
| --- | --- |
| Windows | `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` |
| Linux | `$XDG_DATA_HOME/hiqnet-pos/data/hiqnet-pos.db` |
| Linux sin `XDG_DATA_HOME` | `~/.local/share/hiqnet-pos/data/hiqnet-pos.db` |
| macOS | `~/Library/Application Support/hiqnet-pos/data/hiqnet-pos.db` |
| Android | `app_data_dir/data/hiqnet-pos.db` (Tauri) |

Reglas:

1. `HIQNET_POS_DB_PATH` tiene prioridad y se comparte entre aplicación y CLI.
2. `XDG_DATA_HOME` relativo se ignora, como exige la especificación XDG.
3. Si falta el directorio persistente se aborta el arranque con un diagnóstico;
   nunca se crea una base definitiva en el directorio temporal. `HIQNET_POS_DB_PATH`
   sigue siendo la salida explícita para desarrollo, QA y casos de recuperación.
4. Windows no cambia de ubicación; no se migran bases existentes.
5. La automatización de escritorio conserva su ruta explícita por encima de la
   variable de entorno.

## Consecuencias

- Cambiar la ruta de una plataforma exige una migración con copia de seguridad,
  no un cambio silencioso.
- Las pruebas de resolución de rutas se ejecutan en cualquier host porque la
  plataforma es un parámetro, no un `cfg!` oculto.
- Los instaladores y guías deben documentar la ruta efectiva; `--help-backup` la
  imprime.
