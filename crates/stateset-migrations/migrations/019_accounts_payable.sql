-- Accounts Payable tables

-- Bills (supplier invoices)
CREATE TABLE IF NOT EXISTS ap_bills (
    id TEXT PRIMARY KEY,
    bill_number TEXT UNIQUE NOT NULL,
    supplier_id TEXT NOT NULL,
    supplier_name TEXT,
    purchase_order_id TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    bill_date TEXT NOT NULL,
    due_date TEXT NOT NULL,
    payment_terms TEXT,
    subtotal TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_amount TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    total_amount TEXT NOT NULL DEFAULT '0',
    amount_paid TEXT NOT NULL DEFAULT '0',
    amount_due TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    reference_number TEXT,
    memo TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ap_bills_number ON ap_bills(bill_number);
CREATE INDEX IF NOT EXISTS idx_ap_bills_supplier ON ap_bills(supplier_id);
CREATE INDEX IF NOT EXISTS idx_ap_bills_status ON ap_bills(status);
CREATE INDEX IF NOT EXISTS idx_ap_bills_due_date ON ap_bills(due_date);
CREATE INDEX IF NOT EXISTS idx_ap_bills_po ON ap_bills(purchase_order_id);

-- Bill line items
CREATE TABLE IF NOT EXISTS ap_bill_items (
    id TEXT PRIMARY KEY,
    bill_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    description TEXT NOT NULL,
    account_code TEXT,
    quantity TEXT NOT NULL DEFAULT '1',
    unit_price TEXT NOT NULL DEFAULT '0',
    amount TEXT NOT NULL DEFAULT '0',
    tax_rate TEXT,
    tax_amount TEXT NOT NULL DEFAULT '0',
    po_line_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (bill_id) REFERENCES ap_bills(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ap_bill_items_bill ON ap_bill_items(bill_id);

-- Payments to suppliers
CREATE TABLE IF NOT EXISTS ap_payments (
    id TEXT PRIMARY KEY,
    payment_number TEXT UNIQUE NOT NULL,
    supplier_id TEXT NOT NULL,
    payment_date TEXT NOT NULL,
    payment_method TEXT NOT NULL DEFAULT 'check',
    amount TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    reference_number TEXT,
    bank_account TEXT,
    check_number TEXT,
    memo TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ap_payments_number ON ap_payments(payment_number);
CREATE INDEX IF NOT EXISTS idx_ap_payments_supplier ON ap_payments(supplier_id);
CREATE INDEX IF NOT EXISTS idx_ap_payments_status ON ap_payments(status);
CREATE INDEX IF NOT EXISTS idx_ap_payments_date ON ap_payments(payment_date);

-- Payment allocations (payment to bill mapping)
CREATE TABLE IF NOT EXISTS ap_payment_allocations (
    id TEXT PRIMARY KEY,
    payment_id TEXT NOT NULL,
    bill_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (payment_id) REFERENCES ap_payments(id) ON DELETE CASCADE,
    FOREIGN KEY (bill_id) REFERENCES ap_bills(id)
);

CREATE INDEX IF NOT EXISTS idx_ap_alloc_payment ON ap_payment_allocations(payment_id);
CREATE INDEX IF NOT EXISTS idx_ap_alloc_bill ON ap_payment_allocations(bill_id);

-- Payment runs (batch payments)
CREATE TABLE IF NOT EXISTS ap_payment_runs (
    id TEXT PRIMARY KEY,
    run_number TEXT UNIQUE NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    payment_date TEXT NOT NULL,
    payment_method TEXT NOT NULL DEFAULT 'ach',
    total_amount TEXT NOT NULL DEFAULT '0',
    payment_count INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_by TEXT,
    approved_by TEXT,
    approved_at TEXT,
    processed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ap_runs_number ON ap_payment_runs(run_number);
CREATE INDEX IF NOT EXISTS idx_ap_runs_status ON ap_payment_runs(status);

-- Payment run bills (junction table)
CREATE TABLE IF NOT EXISTS ap_payment_run_bills (
    run_id TEXT NOT NULL,
    bill_id TEXT NOT NULL,
    PRIMARY KEY (run_id, bill_id),
    FOREIGN KEY (run_id) REFERENCES ap_payment_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (bill_id) REFERENCES ap_bills(id)
);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS ap_bills_updated_at
AFTER UPDATE ON ap_bills
BEGIN
    UPDATE ap_bills SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS ap_payments_updated_at
AFTER UPDATE ON ap_payments
BEGIN
    UPDATE ap_payments SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS ap_payment_runs_updated_at
AFTER UPDATE ON ap_payment_runs
BEGIN
    UPDATE ap_payment_runs SET updated_at = datetime('now') WHERE id = NEW.id;
END;
