-- Search configuration migration for PostgreSQL
-- Searchable fields, facets, synonyms, and boost rules stored as JSONB

CREATE TABLE IF NOT EXISTS search_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    searchable_fields JSONB NOT NULL DEFAULT '[]'::jsonb,
    facets JSONB NOT NULL DEFAULT '[]'::jsonb,
    synonyms JSONB NOT NULL DEFAULT '[]'::jsonb,
    boost_rules JSONB NOT NULL DEFAULT '[]'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_search_configs_is_active ON search_configs(is_active);
CREATE INDEX IF NOT EXISTS idx_search_configs_name ON search_configs(name);
CREATE INDEX IF NOT EXISTS idx_search_configs_created_at ON search_configs(created_at DESC);
