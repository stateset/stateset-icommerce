-- Promotions and discounts schema
-- Comprehensive promotions engine with coupons, conditions, and usage tracking

-- Main promotions table
CREATE TABLE IF NOT EXISTS promotions (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,  -- Internal reference code (e.g., "PROMO-123456-1234")
    name TEXT NOT NULL,
    description TEXT,
    internal_notes TEXT,

    -- Type and behavior
    promotion_type TEXT NOT NULL DEFAULT 'percentage_off',  -- percentage_off, fixed_amount_off, buy_x_get_y, free_shipping, tiered_discount, bundle_discount, first_order_discount, gift_with_purchase
    trigger TEXT NOT NULL DEFAULT 'automatic',  -- automatic, coupon_code, both
    target TEXT NOT NULL DEFAULT 'order',  -- order, product, category, shipping, line_item
    stacking TEXT NOT NULL DEFAULT 'stackable',  -- stackable, exclusive, selective_stack
    status TEXT NOT NULL DEFAULT 'draft',  -- draft, scheduled, active, paused, expired, exhausted, archived

    -- Discount values
    percentage_off TEXT,  -- Decimal as string (e.g., "0.20" for 20%)
    fixed_amount_off TEXT,  -- Decimal as string
    max_discount_amount TEXT,  -- Cap on discount

    -- Buy X Get Y specifics
    buy_quantity INTEGER,
    get_quantity INTEGER,
    get_discount_percent TEXT,  -- 1.0 = free, 0.5 = 50% off

    -- Tiered discount specifics (JSON array)
    tiers TEXT,  -- JSON array of {min_value, max_value, percentage_off, fixed_amount_off}

    -- Bundle specifics
    bundle_product_ids TEXT,  -- JSON array of UUIDs
    bundle_discount TEXT,

    -- Validity period
    starts_at TEXT NOT NULL DEFAULT (datetime('now')),
    ends_at TEXT,

    -- Usage limits
    total_usage_limit INTEGER,  -- NULL = unlimited
    per_customer_limit INTEGER,  -- NULL = unlimited
    usage_count INTEGER NOT NULL DEFAULT 0,

    -- Targeting - products/categories
    applicable_product_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array
    applicable_category_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array
    applicable_skus TEXT NOT NULL DEFAULT '[]',  -- JSON array
    excluded_product_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array
    excluded_category_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array

    -- Customer targeting
    eligible_customer_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array
    eligible_customer_groups TEXT NOT NULL DEFAULT '[]',  -- JSON array

    -- Currency
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Priority (lower = applied first)
    priority INTEGER NOT NULL DEFAULT 0,

    -- Metadata
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_promotions_code ON promotions(code);
CREATE INDEX IF NOT EXISTS idx_promotions_status ON promotions(status);
CREATE INDEX IF NOT EXISTS idx_promotions_type ON promotions(promotion_type);
CREATE INDEX IF NOT EXISTS idx_promotions_trigger ON promotions(trigger);
CREATE INDEX IF NOT EXISTS idx_promotions_dates ON promotions(starts_at, ends_at);
CREATE INDEX IF NOT EXISTS idx_promotions_priority ON promotions(priority);

-- Promotion conditions (rules that must be met)
CREATE TABLE IF NOT EXISTS promotion_conditions (
    id TEXT PRIMARY KEY,
    promotion_id TEXT NOT NULL,
    condition_type TEXT NOT NULL,  -- minimum_subtotal, minimum_quantity, product_in_cart, category_in_cart, sku_in_cart, customer_group, first_order, customer_email_domain, shipping_country, shipping_state, payment_method, cart_item_count, customer_id
    operator TEXT NOT NULL DEFAULT 'equals',  -- equals, not_equals, greater_than, greater_than_or_equal, less_than, less_than_or_equal, contains, not_contains, in, not_in
    value TEXT NOT NULL,  -- The value to compare against
    is_required INTEGER NOT NULL DEFAULT 1,  -- 1 = AND logic, 0 = OR logic

    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (promotion_id) REFERENCES promotions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_promotion_conditions_promotion ON promotion_conditions(promotion_id);
CREATE INDEX IF NOT EXISTS idx_promotion_conditions_type ON promotion_conditions(condition_type);

-- Coupon codes (link to promotions)
CREATE TABLE IF NOT EXISTS coupon_codes (
    id TEXT PRIMARY KEY,
    promotion_id TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,  -- The code customers enter (e.g., "SAVE20")
    status TEXT NOT NULL DEFAULT 'active',  -- active, disabled, exhausted, expired

    -- Override limits (NULL = use promotion's limits)
    usage_limit INTEGER,
    per_customer_limit INTEGER,
    usage_count INTEGER NOT NULL DEFAULT 0,

    -- Validity (NULL = use promotion's dates)
    starts_at TEXT,
    ends_at TEXT,

    -- Metadata
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (promotion_id) REFERENCES promotions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_coupon_codes_code ON coupon_codes(code);
CREATE INDEX IF NOT EXISTS idx_coupon_codes_promotion ON coupon_codes(promotion_id);
CREATE INDEX IF NOT EXISTS idx_coupon_codes_status ON coupon_codes(status);

-- Promotion usage tracking
CREATE TABLE IF NOT EXISTS promotion_usage (
    id TEXT PRIMARY KEY,
    promotion_id TEXT NOT NULL,
    coupon_id TEXT,
    customer_id TEXT,
    order_id TEXT,
    cart_id TEXT,

    discount_amount TEXT NOT NULL,  -- Decimal as string
    currency TEXT NOT NULL DEFAULT 'USD',

    used_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (promotion_id) REFERENCES promotions(id) ON DELETE CASCADE,
    FOREIGN KEY (coupon_id) REFERENCES coupon_codes(id) ON DELETE SET NULL,
    FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE SET NULL,
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE SET NULL,
    FOREIGN KEY (cart_id) REFERENCES carts(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_promotion_usage_promotion ON promotion_usage(promotion_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_customer ON promotion_usage(customer_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_order ON promotion_usage(order_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_cart ON promotion_usage(cart_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_date ON promotion_usage(used_at);

-- Seed some example promotions

-- 10% off sitewide
INSERT OR IGNORE INTO promotions (id, code, name, description, promotion_type, trigger, target, status, percentage_off, starts_at)
VALUES (
    lower(hex(randomblob(16))),
    'WELCOME10',
    'Welcome 10% Off',
    'Get 10% off your first order',
    'first_order_discount',
    'automatic',
    'order',
    'active',
    '0.10',
    datetime('now')
);

-- $20 off $100+ orders
INSERT OR IGNORE INTO promotions (id, code, name, description, promotion_type, trigger, target, status, fixed_amount_off, starts_at)
VALUES (
    lower(hex(randomblob(16))),
    'SAVE20-100',
    '$20 Off $100+',
    'Get $20 off when you spend $100 or more',
    'fixed_amount_off',
    'coupon_code',
    'order',
    'active',
    '20.00',
    datetime('now')
);

-- Add condition for $100 minimum
INSERT OR IGNORE INTO promotion_conditions (id, promotion_id, condition_type, operator, value, is_required)
SELECT
    lower(hex(randomblob(16))),
    id,
    'minimum_subtotal',
    'greater_than_or_equal',
    '100.00',
    1
FROM promotions WHERE code = 'SAVE20-100';

-- Create coupon code for the $20 off promotion
INSERT OR IGNORE INTO coupon_codes (id, promotion_id, code, status)
SELECT
    lower(hex(randomblob(16))),
    id,
    'SAVE20',
    'active'
FROM promotions WHERE code = 'SAVE20-100';

-- Free shipping on $50+
INSERT OR IGNORE INTO promotions (id, code, name, description, promotion_type, trigger, target, status, starts_at)
VALUES (
    lower(hex(randomblob(16))),
    'FREESHIP50',
    'Free Shipping on $50+',
    'Free shipping on orders over $50',
    'free_shipping',
    'automatic',
    'shipping',
    'active',
    datetime('now')
);

-- Add condition for $50 minimum
INSERT OR IGNORE INTO promotion_conditions (id, promotion_id, condition_type, operator, value, is_required)
SELECT
    lower(hex(randomblob(16))),
    id,
    'minimum_subtotal',
    'greater_than_or_equal',
    '50.00',
    1
FROM promotions WHERE code = 'FREESHIP50';

-- Tiered discount example
INSERT OR IGNORE INTO promotions (id, code, name, description, promotion_type, trigger, target, status, tiers, starts_at)
VALUES (
    lower(hex(randomblob(16))),
    'TIER-SAVE',
    'Spend More Save More',
    'The more you spend, the more you save!',
    'tiered_discount',
    'automatic',
    'order',
    'active',
    '[{"min_value":"50","percentage_off":"0.05"},{"min_value":"100","percentage_off":"0.10"},{"min_value":"200","percentage_off":"0.15"},{"min_value":"500","percentage_off":"0.20"}]',
    datetime('now')
);

-- Buy 2 Get 1 Free example
INSERT OR IGNORE INTO promotions (id, code, name, description, promotion_type, trigger, target, status, buy_quantity, get_quantity, get_discount_percent, starts_at)
VALUES (
    lower(hex(randomblob(16))),
    'B2G1FREE',
    'Buy 2 Get 1 Free',
    'Buy any 2 items, get the 3rd free',
    'buy_x_get_y',
    'automatic',
    'product',
    'draft',
    2,
    1,
    '1.0',
    datetime('now')
);
