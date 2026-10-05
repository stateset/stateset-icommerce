-- Tax engine schema
-- Multi-jurisdiction tax calculation with exemptions and rates

-- Tax jurisdictions (countries, states, counties, cities, districts)
CREATE TABLE IF NOT EXISTS tax_jurisdictions (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,  -- e.g., "US", "US-CA", "US-CA-LA"
    level TEXT NOT NULL,  -- country, state, county, city, district, special
    country_code TEXT NOT NULL,  -- ISO 3166-1 alpha-2
    state_code TEXT,  -- ISO 3166-2
    county TEXT,
    city TEXT,
    postal_codes TEXT NOT NULL DEFAULT '[]',  -- JSON array of covered postal codes
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (parent_id) REFERENCES tax_jurisdictions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_country ON tax_jurisdictions(country_code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_state ON tax_jurisdictions(country_code, state_code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_code ON tax_jurisdictions(code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_level ON tax_jurisdictions(level);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_parent ON tax_jurisdictions(parent_id);

-- Tax rates for jurisdictions and product categories
CREATE TABLE IF NOT EXISTS tax_rates (
    id TEXT PRIMARY KEY,
    jurisdiction_id TEXT NOT NULL,
    tax_type TEXT NOT NULL DEFAULT 'sales_tax',  -- sales_tax, vat, gst, hst, pst, qst, consumption_tax, custom
    product_category TEXT NOT NULL DEFAULT 'standard',  -- standard, reduced, exempt, digital, clothing, food, etc.
    rate TEXT NOT NULL,  -- Decimal as string (e.g., "0.0825" for 8.25%)
    name TEXT NOT NULL,  -- Display name (e.g., "California State Tax")
    description TEXT,
    is_compound INTEGER NOT NULL DEFAULT 0,  -- Whether to apply after other taxes
    priority INTEGER NOT NULL DEFAULT 0,  -- Lower = applied first
    threshold_min TEXT,  -- Minimum amount for tax to apply
    threshold_max TEXT,  -- Maximum amount taxed (cap)
    fixed_amount TEXT,  -- Fixed amount instead of percentage
    effective_from TEXT NOT NULL,  -- Date when rate becomes effective
    effective_to TEXT,  -- Date when rate expires (NULL = no expiration)
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (jurisdiction_id) REFERENCES tax_jurisdictions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tax_rates_jurisdiction ON tax_rates(jurisdiction_id);
CREATE INDEX IF NOT EXISTS idx_tax_rates_type ON tax_rates(tax_type);
CREATE INDEX IF NOT EXISTS idx_tax_rates_category ON tax_rates(product_category);
CREATE INDEX IF NOT EXISTS idx_tax_rates_effective ON tax_rates(effective_from, effective_to);
CREATE INDEX IF NOT EXISTS idx_tax_rates_active ON tax_rates(active);

-- Customer tax exemptions (resale certificates, non-profits, etc.)
CREATE TABLE IF NOT EXISTS tax_exemptions (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    exemption_type TEXT NOT NULL,  -- resale, non_profit, government, educational, etc.
    certificate_number TEXT,
    issuing_authority TEXT,
    jurisdiction_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array of jurisdiction IDs (empty = all)
    exempt_categories TEXT NOT NULL DEFAULT '[]',  -- JSON array of product categories (empty = all)
    effective_from TEXT NOT NULL,
    expires_at TEXT,
    verified INTEGER NOT NULL DEFAULT 0,
    verified_at TEXT,
    notes TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tax_exemptions_customer ON tax_exemptions(customer_id);
CREATE INDEX IF NOT EXISTS idx_tax_exemptions_type ON tax_exemptions(exemption_type);
CREATE INDEX IF NOT EXISTS idx_tax_exemptions_active ON tax_exemptions(active);
CREATE INDEX IF NOT EXISTS idx_tax_exemptions_expires ON tax_exemptions(expires_at);

-- Store tax settings
CREATE TABLE IF NOT EXISTS tax_settings (
    id TEXT PRIMARY KEY DEFAULT 'default',
    enabled INTEGER NOT NULL DEFAULT 1,
    calculation_method TEXT NOT NULL DEFAULT 'exclusive',  -- exclusive (tax added), inclusive (tax included)
    compound_method TEXT NOT NULL DEFAULT 'combined',  -- combined, compound, separate
    tax_shipping INTEGER NOT NULL DEFAULT 1,
    tax_handling INTEGER NOT NULL DEFAULT 1,
    tax_gift_wrap INTEGER NOT NULL DEFAULT 1,
    origin_address TEXT,  -- JSON address object for origin-based tax states
    default_product_category TEXT NOT NULL DEFAULT 'standard',
    rounding_mode TEXT NOT NULL DEFAULT 'half_up',
    decimal_places INTEGER NOT NULL DEFAULT 2,
    validate_addresses INTEGER NOT NULL DEFAULT 0,
    tax_provider TEXT,  -- avalara, taxjar, vertex, none
    provider_credentials TEXT,  -- Encrypted API credentials
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default tax settings
INSERT OR IGNORE INTO tax_settings (id, enabled, calculation_method, tax_shipping, default_product_category)
VALUES ('default', 1, 'exclusive', 1, 'standard');

-- Tax calculation log (audit trail)
CREATE TABLE IF NOT EXISTS tax_calculations (
    id TEXT PRIMARY KEY,
    order_id TEXT,
    cart_id TEXT,
    customer_id TEXT,
    subtotal TEXT NOT NULL,
    total_tax TEXT NOT NULL,
    shipping_tax TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    shipping_address TEXT NOT NULL,  -- JSON address
    billing_address TEXT,  -- JSON address
    line_items TEXT NOT NULL,  -- JSON array of line items with tax
    tax_breakdown TEXT NOT NULL,  -- JSON array of tax breakdown by jurisdiction
    exemptions_applied INTEGER NOT NULL DEFAULT 0,
    exemption_details TEXT,  -- JSON exemption info
    is_estimate INTEGER NOT NULL DEFAULT 1,  -- 0 = committed transaction
    calculated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE SET NULL,
    FOREIGN KEY (cart_id) REFERENCES carts(id) ON DELETE SET NULL,
    FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_tax_calculations_order ON tax_calculations(order_id);
CREATE INDEX IF NOT EXISTS idx_tax_calculations_cart ON tax_calculations(cart_id);
CREATE INDEX IF NOT EXISTS idx_tax_calculations_customer ON tax_calculations(customer_id);
CREATE INDEX IF NOT EXISTS idx_tax_calculations_date ON tax_calculations(calculated_at);

-- Note: tax_amount already exists in orders (001_initial_schema.sql), carts, cart_items (007_carts.sql)
-- Only add columns that don't already exist

-- Add additional tax columns to orders (tax_amount already exists)
-- Using INSERT OR IGNORE approach via temp table for SQLite compatibility
-- SQLite doesn't support ADD COLUMN IF NOT EXISTS, so we need to handle errors gracefully
-- These columns may fail silently if they already exist (in production, use proper migration tooling)

-- For carts: estimated_tax and tax_calculation_id are new
-- Note: tax_amount already exists in carts table

-- Create a temp trigger-based workaround - skip ALTER TABLE for existing columns
-- Instead, these will be handled by the application layer if columns don't exist

-- Seed US tax jurisdictions
INSERT OR IGNORE INTO tax_jurisdictions (id, parent_id, name, code, level, country_code, state_code, postal_codes)
VALUES
    -- Country
    (lower(hex(randomblob(16))), NULL, 'United States', 'US', 'country', 'US', NULL, '[]'),

    -- States with no sales tax
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Alaska', 'US-AK', 'state', 'US', 'AK', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Delaware', 'US-DE', 'state', 'US', 'DE', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Montana', 'US-MT', 'state', 'US', 'MT', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'New Hampshire', 'US-NH', 'state', 'US', 'NH', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Oregon', 'US-OR', 'state', 'US', 'OR', '[]'),

    -- States with sales tax
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'California', 'US-CA', 'state', 'US', 'CA', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Texas', 'US-TX', 'state', 'US', 'TX', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Florida', 'US-FL', 'state', 'US', 'FL', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'New York', 'US-NY', 'state', 'US', 'NY', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Washington', 'US-WA', 'state', 'US', 'WA', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Arizona', 'US-AZ', 'state', 'US', 'AZ', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Colorado', 'US-CO', 'state', 'US', 'CO', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Illinois', 'US-IL', 'state', 'US', 'IL', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Pennsylvania', 'US-PA', 'state', 'US', 'PA', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'US'), 'Ohio', 'US-OH', 'state', 'US', 'OH', '[]');

-- Seed US state tax rates
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'sales_tax',
    'standard',
    CASE code
        WHEN 'US-CA' THEN '0.0725'  -- 7.25%
        WHEN 'US-TX' THEN '0.0625'  -- 6.25%
        WHEN 'US-FL' THEN '0.0600'  -- 6%
        WHEN 'US-NY' THEN '0.0400'  -- 4%
        WHEN 'US-WA' THEN '0.0650'  -- 6.5%
        WHEN 'US-AZ' THEN '0.0560'  -- 5.6%
        WHEN 'US-CO' THEN '0.0290'  -- 2.9%
        WHEN 'US-IL' THEN '0.0625'  -- 6.25%
        WHEN 'US-PA' THEN '0.0600'  -- 6%
        WHEN 'US-OH' THEN '0.0575'  -- 5.75%
        ELSE '0'
    END,
    name || ' State Tax',
    date('now')
FROM tax_jurisdictions
WHERE level = 'state' AND country_code = 'US'
  AND code NOT IN ('US-AK', 'US-DE', 'US-MT', 'US-NH', 'US-OR');

-- Seed EU VAT jurisdictions
INSERT OR IGNORE INTO tax_jurisdictions (id, parent_id, name, code, level, country_code, state_code, postal_codes)
VALUES
    (lower(hex(randomblob(16))), NULL, 'Germany', 'DE', 'country', 'DE', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'France', 'FR', 'country', 'FR', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'United Kingdom', 'GB', 'country', 'GB', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'Italy', 'IT', 'country', 'IT', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'Spain', 'ES', 'country', 'ES', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'Netherlands', 'NL', 'country', 'NL', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'Ireland', 'IE', 'country', 'IE', NULL, '[]'),
    (lower(hex(randomblob(16))), NULL, 'Sweden', 'SE', 'country', 'SE', NULL, '[]');

-- Seed EU VAT rates (standard)
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'vat',
    'standard',
    CASE code
        WHEN 'DE' THEN '0.19'  -- 19%
        WHEN 'FR' THEN '0.20'  -- 20%
        WHEN 'GB' THEN '0.20'  -- 20%
        WHEN 'IT' THEN '0.22'  -- 22%
        WHEN 'ES' THEN '0.21'  -- 21%
        WHEN 'NL' THEN '0.21'  -- 21%
        WHEN 'IE' THEN '0.23'  -- 23%
        WHEN 'SE' THEN '0.25'  -- 25%
        ELSE '0.20'
    END,
    name || ' Standard VAT',
    date('now')
FROM tax_jurisdictions
WHERE level = 'country' AND code IN ('DE', 'FR', 'GB', 'IT', 'ES', 'NL', 'IE', 'SE');

-- Seed EU VAT reduced rates
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'vat',
    'reduced',
    CASE code
        WHEN 'DE' THEN '0.07'  -- 7%
        WHEN 'FR' THEN '0.10'  -- 10%
        WHEN 'GB' THEN '0.05'  -- 5%
        WHEN 'IT' THEN '0.10'  -- 10%
        WHEN 'ES' THEN '0.10'  -- 10%
        WHEN 'NL' THEN '0.09'  -- 9%
        WHEN 'IE' THEN '0.135' -- 13.5%
        WHEN 'SE' THEN '0.12'  -- 12%
        ELSE '0.10'
    END,
    name || ' Reduced VAT',
    date('now')
FROM tax_jurisdictions
WHERE level = 'country' AND code IN ('DE', 'FR', 'GB', 'IT', 'ES', 'NL', 'IE', 'SE');

-- Seed Canadian tax jurisdictions
INSERT OR IGNORE INTO tax_jurisdictions (id, parent_id, name, code, level, country_code, state_code, postal_codes)
VALUES
    (lower(hex(randomblob(16))), NULL, 'Canada', 'CA', 'country', 'CA', NULL, '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'Ontario', 'CA-ON', 'state', 'CA', 'ON', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'Quebec', 'CA-QC', 'state', 'CA', 'QC', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'British Columbia', 'CA-BC', 'state', 'CA', 'BC', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'Alberta', 'CA-AB', 'state', 'CA', 'AB', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'Nova Scotia', 'CA-NS', 'state', 'CA', 'NS', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'New Brunswick', 'CA-NB', 'state', 'CA', 'NB', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'Manitoba', 'CA-MB', 'state', 'CA', 'MB', '[]'),
    (lower(hex(randomblob(16))), (SELECT id FROM tax_jurisdictions WHERE code = 'CA'), 'Saskatchewan', 'CA-SK', 'state', 'CA', 'SK', '[]');

-- Seed Canadian tax rates
-- GST (federal) for all provinces
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'gst',
    'standard',
    '0.05',  -- 5% GST
    'Federal GST',
    date('now')
FROM tax_jurisdictions
WHERE code = 'CA';

-- HST provinces (combined GST+PST)
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'hst',
    'standard',
    CASE code
        WHEN 'CA-ON' THEN '0.13'  -- 13%
        WHEN 'CA-NS' THEN '0.15'  -- 15%
        WHEN 'CA-NB' THEN '0.15'  -- 15%
        ELSE '0.13'
    END,
    name || ' HST',
    date('now')
FROM tax_jurisdictions
WHERE code IN ('CA-ON', 'CA-NS', 'CA-NB');

-- PST provinces (separate from GST)
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'pst',
    'standard',
    CASE code
        WHEN 'CA-BC' THEN '0.07'  -- 7%
        WHEN 'CA-MB' THEN '0.07'  -- 7%
        WHEN 'CA-SK' THEN '0.06'  -- 6%
        ELSE '0.07'
    END,
    name || ' PST',
    date('now')
FROM tax_jurisdictions
WHERE code IN ('CA-BC', 'CA-MB', 'CA-SK');

-- QST for Quebec (applied on top of GST)
INSERT OR IGNORE INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, is_compound, effective_from)
SELECT
    lower(hex(randomblob(16))),
    id,
    'qst',
    'standard',
    '0.09975',  -- 9.975%
    'Quebec QST',
    1,  -- Compound (applied after GST)
    date('now')
FROM tax_jurisdictions
WHERE code = 'CA-QC';
