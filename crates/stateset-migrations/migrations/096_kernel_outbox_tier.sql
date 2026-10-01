-- Two tiers share one outbox. 'governed' rows come from kernel commands and
-- carry policy, budget and a sealed receipt. 'recorded' rows come from ordinary
-- mutations and carry only the fact. Both are written in the same transaction
-- as the mutation, which is the property the VES log depends on.
ALTER TABLE kernel_outbox ADD COLUMN tier TEXT NOT NULL DEFAULT 'governed';

CREATE INDEX IF NOT EXISTS idx_kernel_outbox_tier_unpublished
    ON kernel_outbox (tier, published_at) WHERE published_at IS NULL;
