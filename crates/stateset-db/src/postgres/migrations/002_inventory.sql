-- Inventory management schema (PostgreSQL)

-- Inventory items (SKU master)
CREATE TABLE IF NOT EXISTS inventory_items (
    id BIGSERIAL PRIMARY KEY,
    sku TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    unit_of_measure TEXT NOT NULL DEFAULT 'EA',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inventory_items_sku ON inventory_items(sku);

-- Inventory locations
CREATE TABLE IF NOT EXISTS inventory_locations (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    address JSONB,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Default location
INSERT INTO inventory_locations (id, name, code)
VALUES (1, 'Default Warehouse', 'DEFAULT')
ON CONFLICT (id) DO NOTHING;

-- Inventory balances (stock per location)
CREATE TABLE IF NOT EXISTS inventory_balances (
    id BIGSERIAL PRIMARY KEY,
    item_id BIGINT NOT NULL REFERENCES inventory_items(id) ON DELETE CASCADE,
    location_id BIGINT NOT NULL REFERENCES inventory_locations(id) DEFAULT 1,
    quantity_on_hand NUMERIC(19, 4) NOT NULL DEFAULT 0,
    quantity_allocated NUMERIC(19, 4) NOT NULL DEFAULT 0,
    quantity_available NUMERIC(19, 4) NOT NULL DEFAULT 0,
    reorder_point NUMERIC(19, 4),
    safety_stock NUMERIC(19, 4),
    version INTEGER NOT NULL DEFAULT 1,
    last_counted_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(item_id, location_id)
);

CREATE INDEX IF NOT EXISTS idx_inventory_balances_item ON inventory_balances(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_balances_location ON inventory_balances(location_id);

-- Inventory transactions (audit trail)
CREATE TABLE IF NOT EXISTS inventory_transactions (
    id BIGSERIAL PRIMARY KEY,
    item_id BIGINT NOT NULL REFERENCES inventory_items(id),
    location_id BIGINT NOT NULL DEFAULT 1,
    transaction_type TEXT NOT NULL,
    quantity NUMERIC(19, 4) NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    reason TEXT,
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inventory_transactions_item ON inventory_transactions(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transactions_type ON inventory_transactions(transaction_type);

-- Inventory reservations
CREATE TABLE IF NOT EXISTS inventory_reservations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_id BIGINT NOT NULL REFERENCES inventory_items(id),
    location_id BIGINT NOT NULL DEFAULT 1,
    quantity NUMERIC(19, 4) NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reference_type TEXT NOT NULL,
    reference_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inventory_reservations_item ON inventory_reservations(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_status ON inventory_reservations(status);
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_reference ON inventory_reservations(reference_type, reference_id);
