-- Migración incremental: proveedores, órdenes de compra y recepciones (G2.7).
-- Las existencias se modifican únicamente mediante inventory_movement durante una recepción.
CREATE TABLE IF NOT EXISTS supplier (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    code TEXT,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    contact TEXT,
    rfc TEXT,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    UNIQUE (organization_id, code)
);

CREATE INDEX IF NOT EXISTS idx_supplier_org_name ON supplier (organization_id, name);

CREATE TABLE IF NOT EXISTS purchase_order (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL REFERENCES branch(id),
    supplier_id TEXT NOT NULL REFERENCES supplier(id),
    folio TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'ordered', 'partially_received', 'received')),
    created_by TEXT NOT NULL REFERENCES app_user(id),
    created_at TEXT NOT NULL,
    UNIQUE (organization_id, branch_id, folio)
);

CREATE INDEX IF NOT EXISTS idx_purchase_order_org_branch_status ON purchase_order (organization_id, branch_id, status, created_at DESC);

CREATE TABLE IF NOT EXISTS purchase_order_line (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES purchase_order(id),
    product_id TEXT NOT NULL REFERENCES product(id),
    requested_quantity_milli INTEGER NOT NULL CHECK (requested_quantity_milli > 0),
    received_quantity_milli INTEGER NOT NULL DEFAULT 0 CHECK (received_quantity_milli >= 0 AND received_quantity_milli <= requested_quantity_milli),
    unit_cost_minor INTEGER NOT NULL CHECK (unit_cost_minor >= 0),
    UNIQUE (order_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_purchase_order_line_order ON purchase_order_line (order_id);

CREATE TABLE IF NOT EXISTS purchase_receipt (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL REFERENCES branch(id),
    order_id TEXT NOT NULL REFERENCES purchase_order(id),
    received_by TEXT NOT NULL REFERENCES app_user(id),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    received_at TEXT NOT NULL,
    UNIQUE (organization_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS purchase_receipt_line (
    receipt_id TEXT NOT NULL REFERENCES purchase_receipt(id),
    order_line_id TEXT NOT NULL REFERENCES purchase_order_line(id),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    PRIMARY KEY (receipt_id, order_line_id)
);

CREATE INDEX IF NOT EXISTS idx_purchase_receipt_order ON purchase_receipt (organization_id, order_id, received_at DESC);

-- SQLite no dispone de FKs compuestas sobre las claves históricas; estas
-- comprobaciones impiden referencias de otra organización en los hechos nuevos.
CREATE TRIGGER IF NOT EXISTS purchase_order_same_organization
BEFORE INSERT ON purchase_order
WHEN (SELECT organization_id FROM branch WHERE id = NEW.branch_id) <> NEW.organization_id
  OR (SELECT organization_id FROM supplier WHERE id = NEW.supplier_id) <> NEW.organization_id
BEGIN SELECT RAISE(ABORT, 'purchase_order debe pertenecer a la organización'); END;
CREATE TRIGGER IF NOT EXISTS purchase_receipt_same_organization
BEFORE INSERT ON purchase_receipt
WHEN (SELECT organization_id FROM purchase_order WHERE id = NEW.order_id) <> NEW.organization_id
  OR (SELECT branch_id FROM purchase_order WHERE id = NEW.order_id) <> NEW.branch_id
BEGIN SELECT RAISE(ABORT, 'purchase_receipt debe pertenecer a la orden'); END;

CREATE TRIGGER IF NOT EXISTS purchase_receipt_no_update
BEFORE UPDATE ON purchase_receipt BEGIN SELECT RAISE(ABORT, 'purchase_receipt es append-only'); END;
CREATE TRIGGER IF NOT EXISTS purchase_receipt_no_delete
BEFORE DELETE ON purchase_receipt BEGIN SELECT RAISE(ABORT, 'purchase_receipt es append-only'); END;
CREATE TRIGGER IF NOT EXISTS purchase_receipt_line_no_update
BEFORE UPDATE ON purchase_receipt_line BEGIN SELECT RAISE(ABORT, 'purchase_receipt_line es append-only'); END;
CREATE TRIGGER IF NOT EXISTS purchase_receipt_line_no_delete
BEFORE DELETE ON purchase_receipt_line BEGIN SELECT RAISE(ABORT, 'purchase_receipt_line es append-only'); END;
