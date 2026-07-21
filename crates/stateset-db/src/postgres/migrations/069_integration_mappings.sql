-- Integration mappings: external↔internal value translations per integration
-- and mapping group (carrier names, order statuses, payment methods, etc.).
--
-- Repository: crates/stateset-db/src/postgres/integration_mappings.rs
-- SQLite twin: crates/stateset-db/migrations/048_integration_mappings.sql

CREATE TABLE IF NOT EXISTS integration_mappings (
    id             UUID PRIMARY KEY,
    integration    TEXT NOT NULL,
    mapping_group  TEXT NOT NULL,
    field_name     TEXT NOT NULL,
    external_value TEXT NOT NULL,
    internal_value TEXT NOT NULL,
    is_active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (integration, mapping_group, field_name, external_value)
);
CREATE INDEX IF NOT EXISTS idx_integration_mappings_lookup
    ON integration_mappings (integration, mapping_group, field_name);
