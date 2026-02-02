-- x402 Credit Ledger for Metered Billing (PostgreSQL)
-- Enables prepaid balances and usage-based debits for AI agent commerce

CREATE TABLE IF NOT EXISTS x402_credit_accounts (
    id UUID PRIMARY KEY,
    payer_address TEXT NOT NULL,
    asset TEXT NOT NULL DEFAULT 'usdc',
    network TEXT NOT NULL DEFAULT 'set_chain',
    balance BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (payer_address, asset, network)
);

CREATE TABLE IF NOT EXISTS x402_credit_transactions (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES x402_credit_accounts(id) ON DELETE CASCADE,
    payer_address TEXT NOT NULL,
    asset TEXT NOT NULL,
    network TEXT NOT NULL,
    direction TEXT NOT NULL,
    amount BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    reason TEXT,
    reference_id TEXT,
    metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_x402_credit_accounts_payer
    ON x402_credit_accounts(payer_address);
CREATE INDEX IF NOT EXISTS idx_x402_credit_accounts_asset_network
    ON x402_credit_accounts(asset, network);
CREATE INDEX IF NOT EXISTS idx_x402_credit_tx_account
    ON x402_credit_transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_x402_credit_tx_payer
    ON x402_credit_transactions(payer_address);
CREATE INDEX IF NOT EXISTS idx_x402_credit_tx_direction
    ON x402_credit_transactions(direction);
CREATE INDEX IF NOT EXISTS idx_x402_credit_tx_reference
    ON x402_credit_transactions(reference_id);
