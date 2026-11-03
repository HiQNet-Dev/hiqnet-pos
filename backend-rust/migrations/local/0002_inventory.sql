-- Migración incremental: inventario transaccional por movimientos (ledger append-only).
CREATE TABLE IF NOT EXISTS inventory_movement (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('inbound','outbound','adjustment')),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    reason TEXT,
    actor_user_id TEXT,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (branch_id) REFERENCES branch(id),
    FOREIGN KEY (product_id) REFERENCES product(id)
);

CREATE INDEX IF NOT EXISTS idx_inventory_movement_org_branch_product
    ON inventory_movement (organization_id, branch_id, product_id);

CREATE INDEX IF NOT EXISTS idx_inventory_movement_occurred
    ON inventory_movement (occurred_at);

-- Append-only: bloquear UPDATE y DELETE.
CREATE TRIGGER IF NOT EXISTS inventory_movement_no_update
BEFORE UPDATE ON inventory_movement
BEGIN
    SELECT RAISE(ABORT, 'inventory_movement es append-only');
END;

CREATE TRIGGER IF NOT EXISTS inventory_movement_no_delete
BEFORE DELETE ON inventory_movement
BEGIN
    SELECT RAISE(ABORT, 'inventory_movement es append-only');
END;