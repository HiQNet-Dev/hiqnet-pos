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
2. toma el **bloqueo cooperativo del proceso** (`hiqnet-pos.db.lock`). Si HiQNet
   POS está abierto —aunque esté inactivo— rechaza la operación con
   `cierra HiQNet POS antes de restaurar un respaldo` y no toca ningún archivo;
3. abre la base destino en **modo de bloqueo exclusivo** y ejecuta
   `PRAGMA wal_checkpoint(TRUNCATE)`. En WAL un lector no bloquea a un escritor,
   así que `BEGIN EXCLUSIVE` no detecta lectores; el checkpoint TRUNCATE sí:
   devuelve `busy` mientras exista cualquier otra conexión abierta. Además deja
   el archivo principal al día, de modo que ningún dato pendiente en el WAL se
   pierde;
4. crea la **copia de recuperación** `hiqnet-pos.db.pre-restore-<epoch>.db`
   mediante `VACUUM INTO` y la valida por integridad (no por esquema: así sirve
   también para volver atrás desde una base con una migración fallida). Cada
   restauración crea una copia nueva y nunca sobrescribe una anterior;
5. comprueba que el WAL quedó vacío y **aparta** `-wal`/`-shm` como
   `*.wal.pre-restore-<epoch>` / `*.shm.pre-restore-<epoch>`. No se borran: un
   lector de solo lectura puede dejar residuos sin mantener la base abierta, y
   un WAL con páginas pendientes obliga a abortar;
6. prepara `hiqnet-pos.db.restore-tmp` con el respaldo, lo sincroniza a disco y
   lo verifica; si algo falla, la base actual queda intacta;
7. promueve el temporal con un reemplazo de entrada de directorio y verifica la
   base restaurada;
8. si la promoción o la verificación final fallan, **vuelve a la copia de
   recuperación y verifica esa reversión**. Solo si la reversión también falla
   devuelve `RestoreCritical` (salida `resultado=ERROR`, `critico=true`)
   conservando base, copia de recuperación y temporal para intervención manual.

### Garantías reales

| Aspecto | Garantía |
| --- | --- |
| Otra instancia de HiQNet POS abierta | Rechazo previo; la base no se toca |
| Conexión SQLite externa (lector o escritor) | Rechazo por `wal_checkpoint(TRUNCATE)` |
| Datos pendientes en WAL | Se vuelcan antes de copiar y reemplazar |
| Fallo antes de la promoción | Base original intacta; temporal eliminado |
| Fallo al promover o al verificar | Reversión automática a la copia previa, verificada |
| Fallo de la reversión | `RestoreCritical` con rutas conservadas; no hay recuperación automática |
| Corte eléctrico durante la promoción | **Sin garantía de transacción de sistema de archivos.** Puede quedar la base en el estado restaurado sin confirmar; la copia `pre-restore` permite volver atrás |
| Windows vs. Unix | `fs::rename` es `MoveFileEx(REPLACE_EXISTING)` en Windows y `rename(2)` en Unix: ambos reemplazan la entrada de directorio, ninguno es un `fsync` transaccional |

El bloqueo cooperativo protege frente a otras instancias de HiQNet POS y frente a
utilidades que respeten el archivo; la exclusividad frente a otras conexiones
SQLite la aporta el checkpoint, no el bloqueo.

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
| `critico=true` en la utilidad | No operar: conservar base, `*.pre-restore-*` y `*.restore-tmp`, y contactar a soporte |

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

`backend-rust/crates/pos-storage-sqlite/tests/migration_failure.rs`:

- una migración que no puede aplicarse devuelve `Migrate`;
- los datos previos permanecen, no se crean tablas posteriores y la migración no
  queda registrada como aplicada;
- la restauración del respaldo previo recupera el esquema completo.

`backend-rust/crates/pos-storage-sqlite/src/backup.rs` (pruebas unitarias con
SQLite real y fallos inyectados):

| Escenario | Origen del fallo | Resultado esperado |
| --- | --- | --- |
| Copia de recuperación no disponible | Inyectado | Base intacta, error `Restore` |
| Copia al temporal imposible | Real (temporal ocupado por un directorio) | Base intacta, error `Restore` |
| Fallo durante la copia | Inyectado | Base intacta, sin `.restore-tmp` |
| Validación del temporal | Inyectado | Base intacta, temporal eliminado |
| Promoción del respaldo | Inyectado | Base intacta, temporal eliminado |
| Verificación final | Inyectado | Reversión a la copia previa, estado anterior completo |
| Reversión | Inyectado | `RestoreCritical` con base, copia y temporal conservados |
| Lector externo activo | Real (conexión SQLite con lectura abierta) | `DatabaseInUse`, base intacta |
| Residuos `-wal`/`-shm` de un lector previo | Real | Restauración correcta y residuos apartados |

Los fallos "inyectados" usan puntos de fallo compilados solo en pruebas
(`#[cfg(test)]`); no existen en el binario de producción.

`tools/windows-native-e2e/e2e_restore_guard.py` (ejecutable real, base aislada):

- respaldo creado por la aplicación;
- `--restore-backup` con la aplicación abierta → código 2 y base intacta;
- `--restore-backup` con un lector SQLite externo activo (python, sin respetar el
  bloqueo de HiQNet POS) → código 2 y base intacta;
- `--verify-backup` y `--restore-backup` con todo cerrado → código 0;
- copia `pre-restore` presente y reapertura con los datos restaurados.

`returns_migration.rs` cubre la actualización `0009 → 0010` conservando gastos.

### Conservación de datos de todas las fases

`tools/windows-native-e2e/restore_multiphase_check.py` toma una base generada por
el E2E (ventas, devoluciones, caja, compras, recepciones, gastos, merma,
inventario y auditoría), crea un snapshot, restaura con la utilidad real sobre una
copia con WAL y compara el **contenido exacto** de 27 tablas mediante un hash
SHA-256 determinista por tabla (importes en unidades menores, cantidades en
milésimas, identificadores y marcas de tiempo incluidos).

También comprueba:

- `PRAGMA integrity_check` = `ok` y `PRAGMA foreign_key_check` sin violaciones;
- que el producto escrito después del snapshot desaparece;
- que ninguna fila anterior cambia (diferencia por tabla vacía).

```powershell
python tools/windows-native-e2e/restore_multiphase_check.py `
    <run_directory>\hiqnet-pos.db apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe
```
