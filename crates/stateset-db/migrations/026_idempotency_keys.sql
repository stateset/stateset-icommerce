-- Migration 026: Idempotency keys for payments, refunds, and returns

ALTER TABLE payments ADD COLUMN idempotency_key TEXT;
ALTER TABLE refunds ADD COLUMN idempotency_key TEXT;
ALTER TABLE returns ADD COLUMN idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_payments_idempotency_key ON payments(idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_refunds_idempotency_key ON refunds(idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_returns_idempotency_key ON returns(idempotency_key);
