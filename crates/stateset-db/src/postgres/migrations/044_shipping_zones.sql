-- Shipping zones for geographic rate configuration
CREATE TABLE IF NOT EXISTS shipping_zones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    countries JSONB NOT NULL DEFAULT '[]'::JSONB,
    regions JSONB NOT NULL DEFAULT '[]'::JSONB,
    postal_codes JSONB NOT NULL DEFAULT '[]'::JSONB,
    priority INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_shipping_zones_is_active ON shipping_zones (is_active);
CREATE INDEX IF NOT EXISTS idx_shipping_zones_priority ON shipping_zones (priority ASC);
CREATE INDEX IF NOT EXISTS idx_shipping_zones_created_at ON shipping_zones (created_at DESC);
