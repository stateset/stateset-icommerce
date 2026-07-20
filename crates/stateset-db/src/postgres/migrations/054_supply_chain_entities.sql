-- Supply-chain entities previously SQLite-only:
--   production_batches, supplier_skus, vendor_returns (+items),
--   vendor_credits (+applications).
--
-- Repositories:
--   crates/stateset-db/src/postgres/production_batches.rs
--   crates/stateset-db/src/postgres/supplier_skus.rs
--   crates/stateset-db/src/postgres/vendor_returns.rs
--   crates/stateset-db/src/postgres/vendor_credits.rs

-- Production batches: group work orders into a vendor production run.
CREATE TABLE IF NOT EXISTS production_batches (
    id              UUID PRIMARY KEY,
    name            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'planned',
    vendor_id       UUID,
    work_order_ids  JSONB NOT NULL DEFAULT '[]'::jsonb,
    notes           TEXT,
    scheduled_start TIMESTAMPTZ,
    scheduled_end   TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_production_batches_status ON production_batches (status);
CREATE INDEX IF NOT EXISTS idx_production_batches_vendor ON production_batches (vendor_id) WHERE vendor_id IS NOT NULL;

-- Supplier SKUs: per-supplier SKU / unit-cost overrides for internal products.
CREATE TABLE IF NOT EXISTS supplier_skus (
    id             UUID PRIMARY KEY,
    product_id     UUID NOT NULL,
    supplier_id    UUID NOT NULL,
    sku            TEXT NOT NULL,
    unit_cost      NUMERIC(19,4),
    currency       TEXT NOT NULL DEFAULT 'USD',
    min_order_qty  NUMERIC(19,4),
    lead_time_days INTEGER,
    is_preferred   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (product_id, supplier_id, sku)
);
CREATE INDEX IF NOT EXISTS idx_supplier_skus_supplier ON supplier_skus (supplier_id);
CREATE INDEX IF NOT EXISTS idx_supplier_skus_product ON supplier_skus (product_id);

-- Vendor returns (return-to-supplier / RTV).
CREATE TABLE IF NOT EXISTS vendor_returns (
    id                UUID PRIMARY KEY,
    number            TEXT NOT NULL,
    supplier_id       UUID NOT NULL,
    purchase_order_id UUID,
    status            TEXT NOT NULL DEFAULT 'draft',
    currency          TEXT NOT NULL DEFAULT 'USD',
    credit_generated  BOOLEAN NOT NULL DEFAULT FALSE,
    notes             TEXT,
    processed_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_vendor_returns_supplier ON vendor_returns (supplier_id);
CREATE INDEX IF NOT EXISTS idx_vendor_returns_status ON vendor_returns (status);

CREATE TABLE IF NOT EXISTS vendor_return_items (
    id               UUID PRIMARY KEY,
    vendor_return_id UUID NOT NULL REFERENCES vendor_returns(id) ON DELETE CASCADE,
    product_id       UUID NOT NULL,
    sku              TEXT NOT NULL DEFAULT '',
    quantity         NUMERIC(19,4) NOT NULL,
    unit_cost        NUMERIC(19,4) NOT NULL DEFAULT 0,
    reason           TEXT NOT NULL DEFAULT 'defective'
);
CREATE INDEX IF NOT EXISTS idx_vendor_return_items_return ON vendor_return_items (vendor_return_id);

-- Vendor credits: amounts a supplier owes back, applicable against AP bills
-- or payment obligations.
CREATE TABLE IF NOT EXISTS vendor_credits (
    id               UUID PRIMARY KEY,
    number           TEXT NOT NULL,
    supplier_id      UUID NOT NULL,
    vendor_return_id UUID,
    amount           NUMERIC(19,4) NOT NULL,
    remaining        NUMERIC(19,4) NOT NULL,
    currency         TEXT NOT NULL DEFAULT 'USD',
    status           TEXT NOT NULL DEFAULT 'open',
    memo             TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT vendor_credits_amount_positive CHECK (amount > 0),
    CONSTRAINT vendor_credits_remaining_non_negative CHECK (remaining >= 0)
);
CREATE INDEX IF NOT EXISTS idx_vendor_credits_supplier ON vendor_credits (supplier_id);
CREATE INDEX IF NOT EXISTS idx_vendor_credits_status ON vendor_credits (status);

CREATE TABLE IF NOT EXISTS vendor_credit_applications (
    id               UUID PRIMARY KEY,
    vendor_credit_id UUID NOT NULL REFERENCES vendor_credits(id) ON DELETE CASCADE,
    target_type      TEXT NOT NULL DEFAULT 'bill',
    target_id        UUID NOT NULL,
    amount           NUMERIC(19,4) NOT NULL,
    reversed         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_vendor_credit_apps_credit ON vendor_credit_applications (vendor_credit_id);
