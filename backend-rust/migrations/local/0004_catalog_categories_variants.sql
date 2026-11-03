-- Migración incremental: categorías y variantes de producto (G2.2A).
CREATE TABLE IF NOT EXISTS category (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id)
);

CREATE INDEX IF NOT EXISTS idx_category_org_order
    ON category (organization_id, sort_order, name);

ALTER TABLE product ADD COLUMN category_id TEXT REFERENCES category(id);

CREATE TABLE IF NOT EXISTS product_variant (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    name TEXT NOT NULL,
    sku TEXT,
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (product_id) REFERENCES product(id)
);

CREATE INDEX IF NOT EXISTS idx_product_variant_org_product
    ON product_variant (organization_id, product_id, sort_order);

CREATE UNIQUE INDEX IF NOT EXISTS idx_product_variant_org_sku
    ON product_variant (organization_id, sku)
    WHERE sku IS NOT NULL;