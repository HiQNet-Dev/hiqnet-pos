# ADR-007 — Shell de escritorio y navegación por pantallas

- Estado: aceptado
- Fecha: 2026-11-04
- Contexto: el frontend de 0.1.0 renderizaba todos los módulos apilados en una
  sola página (`WorkspaceScreen`), con scroll global y formularios
  administrativos mezclados con el cobro.

## Decisión

1. El frontend se organiza como **una aplicación de escritorio con navegación
   lateral persistente**: barra superior, menú de secciones y **una sola
   pantalla activa**.
2. Secciones: `Ventas`, `Productos`, `Inventario`, `Cajas y turnos` y `Gestión`.
   La sección visible se deriva de los permisos del `Actor`, pero **la
   autorización real sigue en Rust**: ocultar una sección no concede ni retira
   permiso.
3. El estado compartido vive en `AppShell`:
   - catálogo y categorías (una carga para todas las pantallas),
   - existencias agregadas para la rejilla de venta,
   - caja seleccionada y turno abierto,
   - **ticket actual**, que sobrevive a la navegación entre secciones.
4. Cada pantalla (`SalesScreen`, `CatalogScreen`, `InventoryScreen`,
   `CashScreen`, `AdminScreen`) es responsable de su propio flujo, su scroll
   interno y sus mensajes de error.
5. La configuración de una partida y el cobro usan **diálogos modales** con foco
   atrapado, cierre con `Escape` y restauración de foco.
6. En escritorio, ventas muestra catálogo y ticket **simultáneos** con scroll
   interno independiente. Por debajo de 960 px se alternan catálogo y ticket y
   se conserva una barra inferior con total y cobro.
7. Los importes que se muestran como definitivos provienen de Rust. React solo
   suma enteros de centavos ya calculados y nunca aplica impuestos.

## Motivo

Un cajero opera de pie, con prisa y con una sola mano libre. Apilar módulos
obliga a desplazarse para cobrar y mezcla tareas administrativas con la
operación, lo que produce errores. La separación por pantallas reduce el coste
de cada operación, permite mantener el ticket visible y hace evidente qué
acciones pertenecen a cada rol.

## Consecuencias

- El ticket vive en el shell: cambiar de sección (por ejemplo, revisar el
  inventario) no lo pierde.
- El turno se **deriva** de la caja seleccionada; nunca se muestra un turno de
  otra caja u organización.
- La rejilla de venta necesita disponibilidad sin N consultas, por lo que se
  añadió el comando de lectura `list_stock_levels`.
- La barra superior muestra la sucursal del actor, por lo que se añadió
  `branch_summary`.
- Los cambios de estructura obligan a actualizar los selectores del E2E nativo;
  las comprobaciones comerciales se conservan.
- Nuevas secciones deben registrarse en `shell/navigation.ts` con sus permisos y
  crear su propia pantalla; no se añaden bloques a una pantalla existente.

## Alternativas descartadas

- Mantener el workspace apilado y arreglarlo con `position: sticky`: no resuelve
  la jerarquía ni la mezcla de responsabilidades.
- Un router con URLs (react-router): la app es de escritorio, sin URLs
  compartibles; una unión tipada de secciones basta y evita una dependencia.
- Guardar el ticket en la base local: no existe todavía una venta abierta y
  crearía estado comercial prematuro.
