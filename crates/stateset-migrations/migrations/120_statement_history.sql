CREATE TABLE ar_reversed_payment_applications (
 id TEXT PRIMARY KEY, payment_id TEXT NOT NULL, invoice_id TEXT NOT NULL,
 applied_amount TEXT NOT NULL, applied_date TEXT NOT NULL, reversed_at TEXT NOT NULL
);
CREATE INDEX idx_ar_reversed_app_invoice ON ar_reversed_payment_applications(invoice_id);
