-- Warehouse bins (bin-level sub-allocation of warehouse stock) and
-- return-item disposition. See sqlite migration 068 for the invariant.
-- Numbered 073 (one gap after 071) to sort after a concurrently added 072.

CREATE TABLE IF NOT EXISTS warehouse_bins (
    id SERIAL PRIMARY KEY,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    zone TEXT,
    aisle TEXT,
    rack TEXT,
    shelf TEXT,
    position TEXT,
    bin_type TEXT NOT NULL DEFAULT 'pick',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    capacity NUMERIC(19, 4),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(warehouse_id, code)
);
CREATE INDEX IF NOT EXISTS idx_warehouse_bins_warehouse ON warehouse_bins(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_warehouse_bins_type ON warehouse_bins(bin_type);

CREATE TABLE IF NOT EXISTS inventory_bin_levels (
    bin_id INTEGER NOT NULL REFERENCES warehouse_bins(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    quantity_on_hand NUMERIC(19, 4) NOT NULL DEFAULT 0,
    quantity_allocated NUMERIC(19, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (bin_id, sku)
);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_levels_sku ON inventory_bin_levels(sku);

CREATE TABLE IF NOT EXISTS inventory_bin_movements (
    id UUID PRIMARY KEY,
    movement_type TEXT NOT NULL,
    from_bin_id INTEGER REFERENCES warehouse_bins(id) ON DELETE SET NULL,
    to_bin_id INTEGER REFERENCES warehouse_bins(id) ON DELETE SET NULL,
    sku TEXT NOT NULL,
    quantity NUMERIC(19, 4) NOT NULL,
    reason TEXT,
    reference_type TEXT,
    reference_id TEXT,
    performed_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_movements_sku ON inventory_bin_movements(sku);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_movements_from ON inventory_bin_movements(from_bin_id);
CREATE INDEX IF NOT EXISTS idx_inventory_bin_movements_to ON inventory_bin_movements(to_bin_id);

ALTER TABLE return_items ADD COLUMN IF NOT EXISTS disposition TEXT;
ALTER TABLE return_items ADD COLUMN IF NOT EXISTS disposition_at TIMESTAMPTZ;
ALTER TABLE return_items ADD COLUMN IF NOT EXISTS disposition_by TEXT;
