CREATE INDEX IF NOT EXISTS idx_billing_cycles_payment ON billing_cycles(payment_id)
    WHERE payment_id IS NOT NULL;

CREATE TRIGGER IF NOT EXISTS refund_linked_billing_cycles
AFTER UPDATE OF status ON payments
WHEN NEW.status = 'refunded' AND OLD.status <> NEW.status
BEGIN
    UPDATE billing_cycles
       SET status = 'refunded', updated_at = NEW.updated_at
     WHERE payment_id = NEW.id AND status = 'paid';
END;

-- Reconcile completed refunds recorded before this migration.
UPDATE billing_cycles
   SET status = 'refunded', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
 WHERE status = 'paid' AND payment_id IN (SELECT id FROM payments WHERE status = 'refunded');
