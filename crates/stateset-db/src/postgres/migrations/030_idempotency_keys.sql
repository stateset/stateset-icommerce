-- Migration 030: Idempotency keys for payments, refunds, and returns

ALTER TABLE payments ADD COLUMN IF NOT EXISTS idempotency_key TEXT;
ALTER TABLE refunds ADD COLUMN IF NOT EXISTS idempotency_key TEXT;
ALTER TABLE returns ADD COLUMN IF NOT EXISTS idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_payments_idempotency_key ON payments(idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_refunds_idempotency_key ON refunds(idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_returns_idempotency_key ON returns(idempotency_key);
