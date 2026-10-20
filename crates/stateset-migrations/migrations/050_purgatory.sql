-- Purgatory: orders ingested from a channel that are non-posted, pending SKU
-- mapping / line resolution before they enter inventory and accounting.
--
-- Repository: crates/stateset-db/src/sqlite/purgatory.rs
-- REST:       crates/stateset-http/src/routes/purgatory.rs
--
-- Booleans as INTEGER 0/1; quantities/metadata as TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS purgatory_orders (
    id TEXT PRIMARY KEY,
    channel_id TEXT,
    external_order_id TEXT NOT NULL,
    external_status TEXT,
    is_posted INTEGER NOT NULL DEFAULT 0,
    hold_reason TEXT,
    metadata TEXT NOT NULL DEFAULT 'null',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_purgatory_orders_posted ON purgatory_orders(is_posted);
CREATE INDEX IF NOT EXISTS idx_purgatory_orders_channel ON purgatory_orders(channel_id);

CREATE TABLE IF NOT EXISTS purgatory_line_items (
    id TEXT PRIMARY KEY,
    purgatory_order_id TEXT NOT NULL REFERENCES purgatory_orders(id) ON DELETE CASCADE,
    external_sku TEXT NOT NULL,
    product_id TEXT,
    quantity TEXT NOT NULL DEFAULT '0',
    ignore_item INTEGER NOT NULL DEFAULT 0,
    non_physical INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_purgatory_line_items_order ON purgatory_line_items(purgatory_order_id);
