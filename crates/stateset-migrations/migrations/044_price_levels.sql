-- Price levels: named B2B pricing tiers with catalog-wide adjustments and
-- optional per-product fixed-price entries.
--
-- Repository: crates/stateset-db/src/sqlite/price_levels.rs
-- REST:       crates/stateset-http/src/routes/price_levels.rs
--
-- Money/decimals stored as TEXT; booleans as INTEGER 0/1; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS price_levels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    description TEXT,
    adjustment_type TEXT NOT NULL DEFAULT 'none',
    adjustment_value TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_price_levels_active ON price_levels(is_active);

CREATE TABLE IF NOT EXISTS price_level_entries (
    price_level_id TEXT NOT NULL REFERENCES price_levels(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    price TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (price_level_id, product_id)
);
