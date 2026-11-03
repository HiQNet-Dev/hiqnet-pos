# ADR-002 — SQLite como autoridad de datos operativos

- Estado: aceptado
- Fecha: 2026-11-03

## Decisión

Cada instalación del POS usa un archivo SQLite local como autoridad de ventas,
catálogo, existencias y auditoría. La aplicación debe completar una venta sin
Internet y sin PostgreSQL.

- Ubicación: `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` (configurable con
  `HIQNET_POS_DB_PATH` en desarrollo).
- PRAGMAs: WAL, `foreign_keys = ON`, `busy_timeout = 5s`,
  `synchronous = NORMAL`.
- Un solo pool de conexiones (`max_connections = 1`) para serializar escrituras
  dentro del proceso.

## Invariantes

1. Ninguna mutación auditable se confirma sin su evento de auditoría.
2. `audit_event` es append-only (triggers).
3. Los importes y cantidades son enteros exactos; no se usan flotantes.
4. Una licencia vencida bloquea nuevas operaciones, no la lectura ni el respaldo.
5. El archivo de base y sus WAL/SHM están excluidos de Git.

## Pendiente

- Concurrencia entre dos instancias del POS sobre el mismo archivo.
- Estrategia de respaldo programado y retención.
- Definición de autoridad single-writer para sincronización Pro (D-05).

## Consecuencias

PostgreSQL no reemplaza SQLite al pasar a Pro. La sincronización futura agrega
outbox/inbox y no cambia la autoridad local de la caja.
