-- Backorder Management tables

-- Backorders
CREATE TABLE IF NOT EXISTS backorders (
    id TEXT PRIMARY KEY,
    backorder_number TEXT UNIQUE NOT NULL,
    order_id TEXT NOT NULL,
    order_line_id TEXT,
    customer_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity_ordered TEXT NOT NULL,
    quantity_fulfilled TEXT NOT NULL DEFAULT '0',
    quantity_remaining TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT NOT NULL DEFAULT 'normal',
    expected_date TEXT,
    promised_date TEXT,
    source_location_id INTEGER,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_backorders_number ON backorders(backorder_number);
CREATE INDEX IF NOT EXISTS idx_backorders_order ON backorders(order_id);
CREATE INDEX IF NOT EXISTS idx_backorders_customer ON backorders(customer_id);
CREATE INDEX IF NOT EXISTS idx_backorders_sku ON backorders(sku);
CREATE INDEX IF NOT EXISTS idx_backorders_status ON backorders(status);
CREATE INDEX IF NOT EXISTS idx_backorders_priority ON backorders(priority);
CREATE INDEX IF NOT EXISTS idx_backorders_expected ON backorders(expected_date);

-- Backorder fulfillments (history)
CREATE TABLE IF NOT EXISTS backorder_fulfillments (
    id TEXT PRIMARY KEY,
    backorder_id TEXT NOT NULL,
    quantity TEXT NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'inventory',
    source_id TEXT,
    notes TEXT,
    fulfilled_at TEXT NOT NULL,
    fulfilled_by TEXT,
    FOREIGN KEY (backorder_id) REFERENCES backorders(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_bo_fulfillments_backorder ON backorder_fulfillments(backorder_id);
CREATE INDEX IF NOT EXISTS idx_bo_fulfillments_date ON backorder_fulfillments(fulfilled_at);

-- Backorder allocations
CREATE TABLE IF NOT EXISTS backorder_allocations (
    id TEXT PRIMARY KEY,
    backorder_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity TEXT NOT NULL,
    location_id INTEGER,
    lot_id TEXT,
    status TEXT NOT NULL DEFAULT 'reserved',
    allocated_at TEXT NOT NULL,
    expires_at TEXT,
    FOREIGN KEY (backorder_id) REFERENCES backorders(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_bo_alloc_backorder ON backorder_allocations(backorder_id);
CREATE INDEX IF NOT EXISTS idx_bo_alloc_sku ON backorder_allocations(sku);
CREATE INDEX IF NOT EXISTS idx_bo_alloc_status ON backorder_allocations(status);
CREATE INDEX IF NOT EXISTS idx_bo_alloc_expires ON backorder_allocations(expires_at);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS backorders_updated_at
AFTER UPDATE ON backorders
BEGIN
    UPDATE backorders SET updated_at = datetime('now') WHERE id = NEW.id;
END;
