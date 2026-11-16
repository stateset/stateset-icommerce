CREATE TABLE ar_reversed_payment_applications (
 id UUID PRIMARY KEY, payment_id UUID NOT NULL, invoice_id UUID NOT NULL,
 applied_amount NUMERIC NOT NULL, applied_date TIMESTAMPTZ NOT NULL, reversed_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_ar_reversed_app_invoice ON ar_reversed_payment_applications(invoice_id);
