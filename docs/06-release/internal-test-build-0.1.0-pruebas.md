# Guía de pruebas manuales — Internal Test Build 0.1.0

Para validar HiQNet POS Local en tu propio equipo. Usa **solo datos sintéticos**;
no introduzcas información real de clientes.

## Windows

1. Ejecuta `HiQNet POS_0.1.0_x64-setup.exe`. Si SmartScreen advierte, es porque el
   build interno no está firmado: no se eludió ninguna protección del sistema.
2. Abre HiQNet POS desde el menú Inicio.
3. Completa *Configura tu negocio*: negocio, sucursal, código, propietario,
   usuario y contraseña (mínimo 12 caracteres).
4. Crea una categoría y un producto (`Catálogo`).
5. Da entrada a existencias (`Inventario` → Entrada).
6. Crea una caja y abre un turno con fondo inicial de $100.00.
7. En `Ventas`, añade dos unidades al ticket y cobra en efectivo con $50.00.
   Comprueba folio, total y cambio.
8. Verifica existencias y el saldo esperado de caja.
9. Registra un gasto, una compra con recepción y una merma.
10. Haz una devolución parcial y consulta `Reportes`.
11. Cierra el turno contando el efectivo exacto: la diferencia debe ser 0.
12. Cierra la aplicación y vuelve a abrirla: los datos deben seguir ahí.
13. En `Administración`, crea un respaldo. Anota la ruta que muestra.
14. Desconecta el equipo de Internet y repite el login y una venta.
15. Desinstala desde *Aplicaciones instaladas*. **La base debe conservarse.**
16. Reinstala y comprueba que la aplicación abre con los datos anteriores.
17. Para probar una actualización, instala primero `0.0.9` y después `0.1.0`
    encima; los datos deben sobrevivir y debe poder venderse de nuevo.

Dónde están los datos: `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` y la carpeta
`backups\` junto a ella. No borres `-wal` ni `-shm` a mano.

## Android

1. Instala la APK con `adb install -r <archivo>.apk` (ARM64).
2. Abre la aplicación y completa el onboarding.
3. Repite los pasos 4 a 12 anteriores.
4. Envía la aplicación a segundo plano y vuelve: la sesión y el ticket deben
   conservarse.
5. Activa el modo avión y repite una venta.
6. Fuerza la detención (`Ajustes` → `Detener`) y reabre: no debe haber ventas
   duplicadas ni pérdida de datos.
7. Respaldo: hoy se crea en el almacenamiento privado de la aplicación
   (`Archivos/backups`). La exportación a una carpeta elegida por el usuario
   mediante SAF **todavía no está implementada**.

## Qué NO probar todavía

- Sincronización entre Windows y Android: no existe y no es parte de esta entrega.
- Suscripciones o pagos reales: el Control Plane comercial no está desplegado.
- Restauración SAF en Android (pendiente).
- Instalación en tablet ARM64 física: falta evidencia; solo hay emulador.

## Cómo reportar un fallo

Adjunta: versión (`Ajustes` → *Administración*), pantalla, pasos exactos, texto del
error y, si es posible, una captura. Para fallos de datos, conserva la base y su
`-wal` antes de reintentar.
