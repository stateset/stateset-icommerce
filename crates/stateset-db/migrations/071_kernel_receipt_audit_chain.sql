-- Append-only, tamper-evident execution receipt journal.
CREATE TABLE IF NOT EXISTS kernel_receipt_audit_log (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    receipt_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    previous_audit_hash TEXT,
    audit_hash TEXT NOT NULL UNIQUE,
    receipt TEXT NOT NULL CHECK (json_valid(receipt)),
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kernel_receipt_audit_command
    ON kernel_receipt_audit_log(command_id, sequence);
