-- Subscriptions schema
-- Comprehensive subscription management with plans, billing cycles, and lifecycle events

-- Subscription plans (templates)
CREATE TABLE IF NOT EXISTS subscription_plans (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,  -- Unique plan code (e.g., "MONTHLY-COFFEE")
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'draft',  -- draft, active, archived

    -- Billing configuration
    billing_interval TEXT NOT NULL DEFAULT 'monthly',  -- weekly, biweekly, monthly, bimonthly, quarterly, semiannual, annual, custom
    custom_interval_days INTEGER,  -- Used when billing_interval is 'custom'
    price TEXT NOT NULL,  -- Decimal as string
    setup_fee TEXT,  -- One-time setup/activation fee
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Trial configuration
    trial_days INTEGER NOT NULL DEFAULT 0,
    trial_requires_payment_method INTEGER NOT NULL DEFAULT 1,  -- Boolean

    -- Limits
    min_cycles INTEGER,  -- Minimum billing cycles required
    max_cycles INTEGER,  -- Maximum billing cycles (NULL = unlimited)

    -- Discounts
    discount_percent TEXT,  -- Decimal as string (e.g., "0.10" for 10%)
    discount_amount TEXT,  -- Fixed discount amount

    -- Metadata
    metadata TEXT,  -- JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_subscription_plans_code ON subscription_plans(code);
CREATE INDEX IF NOT EXISTS idx_subscription_plans_status ON subscription_plans(status);
CREATE INDEX IF NOT EXISTS idx_subscription_plans_interval ON subscription_plans(billing_interval);

-- Subscription plan items (products included in plan)
CREATE TABLE IF NOT EXISTS subscription_plan_items (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    min_quantity INTEGER,
    max_quantity INTEGER,
    is_required INTEGER NOT NULL DEFAULT 1,  -- Boolean
    unit_price TEXT,  -- Override price (NULL = use plan price)

    FOREIGN KEY (plan_id) REFERENCES subscription_plans(id) ON DELETE CASCADE,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subscription_plan_items_plan ON subscription_plan_items(plan_id);
CREATE INDEX IF NOT EXISTS idx_subscription_plan_items_product ON subscription_plan_items(product_id);

-- Customer subscriptions
CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY,
    subscription_number TEXT NOT NULL UNIQUE,  -- Human-readable (e.g., "SUB-123456")
    customer_id TEXT NOT NULL,
    plan_id TEXT NOT NULL,
    plan_name TEXT NOT NULL,  -- Snapshot of plan name at subscription time
    status TEXT NOT NULL DEFAULT 'pending',  -- trial, active, paused, past_due, cancelled, expired, pending

    -- Billing
    billing_interval TEXT NOT NULL DEFAULT 'monthly',
    custom_interval_days INTEGER,
    price TEXT NOT NULL,  -- Current price per cycle
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_method_id TEXT,  -- Payment provider's payment method ID

    -- Dates
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    current_period_start TEXT NOT NULL DEFAULT (datetime('now')),
    current_period_end TEXT NOT NULL,
    next_billing_date TEXT,
    trial_ends_at TEXT,
    cancelled_at TEXT,
    ends_at TEXT,
    paused_at TEXT,
    resume_at TEXT,

    -- Cycle tracking
    billing_cycle_count INTEGER NOT NULL DEFAULT 0,
    failed_payment_attempts INTEGER NOT NULL DEFAULT 0,

    -- Addresses (JSON)
    shipping_address TEXT,
    billing_address TEXT,

    -- Discounts
    discount_percent TEXT,
    discount_amount TEXT,
    coupon_code TEXT,

    -- Metadata
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE,
    FOREIGN KEY (plan_id) REFERENCES subscription_plans(id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_number ON subscriptions(subscription_number);
CREATE INDEX IF NOT EXISTS idx_subscriptions_customer ON subscriptions(customer_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_plan ON subscriptions(plan_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);
CREATE INDEX IF NOT EXISTS idx_subscriptions_next_billing ON subscriptions(next_billing_date);
CREATE INDEX IF NOT EXISTS idx_subscriptions_trial_ends ON subscriptions(trial_ends_at);

-- Subscription items (line items in a subscription)
CREATE TABLE IF NOT EXISTS subscription_items (
    id TEXT PRIMARY KEY,
    subscription_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price TEXT NOT NULL,  -- Decimal as string
    line_total TEXT NOT NULL,  -- Decimal as string (quantity * unit_price)

    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subscription_items_subscription ON subscription_items(subscription_id);
CREATE INDEX IF NOT EXISTS idx_subscription_items_product ON subscription_items(product_id);

-- Billing cycles (each billing period)
CREATE TABLE IF NOT EXISTS billing_cycles (
    id TEXT PRIMARY KEY,
    subscription_id TEXT NOT NULL,
    cycle_number INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled',  -- scheduled, processing, paid, failed, skipped, refunded, voided

    -- Period
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    billed_at TEXT,

    -- Amounts
    subtotal TEXT NOT NULL DEFAULT '0',
    discount TEXT NOT NULL DEFAULT '0',
    tax TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Payment tracking
    payment_id TEXT,  -- Payment provider's payment ID
    order_id TEXT,  -- Order created from this cycle
    invoice_id TEXT,  -- Invoice created for this cycle

    -- Failure tracking
    failure_reason TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,

    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE SET NULL,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_billing_cycles_subscription ON billing_cycles(subscription_id);
CREATE INDEX IF NOT EXISTS idx_billing_cycles_status ON billing_cycles(status);
CREATE INDEX IF NOT EXISTS idx_billing_cycles_period ON billing_cycles(period_start, period_end);
CREATE INDEX IF NOT EXISTS idx_billing_cycles_order ON billing_cycles(order_id);

-- Subscription events (audit log)
CREATE TABLE IF NOT EXISTS subscription_events (
    id TEXT PRIMARY KEY,
    subscription_id TEXT NOT NULL,
    event_type TEXT NOT NULL,  -- created, activated, trial_started, trial_ended, renewed, payment_failed, payment_retry_succeeded, paused, resumed, skipped, cancelled, expired, plan_changed, items_modified, quantity_changed, address_updated, payment_method_updated, discount_applied, discount_removed, refunded
    description TEXT NOT NULL,
    data TEXT,  -- JSON event data
    triggered_by TEXT,  -- customer_id, "system", "admin"
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subscription_events_subscription ON subscription_events(subscription_id);
CREATE INDEX IF NOT EXISTS idx_subscription_events_type ON subscription_events(event_type);
CREATE INDEX IF NOT EXISTS idx_subscription_events_date ON subscription_events(created_at);

-- Seed some example subscription plans

-- Monthly Coffee Subscription
INSERT OR IGNORE INTO subscription_plans (id, code, name, description, status, billing_interval, price, trial_days, currency)
VALUES (
    lower(hex(randomblob(16))),
    'MONTHLY-COFFEE',
    'Monthly Coffee Box',
    'Get fresh roasted coffee beans delivered monthly',
    'active',
    'monthly',
    '29.99',
    14,
    'USD'
);

-- Weekly Meal Kit
INSERT OR IGNORE INTO subscription_plans (id, code, name, description, status, billing_interval, price, trial_days, currency)
VALUES (
    lower(hex(randomblob(16))),
    'WEEKLY-MEALS',
    'Weekly Meal Kit',
    'Fresh ingredients and recipes delivered weekly',
    'active',
    'weekly',
    '79.99',
    7,
    'USD'
);

-- Annual Software License
INSERT OR IGNORE INTO subscription_plans (id, code, name, description, status, billing_interval, price, discount_percent, currency)
VALUES (
    lower(hex(randomblob(16))),
    'ANNUAL-PRO',
    'Pro Annual Plan',
    'Full access to all features, billed annually',
    'active',
    'annual',
    '199.00',
    '0.20',  -- 20% discount for annual
    'USD'
);

-- Quarterly Wellness Box
INSERT OR IGNORE INTO subscription_plans (id, code, name, description, status, billing_interval, price, setup_fee, currency)
VALUES (
    lower(hex(randomblob(16))),
    'QUARTERLY-WELLNESS',
    'Quarterly Wellness Box',
    'Curated wellness products delivered quarterly',
    'active',
    'quarterly',
    '89.99',
    '9.99',  -- One-time setup fee
    'USD'
);
