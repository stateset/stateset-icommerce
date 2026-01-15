-- Accounts Payable schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS ap_bills (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bill_number TEXT UNIQUE NOT NULL,
    supplier_id UUID NOT NULL,
    supplier_name TEXT,
    purchase_order_id UUID,
    status TEXT NOT NULL DEFAULT 'draft',
    bill_date DATE NOT NULL,
    due_date DATE NOT NULL,
    payment_terms TEXT,
    subtotal NUMERIC(12, 4) NOT NULL DEFAULT 0,
    tax_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    shipping_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    discount_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    total_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    amount_paid NUMERIC(12, 4) NOT NULL DEFAULT 0,
    amount_due NUMERIC(12, 4) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    reference_number TEXT,
    memo TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ap_bills_number ON ap_bills(bill_number);
CREATE INDEX IF NOT EXISTS idx_ap_bills_supplier ON ap_bills(supplier_id);
CREATE INDEX IF NOT EXISTS idx_ap_bills_status ON ap_bills(status);
CREATE INDEX IF NOT EXISTS idx_ap_bills_due_date ON ap_bills(due_date);
CREATE INDEX IF NOT EXISTS idx_ap_bills_po ON ap_bills(purchase_order_id);

CREATE TABLE IF NOT EXISTS ap_bill_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bill_id UUID NOT NULL REFERENCES ap_bills(id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    description TEXT NOT NULL,
    account_code TEXT,
    quantity NUMERIC(12, 4) NOT NULL DEFAULT 1,
    unit_price NUMERIC(12, 4) NOT NULL DEFAULT 0,
    amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    tax_rate NUMERIC(12, 6),
    tax_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    po_line_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ap_bill_items_bill ON ap_bill_items(bill_id);

CREATE TABLE IF NOT EXISTS ap_payments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payment_number TEXT UNIQUE NOT NULL,
    supplier_id UUID NOT NULL,
    payment_date DATE NOT NULL,
    payment_method TEXT NOT NULL DEFAULT 'check',
    amount NUMERIC(12, 4) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    reference_number TEXT,
    bank_account TEXT,
    check_number TEXT,
    memo TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ap_payments_number ON ap_payments(payment_number);
CREATE INDEX IF NOT EXISTS idx_ap_payments_supplier ON ap_payments(supplier_id);
CREATE INDEX IF NOT EXISTS idx_ap_payments_status ON ap_payments(status);
CREATE INDEX IF NOT EXISTS idx_ap_payments_date ON ap_payments(payment_date);

CREATE TABLE IF NOT EXISTS ap_payment_allocations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payment_id UUID NOT NULL REFERENCES ap_payments(id) ON DELETE CASCADE,
    bill_id UUID NOT NULL REFERENCES ap_bills(id),
    amount NUMERIC(12, 4) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ap_alloc_payment ON ap_payment_allocations(payment_id);
CREATE INDEX IF NOT EXISTS idx_ap_alloc_bill ON ap_payment_allocations(bill_id);

CREATE TABLE IF NOT EXISTS ap_payment_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    run_number TEXT UNIQUE NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    payment_date DATE NOT NULL,
    payment_method TEXT NOT NULL DEFAULT 'ach',
    total_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    payment_count INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_by TEXT,
    approved_by TEXT,
    approved_at TIMESTAMPTZ,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ap_runs_number ON ap_payment_runs(run_number);
CREATE INDEX IF NOT EXISTS idx_ap_runs_status ON ap_payment_runs(status);

CREATE TABLE IF NOT EXISTS ap_payment_run_bills (
    run_id UUID NOT NULL REFERENCES ap_payment_runs(id) ON DELETE CASCADE,
    bill_id UUID NOT NULL REFERENCES ap_bills(id),
    PRIMARY KEY (run_id, bill_id)
);
