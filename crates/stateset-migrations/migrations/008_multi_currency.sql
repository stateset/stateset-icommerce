-- Multi-currency support
-- Exchange rates, currency settings, and multi-currency pricing

-- Exchange rates table
CREATE TABLE IF NOT EXISTS exchange_rates (
    id TEXT PRIMARY KEY,
    base_currency TEXT NOT NULL,
    quote_currency TEXT NOT NULL,
    rate TEXT NOT NULL,  -- Stored as decimal string for precision
    source TEXT NOT NULL DEFAULT 'manual',
    rate_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    -- Unique constraint: one rate per currency pair
    UNIQUE (base_currency, quote_currency)
);

-- Index for faster lookups
CREATE INDEX IF NOT EXISTS idx_exchange_rates_base ON exchange_rates(base_currency);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_quote ON exchange_rates(quote_currency);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_rate_at ON exchange_rates(rate_at);

-- Store currency settings
CREATE TABLE IF NOT EXISTS store_currency_settings (
    id TEXT PRIMARY KEY DEFAULT 'default',
    base_currency TEXT NOT NULL DEFAULT 'USD',
    enabled_currencies TEXT NOT NULL DEFAULT '["USD","EUR","GBP"]',  -- JSON array
    auto_convert INTEGER NOT NULL DEFAULT 1,
    rounding_mode TEXT NOT NULL DEFAULT 'half_up',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default settings if not exists
INSERT OR IGNORE INTO store_currency_settings (id, base_currency, enabled_currencies, auto_convert, rounding_mode)
VALUES ('default', 'USD', '["USD","EUR","GBP"]', 1, 'half_up');

-- Product multi-currency prices (optional override prices)
CREATE TABLE IF NOT EXISTS product_currency_prices (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    variant_id TEXT,  -- NULL for base product, set for variant
    currency TEXT NOT NULL,
    price TEXT NOT NULL,  -- Stored as decimal string
    compare_at_price TEXT,  -- Optional compare/original price
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE (product_id, variant_id, currency),
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_product_currency_prices_product ON product_currency_prices(product_id);
CREATE INDEX IF NOT EXISTS idx_product_currency_prices_currency ON product_currency_prices(currency);

-- Orders already include `currency` (see initial schema).
-- Add additional multi-currency metadata to orders.
ALTER TABLE orders ADD COLUMN exchange_rate TEXT;  -- Rate at time of order
ALTER TABLE orders ADD COLUMN base_currency_total TEXT;  -- Total in store's base currency

-- Add currency to order items (for multi-currency orders)
ALTER TABLE order_items ADD COLUMN currency TEXT DEFAULT 'USD';
ALTER TABLE order_items ADD COLUMN unit_price_base TEXT;  -- Price in base currency

-- Add currency to cart items
ALTER TABLE cart_items ADD COLUMN currency TEXT DEFAULT 'USD';

-- Exchange rate history for auditing
CREATE TABLE IF NOT EXISTS exchange_rate_history (
    id TEXT PRIMARY KEY,
    base_currency TEXT NOT NULL,
    quote_currency TEXT NOT NULL,
    rate TEXT NOT NULL,
    source TEXT NOT NULL,
    rate_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_exchange_rate_history_currencies ON exchange_rate_history(base_currency, quote_currency);
CREATE INDEX IF NOT EXISTS idx_exchange_rate_history_rate_at ON exchange_rate_history(rate_at);

-- Seed some initial exchange rates (approximate, should be updated from real source)
INSERT OR IGNORE INTO exchange_rates (id, base_currency, quote_currency, rate, source, rate_at)
VALUES
    (lower(hex(randomblob(16))), 'USD', 'EUR', '0.92', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'GBP', '0.79', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'JPY', '149.50', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'CAD', '1.36', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'AUD', '1.53', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'CHF', '0.88', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'CNY', '7.24', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'INR', '83.50', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'MXN', '17.15', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'USD', 'BRL', '4.97', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'EUR', 'USD', '1.09', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'EUR', 'GBP', '0.86', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'GBP', 'USD', '1.27', 'seed', datetime('now')),
    (lower(hex(randomblob(16))), 'GBP', 'EUR', '1.16', 'seed', datetime('now'));
