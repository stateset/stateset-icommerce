-- x402 Payment Intents and Agent Cards for Agent-to-Agent (A2A) Commerce (PostgreSQL)
-- Enables AI agents to buy and sell from each other using stablecoin payments

-- ==========================================================================
-- x402 Payment Intents - Off-chain signed payment requests
-- ==========================================================================

CREATE TABLE IF NOT EXISTS x402_payment_intents (
    id UUID PRIMARY KEY,
    version TEXT NOT NULL DEFAULT '1.0',
    status TEXT NOT NULL DEFAULT 'created',

    -- Payment parameters (signed fields)
    payer_address TEXT NOT NULL,
    payee_address TEXT NOT NULL,
    amount BIGINT NOT NULL,
    amount_decimal NUMERIC(38, 18) NOT NULL,
    asset TEXT NOT NULL DEFAULT 'usdc',
    network TEXT NOT NULL DEFAULT 'set_chain',
    chain_id BIGINT NOT NULL,
    token_address TEXT,

    -- Validity & replay protection
    created_at_unix BIGINT NOT NULL,
    valid_until BIGINT NOT NULL,
    nonce BIGINT NOT NULL,
    idempotency_key TEXT,

    -- Resource & context
    resource_uri TEXT,
    resource_method TEXT,
    description TEXT,
    cart_id UUID REFERENCES carts(id) ON DELETE SET NULL,
    order_id UUID REFERENCES orders(id) ON DELETE SET NULL,
    invoice_id UUID REFERENCES invoices(id) ON DELETE SET NULL,
    merchant_id TEXT,

    -- Cryptographic fields
    signing_hash TEXT,
    payer_signature TEXT,
    payer_public_key TEXT,

    -- Sequencer fields (after submission)
    sequence_number BIGINT,
    sequenced_at TIMESTAMPTZ,
    batch_id UUID,
    batch_merkle_root TEXT,
    inclusion_proof JSONB,

    -- Settlement fields (after on-chain execution)
    tx_hash TEXT,
    block_number BIGINT,
    gas_used BIGINT,
    settled_at TIMESTAMPTZ,

    -- Metadata
    metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_x402_intents_payer ON x402_payment_intents(payer_address);
CREATE INDEX IF NOT EXISTS idx_x402_intents_payee ON x402_payment_intents(payee_address);
CREATE INDEX IF NOT EXISTS idx_x402_intents_status ON x402_payment_intents(status);
CREATE INDEX IF NOT EXISTS idx_x402_intents_cart ON x402_payment_intents(cart_id);
CREATE INDEX IF NOT EXISTS idx_x402_intents_order ON x402_payment_intents(order_id);
CREATE INDEX IF NOT EXISTS idx_x402_intents_batch ON x402_payment_intents(batch_id);
CREATE INDEX IF NOT EXISTS idx_x402_intents_nonce ON x402_payment_intents(payer_address, nonce);
CREATE INDEX IF NOT EXISTS idx_x402_intents_valid_until ON x402_payment_intents(valid_until);
CREATE INDEX IF NOT EXISTS idx_x402_intents_idempotency ON x402_payment_intents(idempotency_key);

-- ==========================================================================
-- Agent Cards - Capability advertisement for AI agents
-- ==========================================================================

CREATE TABLE IF NOT EXISTS agent_cards (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,

    -- Identity & authentication
    wallet_address TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,

    -- Payment capabilities (JSON arrays)
    supported_networks JSONB NOT NULL,
    supported_assets JSONB NOT NULL,

    -- A2A commerce capabilities (JSON array)
    a2a_skills JSONB,

    -- Trust & verification
    trust_level TEXT NOT NULL DEFAULT 'standard',
    verified_at TIMESTAMPTZ,
    verification_method TEXT,

    -- Endpoint for A2A communication
    endpoint_url TEXT,
    endpoint_protocol TEXT DEFAULT 'https',

    -- Merchant/business info
    merchant_id TEXT,
    merchant_name TEXT,
    business_category TEXT,

    -- Limits & policies
    max_transaction_amount BIGINT,
    daily_volume_limit BIGINT,
    requires_kyc BOOLEAN NOT NULL DEFAULT FALSE,

    -- Status
    active BOOLEAN NOT NULL DEFAULT TRUE,
    suspended_at TIMESTAMPTZ,
    suspension_reason TEXT,

    -- Metadata
    metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agent_cards_wallet ON agent_cards(wallet_address);
CREATE INDEX IF NOT EXISTS idx_agent_cards_trust_level ON agent_cards(trust_level);
CREATE INDEX IF NOT EXISTS idx_agent_cards_active ON agent_cards(active);
CREATE INDEX IF NOT EXISTS idx_agent_cards_merchant ON agent_cards(merchant_id);
CREATE INDEX IF NOT EXISTS idx_agent_cards_supported_networks ON agent_cards USING GIN (supported_networks);
CREATE INDEX IF NOT EXISTS idx_agent_cards_supported_assets ON agent_cards USING GIN (supported_assets);
CREATE INDEX IF NOT EXISTS idx_agent_cards_a2a_skills ON agent_cards USING GIN (a2a_skills);

-- ==========================================================================
-- A2A Commerce Quotes - Price quotes between agents
-- ==========================================================================

CREATE TABLE IF NOT EXISTS a2a_quotes (
    id UUID PRIMARY KEY,
    quote_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending',

    -- Participants
    buyer_agent_id UUID NOT NULL,
    seller_agent_id UUID NOT NULL,

    -- Quote details (JSON)
    items JSONB NOT NULL,

    -- Pricing
    subtotal NUMERIC(38, 18) NOT NULL,
    tax_amount NUMERIC(38, 18) NOT NULL DEFAULT 0,
    shipping_amount NUMERIC(38, 18) NOT NULL DEFAULT 0,
    discount_amount NUMERIC(38, 18) NOT NULL DEFAULT 0,
    total NUMERIC(38, 18) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Payment terms
    payment_network TEXT,
    payment_asset TEXT,

    -- Shipping (if physical goods)
    shipping_address JSONB,

    -- Validity
    valid_until TIMESTAMPTZ NOT NULL,

    -- Conversion to purchase
    purchase_id UUID,
    payment_intent_id UUID,

    -- Metadata
    notes TEXT,
    metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    FOREIGN KEY (buyer_agent_id) REFERENCES agent_cards(id) ON DELETE CASCADE,
    FOREIGN KEY (seller_agent_id) REFERENCES agent_cards(id) ON DELETE CASCADE,
    FOREIGN KEY (payment_intent_id) REFERENCES x402_payment_intents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_quotes_buyer ON a2a_quotes(buyer_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_seller ON a2a_quotes(seller_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_status ON a2a_quotes(status);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_valid_until ON a2a_quotes(valid_until);

-- ==========================================================================
-- A2A Purchases - Completed agent-to-agent transactions
-- ==========================================================================

CREATE TABLE IF NOT EXISTS a2a_purchases (
    id UUID PRIMARY KEY,
    purchase_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'initiated',

    -- Participants
    buyer_agent_id UUID NOT NULL,
    seller_agent_id UUID NOT NULL,

    -- References
    quote_id UUID,
    cart_id UUID,
    order_id UUID,
    payment_intent_id UUID,

    -- Purchase details
    items JSONB NOT NULL,
    total NUMERIC(38, 18) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Fulfillment
    fulfillment_type TEXT,
    tracking_info JSONB,
    delivered_at TIMESTAMPTZ,
    delivery_confirmed_at TIMESTAMPTZ,
    delivery_confirmation_signature TEXT,

    -- Rating & feedback
    buyer_rating INTEGER,
    buyer_feedback TEXT,
    seller_rating INTEGER,
    seller_feedback TEXT,

    -- Metadata
    notes TEXT,
    metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    FOREIGN KEY (buyer_agent_id) REFERENCES agent_cards(id) ON DELETE CASCADE,
    FOREIGN KEY (seller_agent_id) REFERENCES agent_cards(id) ON DELETE CASCADE,
    FOREIGN KEY (quote_id) REFERENCES a2a_quotes(id) ON DELETE SET NULL,
    FOREIGN KEY (cart_id) REFERENCES carts(id) ON DELETE SET NULL,
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE SET NULL,
    FOREIGN KEY (payment_intent_id) REFERENCES x402_payment_intents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_purchases_buyer ON a2a_purchases(buyer_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_seller ON a2a_purchases(seller_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_status ON a2a_purchases(status);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_order ON a2a_purchases(order_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_payment ON a2a_purchases(payment_intent_id);

-- ==========================================================================
-- Add x402 payment fields to carts table
-- ==========================================================================

ALTER TABLE carts ADD COLUMN IF NOT EXISTS x402_payer_address TEXT;
ALTER TABLE carts ADD COLUMN IF NOT EXISTS x402_network TEXT;
ALTER TABLE carts ADD COLUMN IF NOT EXISTS x402_asset TEXT;
ALTER TABLE carts ADD COLUMN IF NOT EXISTS x402_intent_id UUID;
ALTER TABLE carts ADD COLUMN IF NOT EXISTS x402_status TEXT;
