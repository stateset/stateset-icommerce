-- Shipments migration for PostgreSQL
-- Tracks order fulfillment and delivery

-- Main shipments table
CREATE TABLE IF NOT EXISTS shipments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shipment_number VARCHAR(50) UNIQUE NOT NULL,
    order_id UUID NOT NULL REFERENCES orders(id),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    carrier VARCHAR(50) NOT NULL DEFAULT 'other',
    shipping_method VARCHAR(50) NOT NULL DEFAULT 'standard',
    tracking_number VARCHAR(100),
    tracking_url TEXT,

    -- Recipient info
    recipient_name VARCHAR(255) NOT NULL,
    recipient_email VARCHAR(255),
    recipient_phone VARCHAR(50),
    shipping_address TEXT NOT NULL,

    -- Package details
    weight_kg DECIMAL(10, 3),
    dimensions VARCHAR(100),
    shipping_cost DECIMAL(12, 2),
    insurance_amount DECIMAL(12, 2),
    signature_required BOOLEAN NOT NULL DEFAULT false,

    -- Timestamps
    shipped_at TIMESTAMPTZ,
    estimated_delivery TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,

    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Shipment items table
CREATE TABLE IF NOT EXISTS shipment_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shipment_id UUID NOT NULL REFERENCES shipments(id) ON DELETE CASCADE,
    order_item_id UUID,
    product_id UUID,
    sku VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Shipment tracking events table
CREATE TABLE IF NOT EXISTS shipment_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shipment_id UUID NOT NULL REFERENCES shipments(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    location VARCHAR(255),
    description TEXT,
    event_time TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_shipments_order_id ON shipments(order_id);
CREATE INDEX IF NOT EXISTS idx_shipments_status ON shipments(status);
CREATE INDEX IF NOT EXISTS idx_shipments_carrier ON shipments(carrier);
CREATE INDEX IF NOT EXISTS idx_shipments_tracking_number ON shipments(tracking_number);
CREATE INDEX IF NOT EXISTS idx_shipment_items_shipment_id ON shipment_items(shipment_id);
CREATE INDEX IF NOT EXISTS idx_shipment_events_shipment_id ON shipment_events(shipment_id);
