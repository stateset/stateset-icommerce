-- Tax engine schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS tax_jurisdictions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    parent_id UUID,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    level TEXT NOT NULL,
    country_code TEXT NOT NULL,
    state_code TEXT,
    county TEXT,
    city TEXT,
    postal_codes JSONB NOT NULL DEFAULT '[]'::jsonb,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (parent_id) REFERENCES tax_jurisdictions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_country ON tax_jurisdictions(country_code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_state ON tax_jurisdictions(country_code, state_code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_code ON tax_jurisdictions(code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_level ON tax_jurisdictions(level);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_parent ON tax_jurisdictions(parent_id);

CREATE TABLE IF NOT EXISTS tax_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    jurisdiction_id UUID NOT NULL REFERENCES tax_jurisdictions(id) ON DELETE CASCADE,
    tax_type TEXT NOT NULL DEFAULT 'sales_tax',
    product_category TEXT NOT NULL DEFAULT 'standard',
    rate NUMERIC(12, 6) NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_compound BOOLEAN NOT NULL DEFAULT FALSE,
    priority INTEGER NOT NULL DEFAULT 0,
    threshold_min NUMERIC(12, 4),
    threshold_max NUMERIC(12, 4),
    fixed_amount NUMERIC(12, 4),
    effective_from DATE NOT NULL,
    effective_to DATE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tax_rates_jurisdiction ON tax_rates(jurisdiction_id);
CREATE INDEX IF NOT EXISTS idx_tax_rates_type ON tax_rates(tax_type);
CREATE INDEX IF NOT EXISTS idx_tax_rates_category ON tax_rates(product_category);
CREATE INDEX IF NOT EXISTS idx_tax_rates_effective ON tax_rates(effective_from, effective_to);
CREATE INDEX IF NOT EXISTS idx_tax_rates_active ON tax_rates(active);

CREATE TABLE IF NOT EXISTS tax_exemptions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    exemption_type TEXT NOT NULL,
    certificate_number TEXT,
    issuing_authority TEXT,
    jurisdiction_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    exempt_categories JSONB NOT NULL DEFAULT '[]'::jsonb,
    effective_from DATE NOT NULL,
    expires_at DATE,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    verified_at TIMESTAMPTZ,
    notes TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tax_exemptions_customer ON tax_exemptions(customer_id);
CREATE INDEX IF NOT EXISTS idx_tax_exemptions_type ON tax_exemptions(exemption_type);
CREATE INDEX IF NOT EXISTS idx_tax_exemptions_active ON tax_exemptions(active);
CREATE INDEX IF NOT EXISTS idx_tax_exemptions_expires ON tax_exemptions(expires_at);

CREATE TABLE IF NOT EXISTS tax_settings (
    id TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    calculation_method TEXT NOT NULL DEFAULT 'exclusive',
    compound_method TEXT NOT NULL DEFAULT 'combined',
    tax_shipping BOOLEAN NOT NULL DEFAULT TRUE,
    tax_handling BOOLEAN NOT NULL DEFAULT TRUE,
    tax_gift_wrap BOOLEAN NOT NULL DEFAULT TRUE,
    origin_address JSONB,
    default_product_category TEXT NOT NULL DEFAULT 'standard',
    rounding_mode TEXT NOT NULL DEFAULT 'half_up',
    decimal_places INTEGER NOT NULL DEFAULT 2,
    validate_addresses BOOLEAN NOT NULL DEFAULT FALSE,
    tax_provider TEXT,
    provider_credentials TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO tax_settings (id)
VALUES ('default')
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS tax_calculations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID REFERENCES orders(id) ON DELETE SET NULL,
    cart_id UUID REFERENCES carts(id) ON DELETE SET NULL,
    customer_id UUID REFERENCES customers(id) ON DELETE SET NULL,
    subtotal NUMERIC(12, 4) NOT NULL,
    total_tax NUMERIC(12, 4) NOT NULL,
    shipping_tax NUMERIC(12, 4) NOT NULL DEFAULT 0,
    total NUMERIC(12, 4) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    shipping_address JSONB NOT NULL,
    billing_address JSONB,
    line_items JSONB NOT NULL,
    tax_breakdown JSONB NOT NULL,
    exemptions_applied BOOLEAN NOT NULL DEFAULT FALSE,
    exemption_details JSONB,
    is_estimate BOOLEAN NOT NULL DEFAULT TRUE,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tax_calculations_order ON tax_calculations(order_id);
CREATE INDEX IF NOT EXISTS idx_tax_calculations_cart ON tax_calculations(cart_id);
CREATE INDEX IF NOT EXISTS idx_tax_calculations_customer ON tax_calculations(customer_id);
CREATE INDEX IF NOT EXISTS idx_tax_calculations_date ON tax_calculations(calculated_at);
