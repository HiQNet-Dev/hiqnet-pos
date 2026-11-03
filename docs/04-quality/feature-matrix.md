# Matriz de funcionalidades — 0.1.0

Estados: **VERIFICADO** (implementado y probado), **PARCIAL**, **PENDIENTE**,
**PROPUESTO**, **BLOQUEADO**.

| Capacidad | Estado | Evidencia / nota |
| --- | --- | --- |
| Workspace Rust con 5 crates | VERIFICADO | `cargo test --workspace`, `clippy -D warnings` |
| `Money` / `Quantity` exactos | VERIFICADO | Pruebas unitarias en `pos-core` |
| Organización y sucursal | VERIFICADO | Bootstrap transaccional + pruebas SQLite |
| Identidad local y Argon2id | VERIFICADO | Hash/verify + login + rechazo de credenciales |
| RBAC deny-by-default | VERIFICADO | Prueba: cajero no puede administrar catálogo |
| Auditoría append-only | VERIFICADO | Triggers + prueba de `UPDATE` rechazado |
| Migración inicial SQLite | VERIFICADO | `0001_init.sql` aplicada al arrancar |
| Persistencia entre reinicios | VERIFICADO | Prueba de cierre y reapertura |
| Respaldo consistente | VERIFICADO | `VACUUM INTO` + verificación de integridad |
| Restauración desde respaldo | VERIFICADO | Prueba de restauración a nivel de almacenamiento |
| Restauración desde UI | PENDIENTE | Requiere cierre controlado de la conexión |
| Trial de 30 días / solo lectura | VERIFICADO | Reloj fijo y bloqueo de mutaciones |
| Verificación Ed25519 | VERIFICADO | Firma válida, payload manipulado, clave ajena |
| Emisión de licencias | PENDIENTE | Responsabilidad de HiQNet Platform |
| IPC tipado React ↔ Rust | VERIFICADO | Comandos + validación Zod + pruebas de UI |
| App Tauri arranca en Windows | VERIFICADO | Ejecutable construido y arrancado; base creada |
| Instalador NSIS firmado | PENDIENTE | Requiere decisión de firma (D-09) |
| Smoke E2E con Playwright | PARCIAL | Smoke de UI en navegador; E2E Tauri pendiente |
| Ventas y pagos | PENDIENTE | Fase G2 |
| Caja, turnos y arqueo | PENDIENTE | Fase G2 |
| Inventario y ledger | PARCIAL | Dominio, migraciones 0002/0003, IPC y UI base; faltan E2E Tauri |
| Ajustes positivos/negativos | VERIFICADO | Motivo obligatorio + pruebas SQLite |
| Ledger append-only | VERIFICADO | Triggers y prueba de UPDATE/DELETE |
| Concurrencia de salidas | VERIFICADO | Prueba con dos solicitudes y stock limitado |
| Migración 0002 → 0003 | VERIFICADO | Prueba con datos sintéticos y triggers recreados |
| Proveedores y compras | PENDIENTE | Fase G2 |
| Gastos y devoluciones | PENDIENTE | Fase G2 |
| Reportes | PENDIENTE | Fase G2 |
| Sincronización Pro | PENDIENTE | Fase G6; requiere ADR de autoridad |
| Billing real | PENDIENTE | D-07 sin decidir |
| Portal administrativo | PENDIENTE | HiQNet Platform |

## No afirmado

No se declara soporte de multicaja offline, multi-master, CFDI, pagos con
terminal física, SLA, alta disponibilidad ni instaladores firmados.
