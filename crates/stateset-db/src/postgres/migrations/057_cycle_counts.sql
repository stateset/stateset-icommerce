-- Cycle counts: scheduled physical inventory counts with per-SKU expected /
-- counted quantities and variances. Completing a count applies variance
-- adjustments to location_inventory and records cycle_count movements.

CREATE TABLE IF NOT EXISTS cycle_counts (
    id UUID PRIMARY KEY,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    location_id INTEGER REFERENCES locations(id),
    status TEXT NOT NULL DEFAULT 'draft',
    scheduled_date TIMESTAMPTZ,
    counted_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_cycle_counts_warehouse ON cycle_counts(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_cycle_counts_status ON cycle_counts(status);

CREATE TABLE IF NOT EXISTS cycle_count_lines (
    id UUID PRIMARY KEY,
    cycle_count_id UUID NOT NULL REFERENCES cycle_counts(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    -- Lot-less lines store the nil UUID to match the location_inventory convention.
    lot_id UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000',
    expected_quantity NUMERIC NOT NULL DEFAULT 0,
    counted_quantity NUMERIC,
    variance NUMERIC,
    UNIQUE (cycle_count_id, sku, lot_id)
);
CREATE INDEX IF NOT EXISTS idx_cycle_count_lines_cc ON cycle_count_lines(cycle_count_id);
