ALTER TABLE kernel_outbox ADD COLUMN IF NOT EXISTS lease_owner TEXT;
ALTER TABLE kernel_outbox ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;
ALTER TABLE kernel_outbox ADD COLUMN IF NOT EXISTS next_attempt_at TIMESTAMPTZ;
ALTER TABLE kernel_outbox ADD COLUMN IF NOT EXISTS dead_lettered_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_kernel_outbox_delivery
    ON kernel_outbox(next_attempt_at, created_at, id)
    WHERE published_at IS NULL AND dead_lettered_at IS NULL;
