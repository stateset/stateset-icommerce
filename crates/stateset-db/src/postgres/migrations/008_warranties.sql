-- Warranties migration for PostgreSQL
-- Warranty registration and claims processing

-- Main warranties table
CREATE TABLE IF NOT EXISTS warranties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    warranty_number VARCHAR(50) UNIQUE NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(id),
    order_id UUID REFERENCES orders(id),
    order_item_id UUID,
    product_id UUID,
    sku VARCHAR(100),
    serial_number VARCHAR(100),
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    warranty_type VARCHAR(50) NOT NULL DEFAULT 'standard',
    provider VARCHAR(255),
    coverage_description TEXT,

    -- Dates
    purchase_date TIMESTAMPTZ NOT NULL,
    start_date TIMESTAMPTZ NOT NULL,
    end_date TIMESTAMPTZ,
    duration_months INTEGER,

    -- Coverage limits
    max_coverage_amount DECIMAL(12, 2),
    deductible DECIMAL(12, 2),
    max_claims INTEGER,
    claims_used INTEGER NOT NULL DEFAULT 0,

    -- Additional info
    terms TEXT,
    notes TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Warranty claims table
CREATE TABLE IF NOT EXISTS warranty_claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_number VARCHAR(50) UNIQUE NOT NULL,
    warranty_id UUID NOT NULL REFERENCES warranties(id),
    customer_id UUID NOT NULL REFERENCES customers(id),
    status VARCHAR(50) NOT NULL DEFAULT 'submitted',
    resolution VARCHAR(50) NOT NULL DEFAULT 'none',

    -- Issue details
    issue_description TEXT NOT NULL,
    issue_category VARCHAR(100),
    issue_date TIMESTAMPTZ,

    -- Contact info
    contact_phone VARCHAR(50),
    contact_email VARCHAR(255),
    shipping_address TEXT,

    -- Resolution details
    repair_cost DECIMAL(12, 2),
    replacement_product_id UUID,
    refund_amount DECIMAL(12, 2),
    denial_reason TEXT,

    -- Notes
    internal_notes TEXT,
    customer_notes TEXT,

    -- Timestamps
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    approved_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_warranties_customer_id ON warranties(customer_id);
CREATE INDEX IF NOT EXISTS idx_warranties_order_id ON warranties(order_id);
CREATE INDEX IF NOT EXISTS idx_warranties_status ON warranties(status);
CREATE INDEX IF NOT EXISTS idx_warranties_serial_number ON warranties(serial_number);
CREATE INDEX IF NOT EXISTS idx_warranties_warranty_number ON warranties(warranty_number);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_warranty_id ON warranty_claims(warranty_id);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_customer_id ON warranty_claims(customer_id);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_status ON warranty_claims(status);
