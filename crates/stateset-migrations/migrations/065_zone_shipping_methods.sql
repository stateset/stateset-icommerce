-- Zone shipping methods: configurable shipping methods within a geographic
-- shipping zone, with rate conditions evaluated at quote time.
--
-- Repository: crates/stateset-db/src/sqlite/zone_shipping_methods.rs
-- REST:       crates/stateset-http/src/routes/shipping_zones.rs
--
-- The SQLite store shipped without this migration, so method and rate calls
-- failed with "no such table" while `shipping_zones` itself reported as
-- supported. Mirrors the PostgreSQL schema in
-- src/postgres/migrations/047_zone_shipping_methods.sql.
--
-- Money as TEXT (exact decimal strings); timestamps RFC3339 TEXT;
-- `conditions` is a JSON array serialized to TEXT.

CREATE TABLE IF NOT EXISTS zone_shipping_methods (
    id TEXT PRIMARY KEY,
    zone_id TEXT NOT NULL,
    name TEXT NOT NULL,
    carrier TEXT,
    method_type TEXT NOT NULL DEFAULT 'flat',
    base_rate TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    min_delivery_days INTEGER,
    max_delivery_days INTEGER,
    conditions TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_zone_id
    ON zone_shipping_methods(zone_id);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_carrier
    ON zone_shipping_methods(carrier);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_method_type
    ON zone_shipping_methods(method_type);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_is_active
    ON zone_shipping_methods(is_active);
CREATE INDEX IF NOT EXISTS idx_zone_shipping_methods_created_at
    ON zone_shipping_methods(created_at DESC);
