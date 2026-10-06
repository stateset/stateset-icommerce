-- The amount a payment actually captured, separate from what it authorized.
--
-- `amount` is the authorized amount. A processor may capture less (a partial
-- capture); only what moved can be refunded, and only that much of the order
-- total is consumed. NULL means "not captured yet": an in-flight payment still
-- reserves its whole `amount`. TEXT money, like every other money column.
ALTER TABLE payments ADD COLUMN captured_amount TEXT;

-- Every payment that already captured money captured all of it (partial
-- capture was refused before this column existed).
UPDATE payments
   SET captured_amount = amount
 WHERE captured_amount IS NULL
   AND status IN ('completed', 'partially_refunded', 'refunded', 'disputed');

-- Pending-refund reconciliation scans in-flight refunds by processor id.
CREATE INDEX IF NOT EXISTS idx_refunds_in_flight
    ON refunds (status, created_at)
    WHERE status IN ('pending', 'processing') AND external_id IS NOT NULL;
