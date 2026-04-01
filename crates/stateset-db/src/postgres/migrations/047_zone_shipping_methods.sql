-- Zone shipping method migration for PostgreSQL
-- Configurable shipping methods within geographic zones with rate conditions

CREATE TABLE IF NOT EXISTS zone_shipping_methods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zone_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    carrier VARCHAR(255),
    method_type VARCHAR(50) NOT NULL DEFAULT 'flat',
    base_rate NUMERIC(12, 4) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    min_delivery_days INTEGER,
    max_delivery_days INTEGER,
    conditions JSONB NOT NULL DEFAULT '[]'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_zone_id ON zone_shipping_methods(zone_id);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_carrier ON zone_shipping_methods(carrier);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_method_type ON zone_shipping_methods(method_type);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_is_active ON zone_shipping_methods(is_active);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_created_at ON zone_shipping_methods(created_at DESC);

-- Check constraints
ALTER TABLE zone_shipping_methods ADD CONSTRAINT zone_shipping_methods_base_rate_non_negative CHECK (base_rate >= 0);
