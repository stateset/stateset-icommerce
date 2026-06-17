-- Integration field mappings: field-path → destination-field mappings with
-- optional template, transform, and fallback, scoped to an integration account.
-- Distinct from integration_mappings (which maps discrete values).
--
-- Repository: crates/stateset-db/src/sqlite/integration_field_mappings.rs
-- REST:       crates/stateset-http/src/routes/integration_field_mappings.rs
--
-- Booleans as INTEGER 0/1; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS integration_field_mappings (
    id TEXT PRIMARY KEY,
    integration_account TEXT NOT NULL,
    mapping_group TEXT NOT NULL,
    source_field TEXT NOT NULL,
    destination_field TEXT NOT NULL,
    template TEXT,
    transform TEXT NOT NULL DEFAULT 'none',
    fallback TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_ifm_account_group
    ON integration_field_mappings(integration_account, mapping_group);
