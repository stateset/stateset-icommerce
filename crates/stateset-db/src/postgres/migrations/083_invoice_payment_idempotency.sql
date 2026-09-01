-- Migration 083: Idempotency ledger for direct invoice payments
--
-- `RecordInvoicePayment.payment_id` was accepted and ignored, so a retried
-- payment (a client timeout, a queue redelivery) applied the amount twice and
-- silently doubled `amount_paid` / `direct_amount_paid`. One row per
-- (invoice, payment) records that a direct payment has already been applied;
-- `record_payment_async` checks and writes it inside the payment's own
-- transaction (the invoice row is already held with FOR UPDATE), so a
-- concurrent retry cannot slip between the check and the write.
--
-- The key is (invoice_id, payment_id), not payment_id alone: one payment may
-- legitimately be recorded against several invoices, and only a repeat against
-- the SAME invoice is a retry. A NULL payment_id records no row and keeps the
-- previous (non-idempotent) behavior.

CREATE TABLE IF NOT EXISTS invoice_direct_payments (
    invoice_id UUID NOT NULL,
    payment_id UUID NOT NULL,
    amount DECIMAL(12, 2) NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (invoice_id, payment_id)
);

CREATE INDEX IF NOT EXISTS idx_invoice_direct_payments_payment
    ON invoice_direct_payments(payment_id);
