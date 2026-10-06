-- The amount a payment actually captured, separate from what it authorized.
--
-- `amount` is the authorized amount. A processor may capture less (a partial
-- capture); only what moved can be refunded, and only that much of the order
-- total is consumed. NULL means "not captured yet": an in-flight payment still
-- reserves its whole `amount`. Same precision as `amount`.
ALTER TABLE payments ADD COLUMN IF NOT EXISTS captured_amount DECIMAL(12, 2);

-- Every payment that already captured money captured all of it (partial
-- capture was refused before this column existed).
UPDATE payments
   SET captured_amount = amount
 WHERE captured_amount IS NULL
   AND status IN ('completed', 'partially_refunded', 'refunded', 'disputed');

ALTER TABLE payments DROP CONSTRAINT IF EXISTS payments_captured_amount_bounds;
ALTER TABLE payments ADD CONSTRAINT payments_captured_amount_bounds
    CHECK (captured_amount IS NULL OR (captured_amount >= 0 AND captured_amount <= amount))
    NOT VALID;

-- Pending-refund reconciliation scans in-flight refunds by processor id.
CREATE INDEX IF NOT EXISTS idx_refunds_in_flight
    ON refunds (status, created_at)
    WHERE status IN ('pending', 'processing') AND external_id IS NOT NULL;
