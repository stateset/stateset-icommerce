-- Vendor returns (return-to-supplier / RTV): sending received goods back to a
-- supplier, the AP-side mirror of customer returns.
--
-- Repository: crates/stateset-db/src/sqlite/vendor_returns.rs
-- REST:       crates/stateset-http/src/routes/vendor_returns.rs
--
-- Money/decimals stored as TEXT; booleans as INTEGER 0/1; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS vendor_returns (
    id TEXT PRIMARY KEY,
    number TEXT NOT NULL,
    supplier_id TEXT NOT NULL,
    purchase_order_id TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    currency TEXT NOT NULL DEFAULT 'USD',
    credit_generated INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    processed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_vendor_returns_supplier ON vendor_returns(supplier_id);
CREATE INDEX IF NOT EXISTS idx_vendor_returns_status ON vendor_returns(status);

CREATE TABLE IF NOT EXISTS vendor_return_items (
    id TEXT PRIMARY KEY,
    vendor_return_id TEXT NOT NULL REFERENCES vendor_returns(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_cost TEXT NOT NULL DEFAULT '0',
    reason TEXT NOT NULL DEFAULT 'defective'
);
CREATE INDEX IF NOT EXISTS idx_vendor_return_items_return ON vendor_return_items(vendor_return_id);
