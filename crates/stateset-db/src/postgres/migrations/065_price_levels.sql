-- Price levels: named B2B pricing tiers with catalog-wide adjustments and
-- optional per-product fixed-price entries.
--
-- Repository: crates/stateset-db/src/postgres/price_levels.rs

CREATE TABLE IF NOT EXISTS price_levels (
    id               UUID PRIMARY KEY,
    name             TEXT NOT NULL,
    code             TEXT NOT NULL UNIQUE,
    description      TEXT,
    adjustment_type  TEXT NOT NULL DEFAULT 'none',
    adjustment_value NUMERIC(19,4) NOT NULL DEFAULT 0,
    currency         TEXT NOT NULL DEFAULT 'USD',
    is_active        BOOLEAN NOT NULL DEFAULT TRUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_price_levels_active ON price_levels (is_active);

CREATE TABLE IF NOT EXISTS price_level_entries (
    price_level_id UUID NOT NULL REFERENCES price_levels(id) ON DELETE CASCADE,
    product_id     UUID NOT NULL,
    price          NUMERIC(19,4) NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (price_level_id, product_id)
);
