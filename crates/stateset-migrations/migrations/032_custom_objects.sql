-- Custom Objects (custom states / metaobjects)
--
-- Provides a schema-driven custom data layer:
-- - custom_object_types: schema definitions (handle + field definitions)
-- - custom_object_records: validated instances linked to an optional owner

CREATE TABLE IF NOT EXISTS custom_object_types (
    id TEXT PRIMARY KEY,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    fields_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS custom_object_records (
    id TEXT PRIMARY KEY,
    type_id TEXT NOT NULL,
    handle TEXT,
    owner_type TEXT,
    owner_id TEXT,
    values_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (type_id) REFERENCES custom_object_types(id) ON DELETE CASCADE,
    CHECK (
        (owner_type IS NULL AND owner_id IS NULL) OR
        (owner_type IS NOT NULL AND owner_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_custom_object_records_type_handle
    ON custom_object_records(type_id, handle);

CREATE INDEX IF NOT EXISTS idx_custom_object_records_type_id
    ON custom_object_records(type_id);

CREATE INDEX IF NOT EXISTS idx_custom_object_records_owner
    ON custom_object_records(owner_type, owner_id);

