-- B2B / ERP-ops entity tables for the live SQLite engine.
--
-- Adds channels (+ SKU mappings), companies (+ shipping addresses, contacts,
-- price overrides), transfer orders (+ items), units of measure (classes,
-- UOMs, conversion rules), and production batches. Each has a repository
-- implementation in `crates/stateset-db/src/sqlite/<entity>.rs` and a mounted
-- REST endpoint in `crates/stateset-http/src/routes/<entity>.rs`.
--
-- Conventions mirror the rest of the codebase: money/decimals stored as TEXT,
-- booleans as INTEGER 0/1, timestamps as RFC3339 TEXT, JSON arrays as TEXT.

-- ============================================================================
-- Channels (crates/stateset-db/src/sqlite/channels.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    channel_type TEXT NOT NULL DEFAULT 'sales_channel',
    integration TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    api_locked INTEGER NOT NULL DEFAULT 0,
    default_warehouse_id TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    metadata TEXT NOT NULL DEFAULT 'null',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_channels_type ON channels(channel_type);
CREATE INDEX IF NOT EXISTS idx_channels_status ON channels(status);

CREATE TABLE IF NOT EXISTS channel_product_mappings (
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    channel_sku TEXT NOT NULL,
    product_id TEXT NOT NULL,
    internal_sku TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (channel_id, channel_sku)
);
CREATE INDEX IF NOT EXISTS idx_channel_mappings_product ON channel_product_mappings(product_id);

-- ============================================================================
-- Companies (crates/stateset-db/src/sqlite/companies.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS companies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    reference TEXT,
    email TEXT,
    phone TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_terms_days INTEGER,
    status TEXT NOT NULL DEFAULT 'active',
    tags TEXT NOT NULL DEFAULT '[]',
    metadata TEXT NOT NULL DEFAULT 'null',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_companies_status ON companies(status);
CREATE INDEX IF NOT EXISTS idx_companies_name ON companies(name);

CREATE TABLE IF NOT EXISTS company_shipping_addresses (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    label TEXT,
    name TEXT,
    line1 TEXT NOT NULL,
    line2 TEXT,
    city TEXT NOT NULL,
    region TEXT,
    postal_code TEXT,
    country TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_company_addresses_company ON company_shipping_addresses(company_id);

CREATE TABLE IF NOT EXISTS contacts (
    id TEXT PRIMARY KEY,
    first_name TEXT NOT NULL,
    last_name TEXT,
    email TEXT,
    phone TEXT,
    title TEXT,
    company_ids TEXT NOT NULL DEFAULT '[]',
    portal_enabled INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS company_price_overrides (
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    price TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (company_id, product_id)
);

-- ============================================================================
-- Transfer orders (crates/stateset-db/src/sqlite/transfer_orders.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS transfer_orders (
    id TEXT PRIMARY KEY,
    number TEXT NOT NULL,
    source_warehouse_id TEXT NOT NULL,
    destination_warehouse_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    expected_at TEXT,
    shipped_at TEXT,
    received_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_transfer_orders_status ON transfer_orders(status);
CREATE INDEX IF NOT EXISTS idx_transfer_orders_source ON transfer_orders(source_warehouse_id);
CREATE INDEX IF NOT EXISTS idx_transfer_orders_dest ON transfer_orders(destination_warehouse_id);

CREATE TABLE IF NOT EXISTS transfer_order_items (
    id TEXT PRIMARY KEY,
    transfer_order_id TEXT NOT NULL REFERENCES transfer_orders(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    sku TEXT NOT NULL,
    quantity TEXT NOT NULL,
    quantity_shipped TEXT NOT NULL DEFAULT '0',
    quantity_received TEXT NOT NULL DEFAULT '0'
);
CREATE INDEX IF NOT EXISTS idx_transfer_order_items_order ON transfer_order_items(transfer_order_id);

-- ============================================================================
-- Units of measure (crates/stateset-db/src/sqlite/units_of_measure.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS unit_classes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    base_uom_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS units_of_measure (
    id TEXT PRIMARY KEY,
    unit_class_id TEXT NOT NULL REFERENCES unit_classes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    abbreviation TEXT NOT NULL,
    factor TEXT NOT NULL DEFAULT '1',
    is_base INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_uoms_class ON units_of_measure(unit_class_id);

CREATE TABLE IF NOT EXISTS unit_conversion_rules (
    id TEXT PRIMARY KEY,
    rule_type TEXT NOT NULL DEFAULT 'SYSTEM',
    product_id TEXT,
    from_uom_id TEXT NOT NULL,
    to_uom_id TEXT NOT NULL,
    factor TEXT NOT NULL DEFAULT '1',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_conversion_rules_product ON unit_conversion_rules(product_id);

-- ============================================================================
-- Production batches (crates/stateset-db/src/sqlite/production_batches.rs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS production_batches (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned',
    vendor_id TEXT,
    work_order_ids TEXT NOT NULL DEFAULT '[]',
    notes TEXT,
    scheduled_start TEXT,
    scheduled_end TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_production_batches_status ON production_batches(status);
CREATE INDEX IF NOT EXISTS idx_production_batches_vendor ON production_batches(vendor_id);
