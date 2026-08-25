-- Warehouse bins (bin-level sub-allocation of warehouse stock) and
-- return-item disposition.
--
-- Bins are a sub-allocation of warehouse-level inventory
-- (inventory_balances, keyed by location_id = warehouses.id): for every
-- (warehouse, sku) the sum of inventory_bin_levels.quantity_on_hand must
-- equal inventory_balances.quantity_on_hand. adjust_bin_level applies its
-- delta to both in one transaction; move_between_bins is stock-neutral.
--
-- Repository: crates/stateset-db/src/sqlite/bins.rs, sqlite/returns.rs
-- REST:       crates/stateset-http/src/routes/warehouse.rs, routes/returns.rs
--
-- Numbered 068 (one gap after 066) to sort after a concurrently added 067.
-- Quantities as TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS warehouse_bins (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    zone TEXT,
    aisle TEXT,
    rack TEXT,
    shelf TEXT,
    position TEXT,
    -- One of pick, bulk, receiving, staging, quarantine, returns.
    bin_type TEXT NOT NULL DEFAULT 'pick',
    is_active INTEGER NOT NULL DEFAULT 1,
    capacity TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(warehouse_id, code)
);
CREATE INDEX IF NOT EXISTS idx_warehouse_bins_warehouse ON warehouse_bins(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_warehouse_bins_type ON warehouse_bins(bin_type);

CREATE TABLE IF NOT EXISTS inventory_bin_levels (
    bin_id INTEGER NOT NULL REFERENCES warehouse_bins(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    quantity_on_hand TEXT NOT NULL DEFAULT '0',
    quantity_allocated TEXT NOT NULL DEFAULT '0',
    updated_at TEXT NOT NULL,
    PRIMARY KEY (bin_id, sku)
);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_levels_sku ON inventory_bin_levels(sku);

CREATE TABLE IF NOT EXISTS inventory_bin_movements (
    id TEXT PRIMARY KEY,
    -- One of transfer, adjustment, return_disposition.
    movement_type TEXT NOT NULL,
    from_bin_id INTEGER REFERENCES warehouse_bins(id) ON DELETE SET NULL,
    to_bin_id INTEGER REFERENCES warehouse_bins(id) ON DELETE SET NULL,
    sku TEXT NOT NULL,
    quantity TEXT NOT NULL,
    reason TEXT,
    reference_type TEXT,
    reference_id TEXT,
    performed_by TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_movements_sku ON inventory_bin_movements(sku);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_movements_from ON inventory_bin_movements(from_bin_id);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_movements_to ON inventory_bin_movements(to_bin_id);

-- Return item disposition (restock, refurbish, scrap, return_to_vendor, quarantine).
ALTER TABLE return_items ADD COLUMN disposition TEXT;
ALTER TABLE return_items ADD COLUMN disposition_at TEXT;
ALTER TABLE return_items ADD COLUMN disposition_by TEXT;
