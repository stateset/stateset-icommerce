-- Cycle counts: scheduled physical inventory counts with per-SKU expected /
-- counted quantities and variances. Completing a count applies variance
-- adjustments to location_inventory and records cycle_count movements.
--
-- Repository: crates/stateset-db/src/sqlite/warehouse.rs
-- REST:       crates/stateset-http/src/routes/warehouse.rs
--
-- Quantities as TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS cycle_counts (
    id TEXT PRIMARY KEY,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    location_id INTEGER REFERENCES locations(id),
    status TEXT NOT NULL DEFAULT 'draft',
    scheduled_date TEXT,
    counted_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_cycle_counts_warehouse ON cycle_counts(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_cycle_counts_status ON cycle_counts(status);

CREATE TABLE IF NOT EXISTS cycle_count_lines (
    id TEXT PRIMARY KEY,
    cycle_count_id TEXT NOT NULL REFERENCES cycle_counts(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    -- Lot-less lines store '' to match the location_inventory convention.
    lot_id TEXT NOT NULL DEFAULT '',
    expected_quantity TEXT NOT NULL DEFAULT '0',
    counted_quantity TEXT,
    variance TEXT,
    UNIQUE (cycle_count_id, sku, lot_id)
);
CREATE INDEX IF NOT EXISTS idx_cycle_count_lines_cc ON cycle_count_lines(cycle_count_id);
