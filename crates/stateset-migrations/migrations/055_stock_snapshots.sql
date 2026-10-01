-- Stock snapshots: point-in-time captures of on-hand / available inventory per
-- SKU, for valuation, reconciliation, and export.
--
-- Repository: crates/stateset-db/src/sqlite/stock_snapshots.rs
-- REST:       crates/stateset-http/src/routes/stock_snapshots.rs
--
-- Quantities as TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS stock_snapshots (
    id TEXT PRIMARY KEY,
    label TEXT,
    total_skus INTEGER NOT NULL DEFAULT 0,
    total_units TEXT NOT NULL DEFAULT '0',
    captured_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_stock_snapshots_captured ON stock_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS stock_snapshot_lines (
    id TEXT PRIMARY KEY,
    stock_snapshot_id TEXT NOT NULL REFERENCES stock_snapshots(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity_on_hand TEXT NOT NULL DEFAULT '0',
    quantity_available TEXT NOT NULL DEFAULT '0',
    location TEXT
);
CREATE INDEX IF NOT EXISTS idx_stock_snapshot_lines_snapshot ON stock_snapshot_lines(stock_snapshot_id);
