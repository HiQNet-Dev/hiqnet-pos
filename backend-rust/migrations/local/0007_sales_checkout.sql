-- Migración incremental: ventas, partidas y pagos (G2.4).
CREATE TABLE IF NOT EXISTS sale (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    register_id TEXT NOT NULL,
    shift_id TEXT NOT NULL,
    operator_id TEXT NOT NULL,
    folio TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('completed')),
    subtotal_minor INTEGER NOT NULL CHECK (subtotal_minor >= 0),
    tax_minor INTEGER NOT NULL CHECK (tax_minor >= 0),
    total_minor INTEGER NOT NULL CHECK (total_minor >= 0),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (branch_id) REFERENCES branch(id),
    FOREIGN KEY (register_id) REFERENCES cash_register(id),
    FOREIGN KEY (shift_id) REFERENCES cash_shift(id),
    UNIQUE (organization_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_sale_org_occurred
    ON sale (organization_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_sale_shift
    ON sale (organization_id, shift_id);

CREATE TABLE IF NOT EXISTS sale_item (
    id TEXT PRIMARY KEY,
    sale_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    product_name TEXT NOT NULL,
    variant_name TEXT,
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    unit_tax_minor INTEGER NOT NULL CHECK (unit_tax_minor >= 0),
    line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0),
    modifiers_summary TEXT NOT NULL,
    FOREIGN KEY (sale_id) REFERENCES sale(id),
    FOREIGN KEY (product_id) REFERENCES product(id)
);

CREATE INDEX IF NOT EXISTS idx_sale_item_sale
    ON sale_item (sale_id);

CREATE TABLE IF NOT EXISTS sale_payment (
    id TEXT PRIMARY KEY,
    sale_id TEXT NOT NULL,
    method TEXT NOT NULL CHECK (method IN ('cash')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0),
    change_minor INTEGER NOT NULL CHECK (change_minor >= 0),
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (sale_id) REFERENCES sale(id)
);

CREATE INDEX IF NOT EXISTS idx_sale_payment_sale
    ON sale_payment (sale_id);