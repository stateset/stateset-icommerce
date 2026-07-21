-- Integration field mappings: field-path → destination-field mappings with
-- optional template, transform, and fallback, scoped to an integration account.
-- Distinct from integration_mappings (which maps discrete values).
--
-- Repository: crates/stateset-db/src/postgres/integration_field_mappings.rs
-- SQLite twin: crates/stateset-db/migrations/053_integration_field_mappings.sql

CREATE TABLE IF NOT EXISTS integration_field_mappings (
    id                  UUID PRIMARY KEY,
    integration_account TEXT NOT NULL,
    mapping_group       TEXT NOT NULL,
    source_field        TEXT NOT NULL,
    destination_field   TEXT NOT NULL,
    template            TEXT,
    transform           TEXT NOT NULL DEFAULT 'none',
    fallback            TEXT,
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_ifm_account_group
    ON integration_field_mappings (integration_account, mapping_group);
