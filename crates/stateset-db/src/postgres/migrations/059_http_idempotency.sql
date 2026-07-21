-- Migration 059: Durable HTTP idempotency response store
--
-- Backs the HTTP layer's Idempotency-Key middleware so replays survive
-- process restarts and work across replicas sharing a database.

CREATE TABLE IF NOT EXISTS http_idempotency_keys (
    tenant TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    response_status INTEGER NOT NULL,
    content_type TEXT,
    response_body BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_http_idempotency_keys_created_at
    ON http_idempotency_keys(created_at);
