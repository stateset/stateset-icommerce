-- Accounts Receivable schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS ar_collection_activities (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    invoice_id UUID NOT NULL REFERENCES invoices(id),
    customer_id UUID NOT NULL,
    activity_type TEXT NOT NULL DEFAULT 'note',
    activity_date TIMESTAMPTZ NOT NULL,
    dunning_letter_type TEXT,
    notes TEXT,
    contact_method TEXT,
    contact_result TEXT,
    promise_to_pay_date TIMESTAMPTZ,
    promise_to_pay_amount NUMERIC(12, 4),
    performed_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ar_collection_invoice ON ar_collection_activities(invoice_id);
CREATE INDEX IF NOT EXISTS idx_ar_collection_customer ON ar_collection_activities(customer_id);
CREATE INDEX IF NOT EXISTS idx_ar_collection_date ON ar_collection_activities(activity_date);
CREATE INDEX IF NOT EXISTS idx_ar_collection_type ON ar_collection_activities(activity_type);

ALTER TABLE invoices ADD COLUMN IF NOT EXISTS collection_status TEXT DEFAULT 'none';
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS last_dunning_date TIMESTAMPTZ;
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS dunning_count INTEGER DEFAULT 0;

CREATE TABLE IF NOT EXISTS ar_write_offs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    write_off_number TEXT UNIQUE NOT NULL,
    invoice_id UUID NOT NULL REFERENCES invoices(id),
    customer_id UUID NOT NULL,
    amount NUMERIC(12, 4) NOT NULL,
    reason TEXT NOT NULL DEFAULT 'uncollectible',
    notes TEXT,
    write_off_date DATE NOT NULL,
    approved_by TEXT,
    approved_at TIMESTAMPTZ,
    reversed_at TIMESTAMPTZ,
    gl_journal_entry_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ar_writeoff_number ON ar_write_offs(write_off_number);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_invoice ON ar_write_offs(invoice_id);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_customer ON ar_write_offs(customer_id);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_date ON ar_write_offs(write_off_date);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_reason ON ar_write_offs(reason);

CREATE TABLE IF NOT EXISTS ar_credit_memos (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    credit_memo_number TEXT UNIQUE NOT NULL,
    customer_id UUID NOT NULL,
    original_invoice_id UUID REFERENCES invoices(id),
    reason TEXT NOT NULL DEFAULT 'other',
    amount NUMERIC(12, 4) NOT NULL,
    applied_amount NUMERIC(12, 4) NOT NULL DEFAULT 0,
    unapplied_amount NUMERIC(12, 4) NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    notes TEXT,
    issue_date DATE NOT NULL,
    gl_journal_entry_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_number ON ar_credit_memos(credit_memo_number);
CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_customer ON ar_credit_memos(customer_id);
CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_status ON ar_credit_memos(status);
CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_date ON ar_credit_memos(issue_date);

CREATE TABLE IF NOT EXISTS ar_credit_memo_applications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    credit_memo_id UUID NOT NULL REFERENCES ar_credit_memos(id),
    invoice_id UUID NOT NULL REFERENCES invoices(id),
    applied_amount NUMERIC(12, 4) NOT NULL,
    applied_date TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ar_cm_app_memo ON ar_credit_memo_applications(credit_memo_id);
CREATE INDEX IF NOT EXISTS idx_ar_cm_app_invoice ON ar_credit_memo_applications(invoice_id);

CREATE TABLE IF NOT EXISTS ar_payment_applications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payment_id UUID NOT NULL REFERENCES payments(id),
    invoice_id UUID NOT NULL REFERENCES invoices(id),
    applied_amount NUMERIC(12, 4) NOT NULL,
    applied_date TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ar_pay_app_payment ON ar_payment_applications(payment_id);
CREATE INDEX IF NOT EXISTS idx_ar_pay_app_invoice ON ar_payment_applications(invoice_id);

CREATE OR REPLACE VIEW v_ar_aging AS
SELECT
    i.customer_id,
    c.first_name || ' ' || c.last_name AS customer_name,
    c.email AS customer_email,
    SUM(CASE WHEN i.due_date >= NOW() THEN i.balance_due ELSE 0 END) AS current_amount,
    SUM(CASE WHEN i.due_date < NOW() AND i.due_date >= NOW() - INTERVAL '30 days' THEN i.balance_due ELSE 0 END) AS days_1_30,
    SUM(CASE WHEN i.due_date < NOW() - INTERVAL '30 days' AND i.due_date >= NOW() - INTERVAL '60 days' THEN i.balance_due ELSE 0 END) AS days_31_60,
    SUM(CASE WHEN i.due_date < NOW() - INTERVAL '60 days' AND i.due_date >= NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END) AS days_61_90,
    SUM(CASE WHEN i.due_date < NOW() - INTERVAL '90 days' THEN i.balance_due ELSE 0 END) AS days_over_90,
    SUM(i.balance_due) AS total_outstanding,
    COUNT(*) AS invoice_count,
    MIN(i.created_at) AS oldest_invoice_date
FROM invoices i
LEFT JOIN customers c ON i.customer_id = c.id
WHERE i.status NOT IN ('paid', 'voided', 'written_off')
  AND i.balance_due > 0
GROUP BY i.customer_id, c.first_name, c.last_name, c.email;
