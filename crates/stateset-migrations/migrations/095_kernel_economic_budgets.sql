-- Durable, exact monetary authority for governed economic commands.
CREATE TABLE IF NOT EXISTS kernel_economic_budgets (
    budget_id TEXT PRIMARY KEY NOT NULL,
    principal_id TEXT NOT NULL,
    tenant_id TEXT,
    store_id TEXT,
    limit_amount TEXT NOT NULL,
    committed_amount TEXT NOT NULL DEFAULT '0',
    currency TEXT NOT NULL,
    valid_from TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kernel_economic_budgets_principal
    ON kernel_economic_budgets(principal_id, tenant_id, store_id);

-- One immutable debit per successfully committed semantic command. The
-- unique retry key is a database backstop in addition to kernel receipts.
CREATE TABLE IF NOT EXISTS kernel_budget_commitments (
    command_id TEXT PRIMARY KEY NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    budget_id TEXT NOT NULL,
    amount TEXT NOT NULL,
    currency TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (budget_id) REFERENCES kernel_economic_budgets(budget_id)
);
CREATE INDEX IF NOT EXISTS idx_kernel_budget_commitments_budget
    ON kernel_budget_commitments(budget_id, created_at);
