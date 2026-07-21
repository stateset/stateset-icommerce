-- Sales / fulfillment channels (+ channel SKU mappings), previously
-- SQLite-only.
--
-- Repository: crates/stateset-db/src/postgres/channels.rs

-- Channels: external integration points through which orders flow in
-- (sales channels) and/or out (fulfillment channels).
CREATE TABLE IF NOT EXISTS channels (
    id                   UUID PRIMARY KEY,
    name                 TEXT NOT NULL,
    channel_type         TEXT NOT NULL DEFAULT 'sales_channel',
    integration          TEXT,
    status               TEXT NOT NULL DEFAULT 'active',
    api_locked           BOOLEAN NOT NULL DEFAULT FALSE,
    default_warehouse_id UUID,
    tags                 JSONB NOT NULL DEFAULT '[]'::jsonb,
    metadata             JSONB NOT NULL DEFAULT 'null'::jsonb,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_channels_type ON channels (channel_type);
CREATE INDEX IF NOT EXISTS idx_channels_status ON channels (status);

-- Channel product mappings: translate a channel-specific SKU to an internal
-- product / SKU. One mapping per (channel, channel_sku).
CREATE TABLE IF NOT EXISTS channel_product_mappings (
    channel_id   UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    channel_sku  TEXT NOT NULL,
    product_id   UUID NOT NULL,
    internal_sku TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (channel_id, channel_sku)
);
CREATE INDEX IF NOT EXISTS idx_channel_mappings_product ON channel_product_mappings (product_id);
