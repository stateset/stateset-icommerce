-- Direct invoice payments (record_payment) previously wrote only to
-- invoices.amount_paid, while the AR recalculation REPLACED amount_paid with
-- SUM(ar_payment_applications) + SUM(ar_credit_memo_applications) — silently
-- erasing direct payments the next time a credit memo or payment application
-- touched the invoice. Track direct payments in their own bookkeeping column
-- so recalculation can add them back in:
--
--   amount_paid == direct_amount_paid + payment applications + credit memos
ALTER TABLE invoices
    ADD COLUMN IF NOT EXISTS direct_amount_paid DECIMAL(12, 2) NOT NULL DEFAULT 0;

-- Backfill: NUMERIC arithmetic is exact on Postgres, so recover the direct
-- portion of what has already been paid as whatever amount_paid exceeds the
-- recorded applications by (never negative).
UPDATE invoices i
SET direct_amount_paid = GREATEST(
    i.amount_paid
        - COALESCE((SELECT SUM(a.applied_amount)
                    FROM ar_payment_applications a
                    WHERE a.invoice_id = i.id), 0)
        - COALESCE((SELECT SUM(c.applied_amount)
                    FROM ar_credit_memo_applications c
                    WHERE c.invoice_id = i.id), 0),
    0
);
