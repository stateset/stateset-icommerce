-- Commerce entity tables for the live SQLite engine (shipping zones, gift cards,
-- reviews, segments, store credits, wishlists, loyalty).
--
-- These entities have repository implementations in
-- `crates/stateset-db/src/sqlite/<entity>.rs` and mounted REST endpoints in
-- `crates/stateset-http/src/routes/<entity>.rs`, but their tables were only ever
-- created inside `#[cfg(test)]` blocks (and in the PostgreSQL backend). The live
-- SQLite engine never provisioned them, so the mounted endpoints returned
-- HTTP 500 `no such table` at runtime.
--
-- Column names, types, and constraints below mirror exactly what each SQLite
-- repository impl inserts and selects (money stored as TEXT per codebase
-- convention; booleans as INTEGER 0/1; timestamps as RFC3339 TEXT).

-- ============================================================================
-- Shipping zones (crates/stateset-db/src/sqlite/shipping_zones.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS shipping_zones (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    countries TEXT NOT NULL DEFAULT '[]',
    regions TEXT NOT NULL DEFAULT '[]',
    postal_codes TEXT NOT NULL DEFAULT '[]',
    priority INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shipping_zones_active ON shipping_zones(is_active);
CREATE INDEX IF NOT EXISTS idx_shipping_zones_priority ON shipping_zones(priority);

-- ============================================================================
-- Gift cards (crates/stateset-db/src/sqlite/gift_cards.rs)
-- ============================================================================
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

CREATE TABLE IF NOT EXISTS gift_card_transactions (
    id TEXT PRIMARY KEY,
    gift_card_id TEXT NOT NULL REFERENCES gift_cards(id),
    amount TEXT NOT NULL,
    balance_after TEXT NOT NULL,
    type TEXT NOT NULL,
    reference_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_gift_card_tx_card ON gift_card_transactions(gift_card_id);

-- ============================================================================
-- Reviews (crates/stateset-db/src/sqlite/reviews.rs)
-- ============================================================================
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

-- ============================================================================
-- Segments (crates/stateset-db/src/sqlite/segments.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS segments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    segment_type TEXT NOT NULL DEFAULT 'static',
    rules TEXT NOT NULL DEFAULT '[]',
    member_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_segments_type ON segments(segment_type);

CREATE TABLE IF NOT EXISTS segment_memberships (
    segment_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (segment_id, customer_id),
    FOREIGN KEY (segment_id) REFERENCES segments(id)
);
CREATE INDEX IF NOT EXISTS idx_segment_memberships_customer ON segment_memberships(customer_id);

-- ============================================================================
-- Store credits (crates/stateset-db/src/sqlite/store_credits.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS store_credits (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    original_balance TEXT NOT NULL,
    current_balance TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'active',
    reason TEXT NOT NULL DEFAULT 'return',
    reference_id TEXT,
    note TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_store_credits_customer ON store_credits(customer_id);
CREATE INDEX IF NOT EXISTS idx_store_credits_status ON store_credits(status);

CREATE TABLE IF NOT EXISTS store_credit_transactions (
    id TEXT PRIMARY KEY,
    store_credit_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    balance_after TEXT NOT NULL,
    transaction_type TEXT NOT NULL,
    reference_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (store_credit_id) REFERENCES store_credits(id)
);
CREATE INDEX IF NOT EXISTS idx_store_credit_tx_credit ON store_credit_transactions(store_credit_id);

-- ============================================================================
-- Wishlists (crates/stateset-db/src/sqlite/wishlists.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS wishlists (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT 'My Wishlist',
    is_public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_wishlists_customer ON wishlists(customer_id);

CREATE TABLE IF NOT EXISTS wishlist_items (
    id TEXT PRIMARY KEY,
    wishlist_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    notes TEXT,
    UNIQUE(wishlist_id, product_id)
);
CREATE INDEX IF NOT EXISTS idx_wishlist_items_wishlist ON wishlist_items(wishlist_id);

-- ============================================================================
-- Loyalty (crates/stateset-db/src/sqlite/loyalty.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS loyalty_programs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    points_per_dollar INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_loyalty_programs_status ON loyalty_programs(status);

CREATE TABLE IF NOT EXISTS loyalty_accounts (
    id TEXT PRIMARY KEY,
    program_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    points_balance INTEGER NOT NULL DEFAULT 0,
    lifetime_points INTEGER NOT NULL DEFAULT 0,
    tier TEXT NOT NULL DEFAULT 'bronze',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_customer ON loyalty_accounts(customer_id);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_program ON loyalty_accounts(program_id);

CREATE TABLE IF NOT EXISTS loyalty_transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    points INTEGER NOT NULL,
    type TEXT NOT NULL,
    reference_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_loyalty_tx_account ON loyalty_transactions(account_id);
