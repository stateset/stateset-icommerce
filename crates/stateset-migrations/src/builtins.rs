//! Built-in migrations for the StateSet iCommerce schema.
//!
//! These migrations define the complete database schema evolution path.
//! They are organized into four major versions:
//!
//! - **V1**: Core tables (customers, products, orders, inventory, returns,
//!   manufacturing, shipments, payments, warranties, purchase orders, invoices)
//! - **V2**: Commerce extensions (carts, multi-currency, tax, promotions,
//!   subscriptions)
//! - **V3**: A2A commerce (x402 payment intents, agent cards, A2A quotes and
//!   purchases, x402 credits, ERC-8004 identity, custom objects)
//! - **V4**: New entity tables (fraud, gift cards, loyalty, reviews, segments,
//!   shipping zones, store credits, wishlists)

use crate::migration::Migration;
use crate::registry::MigrationRegistry;

/// Build the complete built-in migration registry for StateSet iCommerce.
///
/// # Errors
///
/// Returns an error if any migration version conflicts (should never happen
/// with the built-in set).
pub fn builtin_registry() -> crate::error::Result<MigrationRegistry> {
    MigrationRegistry::builder()
        .add(v1_core_tables())
        .add(v2_commerce_extensions())
        .add(v3_a2a_tables())
        .add(v4_new_entities())
        .build()
}

/// V1 — Core tables: customers, products, orders, inventory, returns,
/// manufacturing, shipments, payments, warranties, purchase orders, invoices.
#[must_use]
pub fn v1_core_tables() -> Migration {
    Migration::with_down(
        1,
        "core_tables",
        V1_UP,
        V1_DOWN,
    )
}

/// V2 — Commerce extensions: carts, multi-currency, tax, promotions,
/// subscriptions.
#[must_use]
pub fn v2_commerce_extensions() -> Migration {
    Migration::with_down(
        2,
        "commerce_extensions",
        V2_UP,
        V2_DOWN,
    )
}

/// V3 — A2A tables: x402 payment intents, agent cards, A2A quotes/purchases,
/// x402 credits, ERC-8004 identity, custom objects.
#[must_use]
pub fn v3_a2a_tables() -> Migration {
    Migration::with_down(
        3,
        "a2a_tables",
        V3_UP,
        V3_DOWN,
    )
}

/// V4 — New entity tables: fraud, gift cards, loyalty, reviews, segments,
/// shipping zones, store credits, wishlists.
#[must_use]
pub fn v4_new_entities() -> Migration {
    Migration::with_down(
        4,
        "new_entities",
        V4_UP,
        V4_DOWN,
    )
}

// ---------------------------------------------------------------------------
// V1 — Core Tables
// ---------------------------------------------------------------------------

const V1_UP: &str = r#"
-- ============================================================================
-- V1: Core Tables
-- ============================================================================

