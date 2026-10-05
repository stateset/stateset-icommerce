-- Activity logs: append-only change history for subject records (sales orders,
-- fulfillment orders, shipments, and any other entity).
--
-- Repository: crates/stateset-db/src/sqlite/activity_logs.rs
-- REST:       crates/stateset-http/src/routes/activity_logs.rs
--
-- Metadata stored as JSON TEXT; timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS activity_logs (
    id TEXT PRIMARY KEY,
    subject_type TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    action TEXT NOT NULL,
    summary TEXT NOT NULL,
    actor_kind TEXT NOT NULL DEFAULT 'system',
    actor TEXT,
    metadata TEXT NOT NULL DEFAULT 'null',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_activity_logs_subject ON activity_logs(subject_type, subject_id);
CREATE INDEX IF NOT EXISTS idx_activity_logs_action ON activity_logs(action);
CREATE INDEX IF NOT EXISTS idx_activity_logs_created ON activity_logs(created_at);
