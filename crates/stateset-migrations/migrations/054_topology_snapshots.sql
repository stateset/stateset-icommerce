-- Customer operational topology snapshots: point-in-time read-only snapshots of
-- the tenant's landscape (channel/warehouse/product counts, open-order backlog,
-- health signals) with a derived health grade.
--
-- Repository: crates/stateset-db/src/sqlite/topology_snapshots.rs
-- REST:       crates/stateset-http/src/routes/topology_snapshots.rs
--
-- Counts as INTEGER; signals as JSON TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS topology_snapshots (
    id TEXT PRIMARY KEY,
    channels_total INTEGER NOT NULL DEFAULT 0,
    channels_active INTEGER NOT NULL DEFAULT 0,
    warehouses_total INTEGER NOT NULL DEFAULT 0,
    products_total INTEGER NOT NULL DEFAULT 0,
    open_orders INTEGER NOT NULL DEFAULT 0,
    health TEXT NOT NULL DEFAULT 'unknown',
    signals TEXT NOT NULL DEFAULT 'null',
    captured_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_topology_snapshots_captured ON topology_snapshots(captured_at);
CREATE INDEX IF NOT EXISTS idx_topology_snapshots_health ON topology_snapshots(health);
