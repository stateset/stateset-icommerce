-- Wishlists table
CREATE TABLE IF NOT EXISTS wishlists (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL,
    name TEXT NOT NULL DEFAULT 'My Wishlist',
    is_public BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Wishlist items (separate table for the items collection)
CREATE TABLE IF NOT EXISTS wishlist_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wishlist_id UUID NOT NULL REFERENCES wishlists(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    variant_id TEXT,
    priority INTEGER,
    notes TEXT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(wishlist_id, product_id)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_wishlists_customer_id ON wishlists (customer_id);
CREATE INDEX IF NOT EXISTS idx_wishlists_is_public ON wishlists (is_public);
CREATE INDEX IF NOT EXISTS idx_wishlists_created_at ON wishlists (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_wishlist_items_wishlist_id ON wishlist_items (wishlist_id);
CREATE INDEX IF NOT EXISTS idx_wishlist_items_product_id ON wishlist_items (product_id);
