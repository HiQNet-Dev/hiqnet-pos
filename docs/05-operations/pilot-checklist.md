# Lista de comprobación — entrega de una instalación piloto

Uso interno. Completar y archivar junto con el equipo entregado.

## Antes de entregar

- [ ] `main` en verde y sin PR de estabilización pendiente sin revisar.
- [ ] Versión coherente en `backend-rust/Cargo.toml`, Tauri, `package.json` y
      `tauri.conf.json`.
- [ ] Instalador NSIS generado desde el commit aprobado.
- [ ] Instalador identificado como **sin firma / prueba interna**; no distribuido
      como versión comercial.
- [ ] Actualizador deshabilitado (endpoints vacíos).
- [ ] Respaldo de la base del piloto anterior (si existe) verificado con
      `pos-desktop.exe --verify-backup`.
- [ ] Equipo con Windows 10/11 y WebView2 disponible.
- [ ] Espacio libre y permisos de escritura en `%LOCALAPPDATA%`.

## Instalación

- [ ] Ejecutar el instalador en el equipo del piloto.
- [ ] Abrir HiQNet POS y completar organización, sucursal y Owner.
- [ ] Confirmar que la base se creó en
      `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db`.
- [ ] Cerrar y reabrir la aplicación; iniciar sesión.
- [ ] Crear un producto, una caja y un turno de prueba.
- [ ] Registrar una venta en efectivo y comprobar folio y cambio.

## Respaldo y recuperación

- [ ] Crear un respaldo desde Gestión.
- [ ] Verificarlo: `pos-desktop.exe --verify-backup <ruta>`.
- [ ] Guardar una copia del respaldo fuera del equipo.
- [ ] Documentar la ruta de la base y del respaldo en la ficha del piloto.
- [ ] Explicar al operador que restaurar vuelve al estado del respaldo.

## Operación

- [ ] Confirmar que vender, cobrar, inventario, compras, gastos, merma,
      devoluciones y reportes funcionan sin Internet.
- [ ] Comprobar que el cajero no ve Gestión, Compras, Gastos, Devoluciones ni
      Reportes.
- [ ] Verificar que el arqueo coincide con el efectivo contado del día.
- [ ] Registrar el comportamiento observado y cualquier mensaje inesperado.

## Cierre

- [ ] Exportar respaldo final del piloto.
- [ ] Registrar incidencias con hora, pantalla y mensaje.
- [ ] No desinstalar si hay datos: primero respaldar.
- [ ] Notificar a soporte cualquier diferencia de arqueo o error de datos.
