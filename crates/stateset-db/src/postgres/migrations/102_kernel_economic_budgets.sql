-- Durable, exact monetary authority for governed economic commands.
CREATE TABLE IF NOT EXISTS kernel_economic_budgets (
    budget_id TEXT PRIMARY KEY,
    principal_id TEXT NOT NULL,
    tenant_id TEXT,
    store_id TEXT,
    limit_amount NUMERIC NOT NULL CHECK (limit_amount >= 0),
    committed_amount NUMERIC NOT NULL DEFAULT 0 CHECK (committed_amount >= 0),
    currency TEXT NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (committed_amount <= limit_amount),
    CHECK (expires_at > valid_from)
);
CREATE INDEX IF NOT EXISTS idx_kernel_economic_budgets_principal
    ON kernel_economic_budgets(principal_id, tenant_id, store_id);

CREATE TABLE IF NOT EXISTS kernel_budget_commitments (
    command_id UUID PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    budget_id TEXT NOT NULL REFERENCES kernel_economic_budgets(budget_id),
    amount NUMERIC NOT NULL CHECK (amount >= 0),
    currency TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kernel_budget_commitments_budget
    ON kernel_budget_commitments(budget_id, created_at);
