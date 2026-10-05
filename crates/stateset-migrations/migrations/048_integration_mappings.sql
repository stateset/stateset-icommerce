-- Integration mappings: external↔internal value translations per integration
-- and mapping group (carrier names, order statuses, payment methods, etc.).
--
-- Repository: crates/stateset-db/src/sqlite/integration_mappings.rs
-- REST:       crates/stateset-http/src/routes/integration_mappings.rs
--
-- Booleans as INTEGER 0/1; timestamps RFC3339 TEXT. Unique on the lookup key.

CREATE TABLE IF NOT EXISTS integration_mappings (
    id TEXT PRIMARY KEY,
    integration TEXT NOT NULL,
    mapping_group TEXT NOT NULL,
    field_name TEXT NOT NULL,
    external_value TEXT NOT NULL,
    internal_value TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (integration, mapping_group, field_name, external_value)
);
CREATE INDEX IF NOT EXISTS idx_integration_mappings_lookup
    ON integration_mappings(integration, mapping_group, field_name);
