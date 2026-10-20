-- Accounts Receivable tables

-- Collection activities (dunning history)
CREATE TABLE IF NOT EXISTS ar_collection_activities (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    activity_type TEXT NOT NULL DEFAULT 'note',
    activity_date TEXT NOT NULL,
    dunning_letter_type TEXT,
    notes TEXT,
    contact_method TEXT,
    contact_result TEXT,
    promise_to_pay_date TEXT,
    promise_to_pay_amount TEXT,
    performed_by TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
);

CREATE INDEX IF NOT EXISTS idx_ar_collection_invoice ON ar_collection_activities(invoice_id);
CREATE INDEX IF NOT EXISTS idx_ar_collection_customer ON ar_collection_activities(customer_id);
CREATE INDEX IF NOT EXISTS idx_ar_collection_date ON ar_collection_activities(activity_date);
CREATE INDEX IF NOT EXISTS idx_ar_collection_type ON ar_collection_activities(activity_type);

-- Invoice collection status extension (add columns to existing invoices table)
-- Note: SQLite ALTER TABLE limitations mean we add columns individually
ALTER TABLE invoices ADD COLUMN collection_status TEXT DEFAULT 'none';
ALTER TABLE invoices ADD COLUMN last_dunning_date TEXT;
ALTER TABLE invoices ADD COLUMN dunning_count INTEGER DEFAULT 0;

-- Write-offs
CREATE TABLE IF NOT EXISTS ar_write_offs (
    id TEXT PRIMARY KEY,
    write_off_number TEXT UNIQUE NOT NULL,
    invoice_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT 'uncollectible',
    notes TEXT,
    write_off_date TEXT NOT NULL,
    approved_by TEXT,
    approved_at TEXT,
    reversed_at TEXT,
    gl_journal_entry_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
);

CREATE INDEX IF NOT EXISTS idx_ar_writeoff_number ON ar_write_offs(write_off_number);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_invoice ON ar_write_offs(invoice_id);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_customer ON ar_write_offs(customer_id);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_date ON ar_write_offs(write_off_date);
CREATE INDEX IF NOT EXISTS idx_ar_writeoff_reason ON ar_write_offs(reason);

-- Credit memos
CREATE TABLE IF NOT EXISTS ar_credit_memos (
    id TEXT PRIMARY KEY,
    credit_memo_number TEXT UNIQUE NOT NULL,
    customer_id TEXT NOT NULL,
    original_invoice_id TEXT,
    reason TEXT NOT NULL DEFAULT 'other',
    amount TEXT NOT NULL,
    applied_amount TEXT NOT NULL DEFAULT '0',
    unapplied_amount TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    notes TEXT,
    issue_date TEXT NOT NULL,
    gl_journal_entry_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (original_invoice_id) REFERENCES invoices(id)
);

CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_number ON ar_credit_memos(credit_memo_number);
CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_customer ON ar_credit_memos(customer_id);
CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_status ON ar_credit_memos(status);
CREATE INDEX IF NOT EXISTS idx_ar_creditmemo_date ON ar_credit_memos(issue_date);

-- Credit memo applications (to invoices)
CREATE TABLE IF NOT EXISTS ar_credit_memo_applications (
    id TEXT PRIMARY KEY,
    credit_memo_id TEXT NOT NULL,
    invoice_id TEXT NOT NULL,
    applied_amount TEXT NOT NULL,
    applied_date TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (credit_memo_id) REFERENCES ar_credit_memos(id),
    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
);

CREATE INDEX IF NOT EXISTS idx_ar_cm_app_memo ON ar_credit_memo_applications(credit_memo_id);
CREATE INDEX IF NOT EXISTS idx_ar_cm_app_invoice ON ar_credit_memo_applications(invoice_id);

-- Payment applications (AR receipts to invoices)
CREATE TABLE IF NOT EXISTS ar_payment_applications (
    id TEXT PRIMARY KEY,
    payment_id TEXT NOT NULL,
    invoice_id TEXT NOT NULL,
    applied_amount TEXT NOT NULL,
    applied_date TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (payment_id) REFERENCES payments(id),
    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
);

CREATE INDEX IF NOT EXISTS idx_ar_pay_app_payment ON ar_payment_applications(payment_id);
CREATE INDEX IF NOT EXISTS idx_ar_pay_app_invoice ON ar_payment_applications(invoice_id);

-- Auto-update timestamps
CREATE TRIGGER IF NOT EXISTS ar_credit_memos_updated_at
AFTER UPDATE ON ar_credit_memos
BEGIN
    UPDATE ar_credit_memos SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- View for AR aging (useful for reporting)
CREATE VIEW IF NOT EXISTS v_ar_aging AS
SELECT
    i.customer_id,
    c.first_name || ' ' || c.last_name as customer_name,
    c.email as customer_email,
    SUM(CASE WHEN i.due_date >= datetime('now') THEN CAST(i.balance_due AS REAL) ELSE 0 END) as current_amount,
    SUM(CASE WHEN i.due_date < datetime('now') AND i.due_date >= datetime('now', '-30 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END) as days_1_30,
    SUM(CASE WHEN i.due_date < datetime('now', '-30 days') AND i.due_date >= datetime('now', '-60 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END) as days_31_60,
    SUM(CASE WHEN i.due_date < datetime('now', '-60 days') AND i.due_date >= datetime('now', '-90 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END) as days_61_90,
    SUM(CASE WHEN i.due_date < datetime('now', '-90 days') THEN CAST(i.balance_due AS REAL) ELSE 0 END) as days_over_90,
    SUM(CAST(i.balance_due AS REAL)) as total_outstanding,
    COUNT(*) as invoice_count,
    MIN(i.created_at) as oldest_invoice_date
FROM invoices i
LEFT JOIN customers c ON i.customer_id = c.id
WHERE i.status NOT IN ('paid', 'voided', 'written_off')
  AND CAST(i.balance_due AS REAL) > 0
GROUP BY i.customer_id;
