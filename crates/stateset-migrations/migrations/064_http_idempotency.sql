-- Migration 064: Durable HTTP idempotency response store
--
-- Backs the HTTP layer's Idempotency-Key middleware so replays survive
-- process restarts and work across replicas sharing a database. One row per
-- (tenant, key); created_at is a unix-epoch millisecond timestamp used for
-- TTL expiry.

CREATE TABLE IF NOT EXISTS http_idempotency_keys (
    tenant TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    response_status INTEGER NOT NULL,
    content_type TEXT,
    response_body BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (tenant, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_http_idempotency_keys_created_at
    ON http_idempotency_keys(created_at);
