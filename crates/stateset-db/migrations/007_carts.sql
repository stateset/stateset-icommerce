-- Cart/Checkout Session tables
-- Based on the Agentic Commerce Protocol (ACP) checkout system

-- Main carts table
CREATE TABLE IF NOT EXISTS carts (
    id TEXT PRIMARY KEY,
    cart_number TEXT NOT NULL UNIQUE,
    customer_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Totals
    subtotal TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_amount TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    grand_total TEXT NOT NULL DEFAULT '0',

    -- Customer info (for guest checkout)
    customer_email TEXT,
    customer_phone TEXT,
    customer_name TEXT,

    -- Addresses (stored as JSON)
    shipping_address TEXT,
    billing_address TEXT,
    billing_same_as_shipping INTEGER NOT NULL DEFAULT 1,

    -- Fulfillment
    fulfillment_type TEXT,
    shipping_method TEXT,
    shipping_carrier TEXT,
    estimated_delivery TEXT,

    -- Payment
    payment_method TEXT,
    payment_token TEXT,
    payment_status TEXT NOT NULL DEFAULT 'none',

    -- Discount/Promo
    coupon_code TEXT,
    discount_description TEXT,

    -- Order reference (after checkout completes)
    order_id TEXT,
    order_number TEXT,

    -- Metadata
    notes TEXT,
    metadata TEXT,

    -- Inventory reservations
    inventory_reserved INTEGER NOT NULL DEFAULT 0,
    reservation_expires_at TEXT,

    -- Timestamps
    expires_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Cart items table
CREATE TABLE IF NOT EXISTS cart_items (
    id TEXT PRIMARY KEY,
    cart_id TEXT NOT NULL,
    product_id TEXT,
    variant_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    image_url TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price TEXT NOT NULL DEFAULT '0',
    original_price TEXT,
    discount_amount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL DEFAULT '0',
    weight TEXT,
    requires_shipping INTEGER NOT NULL DEFAULT 1,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (cart_id) REFERENCES carts(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_carts_customer_id ON carts(customer_id);
CREATE INDEX IF NOT EXISTS idx_carts_customer_email ON carts(customer_email);
CREATE INDEX IF NOT EXISTS idx_carts_status ON carts(status);
CREATE INDEX IF NOT EXISTS idx_carts_created_at ON carts(created_at);
CREATE INDEX IF NOT EXISTS idx_carts_expires_at ON carts(expires_at);
CREATE INDEX IF NOT EXISTS idx_cart_items_cart_id ON cart_items(cart_id);
CREATE INDEX IF NOT EXISTS idx_cart_items_sku ON cart_items(sku);
