-- Durable transactional events for kernel command execution.
CREATE TABLE IF NOT EXISTS kernel_outbox (
    id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload TEXT NOT NULL CHECK (json_valid(payload)),
    command_id TEXT,
    idempotency_key TEXT,
    principal_type TEXT,
    principal_id TEXT,
    correlation_id TEXT,
    causation_id TEXT,
    created_at TEXT NOT NULL,
    published_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT
);
CREATE INDEX IF NOT EXISTS idx_kernel_outbox_unpublished
    ON kernel_outbox(created_at, id) WHERE published_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_kernel_outbox_aggregate
    ON kernel_outbox(aggregate_type, aggregate_id, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_kernel_outbox_command_event
    ON kernel_outbox(command_id, event_type, aggregate_id) WHERE command_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS kernel_receipts (
    command_id TEXT PRIMARY KEY NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    command_type TEXT NOT NULL,
    contract_version TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    receipt TEXT NOT NULL CHECK (json_valid(receipt)),
    created_at TEXT NOT NULL,
    completed_at TEXT NOT NULL
);
