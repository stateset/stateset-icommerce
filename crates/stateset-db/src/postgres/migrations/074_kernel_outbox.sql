CREATE TABLE IF NOT EXISTS kernel_outbox (
    id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    command_id UUID,
    idempotency_key TEXT,
    principal_type TEXT,
    principal_id TEXT,
    correlation_id UUID,
    causation_id UUID,
    created_at TIMESTAMPTZ NOT NULL,
    published_at TIMESTAMPTZ,
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
    command_id UUID PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    command_type TEXT NOT NULL,
    contract_version TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    receipt JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL
);
