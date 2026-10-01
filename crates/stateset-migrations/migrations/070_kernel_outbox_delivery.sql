ALTER TABLE kernel_outbox ADD COLUMN lease_owner TEXT;
ALTER TABLE kernel_outbox ADD COLUMN lease_expires_at TEXT;
ALTER TABLE kernel_outbox ADD COLUMN next_attempt_at TEXT;
ALTER TABLE kernel_outbox ADD COLUMN dead_lettered_at TEXT;

CREATE INDEX IF NOT EXISTS idx_kernel_outbox_delivery
    ON kernel_outbox(next_attempt_at, created_at, id)
    WHERE published_at IS NULL AND dead_lettered_at IS NULL;
