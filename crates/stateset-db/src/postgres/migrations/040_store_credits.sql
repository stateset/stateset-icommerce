-- Store credits and store credit transactions
CREATE TABLE IF NOT EXISTS store_credits (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id     UUID NOT NULL REFERENCES customers(id),
    original_balance NUMERIC(19,4) NOT NULL,
    current_balance  NUMERIC(19,4) NOT NULL,
    currency        TEXT NOT NULL DEFAULT 'USD',
    status          TEXT NOT NULL DEFAULT 'active',
    reason          TEXT NOT NULL DEFAULT 'return',
    reference_id    TEXT,
    note            TEXT,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT store_credits_balance_non_negative CHECK (current_balance >= 0),
    CONSTRAINT store_credits_original_positive CHECK (original_balance > 0)
);

CREATE INDEX IF NOT EXISTS idx_store_credits_customer_id ON store_credits (customer_id);
CREATE INDEX IF NOT EXISTS idx_store_credits_status ON store_credits (status);
CREATE INDEX IF NOT EXISTS idx_store_credits_reason ON store_credits (reason);
CREATE INDEX IF NOT EXISTS idx_store_credits_expires_at ON store_credits (expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS store_credit_transactions (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_credit_id  UUID NOT NULL REFERENCES store_credits(id),
    amount           NUMERIC(19,4) NOT NULL,
    balance_after    NUMERIC(19,4) NOT NULL,
    transaction_type TEXT NOT NULL,
    reference_id     TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_store_credit_txns_credit_id ON store_credit_transactions (store_credit_id);
CREATE INDEX IF NOT EXISTS idx_store_credit_txns_type ON store_credit_transactions (transaction_type);
