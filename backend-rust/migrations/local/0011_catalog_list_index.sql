-- El catálogo se lee completo por organización y se ordena por nombre para Ventas.
-- Este índice compuesto evita ordenar temporalmente catálogos grandes al resolver
-- `list_products`; no cambia datos comerciales ni el contrato IPC.
CREATE INDEX IF NOT EXISTS idx_product_org_name
    ON product (organization_id, name COLLATE NOCASE);
