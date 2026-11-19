CREATE INDEX IF NOT EXISTS idx_billing_cycles_payment ON billing_cycles(payment_id)
    WHERE payment_id IS NOT NULL;

CREATE FUNCTION refund_linked_billing_cycles() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE billing_cycles
       SET status = 'refunded', updated_at = NEW.updated_at
     WHERE payment_id = NEW.id AND status = 'paid';
    RETURN NEW;
END;
$$;

CREATE TRIGGER refund_linked_billing_cycles AFTER UPDATE OF status ON payments
    FOR EACH ROW WHEN (NEW.status = 'refunded' AND OLD.status <> NEW.status)
    EXECUTE FUNCTION refund_linked_billing_cycles();

-- Reconcile completed refunds recorded before this migration.
UPDATE billing_cycles
   SET status = 'refunded', updated_at = CURRENT_TIMESTAMP
 WHERE status = 'paid' AND payment_id IN (SELECT id FROM payments WHERE status = 'refunded');
