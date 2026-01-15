-- Backorder Management schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS backorders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    backorder_number TEXT UNIQUE NOT NULL,
    order_id UUID NOT NULL,
    order_line_id UUID,
    customer_id UUID NOT NULL,
    sku TEXT NOT NULL,
    quantity_ordered NUMERIC(12, 4) NOT NULL,
    quantity_fulfilled NUMERIC(12, 4) NOT NULL DEFAULT 0,
    quantity_remaining NUMERIC(12, 4) NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT NOT NULL DEFAULT 'normal',
    expected_date TIMESTAMPTZ,
    promised_date TIMESTAMPTZ,
    source_location_id INTEGER,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_backorders_number ON backorders(backorder_number);
CREATE INDEX IF NOT EXISTS idx_backorders_order ON backorders(order_id);
CREATE INDEX IF NOT EXISTS idx_backorders_customer ON backorders(customer_id);
CREATE INDEX IF NOT EXISTS idx_backorders_sku ON backorders(sku);
CREATE INDEX IF NOT EXISTS idx_backorders_status ON backorders(status);
CREATE INDEX IF NOT EXISTS idx_backorders_priority ON backorders(priority);
CREATE INDEX IF NOT EXISTS idx_backorders_expected ON backorders(expected_date);

CREATE TABLE IF NOT EXISTS backorder_fulfillments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    backorder_id UUID NOT NULL REFERENCES backorders(id) ON DELETE CASCADE,
    quantity NUMERIC(12, 4) NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'inventory',
    source_id UUID,
    notes TEXT,
    fulfilled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    fulfilled_by TEXT
);

CREATE INDEX IF NOT EXISTS idx_bo_fulfillments_backorder ON backorder_fulfillments(backorder_id);
CREATE INDEX IF NOT EXISTS idx_bo_fulfillments_date ON backorder_fulfillments(fulfilled_at);

CREATE TABLE IF NOT EXISTS backorder_allocations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    backorder_id UUID NOT NULL REFERENCES backorders(id) ON DELETE CASCADE,
    sku TEXT NOT NULL,
    quantity NUMERIC(12, 4) NOT NULL,
    location_id INTEGER,
    lot_id UUID,
    status TEXT NOT NULL DEFAULT 'reserved',
    allocated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_bo_alloc_backorder ON backorder_allocations(backorder_id);
CREATE INDEX IF NOT EXISTS idx_bo_alloc_sku ON backorder_allocations(sku);
CREATE INDEX IF NOT EXISTS idx_bo_alloc_status ON backorder_allocations(status);
CREATE INDEX IF NOT EXISTS idx_bo_alloc_expires ON backorder_allocations(expires_at);
