-- Durable A2A credit terms and agent messaging.
--
-- The HTTP routes for `/a2a/credit` and `/a2a/messages` kept their state in a
-- process-local `HashMap` / `MessageQueue`, so credit lines and messages
-- vanished on restart, were invisible to a second replica, and were not
-- tenant-scoped. These tables give them the same durability as the rest of
-- the agentic ledger. Legacy-safe: pure CREATE IF NOT EXISTS.

CREATE TABLE IF NOT EXISTS a2a_credit_terms (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    creditor_agent_id TEXT NOT NULL,
    debtor_agent_id TEXT NOT NULL,
    credit_limit TEXT NOT NULL,             -- exact decimal
    outstanding_balance TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL DEFAULT 'USD',
    payment_terms TEXT NOT NULL DEFAULT 'net_30',
    status TEXT NOT NULL DEFAULT 'active',
    min_trust_tier TEXT NOT NULL DEFAULT 'standard',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_credit_terms_scope
    ON a2a_credit_terms(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_a2a_credit_terms_debtor
    ON a2a_credit_terms(tenant_id, debtor_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_credit_terms_creditor
    ON a2a_credit_terms(tenant_id, creditor_agent_id);

CREATE TABLE IF NOT EXISTS a2a_credit_entries (
    id TEXT PRIMARY KEY,
    terms_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    entry_type TEXT NOT NULL,               -- charge | payment
    amount TEXT NOT NULL,                   -- exact decimal
    balance_after TEXT NOT NULL,            -- exact decimal
    reference_id TEXT,
    notes TEXT,
    due_date TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (terms_id) REFERENCES a2a_credit_terms(id)
);

CREATE INDEX IF NOT EXISTS idx_a2a_credit_entries_terms
    ON a2a_credit_entries(tenant_id, terms_id, created_at);

CREATE TABLE IF NOT EXISTS a2a_agent_messages (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    conversation_id TEXT NOT NULL,
    from_agent_id TEXT NOT NULL,
    to_agent_id TEXT NOT NULL,
    message_type TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',     -- JSON
    status TEXT NOT NULL DEFAULT 'pending',
    sequence_number INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    next_retry_at TEXT,
    acknowledged_at TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (tenant_id, conversation_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_a2a_agent_messages_scope
    ON a2a_agent_messages(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_a2a_agent_messages_recipient
    ON a2a_agent_messages(tenant_id, to_agent_id, status);
