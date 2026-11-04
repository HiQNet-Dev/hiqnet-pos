-- Migración incremental: cajas, turnos y movimientos de efectivo (G2.3).
CREATE TABLE IF NOT EXISTS cash_register (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    name TEXT NOT NULL,
    code TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (branch_id) REFERENCES branch(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_cash_register_org_branch_code
    ON cash_register (organization_id, branch_id, code);

CREATE TABLE IF NOT EXISTS cash_shift (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    register_id TEXT NOT NULL,
    opened_by TEXT NOT NULL,
    opened_at TEXT NOT NULL,
    opening_amount_minor INTEGER NOT NULL CHECK (opening_amount_minor >= 0),
    status TEXT NOT NULL CHECK (status IN ('open', 'closed')),
    closed_by TEXT,
    closed_at TEXT,
    expected_amount_minor INTEGER,
    counted_amount_minor INTEGER,
    difference_minor INTEGER,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (branch_id) REFERENCES branch(id),
    FOREIGN KEY (register_id) REFERENCES cash_register(id)
);

-- Un único turno abierto por caja.
CREATE UNIQUE INDEX IF NOT EXISTS idx_cash_shift_one_open
    ON cash_shift (register_id)
    WHERE status = 'open';

CREATE INDEX IF NOT EXISTS idx_cash_shift_org_register_opened
    ON cash_shift (organization_id, register_id, opened_at);

CREATE TABLE IF NOT EXISTS cash_movement (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    register_id TEXT NOT NULL,
    shift_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('in', 'out')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    reason TEXT NOT NULL,
    actor_user_id TEXT,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (branch_id) REFERENCES branch(id),
    FOREIGN KEY (register_id) REFERENCES cash_register(id),
    FOREIGN KEY (shift_id) REFERENCES cash_shift(id)
);

CREATE INDEX IF NOT EXISTS idx_cash_movement_shift
    ON cash_movement (organization_id, shift_id, occurred_at);

CREATE TRIGGER IF NOT EXISTS cash_movement_no_update
BEFORE UPDATE ON cash_movement
BEGIN
    SELECT RAISE(ABORT, 'cash_movement es append-only');
END;

CREATE TRIGGER IF NOT EXISTS cash_movement_no_delete
BEFORE DELETE ON cash_movement
BEGIN
    SELECT RAISE(ABORT, 'cash_movement es append-only');
END;