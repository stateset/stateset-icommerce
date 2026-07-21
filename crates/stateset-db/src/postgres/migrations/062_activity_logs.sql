-- Activity logs: append-only change history for subject records (sales
-- orders, fulfillment orders, shipments, and any other entity), previously
-- SQLite-only.
--
-- Repository: crates/stateset-db/src/postgres/activity_logs.rs

CREATE TABLE IF NOT EXISTS activity_logs (
    id           UUID PRIMARY KEY,
    subject_type TEXT NOT NULL,
    subject_id   UUID NOT NULL,
    action       TEXT NOT NULL,
    summary      TEXT NOT NULL,
    actor_kind   TEXT NOT NULL DEFAULT 'system',
    actor        TEXT,
    metadata     JSONB NOT NULL DEFAULT 'null'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_activity_logs_subject ON activity_logs (subject_type, subject_id);
CREATE INDEX IF NOT EXISTS idx_activity_logs_action ON activity_logs (action);
CREATE INDEX IF NOT EXISTS idx_activity_logs_created ON activity_logs (created_at);
