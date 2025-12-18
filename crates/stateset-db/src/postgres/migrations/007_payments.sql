-- Payments migration for PostgreSQL
-- Payment processing, refunds, and payment methods

-- Main payments table
CREATE TABLE IF NOT EXISTS payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_number VARCHAR(50) UNIQUE NOT NULL,
    order_id UUID REFERENCES orders(id),
    invoice_id UUID,
    customer_id UUID REFERENCES customers(id),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    payment_method VARCHAR(50) NOT NULL DEFAULT 'credit_card',
    amount DECIMAL(12, 2) NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    amount_refunded DECIMAL(12, 2) NOT NULL DEFAULT 0,
    external_id VARCHAR(255),
    processor VARCHAR(100),

    -- Card details (if applicable)
    card_brand VARCHAR(50),
    card_last4 VARCHAR(4),
    card_exp_month INTEGER,
    card_exp_year INTEGER,

    -- Billing info
    billing_email VARCHAR(255),
    billing_name VARCHAR(255),
    billing_address TEXT,

    -- Additional info
    description TEXT,
    failure_reason TEXT,
    failure_code VARCHAR(100),
    metadata TEXT,

    -- Timestamps
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Refunds table
CREATE TABLE IF NOT EXISTS refunds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    refund_number VARCHAR(50) UNIQUE NOT NULL,
    payment_id UUID NOT NULL REFERENCES payments(id),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    amount DECIMAL(12, 2) NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    reason TEXT,
    external_id VARCHAR(255),
    failure_reason TEXT,
    notes TEXT,
    refunded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Payment methods table (stored payment methods for customers)
CREATE TABLE IF NOT EXISTS payment_methods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL REFERENCES customers(id),
    method_type VARCHAR(50) NOT NULL DEFAULT 'credit_card',
    is_default BOOLEAN NOT NULL DEFAULT false,

    -- Card details
    card_brand VARCHAR(50),
    card_last4 VARCHAR(4),
    card_exp_month INTEGER,
    card_exp_year INTEGER,
    cardholder_name VARCHAR(255),

    -- Bank details (for ACH/bank transfer)
    bank_name VARCHAR(255),
    account_last4 VARCHAR(4),

    -- External reference
    external_id VARCHAR(255),
    billing_address TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_payments_order_id ON payments(order_id);
CREATE INDEX IF NOT EXISTS idx_payments_customer_id ON payments(customer_id);
CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status);
CREATE INDEX IF NOT EXISTS idx_payments_external_id ON payments(external_id);
CREATE INDEX IF NOT EXISTS idx_payments_payment_number ON payments(payment_number);
CREATE INDEX IF NOT EXISTS idx_refunds_payment_id ON refunds(payment_id);
CREATE INDEX IF NOT EXISTS idx_refunds_status ON refunds(status);
CREATE INDEX IF NOT EXISTS idx_payment_methods_customer_id ON payment_methods(customer_id);
