-- Supplier SKUs: per-supplier SKU and unit-cost overrides for internal products.
--
-- Repository: crates/stateset-db/src/sqlite/supplier_skus.rs
-- REST:       crates/stateset-http/src/routes/supplier_skus.rs
--
-- Money/decimals stored as TEXT; booleans as INTEGER 0/1; timestamps RFC3339 TEXT.
-- The (product_id, supplier_id, sku) triple is unique.

CREATE TABLE IF NOT EXISTS supplier_skus (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    unit_cost TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    min_order_qty TEXT,
    lead_time_days INTEGER,
    is_preferred INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (product_id, supplier_id, sku)
);
CREATE INDEX IF NOT EXISTS idx_supplier_skus_supplier ON supplier_skus(supplier_id);
CREATE INDEX IF NOT EXISTS idx_supplier_skus_product ON supplier_skus(product_id);
