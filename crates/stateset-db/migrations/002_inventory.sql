-- Inventory management schema

-- Inventory items (SKU master)
CREATE TABLE IF NOT EXISTS inventory_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sku TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    unit_of_measure TEXT NOT NULL DEFAULT 'EA',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_inventory_items_sku ON inventory_items(sku);

-- Inventory locations
CREATE TABLE IF NOT EXISTS inventory_locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    address TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Default location
INSERT OR IGNORE INTO inventory_locations (id, name, code) VALUES (1, 'Default Warehouse', 'DEFAULT');

-- Inventory balances (stock per location)
CREATE TABLE IF NOT EXISTS inventory_balances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES inventory_items(id),
    location_id INTEGER NOT NULL REFERENCES inventory_locations(id) DEFAULT 1,
    quantity_on_hand TEXT NOT NULL DEFAULT '0',
    quantity_allocated TEXT NOT NULL DEFAULT '0',
    quantity_available TEXT NOT NULL DEFAULT '0',
    reorder_point TEXT,
    safety_stock TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    last_counted_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(item_id, location_id)
);

CREATE INDEX IF NOT EXISTS idx_inventory_balances_item ON inventory_balances(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_balances_location ON inventory_balances(location_id);

-- Inventory transactions (audit trail)
CREATE TABLE IF NOT EXISTS inventory_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES inventory_items(id),
    location_id INTEGER NOT NULL DEFAULT 1,
    transaction_type TEXT NOT NULL,
    quantity TEXT NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    reason TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_inventory_transactions_item ON inventory_transactions(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transactions_type ON inventory_transactions(transaction_type);

-- Inventory reservations
CREATE TABLE IF NOT EXISTS inventory_reservations (
    id TEXT PRIMARY KEY,
    item_id INTEGER NOT NULL REFERENCES inventory_items(id),
    location_id INTEGER NOT NULL DEFAULT 1,
    quantity TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reference_type TEXT NOT NULL,
    reference_id TEXT NOT NULL,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_inventory_reservations_item ON inventory_reservations(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_status ON inventory_reservations(status);
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_reference ON inventory_reservations(reference_type, reference_id);
