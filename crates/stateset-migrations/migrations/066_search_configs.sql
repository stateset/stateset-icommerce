-- Search configuration profiles (SQLite).
--
-- Mirrors postgres/migrations/046_search_configs.sql. This table was previously
-- missing from the embedded SQLite migration set, so every search-config
-- operation failed at runtime with "no such table: search_configs" on the
-- default backend even though the capability reported as supported. The schema
-- matches the repository code in sqlite/search_configs.rs (JSON TEXT columns).

CREATE TABLE IF NOT EXISTS search_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    searchable_fields TEXT NOT NULL DEFAULT '[]',
    facets TEXT NOT NULL DEFAULT '[]',
    synonyms TEXT NOT NULL DEFAULT '[]',
    boost_rules TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_search_configs_is_active ON search_configs(is_active);
CREATE INDEX IF NOT EXISTS idx_search_configs_name ON search_configs(name);
CREATE INDEX IF NOT EXISTS idx_search_configs_created_at ON search_configs(created_at DESC);
