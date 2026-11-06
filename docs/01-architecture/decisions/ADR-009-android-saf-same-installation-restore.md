# ADR-009 — Restauración Android SAF limitada a la misma instalación

- Estado: **aceptada para HiQNet POS Local 0.1.0**
- Fecha: 2026-11-06

## Contexto

Android necesita exportar y recuperar SQLite sin permisos generales de almacenamiento y sin
reemplazar una base que aún tenga un pool o transacciones activos. `local_installation_state`
contiene identidad de instalación, organización y estado comercial; restaurar esos campos desde
un snapshot antiguo podría extender un Trial o reactivar derechos sin autorización.

## Decisión

1. Android usa Storage Access Framework (SAF): `ACTION_CREATE_DOCUMENT` para exportar y
   `ACTION_OPEN_DOCUMENT` para importar. El adaptador móvil usa `ContentResolver`; nunca trata
   un `content://` como una ruta ni solicita `MANAGE_EXTERNAL_STORAGE`.
2. El plugin móvil local solo transporta archivos entre el proveedor de documentos y temporales
   privados de la app. React no conoce ni entrega rutas. Rust genera snapshots, valida SQLite,
   valida identidad, conserva estado comercial y coordina la restauración.
3. Solo se permite restaurar un respaldo cuyo `installation_id` y `organization_id` coincidan
   exactamente con la instalación activa. La recuperación entre instalaciones queda fuera de
   0.1.0 y requiere un proceso formal de licencia/reactivación.
4. Antes de confirmar se validan `integrity_check`, claves foráneas, tablas, historial/checksum
   de migraciones y compatibilidad de esquema. Tras confirmar se escribe un marcador privado con
   SHA-256 del archivo importado y con el estado comercial vigente; el hash se vuelve a comprobar
   al arrancar.
5. La restauración se ejecuta únicamente en el siguiente arranque, antes de `open_locked` y de
   crear el pool SQLite. El marcador se conserva hasta terminar; repetir el arranque es
   idempotente. `restore_from_backup` sigue creando copia de recuperación, valida la promoción y
   revierte cuando corresponde.
6. El estado comercial vigente se reescribe sobre la base restaurada. Así, un Trial antiguo no
   puede ampliar `valid_until`, ni un snapshot puede cambiar plan, estado o funciones.
7. La operación es reanudable. Si el proceso muere después de promover el snapshot pero antes de
   reponer el estado comercial, el arranque siguiente exige la misma identidad, vuelve a validar
   la huella y repite la promoción y la reposición. Solo se exige que `installation_id` y
   `organization_id` sigan siendo los mismos; comparar todo el estado comercial impediría
   reanudar. Tras el éxito se eliminan el marcador y la copia importada privada.
8. Mientras el arranque temprano no tenga una clave pública de licencias configurada para
   revalidar Ed25519, se **rechaza** toda restauración donde la instalación actual o el respaldo
   contengan `license_grant_json` o `license_signature_b64`. Es una denegación segura: no se
   aceptan firmas sin verificar ni se copia material de licencia desde el respaldo.

## Consecuencias

- La UI avisa que los datos comerciales posteriores al respaldo se perderán, que se conserva el
  estado comercial actual y que la app se cerrará para restaurar.
- Exportar sigue permitido con `backups.manage`; importar, validar y programar una restauración
  exigen el mismo permiso. El frontend no puede omitir la validación de Rust.
- Un fallo antes de confirmar no crea marcador. Un archivo alterado después de confirmar no se
  consume. Un fallo de restauración conserva la base previa o sus artefactos de recuperación
  conforme a `restore_from_backup`.
- Esta decisión no implementa migración de dispositivos, transferencia de licencia, SAF en iOS,
  ni validación de hardware ARM64 físico.

## Límites de transporte

- El plugin permite **una sola operación** de documentos a la vez y la mantiene activa hasta que
  la copia termina, no solo hasta que el selector devuelve el control.
- Las copias se hacen en streaming con un tope de 512 MiB; no se cargan en memoria. La
  verificación Rust aplica el mismo tope a cualquier respaldo.
- Las respuestas al proceso se ignoran si la Activity ya no escucha; los flujos se cierran con
  `use` y un documento parcial se borra de forma best-effort.
- Al arrancar, después de consumir el marcador, se eliminan los temporales SAF huérfanos;
  mientras exista un marcador no se limpia nada.
