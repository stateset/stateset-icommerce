-- Warehouse and Location Management Schema
-- Migration: 016_warehouse.sql

-- ============================================================================
-- Warehouses
-- ============================================================================

CREATE TABLE IF NOT EXISTS warehouses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    warehouse_type TEXT NOT NULL DEFAULT 'distribution',
    -- Address fields (embedded JSON for simplicity)
    address_json TEXT NOT NULL DEFAULT '{}',
    timezone TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_warehouses_code ON warehouses(code);
CREATE INDEX IF NOT EXISTS idx_warehouses_type ON warehouses(warehouse_type);
CREATE INDEX IF NOT EXISTS idx_warehouses_active ON warehouses(is_active);

-- ============================================================================
-- Zones (groupings of locations within a warehouse)
-- ============================================================================

CREATE TABLE IF NOT EXISTS warehouse_zones (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(warehouse_id, code)
);

CREATE INDEX IF NOT EXISTS idx_warehouse_zones_warehouse ON warehouse_zones(warehouse_id);

-- ============================================================================
-- Locations (bins/slots within a warehouse)
-- ============================================================================

CREATE TABLE IF NOT EXISTS locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    location_type TEXT NOT NULL DEFAULT 'bulk',
    zone TEXT,
    aisle TEXT,
    rack TEXT,
    level TEXT,
    bin TEXT,
    -- Capacity constraints
    max_weight_kg TEXT,
    max_volume_m3 TEXT,
    current_weight_kg TEXT,
    current_volume_m3 TEXT,
    -- Flags
    is_pickable INTEGER NOT NULL DEFAULT 1,
    is_receivable INTEGER NOT NULL DEFAULT 1,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(warehouse_id, code)
);

CREATE INDEX IF NOT EXISTS idx_locations_warehouse ON locations(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_locations_type ON locations(location_type);
CREATE INDEX IF NOT EXISTS idx_locations_zone ON locations(zone);
CREATE INDEX IF NOT EXISTS idx_locations_pickable ON locations(is_pickable);
CREATE INDEX IF NOT EXISTS idx_locations_receivable ON locations(is_receivable);
CREATE INDEX IF NOT EXISTS idx_locations_active ON locations(is_active);

-- ============================================================================
-- Location Inventory (inventory at each location)
-- ============================================================================

CREATE TABLE IF NOT EXISTS location_inventory (
    location_id INTEGER NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    lot_id TEXT NOT NULL DEFAULT '',
    quantity_on_hand TEXT NOT NULL DEFAULT '0',
    quantity_reserved TEXT NOT NULL DEFAULT '0',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (location_id, sku, lot_id)
);

CREATE INDEX IF NOT EXISTS idx_location_inventory_sku ON location_inventory(sku);
CREATE INDEX IF NOT EXISTS idx_location_inventory_lot ON location_inventory(lot_id);

-- ============================================================================
-- Inventory Movements (audit trail of inventory changes)
-- ============================================================================

CREATE TABLE IF NOT EXISTS inventory_movements (
    id TEXT PRIMARY KEY NOT NULL,
    movement_type TEXT NOT NULL,
    from_location_id INTEGER REFERENCES locations(id),
    to_location_id INTEGER REFERENCES locations(id),
    sku TEXT NOT NULL,
    lot_id TEXT,
    quantity TEXT NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    reason TEXT,
    performed_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_movements_type ON inventory_movements(movement_type);
CREATE INDEX IF NOT EXISTS idx_movements_from ON inventory_movements(from_location_id);
CREATE INDEX IF NOT EXISTS idx_movements_to ON inventory_movements(to_location_id);
CREATE INDEX IF NOT EXISTS idx_movements_sku ON inventory_movements(sku);
CREATE INDEX IF NOT EXISTS idx_movements_lot ON inventory_movements(lot_id);
CREATE INDEX IF NOT EXISTS idx_movements_created ON inventory_movements(created_at);
CREATE INDEX IF NOT EXISTS idx_movements_reference ON inventory_movements(reference_type, reference_id);

-- ============================================================================
-- Triggers
-- ============================================================================

-- Auto-update updated_at for warehouses
CREATE TRIGGER IF NOT EXISTS update_warehouses_timestamp
    AFTER UPDATE ON warehouses
    FOR EACH ROW
BEGIN
    UPDATE warehouses SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Auto-update updated_at for locations
CREATE TRIGGER IF NOT EXISTS update_locations_timestamp
    AFTER UPDATE ON locations
    FOR EACH ROW
BEGIN
    UPDATE locations SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Auto-update updated_at for location_inventory
CREATE TRIGGER IF NOT EXISTS update_location_inventory_timestamp
    AFTER UPDATE ON location_inventory
    FOR EACH ROW
BEGIN
    UPDATE location_inventory SET updated_at = datetime('now')
    WHERE location_id = NEW.location_id AND sku = NEW.sku AND COALESCE(lot_id, '') = COALESCE(NEW.lot_id, '');
END;
