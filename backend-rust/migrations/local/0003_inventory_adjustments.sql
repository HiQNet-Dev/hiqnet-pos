-- Migración incremental: soporta ajustes positivos y negativos.
-- Re-construye la tabla para ampliar el CHECK de kinds sin perder datos.
CREATE TABLE IF NOT EXISTS inventory_movement_new (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('inbound','outbound','adjustment_in','adjustment_out')),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    reason TEXT,
    actor_user_id TEXT,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (branch_id) REFERENCES branch(id),
    FOREIGN KEY (product_id) REFERENCES product(id)
);

INSERT INTO inventory_movement_new
SELECT id, organization_id, branch_id, product_id,
       CASE kind
         WHEN 'adjustment' THEN 'adjustment_in'
         ELSE kind
       END,
       quantity_milli, reason, actor_user_id, occurred_at
FROM inventory_movement;

DROP TRIGGER IF EXISTS inventory_movement_no_update;
DROP TRIGGER IF EXISTS inventory_movement_no_delete;

DROP TABLE inventory_movement;
ALTER TABLE inventory_movement_new RENAME TO inventory_movement;

CREATE INDEX IF NOT EXISTS idx_inventory_movement_org_branch_product
    ON inventory_movement (organization_id, branch_id, product_id);
CREATE INDEX IF NOT EXISTS idx_inventory_movement_occurred
    ON inventory_movement (occurred_at);

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