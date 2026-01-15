-- Warehouse and Location Management schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS warehouses (
    id SERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    warehouse_type TEXT NOT NULL DEFAULT 'distribution',
    address_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    timezone TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_warehouses_code ON warehouses(code);
CREATE INDEX IF NOT EXISTS idx_warehouses_type ON warehouses(warehouse_type);
CREATE INDEX IF NOT EXISTS idx_warehouses_active ON warehouses(is_active);

CREATE TABLE IF NOT EXISTS warehouse_zones (
    id SERIAL PRIMARY KEY,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (warehouse_id, code)
);

CREATE INDEX IF NOT EXISTS idx_warehouse_zones_warehouse ON warehouse_zones(warehouse_id);

CREATE TABLE IF NOT EXISTS locations (
    id SERIAL PRIMARY KEY,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    location_type TEXT NOT NULL DEFAULT 'bulk',
    zone TEXT,
    aisle TEXT,
    rack TEXT,
    level TEXT,
    bin TEXT,
    max_weight_kg NUMERIC(12, 4),
    max_volume_m3 NUMERIC(12, 4),
    current_weight_kg NUMERIC(12, 4),
    current_volume_m3 NUMERIC(12, 4),
    is_pickable BOOLEAN NOT NULL DEFAULT TRUE,
    is_receivable BOOLEAN NOT NULL DEFAULT TRUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (warehouse_id, code)
);

CREATE INDEX IF NOT EXISTS idx_locations_warehouse ON locations(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_locations_type ON locations(location_type);
CREATE INDEX IF NOT EXISTS idx_locations_zone ON locations(zone);
CREATE INDEX IF NOT EXISTS idx_locations_pickable ON locations(is_pickable);
CREATE INDEX IF NOT EXISTS idx_locations_receivable ON locations(is_receivable);
CREATE INDEX IF NOT EXISTS idx_locations_active ON locations(is_active);

CREATE TABLE IF NOT EXISTS location_inventory (
    location_id INTEGER NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    lot_id UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000',
    quantity_on_hand NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_reserved NUMERIC(12, 4) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (location_id, sku, lot_id)
);

CREATE INDEX IF NOT EXISTS idx_location_inventory_sku ON location_inventory(sku);
CREATE INDEX IF NOT EXISTS idx_location_inventory_lot ON location_inventory(lot_id);

CREATE TABLE IF NOT EXISTS inventory_movements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    movement_type TEXT NOT NULL,
    from_location_id INTEGER REFERENCES locations(id),
    to_location_id INTEGER REFERENCES locations(id),
    sku TEXT NOT NULL,
    lot_id UUID,
    quantity NUMERIC(12, 4) NOT NULL,
    reference_type TEXT,
    reference_id UUID,
    reason TEXT,
    performed_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_movements_type ON inventory_movements(movement_type);
CREATE INDEX IF NOT EXISTS idx_movements_from ON inventory_movements(from_location_id);
CREATE INDEX IF NOT EXISTS idx_movements_to ON inventory_movements(to_location_id);
CREATE INDEX IF NOT EXISTS idx_movements_sku ON inventory_movements(sku);
CREATE INDEX IF NOT EXISTS idx_movements_lot ON inventory_movements(lot_id);
CREATE INDEX IF NOT EXISTS idx_movements_created ON inventory_movements(created_at);
CREATE INDEX IF NOT EXISTS idx_movements_reference ON inventory_movements(reference_type, reference_id);
