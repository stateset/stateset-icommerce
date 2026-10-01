-- Shipments migration
-- Tracks order fulfillment and delivery

-- Main shipments table
CREATE TABLE IF NOT EXISTS shipments (
    id TEXT PRIMARY KEY,
    shipment_number TEXT UNIQUE NOT NULL,
    order_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    carrier TEXT NOT NULL DEFAULT 'other',
    shipping_method TEXT NOT NULL DEFAULT 'standard',
    tracking_number TEXT,
    tracking_url TEXT,

    -- Recipient info
    recipient_name TEXT NOT NULL,
    recipient_email TEXT,
    recipient_phone TEXT,
    shipping_address TEXT NOT NULL,

    -- Package details
    weight_kg TEXT,
    dimensions TEXT,
    shipping_cost TEXT,
    insurance_amount TEXT,
    signature_required INTEGER NOT NULL DEFAULT 0,

    -- Timestamps
    shipped_at TEXT,
    estimated_delivery TEXT,
    delivered_at TEXT,

    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Shipment items table
CREATE TABLE IF NOT EXISTS shipment_items (
    id TEXT PRIMARY KEY,
    shipment_id TEXT NOT NULL,
    order_item_id TEXT,
    product_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (shipment_id) REFERENCES shipments(id) ON DELETE CASCADE
);

-- Shipment tracking events table
CREATE TABLE IF NOT EXISTS shipment_events (
    id TEXT PRIMARY KEY,
    shipment_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    location TEXT,
    description TEXT,
    event_time TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (shipment_id) REFERENCES shipments(id) ON DELETE CASCADE
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_shipments_order_id ON shipments(order_id);
CREATE INDEX IF NOT EXISTS idx_shipments_status ON shipments(status);
CREATE INDEX IF NOT EXISTS idx_shipments_carrier ON shipments(carrier);
CREATE INDEX IF NOT EXISTS idx_shipments_tracking_number ON shipments(tracking_number);
CREATE INDEX IF NOT EXISTS idx_shipments_shipment_number ON shipments(shipment_number);
CREATE INDEX IF NOT EXISTS idx_shipment_items_shipment_id ON shipment_items(shipment_id);
CREATE INDEX IF NOT EXISTS idx_shipment_events_shipment_id ON shipment_events(shipment_id);
