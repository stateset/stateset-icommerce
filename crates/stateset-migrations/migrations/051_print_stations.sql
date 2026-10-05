-- Print stations: paired agents (warehouse PCs) that long-poll for print jobs,
-- plus the per-station job queue. Only the SHA-256 hash of the pairing token is
-- stored; the plaintext token is returned once at pairing.
--
-- Repository: crates/stateset-db/src/sqlite/print_stations.rs
-- REST:       crates/stateset-http/src/routes/print_stations.rs
--
-- Booleans as INTEGER 0/1; printers as JSON TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS print_stations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    printers TEXT NOT NULL DEFAULT '[]',
    revoked INTEGER NOT NULL DEFAULT 0,
    last_seen_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_print_stations_revoked ON print_stations(revoked);

CREATE TABLE IF NOT EXISTS print_jobs (
    id TEXT PRIMARY KEY,
    station_id TEXT NOT NULL REFERENCES print_stations(id) ON DELETE CASCADE,
    printer_name TEXT,
    payload_kind TEXT NOT NULL DEFAULT 'zpl',
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    picked_up_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_print_jobs_station ON print_jobs(station_id, status);
