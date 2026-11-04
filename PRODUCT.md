# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Aplicación de escritorio Windows empaquetada con Tauri 2 y WebView2. La interfaz
es web, pero el producto no es un sitio: es una herramienta de trabajo instalada
en el equipo del comercio.

## Users

- **Cajeros**: operan de pie, con prisa y durante jornadas completas. Su tarea es
  registrar productos, cobrar y cerrar el ticket sin perder el contexto de la
  venta.
- **Administradores y propietarios**: gestionan catálogo, inventario, caja,
  respaldos y auditoría desde el mismo equipo.
- **Pequeños y medianos negocios de México**: una o pocas sucursales, con
  operación local y sin dependencia permanente de Internet.

## Product Purpose

HiQNet POS registra ventas, existencias, movimientos de caja y auditoría en el
equipo del comercio. Rust es la autoridad de negocio; React presenta la interfaz
y SQLite conserva los datos locales.

El éxito significa que un cajero puede cobrar rápido y con pocos errores, y que
el propietario puede entender qué pasó en el día sin depender de la nube.

## Positioning

Punto de venta local-first con reglas, importes y permisos ejecutados en Rust.
La interfaz no decide precios, impuestos, existencias ni autorizaciones: los
valida el backend y los muestra sin duplicar la regla.

## Operating Context

- Equipo de escritorio Windows 10/11 con WebView2, ratón y teclado; también se
  usa en tablet.
- Jornadas largas, atención al público y espacio físico reducido.
- Operaciones frecuentes: buscar producto, agregar al ticket, ajustar cantidad,
  cobrar en efectivo, abrir y cerrar turno, consultar existencias.
- Operaciones sensibles: arqueo de caja, ajustes de inventario, respaldos y
  auditoría.
- La interfaz está en español de México; los identificadores técnicos van en
  inglés en el código y en los contratos.

## Capabilities and Constraints

Verificado en el repositorio:

- Bootstrap de organización, sucursal y propietario; login local con Argon2id y
  RBAC deny-by-default.
- Catálogo con categorías, variantes y modificadores; alta y consulta.
- Venta en efectivo con ticket, idempotencia, cálculo autoritativo en Rust y
  descuento de inventario.
- Inventario por producto y ledger append-only.
- Apertura, movimientos, cierre y arqueo de turnos.
- Auditoría append-only y respaldo consistente de SQLite.
- Shell de escritorio con navegación lateral y una sola pantalla activa.

Pendiente o parcial (no se debe afirmar lo contrario):

- Otros medios de pago, devoluciones, compras, gastos y reportes.
- Inventario independiente por variante, recetas y CFDI.
- Emisión productiva de licencias, installer firmado y restauración desde la UI.
- Sincronización cloud y portal administrativo.

Restricciones:

- No se aceptan `organization_id`, `branch_id` ni permisos desde el frontend; se
  derivan del `Actor` autenticado.
- Toda mutación auditable usa una `StoreTx` con auditoría en la misma
  transacción.
- No se publican bases SQLite, respaldos, `.env` ni claves.
- Los precios, límites de plan, plazos de gracia y proveedor de cobro siguen
  pendientes; no se inventan en la interfaz.

## Brand Commitments

- Nombre: **HiQNet POS**.
- Promesa verificable: operación local, sin dependencia permanente de red.
- Voz: directa, operativa y en español; los mensajes de error nombran el
  problema y la recuperación.
- No hay manual de marca ni paleta aprobada fuera del sistema visual existente
  en `frontend/src/styles.css`.

## Evidence on Hand

- `README.md`, `docs/04-quality/feature-matrix.md` y
  `docs/01-architecture/decisions/` describen el estado real del producto.
- `docs/03-contracts/ipc.md` y `frontend/src/contracts.ts` son el contrato IPC.
- `tools/windows-native-e2e/` contiene el E2E nativo de Windows y sus capturas.
- Las capturas de este rediseño se generan con
  `frontend/e2e/visual.spec.ts` sobre un puente Tauri simulado; no sustituyen al
  E2E nativo.

## Product Principles

1. **Cobrar primero.** La venta y su acción principal nunca quedan fuera de
   pantalla ni detrás de pasos administrativos.
2. **Rust decide, React muestra.** Ningún cálculo monetario, de inventario o de
   permiso se reimplementa en la interfaz.
3. **Prevenir antes que corregir.** Las acciones destructivas se confirman y los
   estados inválidos se bloquean antes de enviar.
4. **Densidad con jerarquía.** La pantalla debe aprovechar el espacio sin
   esconder el estado operativo ni la acción siguiente.
5. **Consistencia operativa.** Los mismos patrones de navegación, formulario,
   tabla, error y carga se repiten en todas las pantallas.

## Accessibility & Inclusion

Objetivo del encargo: interfaz accesible y operable con ratón, teclado y tablet.
La implementación actual incluye foco visible, navegación por teclado, diálogos
con foco atrapado y `prefers-reduced-motion`. No existe una certificación formal
ni una auditoría externa registrada.
