# Piloto interno en Windows

Estado de preparación de HiQNet POS para un piloto **interno y controlado** en
Windows. Complementa `pilot-checklist.md`, `windows-installation.md` y
`backup-and-restore.md`.

## Matriz de preparación

| Capacidad | Cómo se verificó | Estado |
|---|---|---|
| Instalar la aplicación | Instalador NSIS 0.1.1-rc.1 instalado en modo silencioso sobre `%LOCALAPPDATA%` | **Aprobado** |
| Primer arranque | Ventana `HiQNet POS` visible, base creada, cierre limpio (código 0) | **Aprobado** |
| Inicializar organización | `e2e_flow` y `bootstrap` en el ejecutable real | **Aprobado** |
| Registrar productos | `e2e_flow`, `e2e_contracts` | **Aprobado** |
| Realizar ventas | `e2e_flow`, `e2e_returns`, `perf_volume` (1 000 ventas) | **Aprobado** |
| Administrar caja | `e2e_flow` (apertura, movimientos, arqueo, cierre) | **Aprobado** |
| Controlar inventario | `e2e_flow`, `verify_db` | **Aprobado** |
| Registrar gastos y devoluciones | `e2e_flow`, `e2e_returns`, `verify_returns_db` | **Aprobado** |
| Operar sin internet | `e2e_offline` (0 conexiones de red) | **Aprobado con restricción**: sin corte de red forzado |
| Generar respaldos | `e2e_restore_guard`, `backup_database` | **Aprobado** |
| Restaurar información | `e2e_restore_guard`, `restore_multiphase_check`, `e2e_restore_corruption` | **Aprobado** |
| Recuperarse de cierres inesperados | `e2e_crash_recovery` (10 cortes, 5 antes del commit) | **Aprobado** |
| Desinstalar preservando datos | Desinstalador NSIS; la base del usuario quedó intacta (hash idéntico) | **Aprobado** |
| Actualización entre versiones | Hay instaladores previos (0.0.9, 0.1.0) y migraciones idempotentes, pero **no se ejecutó una actualización sobre datos del piloto** | **No validado** |
| Firma de código | Instalador y ejecutable `NotSigned` | **Bloqueado** |
| Licencia firmada | El POS verifica firma, identidad, vigencia y entitlements; el grant manipulado cae a solo lectura | **Aprobado** (emisión productiva pendiente) |

## Requisitos del puesto de piloto

- Windows 10/11 x64 con WebView2 (viene de serie en Windows 11; en Windows 10
  puede requerir el runtime de Microsoft).
- Instalación por usuario, sin privilegios de administrador.
- Datos en `%LOCALAPPDATA%\HiQNet\POS\data`: `hiqnet-pos.db` más sus sidecars
  `-wal` y `-shm`, y `backups/`.
- Un solo equipo por base. El bloqueo de proceso impide dos instancias sobre el
  mismo archivo.

## Procedimientos

### Instalación

1. Ejecutar `HiQNet POS_<versión>_x64-setup.exe` y seguir el asistente
   (instalación por usuario, sin administrador).
2. Abrir desde el menú Inicio y completar el asistente inicial: negocio,
   sucursal, código y propietario.
3. Anotar la ruta de datos (`%LOCALAPPDATA%\HiQNet\POS\data`) en la ficha del
   piloto.

### Respaldo

Detalle en `backup-and-restore.md`. En el piloto:

1. Con la aplicación abierta, usar la acción de respaldo de Gestión.
2. El archivo se guarda en `data/backups/` con marca de tiempo.
3. Copiar ese archivo a un medio externo al menos una vez por jornada.
4. Verificar con la utilidad: `pos-desktop.exe --verify-backup <archivo>`.

### Restauración

1. Cerrar HiQNet POS por completo.
2. `pos-desktop.exe --restore-backup <archivo> --database <ruta>`.
3. Abrir la aplicación y comprobar productos, ventas del día y saldo de caja.
4. Si la restauración se rechaza porque la base está en uso, cerrar la
   aplicación (o el visor SQLite) y repetir.

### Actualización (cuando exista instalador firmado)

1. Respaldo y copia externa antes de actualizar.
2. Ejecutar el instalador nuevo sobre la instalación existente.
3. Abrir la aplicación: las migraciones se aplican solas; la base no se
   reemplaza.
4. Si algo falla, restaurar el respaldo del paso 1.

> Este procedimiento **no está validado todavía** sobre datos de piloto.

### Diagnóstico

- Versión mostrada en la interfaz y en el nombre del instalador.
- Log de la aplicación en la consola del webview (modo dev) o en el visor de
  eventos si se habilita `RUST_LOG`.
- Estado de la base: `PRAGMA quick_check` y `PRAGMA integrity_check` abriendo el
  archivo en sólo lectura.
- Bloqueo activo: si existe `hiqnet-pos.db.lock` y la aplicación no está
  abierta, es un resto de un cierre abrupto; abrir la aplicación lo reemplaza.

### Recuperación

| Síntoma | Acción |
|---|---|
| La aplicación no abre y la base no responde | Restaurar el último respaldo verificado |
| Datos faltantes tras un corte de energía | Abrir la aplicación (SQLite recupera el WAL) y verificar totales |
| Base marcada como corrupta al arrancar | Restaurar respaldo; la aplicación se niega a operar sobre una base dañada |
| Restauración rechazada | Cerrar la aplicación y cualquier lector SQLite, reintentar |
| Restauración incompleta (`RestoreCritical`) | Conservar base, copia de recuperación y temporal; escalar a soporte con los tres archivos |

## Fuera de alcance del piloto interno

- Distribución a clientes: falta firma de código y validación de licencia.
- Linux y Android.
- Actualización automática publicada.
- Integración productiva de licencias con Platform.
