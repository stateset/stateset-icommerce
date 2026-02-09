-- Custom Objects (custom states / metaobjects)
--
-- Schema-driven custom data layer:
-- - custom_object_types: schema definitions (handle + field definitions)
-- - custom_object_records: validated instances linked to an optional owner

CREATE TABLE IF NOT EXISTS custom_object_types (
    id UUID PRIMARY KEY,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    fields JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version INT NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS custom_object_records (
    id UUID PRIMARY KEY,
    type_id UUID NOT NULL REFERENCES custom_object_types(id) ON DELETE CASCADE,
    handle TEXT,
    owner_type TEXT,
    owner_id TEXT,
    values JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version INT NOT NULL DEFAULT 1,
    CONSTRAINT custom_object_owner_pair CHECK (
        (owner_type IS NULL AND owner_id IS NULL) OR
        (owner_type IS NOT NULL AND owner_id IS NOT NULL)
    ),
    CONSTRAINT custom_object_type_handle_unique UNIQUE (type_id, handle)
);

CREATE INDEX IF NOT EXISTS idx_custom_object_records_type_id
    ON custom_object_records(type_id);

CREATE INDEX IF NOT EXISTS idx_custom_object_records_owner
    ON custom_object_records(owner_type, owner_id);

