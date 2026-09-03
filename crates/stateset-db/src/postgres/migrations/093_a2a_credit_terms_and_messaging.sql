-- Durable A2A credit terms and agent messaging (see the SQLite twin,
-- 086_a2a_credit_terms_and_messaging.sql, for the rationale). Legacy-safe:
-- pure CREATE IF NOT EXISTS.

CREATE TABLE IF NOT EXISTS a2a_credit_terms (
    id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    creditor_agent_id TEXT NOT NULL,
    debtor_agent_id TEXT NOT NULL,
    credit_limit NUMERIC NOT NULL,
    outstanding_balance NUMERIC NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_terms TEXT NOT NULL DEFAULT 'net_30',
    status TEXT NOT NULL DEFAULT 'active',
    min_trust_tier TEXT NOT NULL DEFAULT 'standard',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_credit_terms_scope
    ON a2a_credit_terms(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_a2a_credit_terms_debtor
    ON a2a_credit_terms(tenant_id, debtor_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_credit_terms_creditor
    ON a2a_credit_terms(tenant_id, creditor_agent_id);

CREATE TABLE IF NOT EXISTS a2a_credit_entries (
    id UUID PRIMARY KEY,
    terms_id UUID NOT NULL REFERENCES a2a_credit_terms(id),
    tenant_id TEXT NOT NULL DEFAULT 'default',
    entry_type TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    balance_after NUMERIC NOT NULL,
    reference_id TEXT,
    notes TEXT,
    due_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_credit_entries_terms
    ON a2a_credit_entries(tenant_id, terms_id, created_at);

CREATE TABLE IF NOT EXISTS a2a_agent_messages (
    id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    conversation_id UUID NOT NULL,
    from_agent_id UUID NOT NULL,
    to_agent_id UUID NOT NULL,
    message_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending',
    sequence_number BIGINT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    next_retry_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, conversation_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_a2a_agent_messages_scope
    ON a2a_agent_messages(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_a2a_agent_messages_recipient
    ON a2a_agent_messages(tenant_id, to_agent_id, status);
