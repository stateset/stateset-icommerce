-- Canonical engine stores previously omitted the audit helper's table.
-- IF NOT EXISTS preserves compatible application/legacy audit history.
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    actor TEXT,
    details TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Fail the migration if an existing table is missing required columns;
-- CREATE TABLE IF NOT EXISTS alone would silently accept that layout.
SELECT id, action, entity_type, entity_id, actor, details, created_at FROM audit_log LIMIT 0;

CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_at);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor);
