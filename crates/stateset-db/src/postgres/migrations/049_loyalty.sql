-- Loyalty programs migration for PostgreSQL
-- Programs, accounts, and point transactions

CREATE TABLE IF NOT EXISTS loyalty_programs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    points_per_dollar INTEGER NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS loyalty_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    program_id UUID NOT NULL REFERENCES loyalty_programs(id),
    customer_id UUID NOT NULL,
    points_balance BIGINT NOT NULL DEFAULT 0,
    lifetime_points BIGINT NOT NULL DEFAULT 0,
    tier VARCHAR(50) NOT NULL DEFAULT 'bronze',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (program_id, customer_id)
);

CREATE TABLE IF NOT EXISTS loyalty_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES loyalty_accounts(id),
    points BIGINT NOT NULL,
    type VARCHAR(50) NOT NULL,
    reference_id TEXT,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_loyalty_programs_status ON loyalty_programs(status);
CREATE INDEX IF NOT EXISTS idx_loyalty_programs_created_at ON loyalty_programs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_program_id ON loyalty_accounts(program_id);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_customer_id ON loyalty_accounts(customer_id);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_tier ON loyalty_accounts(tier);
CREATE INDEX IF NOT EXISTS idx_loyalty_accounts_created_at ON loyalty_accounts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_loyalty_transactions_account_id ON loyalty_transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_loyalty_transactions_created_at ON loyalty_transactions(created_at DESC);

-- Check constraints
ALTER TABLE loyalty_programs ADD CONSTRAINT loyalty_programs_points_per_dollar_positive CHECK (points_per_dollar > 0);
ALTER TABLE loyalty_accounts ADD CONSTRAINT loyalty_accounts_lifetime_points_non_negative CHECK (lifetime_points >= 0);
