-- Migración incremental G2.9: devoluciones de venta.
-- La venta original permanece intacta; los efectos físicos siguen siendo
-- cash_movement (una salida) e inventory_movement (un inbound si es reintegrable).
CREATE TABLE sale_return (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL REFERENCES branch(id),
    sale_id TEXT NOT NULL REFERENCES sale(id),
    shift_id TEXT NOT NULL REFERENCES cash_shift(id),
    cash_movement_id TEXT NOT NULL UNIQUE REFERENCES cash_movement(id),
    operator_id TEXT NOT NULL REFERENCES app_user(id),
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    refund_total_minor INTEGER NOT NULL CHECK (refund_total_minor > 0),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE (organization_id, idempotency_key)
);

CREATE INDEX idx_sale_return_org_branch_occurred
    ON sale_return (organization_id, branch_id, occurred_at DESC);
CREATE INDEX idx_sale_return_sale
    ON sale_return (organization_id, sale_id);

CREATE TABLE sale_return_line (
    id TEXT PRIMARY KEY,
    return_id TEXT NOT NULL REFERENCES sale_return(id),
    sale_item_id TEXT NOT NULL REFERENCES sale_item(id),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    disposition TEXT NOT NULL CHECK (disposition IN ('restock', 'discard')),
    refund_net_minor INTEGER NOT NULL CHECK (refund_net_minor >= 0),
    refund_tax_minor INTEGER NOT NULL CHECK (refund_tax_minor >= 0),
    refund_total_minor INTEGER NOT NULL CHECK (refund_total_minor >= 0),
    inbound_movement_id TEXT UNIQUE REFERENCES inventory_movement(id),
    UNIQUE (return_id, sale_item_id),
    CHECK (refund_total_minor = refund_net_minor + refund_tax_minor),
    CHECK (
        (disposition = 'restock' AND inbound_movement_id IS NOT NULL)
        OR (disposition = 'discard' AND inbound_movement_id IS NULL)
    )
);

CREATE INDEX idx_sale_return_line_return
    ON sale_return_line (return_id);
CREATE INDEX idx_sale_return_line_item
    ON sale_return_line (sale_item_id);

-- La devolución debe pertenecer a la misma organización y sucursal que la venta,
-- y el reembolso debe salir de un turno de esa sucursal.
CREATE TRIGGER sale_return_same_scope
BEFORE INSERT ON sale_return
WHEN (SELECT branch_id FROM sale WHERE id = NEW.sale_id) <> NEW.branch_id
  OR (SELECT organization_id FROM sale WHERE id = NEW.sale_id) <> NEW.organization_id
  OR (SELECT branch_id FROM cash_shift WHERE id = NEW.shift_id) <> NEW.branch_id
  OR (SELECT organization_id FROM cash_shift WHERE id = NEW.shift_id) <> NEW.organization_id
BEGIN SELECT RAISE(ABORT, 'sale_return debe pertenecer a la venta y sucursal'); END;

CREATE TRIGGER sale_return_line_same_sale
BEFORE INSERT ON sale_return_line
WHEN (SELECT sale_id FROM sale_item WHERE id = NEW.sale_item_id)
     <> (SELECT sale_id FROM sale_return WHERE id = NEW.return_id)
BEGIN SELECT RAISE(ABORT, 'sale_return_line debe pertenecer a la venta de la devolución'); END;

-- Guarda de concurrencia: la suma de devoluciones de una partida nunca excede
-- la cantidad vendida, incluso si dos procesos compiten.
CREATE TRIGGER sale_return_line_quantity_guard
BEFORE INSERT ON sale_return_line
WHEN (SELECT COALESCE(SUM(quantity_milli), 0) FROM sale_return_line
      WHERE sale_item_id = NEW.sale_item_id) + NEW.quantity_milli
     > (SELECT quantity_milli FROM sale_item WHERE id = NEW.sale_item_id)
BEGIN SELECT RAISE(ABORT, 'devolucion excede la cantidad vendida'); END;

CREATE TRIGGER sale_return_no_update
BEFORE UPDATE ON sale_return BEGIN SELECT RAISE(ABORT, 'sale_return es append-only'); END;
CREATE TRIGGER sale_return_no_delete
BEFORE DELETE ON sale_return BEGIN SELECT RAISE(ABORT, 'sale_return es append-only'); END;
CREATE TRIGGER sale_return_line_no_update
BEFORE UPDATE ON sale_return_line BEGIN SELECT RAISE(ABORT, 'sale_return_line es append-only'); END;
CREATE TRIGGER sale_return_line_no_delete
BEFORE DELETE ON sale_return_line BEGIN SELECT RAISE(ABORT, 'sale_return_line es append-only'); END;
