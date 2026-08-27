CREATE TABLE IF NOT EXISTS kernel_receipt_audit_log (
    sequence BIGSERIAL PRIMARY KEY,
    receipt_id UUID NOT NULL,
    command_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    previous_audit_hash TEXT,
    audit_hash TEXT NOT NULL UNIQUE,
    receipt JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kernel_receipt_audit_command
    ON kernel_receipt_audit_log(command_id, sequence);
