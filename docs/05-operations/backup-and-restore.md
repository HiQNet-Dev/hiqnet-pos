# Respaldos y restauración

## Regla principal

Los archivos de base, WAL/SHM, respaldos y dumps **nunca** se publican en Git.
El `.gitignore` los excluye y el script `tools/check-no-secrets.sh` ayuda a
detectarlos antes de un push.

## Crear un respaldo

Desde la aplicación, con un usuario que tenga `backups.manage`, usa **Crear
respaldo**. El comando:

1. Ejecuta `VACUUM INTO` sobre una ruta nueva.
2. Registra `backup.created` en auditoría.
3. Devuelve la ruta del archivo.

Destino por defecto:

```text
%LOCALAPPDATA%\HiQNet\POS\data\backups\hiqnet-pos-<epoch>.db
```

El respaldo es un archivo SQLite autocontenido, consistente aunque la aplicación
siga abierta.

## Verificar un respaldo

La verificación está implementada en `pos-storage-sqlite`:

```rust
pos_storage_sqlite::verify_backup(&path).await?;
```

Comprueba `PRAGMA integrity_check`, presencia de las tablas base y que el archivo
sea SQLite. La prueba `respaldo_y_restauracion_conservan_datos` lo ejecuta junto
con la restauración.

## Restaurar

La restauración se ejecuta **con la aplicación cerrada**:

1. Cierra HiQNet POS.
2. Verifica el respaldo (pasos anteriores o la utilidad de soporte).
3. Reemplaza `hiqnet-pos.db` por el respaldo.
4. Elimina `hiqnet-pos.db-wal` y `hiqnet-pos.db-shm` si existen.
5. Abre la aplicación y comprueba catálogo y auditoría.

El código expone `restore_from_backup(database_path, backup_path)` y la prueba de
integración demuestra que los datos vuelven al estado del respaldo. Exponerlo por
IPC requiere cerrar el pool activo; está **pendiente** para no arriesgar la base
de una caja en operación.

## Antes de una actualización

1. Crea un respaldo manual.
2. Copia el archivo fuera del equipo si es crítico.
3. Actualiza la aplicación y deja que SQLx aplique las migraciones.
4. Verifica el estado en la aplicación.

Si una migración falla, la aplicación no debe arrancar en un estado a medias:
restaura el respaldo y reporta el incidente.

## Pruebas ejecutadas

- Respaldo → escritura adicional → restauración → solo existe el estado del
  respaldo.
- Un archivo que no es SQLite es rechazado tanto por `verify_backup` como por
  `restore_from_backup`.
