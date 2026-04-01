-- Gift cards migration for PostgreSQL
-- Gift card issuance, balance tracking, and transaction processing

CREATE TABLE IF NOT EXISTS gift_cards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) UNIQUE NOT NULL,
    initial_balance NUMERIC(12, 4) NOT NULL,
    current_balance NUMERIC(12, 4) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    recipient_email VARCHAR(255),
    sender_name VARCHAR(255),
    message TEXT,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS gift_card_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    gift_card_id UUID NOT NULL REFERENCES gift_cards(id),
    amount NUMERIC(12, 4) NOT NULL,
    balance_after NUMERIC(12, 4) NOT NULL,
    transaction_type VARCHAR(50) NOT NULL,
    reference_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_gift_cards_code ON gift_cards(code);
CREATE INDEX IF NOT EXISTS idx_gift_cards_status ON gift_cards(status);
CREATE INDEX IF NOT EXISTS idx_gift_cards_recipient_email ON gift_cards(recipient_email);
CREATE INDEX IF NOT EXISTS idx_gift_card_transactions_gift_card_id ON gift_card_transactions(gift_card_id);
CREATE INDEX IF NOT EXISTS idx_gift_card_transactions_created_at ON gift_card_transactions(created_at DESC);

-- Check constraints
ALTER TABLE gift_cards ADD CONSTRAINT gift_cards_initial_balance_non_negative CHECK (initial_balance >= 0);
ALTER TABLE gift_cards ADD CONSTRAINT gift_cards_current_balance_non_negative CHECK (current_balance >= 0);
ALTER TABLE gift_card_transactions ADD CONSTRAINT gift_card_transactions_amount_positive CHECK (amount > 0);
ALTER TABLE gift_card_transactions ADD CONSTRAINT gift_card_transactions_balance_after_non_negative CHECK (balance_after >= 0);
