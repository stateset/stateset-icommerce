-- Saga coordinator schema (PostgreSQL)
--
-- This migration is gated behind the `stateset-db/saga` feature.
-- It provides tables to persist multi-step orchestrations with rollback.

CREATE TABLE IF NOT EXISTS sagas (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending',
    current_step INTEGER NOT NULL DEFAULT 0,
    total_steps INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    business_key TEXT
);

CREATE INDEX IF NOT EXISTS idx_sagas_status ON sagas(status);
CREATE INDEX IF NOT EXISTS idx_sagas_business_key ON sagas(business_key);

CREATE TABLE IF NOT EXISTS saga_steps (
    id UUID PRIMARY KEY,
    saga_id UUID NOT NULL REFERENCES sagas(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    step_order INTEGER NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending',
    executed_at TIMESTAMPTZ,
    result JSONB,
    compensation_step_id UUID,
    rollback_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (saga_id, step_order)
);

CREATE INDEX IF NOT EXISTS idx_saga_steps_saga ON saga_steps(saga_id);
CREATE INDEX IF NOT EXISTS idx_saga_steps_status ON saga_steps(status);