-- Customers
CREATE TABLE IF NOT EXISTS customers (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    phone TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    accepts_marketing INTEGER NOT NULL DEFAULT 0,
    email_verified INTEGER NOT NULL DEFAULT 0,
    tags TEXT NOT NULL DEFAULT '[]',
    metadata TEXT,
    default_shipping_address_id TEXT,
    default_billing_address_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_customers_email ON customers(email);
CREATE INDEX IF NOT EXISTS idx_customers_status ON customers(status);

-- Customer addresses
CREATE TABLE IF NOT EXISTS customer_addresses (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL REFERENCES customers(id),
    address_type TEXT NOT NULL DEFAULT 'both',
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    company TEXT,
    line1 TEXT NOT NULL,
    line2 TEXT,
    city TEXT NOT NULL,
    state TEXT,
    postal_code TEXT NOT NULL,
    country TEXT NOT NULL,
    phone TEXT,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_customer_addresses_customer ON customer_addresses(customer_id);

-- Products
CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft',
    product_type TEXT NOT NULL DEFAULT 'simple',
    attributes TEXT NOT NULL DEFAULT '[]',
    seo TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_products_slug ON products(slug);
CREATE INDEX IF NOT EXISTS idx_products_status ON products(status);

-- Product variants
CREATE TABLE IF NOT EXISTS product_variants (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL REFERENCES products(id),
    sku TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    price TEXT NOT NULL,
    compare_at_price TEXT,
    cost TEXT,
    barcode TEXT,
    weight TEXT,
    weight_unit TEXT,
    options TEXT NOT NULL DEFAULT '[]',
    is_default INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_product_variants_product ON product_variants(product_id);
CREATE INDEX IF NOT EXISTS idx_product_variants_sku ON product_variants(sku);

-- Orders
CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY,
    order_number TEXT NOT NULL UNIQUE,
    customer_id TEXT NOT NULL REFERENCES customers(id),
    status TEXT NOT NULL DEFAULT 'pending',
    order_date TEXT NOT NULL DEFAULT (datetime('now')),
    total_amount TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_status TEXT NOT NULL DEFAULT 'pending',
    fulfillment_status TEXT NOT NULL DEFAULT 'unfulfilled',
    payment_method TEXT,
    shipping_method TEXT,
    tracking_number TEXT,
    notes TEXT,
    shipping_address TEXT,
    billing_address TEXT,
    cart_id TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_orders_customer ON orders(customer_id);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
CREATE INDEX IF NOT EXISTS idx_orders_order_number ON orders(order_number);
CREATE INDEX IF NOT EXISTS idx_orders_order_date ON orders(order_date);

-- Order items
CREATE TABLE IF NOT EXISTS order_items (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders(id),
    product_id TEXT NOT NULL,
    variant_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_price TEXT NOT NULL,
    discount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_order_items_order ON order_items(order_id);

-- Events log (for sync)
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    synced_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_events_synced ON events(synced_at);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);

-- Inventory items
CREATE TABLE IF NOT EXISTS inventory_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sku TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    unit_of_measure TEXT NOT NULL DEFAULT 'EA',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_inventory_items_sku ON inventory_items(sku);

-- Inventory locations
CREATE TABLE IF NOT EXISTS inventory_locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    address TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT OR IGNORE INTO inventory_locations (id, name, code) VALUES (1, 'Default Warehouse', 'DEFAULT');

-- Inventory balances
CREATE TABLE IF NOT EXISTS inventory_balances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES inventory_items(id),
    location_id INTEGER NOT NULL REFERENCES inventory_locations(id) DEFAULT 1,
    quantity_on_hand TEXT NOT NULL DEFAULT '0',
    quantity_allocated TEXT NOT NULL DEFAULT '0',
    quantity_available TEXT NOT NULL DEFAULT '0',
    reorder_point TEXT,
    safety_stock TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    last_counted_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(item_id, location_id)
);
CREATE INDEX IF NOT EXISTS idx_inventory_balances_item ON inventory_balances(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_balances_location ON inventory_balances(location_id);

-- Inventory transactions
CREATE TABLE IF NOT EXISTS inventory_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES inventory_items(id),
    location_id INTEGER NOT NULL DEFAULT 1,
    transaction_type TEXT NOT NULL,
    quantity TEXT NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    reason TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_inventory_transactions_item ON inventory_transactions(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_transactions_type ON inventory_transactions(transaction_type);

-- Inventory reservations
CREATE TABLE IF NOT EXISTS inventory_reservations (
    id TEXT PRIMARY KEY,
    item_id INTEGER NOT NULL REFERENCES inventory_items(id),
    location_id INTEGER NOT NULL DEFAULT 1,
    quantity TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reference_type TEXT NOT NULL,
    reference_id TEXT NOT NULL,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_item ON inventory_reservations(item_id);
CREATE INDEX IF NOT EXISTS idx_inventory_reservations_status ON inventory_reservations(status);

-- Returns
CREATE TABLE IF NOT EXISTS returns (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders(id),
    customer_id TEXT NOT NULL REFERENCES customers(id),
    status TEXT NOT NULL DEFAULT 'requested',
    reason TEXT NOT NULL,
    reason_details TEXT,
    refund_amount TEXT,
    refund_method TEXT,
    tracking_number TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_returns_order ON returns(order_id);
CREATE INDEX IF NOT EXISTS idx_returns_customer ON returns(customer_id);
CREATE INDEX IF NOT EXISTS idx_returns_status ON returns(status);

-- Return items
CREATE TABLE IF NOT EXISTS return_items (
    id TEXT PRIMARY KEY,
    return_id TEXT NOT NULL REFERENCES returns(id),
    order_item_id TEXT NOT NULL REFERENCES order_items(id),
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    condition TEXT NOT NULL DEFAULT 'new',
    refund_amount TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_return_items_return ON return_items(return_id);

-- Payments
CREATE TABLE IF NOT EXISTS payments (
    id TEXT PRIMARY KEY,
    payment_number TEXT UNIQUE NOT NULL,
    order_id TEXT,
    invoice_id TEXT,
    customer_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    payment_method TEXT NOT NULL DEFAULT 'credit_card',
    amount TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    amount_refunded TEXT NOT NULL DEFAULT '0',
    external_id TEXT,
    processor TEXT,
    card_brand TEXT,
    card_last4 TEXT,
    card_exp_month INTEGER,
    card_exp_year INTEGER,
    billing_email TEXT,
    billing_name TEXT,
    billing_address TEXT,
    description TEXT,
    failure_reason TEXT,
    failure_code TEXT,
    metadata TEXT,
    paid_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_payments_order ON payments(order_id);
CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status);

-- Refunds
CREATE TABLE IF NOT EXISTS refunds (
    id TEXT PRIMARY KEY,
    refund_number TEXT UNIQUE NOT NULL,
    payment_id TEXT NOT NULL REFERENCES payments(id),
    status TEXT NOT NULL DEFAULT 'pending',
    amount TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    reason TEXT,
    external_id TEXT,
    failure_reason TEXT,
    notes TEXT,
    refunded_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_refunds_payment ON refunds(payment_id);
CREATE INDEX IF NOT EXISTS idx_refunds_status ON refunds(status);

-- Shipments
CREATE TABLE IF NOT EXISTS shipments (
    id TEXT PRIMARY KEY,
    shipment_number TEXT NOT NULL UNIQUE,
    order_id TEXT NOT NULL REFERENCES orders(id),
    status TEXT NOT NULL DEFAULT 'pending',
    carrier TEXT,
    service TEXT,
    tracking_number TEXT,
    tracking_url TEXT,
    ship_from_address TEXT,
    ship_to_address TEXT,
    weight TEXT,
    weight_unit TEXT DEFAULT 'lb',
    dimensions TEXT,
    estimated_delivery TEXT,
    actual_delivery TEXT,
    shipped_at TEXT,
    delivered_at TEXT,
    cost TEXT,
    insurance_amount TEXT,
    label_url TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shipments_order ON shipments(order_id);
CREATE INDEX IF NOT EXISTS idx_shipments_status ON shipments(status);
CREATE INDEX IF NOT EXISTS idx_shipments_tracking ON shipments(tracking_number);

-- Shipment items
CREATE TABLE IF NOT EXISTS shipment_items (
    id TEXT PRIMARY KEY,
    shipment_id TEXT NOT NULL REFERENCES shipments(id),
    order_item_id TEXT NOT NULL REFERENCES order_items(id),
    quantity INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shipment_items_shipment ON shipment_items(shipment_id);

-- Shipment events
CREATE TABLE IF NOT EXISTS shipment_events (
    id TEXT PRIMARY KEY,
    shipment_id TEXT NOT NULL REFERENCES shipments(id),
    status TEXT NOT NULL,
    description TEXT,
    location TEXT,
    occurred_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shipment_events_shipment ON shipment_events(shipment_id);

-- Warranties
CREATE TABLE IF NOT EXISTS warranties (
    id TEXT PRIMARY KEY,
    warranty_number TEXT UNIQUE NOT NULL,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    order_item_id TEXT,
    product_id TEXT,
    sku TEXT,
    serial_number TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    warranty_type TEXT NOT NULL DEFAULT 'standard',
    provider TEXT,
    coverage_description TEXT,
    purchase_date TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    duration_months INTEGER,
    max_coverage_amount TEXT,
    deductible TEXT,
    max_claims INTEGER,
    claims_used INTEGER NOT NULL DEFAULT 0,
    terms TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_warranties_customer ON warranties(customer_id);
CREATE INDEX IF NOT EXISTS idx_warranties_status ON warranties(status);

-- Warranty claims
CREATE TABLE IF NOT EXISTS warranty_claims (
    id TEXT PRIMARY KEY,
    claim_number TEXT UNIQUE NOT NULL,
    warranty_id TEXT NOT NULL REFERENCES warranties(id),
    customer_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'submitted',
    resolution TEXT NOT NULL DEFAULT 'none',
    issue_description TEXT NOT NULL,
    issue_category TEXT,
    issue_date TEXT,
    contact_phone TEXT,
    contact_email TEXT,
    shipping_address TEXT,
    repair_cost TEXT,
    replacement_product_id TEXT,
    refund_amount TEXT,
    denial_reason TEXT,
    internal_notes TEXT,
    customer_notes TEXT,
    submitted_at TEXT NOT NULL,
    approved_at TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_warranty ON warranty_claims(warranty_id);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_status ON warranty_claims(status);

-- Invoices
CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    invoice_number TEXT UNIQUE NOT NULL,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    invoice_type TEXT NOT NULL DEFAULT 'standard',
    invoice_date TEXT NOT NULL,
    due_date TEXT NOT NULL,
    payment_terms TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    subtotal TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL DEFAULT '0',
    amount_paid TEXT NOT NULL DEFAULT '0',
    balance_due TEXT NOT NULL DEFAULT '0',
    notes TEXT,
    sent_at TEXT,
    paid_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_invoices_customer ON invoices(customer_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);

-- Invoice items
CREATE TABLE IF NOT EXISTS invoice_items (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    order_item_id TEXT,
    product_id TEXT,
    sku TEXT,
    description TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_price TEXT NOT NULL,
    discount_amount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    line_total TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice ON invoice_items(invoice_id);
"#;

const V1_DOWN: &str = r#"
DROP TABLE IF EXISTS invoice_items;
DROP TABLE IF EXISTS invoices;
DROP TABLE IF EXISTS warranty_claims;
DROP TABLE IF EXISTS warranties;
DROP TABLE IF EXISTS shipment_events;
DROP TABLE IF EXISTS shipment_items;
DROP TABLE IF EXISTS shipments;
DROP TABLE IF EXISTS refunds;
DROP TABLE IF EXISTS payments;
DROP TABLE IF EXISTS return_items;
DROP TABLE IF EXISTS returns;
DROP TABLE IF EXISTS inventory_reservations;
DROP TABLE IF EXISTS inventory_transactions;
DROP TABLE IF EXISTS inventory_balances;
DROP TABLE IF EXISTS inventory_locations;
DROP TABLE IF EXISTS inventory_items;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS product_variants;
DROP TABLE IF EXISTS products;
DROP TABLE IF EXISTS customer_addresses;
DROP TABLE IF EXISTS customers;
"#;

// ---------------------------------------------------------------------------
// V2 — Commerce Extensions
// ---------------------------------------------------------------------------

const V2_UP: &str = r#"
-- ============================================================================
-- V2: Commerce Extensions
-- ============================================================================

-- Carts
CREATE TABLE IF NOT EXISTS carts (
    id TEXT PRIMARY KEY,
    customer_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    currency TEXT NOT NULL DEFAULT 'USD',
    subtotal TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL DEFAULT '0',
    coupon_code TEXT,
    shipping_address TEXT,
    billing_address TEXT,
    notes TEXT,
    abandoned_at TEXT,
    converted_at TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_carts_customer ON carts(customer_id);
CREATE INDEX IF NOT EXISTS idx_carts_status ON carts(status);

-- Cart items
CREATE TABLE IF NOT EXISTS cart_items (
    id TEXT PRIMARY KEY,
    cart_id TEXT NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price TEXT NOT NULL,
    discount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_cart_items_cart ON cart_items(cart_id);

-- Currency exchange rates
CREATE TABLE IF NOT EXISTS exchange_rates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    base_currency TEXT NOT NULL,
    target_currency TEXT NOT NULL,
    rate TEXT NOT NULL,
    source TEXT,
    effective_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(base_currency, target_currency, effective_date)
);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_pair ON exchange_rates(base_currency, target_currency);

-- Tax rules
CREATE TABLE IF NOT EXISTS tax_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    country TEXT NOT NULL,
    state TEXT,
    postal_code TEXT,
    tax_rate TEXT NOT NULL,
    tax_type TEXT NOT NULL DEFAULT 'sales_tax',
    product_type TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_tax_rules_country ON tax_rules(country);
CREATE INDEX IF NOT EXISTS idx_tax_rules_active ON tax_rules(is_active);

-- Promotions
CREATE TABLE IF NOT EXISTS promotions (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    discount_type TEXT NOT NULL DEFAULT 'percentage',
    discount_value TEXT NOT NULL,
    minimum_purchase TEXT,
    maximum_discount TEXT,
    usage_limit INTEGER,
    usage_count INTEGER NOT NULL DEFAULT 0,
    per_customer_limit INTEGER,
    starts_at TEXT NOT NULL,
    ends_at TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    applies_to TEXT NOT NULL DEFAULT '[]',
    excludes TEXT NOT NULL DEFAULT '[]',
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_promotions_code ON promotions(code);
CREATE INDEX IF NOT EXISTS idx_promotions_active ON promotions(is_active);

-- Subscription plans
CREATE TABLE IF NOT EXISTS subscription_plans (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    billing_interval TEXT NOT NULL DEFAULT 'monthly',
    custom_interval_days INTEGER,
    price TEXT NOT NULL,
    setup_fee TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    trial_days INTEGER NOT NULL DEFAULT 0,
    trial_requires_payment_method INTEGER NOT NULL DEFAULT 1,
    min_cycles INTEGER,
    max_cycles INTEGER,
    discount_percent TEXT,
    discount_amount TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_subscription_plans_code ON subscription_plans(code);
CREATE INDEX IF NOT EXISTS idx_subscription_plans_status ON subscription_plans(status);

-- Subscriptions
CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY,
    subscription_number TEXT NOT NULL UNIQUE,
    customer_id TEXT NOT NULL REFERENCES customers(id),
    plan_id TEXT NOT NULL REFERENCES subscription_plans(id),
    plan_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    billing_interval TEXT NOT NULL DEFAULT 'monthly',
    custom_interval_days INTEGER,
    price TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_method_id TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    current_period_start TEXT NOT NULL DEFAULT (datetime('now')),
    current_period_end TEXT NOT NULL,
    next_billing_date TEXT,
    trial_ends_at TEXT,
    cancelled_at TEXT,
    ends_at TEXT,
    paused_at TEXT,
    resume_at TEXT,
    billing_cycle_count INTEGER NOT NULL DEFAULT 0,
    failed_payment_attempts INTEGER NOT NULL DEFAULT 0,
    shipping_address TEXT,
    billing_address TEXT,
    discount_percent TEXT,
    discount_amount TEXT,
    coupon_code TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_subscriptions_customer ON subscriptions(customer_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_plan ON subscriptions(plan_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);

-- Subscription items
CREATE TABLE IF NOT EXISTS subscription_items (
    id TEXT PRIMARY KEY,
    subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price TEXT NOT NULL,
    line_total TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subscription_items_subscription ON subscription_items(subscription_id);
"#;

const V2_DOWN: &str = r#"
DROP TABLE IF EXISTS subscription_items;
DROP TABLE IF EXISTS subscriptions;
DROP TABLE IF EXISTS subscription_plans;
DROP TABLE IF EXISTS promotions;
DROP TABLE IF EXISTS tax_rules;
DROP TABLE IF EXISTS exchange_rates;
DROP TABLE IF EXISTS cart_items;
DROP TABLE IF EXISTS carts;
"#;

// ---------------------------------------------------------------------------
// V3 — A2A Commerce
// ---------------------------------------------------------------------------

const V3_UP: &str = r#"
-- ============================================================================
-- V3: A2A Commerce Tables
-- ============================================================================

-- x402 Payment Intents
CREATE TABLE IF NOT EXISTS x402_payment_intents (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL DEFAULT '1.0',
    status TEXT NOT NULL DEFAULT 'created',
    payer_address TEXT NOT NULL,
    payee_address TEXT NOT NULL,
    amount INTEGER NOT NULL,
    amount_decimal TEXT NOT NULL,
    asset TEXT NOT NULL DEFAULT 'usdc',
    network TEXT NOT NULL DEFAULT 'set_chain',
    chain_id INTEGER NOT NULL,
    token_address TEXT,
    created_at_unix INTEGER NOT NULL,
    valid_until INTEGER NOT NULL,
    nonce INTEGER NOT NULL,
    idempotency_key TEXT,
    resource_uri TEXT,
    resource_method TEXT,
    description TEXT,
    cart_id TEXT,
    order_id TEXT,
    invoice_id TEXT,
    merchant_id TEXT,
    signing_hash TEXT,
    payer_signature TEXT,
    payer_public_key TEXT,
    sequence_number INTEGER,
    sequenced_at TEXT,
    batch_id TEXT,
    batch_merkle_root TEXT,
    inclusion_proof TEXT,
    tx_hash TEXT,
    block_number INTEGER,
    gas_used INTEGER,
    settled_at TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_x402_intents_payer ON x402_payment_intents(payer_address);
CREATE INDEX IF NOT EXISTS idx_x402_intents_payee ON x402_payment_intents(payee_address);
CREATE INDEX IF NOT EXISTS idx_x402_intents_status ON x402_payment_intents(status);
CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_payer_nonce ON x402_payment_intents(payer_address, nonce);

-- Agent Cards
CREATE TABLE IF NOT EXISTS agent_cards (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    wallet_address TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,
    supported_networks TEXT NOT NULL,
    supported_assets TEXT NOT NULL,
    a2a_skills TEXT,
    trust_level TEXT NOT NULL DEFAULT 'standard',
    verified_at TEXT,
    verification_method TEXT,
    endpoint_url TEXT,
    endpoint_protocol TEXT DEFAULT 'https',
    merchant_id TEXT,
    merchant_name TEXT,
    business_category TEXT,
    max_transaction_amount INTEGER,
    daily_volume_limit INTEGER,
    requires_kyc INTEGER DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    suspended_at TEXT,
    suspension_reason TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_cards_wallet ON agent_cards(wallet_address);
CREATE INDEX IF NOT EXISTS idx_agent_cards_active ON agent_cards(active);

-- A2A Quotes
CREATE TABLE IF NOT EXISTS a2a_quotes (
    id TEXT PRIMARY KEY,
    quote_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending',
    buyer_agent_id TEXT NOT NULL,
    seller_agent_id TEXT NOT NULL,
    items TEXT NOT NULL,
    subtotal TEXT NOT NULL,
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_amount TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_network TEXT,
    payment_asset TEXT,
    shipping_address TEXT,
    valid_until TEXT NOT NULL,
    purchase_id TEXT,
    payment_intent_id TEXT REFERENCES x402_payment_intents(id) ON DELETE SET NULL,
    notes TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_buyer ON a2a_quotes(buyer_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_seller ON a2a_quotes(seller_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_status ON a2a_quotes(status);

-- A2A Purchases
CREATE TABLE IF NOT EXISTS a2a_purchases (
    id TEXT PRIMARY KEY,
    purchase_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'initiated',
    buyer_agent_id TEXT NOT NULL,
    seller_agent_id TEXT NOT NULL,
    quote_id TEXT REFERENCES a2a_quotes(id) ON DELETE SET NULL,
    cart_id TEXT,
    order_id TEXT,
    payment_intent_id TEXT REFERENCES x402_payment_intents(id) ON DELETE SET NULL,
    items TEXT NOT NULL,
    total TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    fulfillment_type TEXT,
    tracking_info TEXT,
    delivered_at TEXT,
    delivery_confirmed_at TEXT,
    delivery_confirmation_signature TEXT,
    buyer_rating INTEGER,
    buyer_feedback TEXT,
    seller_rating INTEGER,
    seller_feedback TEXT,
    notes TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_buyer ON a2a_purchases(buyer_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_seller ON a2a_purchases(seller_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_status ON a2a_purchases(status);

-- x402 Credit Accounts
CREATE TABLE IF NOT EXISTS x402_credit_accounts (
    id TEXT PRIMARY KEY,
    payer_address TEXT NOT NULL,
    asset TEXT NOT NULL DEFAULT 'usdc',
    network TEXT NOT NULL DEFAULT 'set_chain',
    balance INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(payer_address, asset, network)
);
CREATE INDEX IF NOT EXISTS idx_x402_credit_accounts_payer ON x402_credit_accounts(payer_address);

-- x402 Credit Transactions
CREATE TABLE IF NOT EXISTS x402_credit_transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES x402_credit_accounts(id) ON DELETE CASCADE,
    payer_address TEXT NOT NULL,
    asset TEXT NOT NULL,
    network TEXT NOT NULL,
    direction TEXT NOT NULL,
    amount INTEGER NOT NULL,
    balance_after INTEGER NOT NULL,
    reason TEXT,
    reference_id TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_x402_credit_tx_account ON x402_credit_transactions(account_id);

-- Agent Identities (ERC-8004)
CREATE TABLE IF NOT EXISTS agent_identities (
    id TEXT PRIMARY KEY,
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    agent_uri TEXT NOT NULL,
    agent_wallet TEXT,
    owner_address TEXT,
    agent_card_id TEXT REFERENCES agent_cards(id) ON DELETE SET NULL,
    registration TEXT,
    registration_hash TEXT,
    wallet_proof_type TEXT,
    wallet_proof TEXT,
    wallet_proof_chain_id INTEGER,
    wallet_proof_deadline TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(agent_registry, agent_id)
);
CREATE INDEX IF NOT EXISTS idx_agent_identities_registry ON agent_identities(agent_registry);
CREATE INDEX IF NOT EXISTS idx_agent_identities_wallet ON agent_identities(agent_wallet);

-- Custom Objects
CREATE TABLE IF NOT EXISTS custom_object_types (
    id TEXT PRIMARY KEY,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    fields_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS custom_object_records (
    id TEXT PRIMARY KEY,
    type_id TEXT NOT NULL REFERENCES custom_object_types(id) ON DELETE CASCADE,
    handle TEXT,
    owner_type TEXT,
    owner_id TEXT,
    values_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_custom_object_records_type_id ON custom_object_records(type_id);
"#;

const V3_DOWN: &str = r#"
DROP TABLE IF EXISTS custom_object_records;
DROP TABLE IF EXISTS custom_object_types;
DROP TABLE IF EXISTS agent_identities;
DROP TABLE IF EXISTS x402_credit_transactions;
DROP TABLE IF EXISTS x402_credit_accounts;
DROP TABLE IF EXISTS a2a_purchases;
DROP TABLE IF EXISTS a2a_quotes;
DROP TABLE IF EXISTS agent_cards;
DROP TABLE IF EXISTS x402_payment_intents;
"#;

// ---------------------------------------------------------------------------
// V4 — New Entity Tables
// ---------------------------------------------------------------------------

const V4_UP: &str = r#"
-- ============================================================================
-- V4: New Entity Tables
-- ============================================================================

-- Fraud assessments
CREATE TABLE IF NOT EXISTS fraud_assessments (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    risk_score INTEGER NOT NULL DEFAULT 0,
    decision TEXT NOT NULL DEFAULT 'pending',
    signals TEXT NOT NULL DEFAULT '[]',
    reviewer TEXT,
    review_notes TEXT,
    reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_order ON fraud_assessments(order_id);
CREATE INDEX IF NOT EXISTS idx_fraud_assessments_decision ON fraud_assessments(decision);

-- Fraud rules
CREATE TABLE IF NOT EXISTS fraud_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    rule_type TEXT NOT NULL,
    conditions TEXT NOT NULL DEFAULT '{}',
    action TEXT NOT NULL DEFAULT 'flag',
    priority INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_fraud_rules_active ON fraud_rules(is_active);

-- Gift cards
CREATE TABLE IF NOT EXISTS gift_cards (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    initial_balance TEXT NOT NULL,
    current_balance TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    customer_id TEXT,
    issued_by TEXT,
    expires_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_gift_cards_code ON gift_cards(code);
CREATE INDEX IF NOT EXISTS idx_gift_cards_status ON gift_cards(status);
CREATE INDEX IF NOT EXISTS idx_gift_cards_customer ON gift_cards(customer_id);

-- Gift card transactions
CREATE TABLE IF NOT EXISTS gift_card_transactions (
    id TEXT PRIMARY KEY,
    gift_card_id TEXT NOT NULL REFERENCES gift_cards(id),
    transaction_type TEXT NOT NULL,
    amount TEXT NOT NULL,
    balance_after TEXT NOT NULL,
    reference_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_gift_card_tx_card ON gift_card_transactions(gift_card_id);

-- Loyalty programs
CREATE TABLE IF NOT EXISTS loyalty_programs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    points_per_dollar TEXT NOT NULL DEFAULT '1',
    points_currency TEXT NOT NULL DEFAULT 'USD',
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_loyalty_programs_status ON loyalty_programs(status);

-- Loyalty accounts
CREATE TABLE IF NOT EXISTS loyalty_accounts (
    id TEXT PRIMARY KEY,
    program_id TEXT NOT NULL REFERENCES loyalty_programs(id),
    customer_id TEXT NOT NULL,
    points_balance INTEGER NOT NULL DEFAULT 0,
    lifetime_points INTEGER NOT NULL DEFAULT 0,
    tier TEXT NOT NULL DEFAULT 'bronze',
    status TEXT NOT NULL DEFAULT 'active',
    enrolled_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(program_id, customer_id)
);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_customer ON loyalty_accounts(customer_id);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_program ON loyalty_accounts(program_id);

-- Loyalty transactions
CREATE TABLE IF NOT EXISTS loyalty_transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES loyalty_accounts(id),
    transaction_type TEXT NOT NULL,
    points INTEGER NOT NULL,
    balance_after INTEGER NOT NULL,
    reference_type TEXT,
    reference_id TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_loyalty_tx_account ON loyalty_transactions(account_id);

-- Reviews
CREATE TABLE IF NOT EXISTS reviews (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    title TEXT,
    body TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    helpful_count INTEGER NOT NULL DEFAULT 0,
    reported INTEGER NOT NULL DEFAULT 0,
    verified_purchase INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_reviews_product ON reviews(product_id);
CREATE INDEX IF NOT EXISTS idx_reviews_customer ON reviews(customer_id);
CREATE INDEX IF NOT EXISTS idx_reviews_status ON reviews(status);

-- Search configs
CREATE TABLE IF NOT EXISTS search_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    index_name TEXT NOT NULL,
    searchable_fields TEXT NOT NULL DEFAULT '[]',
    filterable_fields TEXT NOT NULL DEFAULT '[]',
    sortable_fields TEXT NOT NULL DEFAULT '[]',
    ranking_rules TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_search_configs_active ON search_configs(is_active);

-- Segments
CREATE TABLE IF NOT EXISTS segments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    segment_type TEXT NOT NULL DEFAULT 'manual',
    conditions TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_segments_type ON segments(segment_type);
CREATE INDEX IF NOT EXISTS idx_segments_active ON segments(is_active);

-- Segment memberships
CREATE TABLE IF NOT EXISTS segment_memberships (
    id TEXT PRIMARY KEY,
    segment_id TEXT NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
    customer_id TEXT NOT NULL,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(segment_id, customer_id)
);
CREATE INDEX IF NOT EXISTS idx_segment_memberships_segment ON segment_memberships(segment_id);
CREATE INDEX IF NOT EXISTS idx_segment_memberships_customer ON segment_memberships(customer_id);

-- Shipping zones
CREATE TABLE IF NOT EXISTS shipping_zones (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    countries TEXT NOT NULL DEFAULT '[]',
    regions TEXT NOT NULL DEFAULT '[]',
    postal_codes TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shipping_zones_active ON shipping_zones(is_active);

-- Zone shipping methods
CREATE TABLE IF NOT EXISTS zone_shipping_methods (
    id TEXT PRIMARY KEY,
    zone_id TEXT NOT NULL REFERENCES shipping_zones(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    method_type TEXT NOT NULL DEFAULT 'flat_rate',
    price TEXT NOT NULL DEFAULT '0',
    min_order_amount TEXT,
    max_order_amount TEXT,
    min_weight TEXT,
    max_weight TEXT,
    estimated_days_min INTEGER,
    estimated_days_max INTEGER,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_zone ON zone_shipping_methods(zone_id);

-- Store credits
CREATE TABLE IF NOT EXISTS store_credits (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    balance TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    reason TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_store_credits_customer ON store_credits(customer_id);
CREATE INDEX IF NOT EXISTS idx_store_credits_status ON store_credits(status);

-- Store credit transactions
CREATE TABLE IF NOT EXISTS store_credit_transactions (
    id TEXT PRIMARY KEY,
    store_credit_id TEXT NOT NULL REFERENCES store_credits(id),
    transaction_type TEXT NOT NULL,
    amount TEXT NOT NULL,
    balance_after TEXT NOT NULL,
    reference_id TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_store_credit_tx_credit ON store_credit_transactions(store_credit_id);

-- Wishlists
CREATE TABLE IF NOT EXISTS wishlists (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT 'My Wishlist',
    is_public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_wishlists_customer ON wishlists(customer_id);

-- Wishlist items
CREATE TABLE IF NOT EXISTS wishlist_items (
    id TEXT PRIMARY KEY,
    wishlist_id TEXT NOT NULL REFERENCES wishlists(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    notes TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(wishlist_id, product_id)
);
CREATE INDEX IF NOT EXISTS idx_wishlist_items_wishlist ON wishlist_items(wishlist_id);
CREATE INDEX IF NOT EXISTS idx_wishlist_items_product ON wishlist_items(product_id);
"#;

const V4_DOWN: &str = r#"
DROP TABLE IF EXISTS wishlist_items;
DROP TABLE IF EXISTS wishlists;
DROP TABLE IF EXISTS store_credit_transactions;
DROP TABLE IF EXISTS store_credits;
DROP TABLE IF EXISTS zone_shipping_methods;
DROP TABLE IF EXISTS shipping_zones;
DROP TABLE IF EXISTS segment_memberships;
DROP TABLE IF EXISTS segments;
DROP TABLE IF EXISTS search_configs;
DROP TABLE IF EXISTS reviews;
DROP TABLE IF EXISTS loyalty_transactions;
DROP TABLE IF EXISTS loyalty_accounts;
DROP TABLE IF EXISTS loyalty_programs;
DROP TABLE IF EXISTS gift_card_transactions;
DROP TABLE IF EXISTS gift_cards;
DROP TABLE IF EXISTS fraud_rules;
DROP TABLE IF EXISTS fraud_assessments;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_builds_successfully() {
        let reg = builtin_registry().unwrap();
        assert_eq!(reg.len(), 4);
    }

    #[test]
    fn builtin_versions_are_sequential() {
        let reg = builtin_registry().unwrap();
        let versions: Vec<u32> = reg.list().iter().map(|m| m.version).collect();
        assert_eq!(versions, vec![1, 2, 3, 4]);
    }

    #[test]
    fn all_builtins_have_down_sql() {
        let reg = builtin_registry().unwrap();
        for m in reg.list() {
            assert!(m.has_down(), "migration v{} '{}' should have down SQL", m.version, m.name);
        }
    }

    #[test]
    fn v1_has_expected_tables() {
        let m = v1_core_tables();
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS customers"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS products"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS orders"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS order_items"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS inventory_items"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS returns"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS payments"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS shipments"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS warranties"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS invoices"));
    }

    #[test]
    fn v2_has_expected_tables() {
        let m = v2_commerce_extensions();
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS carts"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS cart_items"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS subscriptions"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS subscription_plans"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS promotions"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS tax_rules"));
    }

    #[test]
    fn v3_has_expected_tables() {
        let m = v3_a2a_tables();
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS x402_payment_intents"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS agent_cards"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS a2a_quotes"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS a2a_purchases"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS agent_identities"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS custom_object_types"));
    }

    #[test]
    fn v4_has_expected_tables() {
        let m = v4_new_entities();
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS fraud_assessments"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS gift_cards"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS loyalty_programs"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS reviews"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS segments"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS shipping_zones"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS store_credits"));
        assert!(m.up_sql.contains("CREATE TABLE IF NOT EXISTS wishlists"));
    }

    #[test]
    fn checksums_are_stable() {
        let m1 = v1_core_tables();
        let m1b = v1_core_tables();
        assert_eq!(m1.checksum, m1b.checksum);
    }

    #[test]
    fn all_down_sqls_have_drop_statements() {
        let reg = builtin_registry().unwrap();
        for m in reg.list() {
            let down = m.down_sql.as_ref().unwrap();
            assert!(
                down.contains("DROP TABLE"),
                "v{} down SQL should contain DROP TABLE",
                m.version
            );
        }
    }

    #[test]
    fn v1_applies_to_sqlite_memory() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(&v1_core_tables().up_sql).unwrap();
    }

    #[test]
    fn v1_then_v2_applies_cleanly() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(&v1_core_tables().up_sql).unwrap();
        conn.execute_batch(&v2_commerce_extensions().up_sql).unwrap();
    }

    #[test]
    fn all_versions_apply_cleanly() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let reg = builtin_registry().unwrap();
        for m in reg.list() {
            conn.execute_batch(&m.up_sql)
                .unwrap_or_else(|e| panic!("v{} '{}' failed: {e}", m.version, m.name));
        }
    }

    #[test]
    fn idempotent_rerun_v1() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(&v1_core_tables().up_sql).unwrap();
        // Run again — IF NOT EXISTS should make this idempotent
        conn.execute_batch(&v1_core_tables().up_sql).unwrap();
    }

    #[test]
    fn idempotent_rerun_all() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let reg = builtin_registry().unwrap();
        for m in reg.list() {
            conn.execute_batch(&m.up_sql).unwrap();
        }
        // Run all again
        for m in reg.list() {
            conn.execute_batch(&m.up_sql).unwrap();
        }
    }

    #[test]
    fn rollback_v4_then_reapply() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let reg = builtin_registry().unwrap();
        for m in reg.list() {
            conn.execute_batch(&m.up_sql).unwrap();
        }
        // Rollback V4
        let v4 = reg.get(4).unwrap();
        conn.execute_batch(v4.down_sql.as_ref().unwrap()).unwrap();
        // Re-apply V4
        conn.execute_batch(&v4.up_sql).unwrap();
    }

    #[test]
    fn rollback_all_versions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let reg = builtin_registry().unwrap();

        // Apply all
        for m in reg.list() {
            conn.execute_batch(&m.up_sql).unwrap();
        }

        // Rollback in reverse order
        for m in reg.list().into_iter().rev() {
            conn.execute_batch(m.down_sql.as_ref().unwrap())
                .unwrap_or_else(|e| panic!("rollback v{} failed: {e}", m.version));
        }
    }
}
