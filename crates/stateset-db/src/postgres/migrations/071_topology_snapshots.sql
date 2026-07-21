-- Customer operational topology snapshots: point-in-time read-only snapshots of
-- the tenant's landscape (channel/warehouse/product counts, open-order backlog,
-- health signals) with a derived health grade.
--
-- Repository: crates/stateset-db/src/postgres/topology_snapshots.rs
-- SQLite twin: crates/stateset-db/migrations/054_topology_snapshots.sql

CREATE TABLE IF NOT EXISTS topology_snapshots (
    id               UUID PRIMARY KEY,
    channels_total   BIGINT NOT NULL DEFAULT 0,
    channels_active  BIGINT NOT NULL DEFAULT 0,
    warehouses_total BIGINT NOT NULL DEFAULT 0,
    products_total   BIGINT NOT NULL DEFAULT 0,
    open_orders      BIGINT NOT NULL DEFAULT 0,
    health           TEXT NOT NULL DEFAULT 'unknown',
    signals          JSONB NOT NULL DEFAULT 'null'::jsonb,
    captured_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_topology_snapshots_captured ON topology_snapshots (captured_at);
CREATE INDEX IF NOT EXISTS idx_topology_snapshots_health ON topology_snapshots (health);
