-- Multi-currency support for PostgreSQL
-- Exchange rates, currency settings

-- Exchange rates table
CREATE TABLE IF NOT EXISTS exchange_rates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    base_currency VARCHAR(3) NOT NULL,
    quote_currency VARCHAR(3) NOT NULL,
    rate DECIMAL(20, 10) NOT NULL,
    source VARCHAR(255) NOT NULL DEFAULT 'manual',
    rate_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (base_currency, quote_currency)
);

CREATE INDEX IF NOT EXISTS idx_exchange_rates_base ON exchange_rates(base_currency);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_quote ON exchange_rates(quote_currency);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_rate_at ON exchange_rates(rate_at);

-- Store currency settings
CREATE TABLE IF NOT EXISTS store_currency_settings (
    id VARCHAR(50) PRIMARY KEY DEFAULT 'default',
    base_currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    enabled_currencies JSONB NOT NULL DEFAULT '["USD","EUR","GBP"]'::jsonb,
    auto_convert BOOLEAN NOT NULL DEFAULT true,
    rounding_mode VARCHAR(20) NOT NULL DEFAULT 'half_up',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default settings
INSERT INTO store_currency_settings (id, base_currency, enabled_currencies, auto_convert, rounding_mode)
VALUES ('default', 'USD', '["USD","EUR","GBP"]'::jsonb, true, 'half_up')
ON CONFLICT (id) DO NOTHING;

-- Exchange rate history for auditing
CREATE TABLE IF NOT EXISTS exchange_rate_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    base_currency VARCHAR(3) NOT NULL,
    quote_currency VARCHAR(3) NOT NULL,
    rate DECIMAL(20, 10) NOT NULL,
    source VARCHAR(255) NOT NULL,
    rate_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_exchange_rate_history_currencies ON exchange_rate_history(base_currency, quote_currency);
CREATE INDEX IF NOT EXISTS idx_exchange_rate_history_rate_at ON exchange_rate_history(rate_at);

-- Seed initial exchange rates
INSERT INTO exchange_rates (base_currency, quote_currency, rate, source, rate_at)
VALUES
    ('USD', 'EUR', 0.92, 'seed', NOW()),
    ('USD', 'GBP', 0.79, 'seed', NOW()),
    ('USD', 'JPY', 149.50, 'seed', NOW()),
    ('USD', 'CAD', 1.36, 'seed', NOW()),
    ('USD', 'AUD', 1.53, 'seed', NOW()),
    ('EUR', 'USD', 1.09, 'seed', NOW()),
    ('EUR', 'GBP', 0.86, 'seed', NOW()),
    ('GBP', 'USD', 1.27, 'seed', NOW()),
    ('GBP', 'EUR', 1.16, 'seed', NOW())
ON CONFLICT (base_currency, quote_currency) DO NOTHING;
