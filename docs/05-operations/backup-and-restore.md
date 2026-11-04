# Respaldos y restauración

## Regla principal

Los archivos de base, WAL/SHM, respaldos y dumps **nunca** se publican en Git.
El `.gitignore` los excluye y el script `tools/check-no-secrets.sh` ayuda a
detectarlos antes de un push.

## Crear un respaldo

Desde la aplicación, con un usuario que tenga `backups.manage`, usa **Crear
respaldo**. El comando:

1. Ejecuta `VACUUM INTO` sobre una ruta nueva (consistente aunque la app esté
   abierta y con WAL activo).
2. Verifica el archivo resultante: `integrity_check`, claves foráneas, tablas
   requeridas y versión de esquema no posterior al ejecutable.
3. Si la verificación falla, borra el artefacto y no lo ofrece como respaldo.
4. Registra `backup.created` en auditoría y devuelve la ruta.

Destino por defecto:

```text
%LOCALAPPDATA%\HiQNet\POS\data\backups\hiqnet-pos-<epoch>.db
```

> **Nunca copies a mano `hiqnet-pos.db` mientras la aplicación está abierta.**
> Con WAL activo, los datos pueden vivir en `hiqnet-pos.db-wal` y una copia del
> archivo principal quedaría vacía o desactualizada. Usa siempre el respaldo de
> la aplicación o `VACUUM INTO`.

## Verificar un respaldo

Sin abrir la aplicación:

```powershell
pos-desktop.exe --verify-backup <respaldo.db>
```

Salida: `resultado=OK` (código 0) o `resultado=ERROR` (código 2). Se rechazan:

- archivos que no son SQLite o están corruptos;
- bases ajenas sin las tablas de HiQNet POS;
- respaldos con referencias foráneas rotas;
- bases creadas por una versión **posterior** del esquema.

## Restaurar (procedimiento soportado)

La restauración se ejecuta **con HiQNet POS cerrado**, usando el mismo ejecutable
en modo utilidad:

```powershell
# 1. Cierra la aplicación por completo.
# 2. Verifica el respaldo.
pos-desktop.exe --verify-backup D:\respaldos\hiqnet-pos-1700000000.db

# 3. Restaura indicando la base destino (por defecto usa %LOCALAPPDATA%).
pos-desktop.exe --restore-backup D:\respaldos\hiqnet-pos-1700000000.db `
    --database "$env:LOCALAPPDATA\HiQNet\POS\data\hiqnet-pos.db"
```

La utilidad:

1. verifica el respaldo (integridad, tablas, claves foráneas, historial de
   migraciones y versión);
2. toma el **bloqueo exclusivo del proceso** (`hiqnet-pos.db.lock`). Si HiQNet POS
   está abierto —aunque esté inactivo— rechaza la operación con
   `cierra HiQNet POS antes de restaurar un respaldo` y no toca ningún archivo;
3. sondea la base con `BEGIN EXCLUSIVE` para descartar transacciones ajenas
   abiertas por otras herramientas;
4. crea y verifica una **copia de recuperación**
   `hiqnet-pos.db.pre-restore-<epoch>.db` mediante `VACUUM INTO`, incluyendo el
   estado del WAL. Cada restauración crea una copia nueva; nunca sobrescribe una
   anterior;
5. copia el respaldo a `hiqnet-pos.db.restore-tmp`, verifica esa copia y solo
   entonces la mueve sobre la base. Si algo falla antes, la base actual queda
   intacta;
6. verifica la base restaurada y, si falla, revierte a la copia de recuperación.

El bloqueo se mantiene durante todo el reemplazo, de modo que la aplicación no
puede reabrir la base a mitad del procedimiento. Es un bloqueo cooperativo:
protege frente a otras instancias de HiQNet POS y frente a utilidades que respeten
el archivo, no frente a un programa externo que lo ignore deliberadamente.

Al arrancar, la aplicación toma ese mismo bloqueo. Si ya está abierto en el
equipo, muestra `HiQNet POS ya está abierto en este equipo` en lugar de trabajar
sobre la base de otra instancia.

Consecuencia: **todo cambio posterior al respaldo se pierde** (ventas,
deoluciones, compras, gastos, merma, usuarios, licencias y auditoría vuelven al
estado del archivo). La copia de recuperación permite deshacer una restauración
equivocada si aún no se operó sobre la base restaurada.

No se reemplaza la base desde la interfaz en operación: cerrar el pool activo
dentro del proceso seguiría dejando el WebView y la sesión en un estado
inconsistente. La utilidad offline es el procedimiento seguro soportado.

## Recuperación ante fallos

| Situación | Acción |
| --- | --- |
| Migración falla al arrancar | No abrir la app; conservar logs; restaurar el respaldo previo |
| Base corrupta (`quick_check` falla) | Restaurar respaldo verificado; conservar el archivo dañado para análisis |
| Base de versión posterior | Usar el ejecutable correspondiente; no degradar el esquema |
| Restauración equivocada | Cerrar la app y restaurar `*.pre-restore-<epoch>.db` |

La aplicación ejecuta `PRAGMA quick_check` y rechaza esquemas más nuevos al
arrancar. Revertir el ejecutable **no** revierte la base.

## Antes de una actualización

1. Crea un respaldo manual verificado.
2. Copia el archivo fuera del equipo si es crítico.
3. Actualiza la aplicación y deja que SQLx aplique las migraciones.
4. Verifica catálogo, última venta y auditoría.

## Pruebas ejecutadas (automatizadas)

`backend-rust/crates/pos-storage-sqlite/tests/backup_safety.rs`:

- respaldo verificado → cambio posterior → restauración → solo estado respaldado;
- copia de recuperación con el estado previo completo;
- rechazo de archivo ajeno/corrupto y de esquema más nuevo;
- rechazo de restauración con escritura activa y con otra instancia abierta;
- rechazo de historial de migraciones manipulado (checksum distinto);
- no sobrescritura de un respaldo existente ni de una copia de recuperación;
- restauración con respaldo inválido deja la base intacta;
- sin archivos `.restore-tmp` residuales;
- detección de base corrupta al abrir.

`tools/windows-native-e2e/e2e_restore_guard.py` (ejecutable real, base aislada):

- respaldo creado por la aplicación;
- `--restore-backup` con la aplicación abierta → código 2 y base intacta;
- `--verify-backup` y `--restore-backup` con la aplicación cerrada → código 0;
- copia `pre-restore` presente y reapertura con los datos restaurados.

`returns_migration.rs` cubre la actualización `0009 → 0010` conservando gastos.

### Conservación de datos de todas las fases

`tools/windows-native-e2e/restore_multiphase_check.py` toma una base generada por
el E2E (ventas, devoluciones, caja, compras, recepciones, gastos, merma,
inventario y auditoría), crea un snapshot, restaura con la utilidad real sobre una
copia con WAL y compara los registros de cada módulo. Además escribe un producto
posterior al snapshot para comprobar que la restauración lo deshace.

```powershell
python tools/windows-native-e2e/restore_multiphase_check.py `
    <run_directory>\hiqnet-pos.db apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe
```
