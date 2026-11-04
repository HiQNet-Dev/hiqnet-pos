# Matriz de funcionalidades — 0.1.0

Estados: **VERIFICADO** (implementado y probado), **PARCIAL**, **PENDIENTE**,
**PROPUESTO**, **BLOQUEADO**.

| Capacidad | Estado | Evidencia / nota |
| --- | --- | --- |
| Workspace Rust con 5 crates | VERIFICADO | `cargo test --workspace`, `clippy -D warnings` |
| `Money` / `Quantity` exactos | VERIFICADO | Pruebas unitarias en `pos-core` |
| Organización y sucursal | VERIFICADO | Bootstrap transaccional + pruebas SQLite |
| Identidad local y Argon2id | VERIFICADO | Hash/verify + login + rechazo de credenciales |
| RBAC deny-by-default | VERIFICADO | Prueba: cajero no puede administrar catálogo ni usuarios |
| Administración de usuarios locales | VERIFICADO | Alta, listado, activación, desactivación, restablecimiento y cambio de contraseña; `users.manage` en Rust, Argon2id, auditoría transaccional y revocación de sesión |
| Auditoría append-only | VERIFICADO | Triggers + prueba de `UPDATE` rechazado |
| Migración inicial SQLite | VERIFICADO | `0001_init.sql` aplicada al arrancar |
| Persistencia entre reinicios | VERIFICADO | Prueba de cierre y reapertura |
| Respaldo consistente | VERIFICADO | `VACUUM INTO` + verificación de integridad, FK, tablas y versión; el artefacto inválido se descarta |
| Restauración desde respaldo | VERIFICADO | Utilidad offline `--restore-backup` con copia de recuperación, rechazo de esquema posterior y de escrituras activas |
| Restauración desde UI | PENDIENTE | Requiere cerrar el pool y el WebView; el procedimiento seguro es la utilidad offline |
| Trial de 30 días / solo lectura | VERIFICADO | Reloj fijo y bloqueo de mutaciones |
| Verificación Ed25519 | VERIFICADO | Firma válida, payload manipulado, clave ajena |
| Emisión de licencias | PENDIENTE | Responsabilidad de HiQNet Platform |
| IPC tipado React ↔ Rust | VERIFICADO | Comandos + validación Zod + pruebas de UI; argumentos snake_case verificados sobre el ejecutable real |
| Shell de escritorio y navegación | VERIFICADO | Una pantalla activa, menú por permisos, ticket persistente y capturas en 1920/1440/1366/1024/768 |
| App Tauri arranca en Windows | VERIFICADO | Ejecutable construido y arrancado; base creada |
| Instalador NSIS firmado | PENDIENTE | Requiere decisión de firma (D-09) |
| Instalador NSIS interno (sin firma) | PARCIAL | Construido y verificado como `NotSigned`; instalación/desinstalación en VM limpia pendiente |
| Smoke E2E con Playwright | PARCIAL | Smoke de UI en navegador con puente Tauri simulado |
| E2E nativo Windows (tauri-driver + WebView2) | PARCIAL | Flujo comercial completo, devoluciones/reportes y rendimiento verificados en local. No se ejecuta en GitHub Actions |
| Ventas y pagos | PARCIAL | Catálogo + ticket simultáneos, cobro en efectivo verificado de extremo a extremo en el ejecutable; faltan otros métodos de pago |
| Caja, turnos y arqueo | PARCIAL | Migración 0006, IPC, UI, pruebas Rust/SQLite y E2E nativo; faltan justificación obligatoria de diferencias y arqueo ciego |
| Categorías de catálogo | VERIFICADO | Dominio, migración 0004, IPC, asignación y UI; cubierto por E2E nativo |
| Variantes de producto | PARCIAL | SKU por organización, precio exacto, selección en UI y venta; sin inventario independiente por variante |
| Modificadores / extras | PARCIAL | Motor, migración 0005, IPC, configuración en el diálogo de venta y administración básica de grupos/opciones en Productos; la partida persiste el snapshot |
| Inventario y ledger | PARCIAL | Dominio, migraciones 0002/0003, IPC y UI; descuento por venta verificado en E2E nativo |
| Ajustes positivos/negativos | VERIFICADO | Motivo obligatorio + pruebas SQLite |
| Ledger append-only | VERIFICADO | Triggers y prueba de UPDATE/DELETE |
| Concurrencia de salidas | VERIFICADO | Prueba con dos solicitudes y stock limitado |
| Migración 0002 → 0003 | VERIFICADO | Prueba con datos sintéticos y triggers recreados |
| Proveedores y compras | PARCIAL | G2.7: proveedor, orden en borrador/confirmada y recepción parcial/final idempotente que actualiza el ledger; sin pagos a proveedores ni costeo avanzado |
| Gastos operativos y merma | PARCIAL | G2.8: gasto externo o pagado desde caja y merma idempotente con ledger; sin anulación compensatoria, cuentas por pagar ni valuación de merma |
| Reportes operativos | PARCIAL | G2.9: ventas brutas/devueltas/netas, caja, compras, gastos e inventario con intervalo semiabierto UTC y agregados SQLite; sin utilidad, margen ni costo de ventas |
| Devoluciones de venta | PARCIAL | G2.9: devolución parcial/total en efectivo desde turno abierto, reembolso exacto por snapshots, reintegro opcional al ledger, idempotencia y auditoría; sin devoluciones bancarias ni reembolso sin turno |
| Sincronización Pro | PENDIENTE | Fase G6; requiere ADR de autoridad |
| Billing real | PENDIENTE | D-07 sin decidir |
| Portal administrativo | PENDIENTE | HiQNet Platform |

## No afirmado

No se declara soporte de multicaja offline, multi-master, CFDI, pagos con
terminal física, SLA, alta disponibilidad ni instaladores firmados. El E2E nativo
de Windows se ejecutó manualmente en un equipo con WebView2; no está automatizado
en CI ni sustituye pruebas de distribución.
