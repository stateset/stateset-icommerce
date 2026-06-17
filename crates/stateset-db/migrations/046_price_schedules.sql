-- Price schedules: time-bounded sets of product price overrides (promotional
-- windows, seasonal lists). Per-product prices live in entries.
--
-- Repository: crates/stateset-db/src/sqlite/price_schedules.rs
-- REST:       crates/stateset-http/src/routes/price_schedules.rs
--
-- Money stored as TEXT; booleans as INTEGER 0/1; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS price_schedules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    starts_at TEXT,
    ends_at TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_price_schedules_active ON price_schedules(is_active);

CREATE TABLE IF NOT EXISTS price_schedule_entries (
    price_schedule_id TEXT NOT NULL REFERENCES price_schedules(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    price TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (price_schedule_id, product_id)
);
CREATE INDEX IF NOT EXISTS idx_price_schedule_entries_product ON price_schedule_entries(product_id);
