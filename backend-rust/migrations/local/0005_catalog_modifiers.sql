-- Migración incremental: modificadores de catálogo (G2.2B).
CREATE TABLE IF NOT EXISTS modifier_group (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    name TEXT NOT NULL,
    min_selectable INTEGER NOT NULL CHECK (min_selectable >= 0),
    max_selectable INTEGER NOT NULL CHECK (max_selectable >= min_selectable),
    sort_order INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (product_id) REFERENCES product(id)
);

CREATE INDEX IF NOT EXISTS idx_modifier_group_org_product
    ON modifier_group (organization_id, product_id, sort_order);

CREATE TABLE IF NOT EXISTS modifier_option (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('choice', 'extra', 'omission')),
    price_delta_minor INTEGER NOT NULL CHECK (price_delta_minor >= 0),
    sort_order INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (group_id) REFERENCES modifier_group(id)
);

CREATE INDEX IF NOT EXISTS idx_modifier_option_org_group
    ON modifier_option (organization_id, group_id, sort_order);