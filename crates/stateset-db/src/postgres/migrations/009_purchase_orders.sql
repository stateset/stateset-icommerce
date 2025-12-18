-- Purchase Orders migration for PostgreSQL
-- Supplier management and purchase order processing

-- Suppliers table
CREATE TABLE IF NOT EXISTS suppliers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    supplier_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    contact_name VARCHAR(255),
    email VARCHAR(255),
    phone VARCHAR(50),
    website VARCHAR(255),
    address TEXT,
    city VARCHAR(100),
    state VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(100),
    tax_id VARCHAR(100),
    payment_terms VARCHAR(50) NOT NULL DEFAULT 'due_on_receipt',
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    lead_time_days INTEGER,
    minimum_order DECIMAL(12, 2),
    is_active BOOLEAN NOT NULL DEFAULT true,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Purchase orders table
CREATE TABLE IF NOT EXISTS purchase_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    po_number VARCHAR(50) UNIQUE NOT NULL,
    supplier_id UUID NOT NULL REFERENCES suppliers(id),
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    order_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expected_date TIMESTAMPTZ,
    delivered_date TIMESTAMPTZ,

    -- Ship to address
    ship_to_address TEXT,
    ship_to_city VARCHAR(100),
    ship_to_state VARCHAR(100),
    ship_to_postal_code VARCHAR(20),
    ship_to_country VARCHAR(100),

    -- Financial
    payment_terms VARCHAR(50) NOT NULL DEFAULT 'due_on_receipt',
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    subtotal DECIMAL(12, 2) NOT NULL DEFAULT 0,
    tax_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    shipping_cost DECIMAL(12, 2) NOT NULL DEFAULT 0,
    discount_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    total DECIMAL(12, 2) NOT NULL DEFAULT 0,
    amount_paid DECIMAL(12, 2) NOT NULL DEFAULT 0,

    -- References
    supplier_reference VARCHAR(100),
    notes TEXT,
    supplier_notes TEXT,

    -- Approval
    approved_by VARCHAR(255),
    approved_at TIMESTAMPTZ,
    sent_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Purchase order items table
CREATE TABLE IF NOT EXISTS purchase_order_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    purchase_order_id UUID NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    product_id UUID,
    sku VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    supplier_sku VARCHAR(100),
    quantity_ordered DECIMAL(12, 4) NOT NULL,
    quantity_received DECIMAL(12, 4) NOT NULL DEFAULT 0,
    unit_of_measure VARCHAR(50),
    unit_cost DECIMAL(12, 4) NOT NULL,
    line_total DECIMAL(12, 2) NOT NULL,
    tax_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    discount_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    expected_date TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_suppliers_code ON suppliers(supplier_code);
CREATE INDEX IF NOT EXISTS idx_suppliers_is_active ON suppliers(is_active);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_supplier_id ON purchase_orders(supplier_id);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_status ON purchase_orders(status);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_po_number ON purchase_orders(po_number);
CREATE INDEX IF NOT EXISTS idx_purchase_order_items_po_id ON purchase_order_items(purchase_order_id);
