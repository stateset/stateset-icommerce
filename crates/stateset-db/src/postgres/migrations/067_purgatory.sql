-- Purgatory: orders ingested from a channel that are non-posted, pending SKU
-- mapping / line resolution before they enter inventory and accounting.
--
-- Repository: crates/stateset-db/src/postgres/purgatory.rs

CREATE TABLE IF NOT EXISTS purgatory_orders (
    id                UUID PRIMARY KEY,
    channel_id        UUID,
    external_order_id TEXT NOT NULL,
    external_status   TEXT,
    is_posted         BOOLEAN NOT NULL DEFAULT FALSE,
    hold_reason       TEXT,
    metadata          JSONB NOT NULL DEFAULT 'null'::jsonb,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_purgatory_orders_posted ON purgatory_orders (is_posted);
CREATE INDEX IF NOT EXISTS idx_purgatory_orders_channel ON purgatory_orders (channel_id) WHERE channel_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS purgatory_line_items (
    id                 UUID PRIMARY KEY,
    purgatory_order_id UUID NOT NULL REFERENCES purgatory_orders(id) ON DELETE CASCADE,
    external_sku       TEXT NOT NULL,
    product_id         UUID,
    quantity           NUMERIC(19,4) NOT NULL DEFAULT 0,
    ignore_item        BOOLEAN NOT NULL DEFAULT FALSE,
    non_physical       BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX IF NOT EXISTS idx_purgatory_line_items_order ON purgatory_line_items (purgatory_order_id);
