-- Carts/Checkout migration for PostgreSQL
-- Full checkout flow with items, totals, addresses, and fulfillment

-- Main carts table
CREATE TABLE IF NOT EXISTS carts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_number VARCHAR(50) UNIQUE NOT NULL,
    customer_id UUID REFERENCES customers(id),
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',

    -- Totals
    subtotal DECIMAL(12, 2) NOT NULL DEFAULT 0,
    tax_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    shipping_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    discount_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    grand_total DECIMAL(12, 2) NOT NULL DEFAULT 0,

    -- Customer info (for guest checkout)
    customer_email VARCHAR(255),
    customer_phone VARCHAR(50),
    customer_name VARCHAR(255),

    -- Addresses (stored as JSON)
    shipping_address JSONB,
    billing_address JSONB,
    billing_same_as_shipping BOOLEAN NOT NULL DEFAULT true,

    -- Fulfillment
    fulfillment_type VARCHAR(50),
    shipping_method VARCHAR(100),
    shipping_carrier VARCHAR(100),
    estimated_delivery TIMESTAMPTZ,

    -- Payment
    payment_method VARCHAR(100),
    payment_token TEXT,
    payment_status VARCHAR(50) NOT NULL DEFAULT 'none',

    -- Discount/Promo
    coupon_code VARCHAR(100),
    discount_description TEXT,

    -- Order reference (after checkout completes)
    order_id UUID REFERENCES orders(id),
    order_number VARCHAR(50),

    -- Metadata
    notes TEXT,
    metadata JSONB,

    -- Inventory reservations
    inventory_reserved BOOLEAN NOT NULL DEFAULT false,
    reservation_expires_at TIMESTAMPTZ,

    -- Timestamps
    expires_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Cart items table
CREATE TABLE IF NOT EXISTS cart_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id UUID NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    product_id UUID REFERENCES products(id),
    variant_id UUID,
    sku VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    image_url TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price DECIMAL(12, 4) NOT NULL,
    original_price DECIMAL(12, 4),
    discount_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    tax_amount DECIMAL(12, 2) NOT NULL DEFAULT 0,
    total DECIMAL(12, 2) NOT NULL,
    weight DECIMAL(12, 4),
    requires_shipping BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_carts_customer_id ON carts(customer_id);
CREATE INDEX IF NOT EXISTS idx_carts_customer_email ON carts(customer_email);
CREATE INDEX IF NOT EXISTS idx_carts_status ON carts(status);
CREATE INDEX IF NOT EXISTS idx_carts_cart_number ON carts(cart_number);
CREATE INDEX IF NOT EXISTS idx_carts_order_id ON carts(order_id);
CREATE INDEX IF NOT EXISTS idx_carts_expires_at ON carts(expires_at);
CREATE INDEX IF NOT EXISTS idx_cart_items_cart_id ON cart_items(cart_id);
CREATE INDEX IF NOT EXISTS idx_cart_items_product_id ON cart_items(product_id);
