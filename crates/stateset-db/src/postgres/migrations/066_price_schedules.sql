-- Price schedules: time-bounded sets of product price overrides (promotional
-- windows, seasonal lists). Per-product prices live in entries.
--
-- Repository: crates/stateset-db/src/postgres/price_schedules.rs

CREATE TABLE IF NOT EXISTS price_schedules (
    id         UUID PRIMARY KEY,
    name       TEXT NOT NULL,
    code       TEXT,
    currency   TEXT NOT NULL DEFAULT 'USD',
    starts_at  TIMESTAMPTZ,
    ends_at    TIMESTAMPTZ,
    is_active  BOOLEAN NOT NULL DEFAULT TRUE,
    priority   INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_price_schedules_active ON price_schedules (is_active);

CREATE TABLE IF NOT EXISTS price_schedule_entries (
    price_schedule_id UUID NOT NULL REFERENCES price_schedules(id) ON DELETE CASCADE,
    product_id        UUID NOT NULL,
    price             NUMERIC(19,4) NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (price_schedule_id, product_id)
);
CREATE INDEX IF NOT EXISTS idx_price_schedule_entries_product ON price_schedule_entries (product_id);
