-- Promotions and discounts schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS promotions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    internal_notes TEXT,
    promotion_type TEXT NOT NULL DEFAULT 'percentage_off',
    trigger TEXT NOT NULL DEFAULT 'automatic',
    target TEXT NOT NULL DEFAULT 'order',
    stacking TEXT NOT NULL DEFAULT 'stackable',
    status TEXT NOT NULL DEFAULT 'draft',
    percentage_off NUMERIC(12, 6),
    fixed_amount_off NUMERIC(12, 4),
    max_discount_amount NUMERIC(12, 4),
    buy_quantity INTEGER,
    get_quantity INTEGER,
    get_discount_percent NUMERIC(12, 6),
    tiers JSONB,
    bundle_product_ids JSONB,
    bundle_discount NUMERIC(12, 4),
    starts_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ends_at TIMESTAMPTZ,
    total_usage_limit INTEGER,
    per_customer_limit INTEGER,
    usage_count INTEGER NOT NULL DEFAULT 0,
    applicable_product_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    applicable_category_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    applicable_skus JSONB NOT NULL DEFAULT '[]'::jsonb,
    excluded_product_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    excluded_category_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    eligible_customer_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    eligible_customer_groups JSONB NOT NULL DEFAULT '[]'::jsonb,
    currency TEXT NOT NULL DEFAULT 'USD',
    priority INTEGER NOT NULL DEFAULT 0,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_promotions_code ON promotions(code);
CREATE INDEX IF NOT EXISTS idx_promotions_status ON promotions(status);
CREATE INDEX IF NOT EXISTS idx_promotions_type ON promotions(promotion_type);
CREATE INDEX IF NOT EXISTS idx_promotions_trigger ON promotions(trigger);
CREATE INDEX IF NOT EXISTS idx_promotions_dates ON promotions(starts_at, ends_at);
CREATE INDEX IF NOT EXISTS idx_promotions_priority ON promotions(priority);

CREATE TABLE IF NOT EXISTS promotion_conditions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    promotion_id UUID NOT NULL REFERENCES promotions(id) ON DELETE CASCADE,
    condition_type TEXT NOT NULL,
    operator TEXT NOT NULL DEFAULT 'equals',
    value TEXT NOT NULL,
    is_required BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_promotion_conditions_promotion ON promotion_conditions(promotion_id);
CREATE INDEX IF NOT EXISTS idx_promotion_conditions_type ON promotion_conditions(condition_type);

CREATE TABLE IF NOT EXISTS coupon_codes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    promotion_id UUID NOT NULL REFERENCES promotions(id) ON DELETE CASCADE,
    code TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'active',
    usage_limit INTEGER,
    per_customer_limit INTEGER,
    usage_count INTEGER NOT NULL DEFAULT 0,
    starts_at TIMESTAMPTZ,
    ends_at TIMESTAMPTZ,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_coupon_codes_code ON coupon_codes(code);
CREATE INDEX IF NOT EXISTS idx_coupon_codes_promotion ON coupon_codes(promotion_id);
CREATE INDEX IF NOT EXISTS idx_coupon_codes_status ON coupon_codes(status);

CREATE TABLE IF NOT EXISTS promotion_usage (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    promotion_id UUID NOT NULL REFERENCES promotions(id) ON DELETE CASCADE,
    coupon_id UUID REFERENCES coupon_codes(id) ON DELETE SET NULL,
    customer_id UUID REFERENCES customers(id) ON DELETE SET NULL,
    order_id UUID REFERENCES orders(id) ON DELETE SET NULL,
    cart_id UUID REFERENCES carts(id) ON DELETE SET NULL,
    discount_amount NUMERIC(12, 4) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    used_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_promotion_usage_promotion ON promotion_usage(promotion_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_customer ON promotion_usage(customer_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_order ON promotion_usage(order_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_cart ON promotion_usage(cart_id);
CREATE INDEX IF NOT EXISTS idx_promotion_usage_date ON promotion_usage(used_at);
