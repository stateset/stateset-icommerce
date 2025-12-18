-- Invoices migration for PostgreSQL
-- Invoice generation and payment tracking

-- Main invoices table
CREATE TABLE IF NOT EXISTS invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_number VARCHAR(50) UNIQUE NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(id),
    order_id UUID REFERENCES orders(id),
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    invoice_type VARCHAR(50) NOT NULL DEFAULT 'standard',
    invoice_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    due_date TIMESTAMPTZ NOT NULL,
    payment_terms VARCHAR(255),
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',

    -- Billing info
    billing_name VARCHAR(255),
    billing_email VARCHAR(255),
    billing_address TEXT,
    billing_city VARCHAR(100),
    billing_state VARCHAR(100),
    billing_postal_code VARCHAR(20),
    billing_country VARCHAR(100),

    -- Amounts
    subtotal DECIMAL(12, 2) NOT NULL DEFAULT 0,
    discount_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    discount_percent DECIMAL(5, 2),
    tax_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    tax_rate DECIMAL(5, 2),
    shipping_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    total DECIMAL(12, 2) NOT NULL DEFAULT 0,
    amount_paid DECIMAL(12, 2) NOT NULL DEFAULT 0,
    balance_due DECIMAL(12, 2) NOT NULL DEFAULT 0,

    -- Additional info
    po_number VARCHAR(100),
    notes TEXT,
    terms TEXT,
    footer TEXT,

    -- Timestamps
    sent_at TIMESTAMPTZ,
    viewed_at TIMESTAMPTZ,
    paid_at TIMESTAMPTZ,
    voided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Invoice items table
CREATE TABLE IF NOT EXISTS invoice_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    order_item_id UUID,
    product_id UUID,
    sku VARCHAR(100),
    description TEXT NOT NULL,
    quantity DECIMAL(12, 4) NOT NULL,
    unit_of_measure VARCHAR(50),
    unit_price DECIMAL(12, 4) NOT NULL,
    discount_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    tax_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    line_total DECIMAL(12, 2) NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_invoices_customer_id ON invoices(customer_id);
CREATE INDEX IF NOT EXISTS idx_invoices_order_id ON invoices(order_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoices_invoice_number ON invoices(invoice_number);
CREATE INDEX IF NOT EXISTS idx_invoices_due_date ON invoices(due_date);
CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice_id ON invoice_items(invoice_id);
