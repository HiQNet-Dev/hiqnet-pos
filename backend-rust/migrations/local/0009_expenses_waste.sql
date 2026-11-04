-- Migración incremental G2.8: gastos operativos y mermas.
-- Los efectos físicos se registran exclusivamente en cash_movement e inventory_movement.
CREATE TABLE expense_category (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    UNIQUE (organization_id, name)
);

CREATE TABLE expense (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL REFERENCES branch(id),
    category_id TEXT NOT NULL REFERENCES expense_category(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    concept TEXT NOT NULL CHECK (length(trim(concept)) > 0),
    cash_shift_id TEXT REFERENCES cash_shift(id),
    cash_movement_id TEXT UNIQUE REFERENCES cash_movement(id),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES app_user(id),
    occurred_at TEXT NOT NULL,
    UNIQUE (organization_id, idempotency_key),
    CHECK ((cash_shift_id IS NULL AND cash_movement_id IS NULL) OR (cash_shift_id IS NOT NULL AND cash_movement_id IS NOT NULL))
);
CREATE INDEX idx_expense_org_branch_occurred ON expense (organization_id, branch_id, occurred_at DESC);

CREATE TABLE waste_record (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL REFERENCES branch(id),
    product_id TEXT NOT NULL REFERENCES product(id),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    inventory_movement_id TEXT NOT NULL UNIQUE REFERENCES inventory_movement(id),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES app_user(id),
    occurred_at TEXT NOT NULL,
    UNIQUE (organization_id, idempotency_key)
);
CREATE INDEX idx_waste_org_branch_occurred ON waste_record (organization_id, branch_id, occurred_at DESC);

CREATE TRIGGER expense_same_scope
BEFORE INSERT ON expense
WHEN (SELECT organization_id FROM branch WHERE id = NEW.branch_id) <> NEW.organization_id
  OR (SELECT organization_id FROM expense_category WHERE id = NEW.category_id) <> NEW.organization_id
  OR (NEW.cash_shift_id IS NOT NULL AND (SELECT branch_id FROM cash_shift WHERE id = NEW.cash_shift_id) <> NEW.branch_id)
BEGIN SELECT RAISE(ABORT, 'expense debe pertenecer a la organización y sucursal'); END;
CREATE TRIGGER waste_same_scope
BEFORE INSERT ON waste_record
WHEN (SELECT organization_id FROM branch WHERE id = NEW.branch_id) <> NEW.organization_id
  OR (SELECT organization_id FROM product WHERE id = NEW.product_id) <> NEW.organization_id
BEGIN SELECT RAISE(ABORT, 'waste_record debe pertenecer a la organización y sucursal'); END;
CREATE TRIGGER expense_no_update BEFORE UPDATE ON expense BEGIN SELECT RAISE(ABORT, 'expense es append-only'); END;
CREATE TRIGGER expense_no_delete BEFORE DELETE ON expense BEGIN SELECT RAISE(ABORT, 'expense es append-only'); END;
CREATE TRIGGER waste_no_update BEFORE UPDATE ON waste_record BEGIN SELECT RAISE(ABORT, 'waste_record es append-only'); END;
CREATE TRIGGER waste_no_delete BEFORE DELETE ON waste_record BEGIN SELECT RAISE(ABORT, 'waste_record es append-only'); END;
