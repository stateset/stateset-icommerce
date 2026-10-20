-- x402 Payment Intents and Agent Cards for Agent-to-Agent (A2A) Commerce
-- Enables AI agents to buy and sell from each other using stablecoin payments

-- ============================================================================
-- x402 Payment Intents - Off-chain signed payment requests
-- ============================================================================

CREATE TABLE IF NOT EXISTS x402_payment_intents (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL DEFAULT '1.0',
    status TEXT NOT NULL DEFAULT 'created',

    -- Payment parameters (signed fields)
    payer_address TEXT NOT NULL,
    payee_address TEXT NOT NULL,
    amount INTEGER NOT NULL,
    amount_decimal TEXT NOT NULL,
    asset TEXT NOT NULL DEFAULT 'usdc',
    network TEXT NOT NULL DEFAULT 'set_chain',
    chain_id INTEGER NOT NULL,
    token_address TEXT,

    -- Validity & replay protection
    created_at_unix INTEGER NOT NULL,
    valid_until INTEGER NOT NULL,
    nonce INTEGER NOT NULL,
    idempotency_key TEXT,

    -- Resource & context
    resource_uri TEXT,
    resource_method TEXT,
    description TEXT,
    cart_id TEXT,               -- Links to cart during checkout
    order_id TEXT,              -- Links to order after completion
    invoice_id TEXT,
    merchant_id TEXT,

    -- Cryptographic fields
    signing_hash TEXT,
    payer_signature TEXT,       -- Ed25519 signature (hex)
    payer_public_key TEXT,      -- Ed25519 public key (hex)

    -- Sequencer fields (after submission)
    sequence_number INTEGER,
    sequenced_at TEXT,
    batch_id TEXT,
    batch_merkle_root TEXT,
    inclusion_proof TEXT,       -- JSON array of proof hashes

    -- Settlement fields (after on-chain execution)
    tx_hash TEXT,
    block_number INTEGER,
    gas_used INTEGER,
    settled_at TEXT,

    -- Metadata
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Indexes for x402 payment intents
CREATE INDEX IF NOT EXISTS idx_x402_intents_payer ON x402_payment_intents(payer_address);
CREATE INDEX IF NOT EXISTS idx_x402_intents_payee ON x402_payment_intents(payee_address);
CREATE INDEX IF NOT EXISTS idx_x402_intents_status ON x402_payment_intents(status);
CREATE INDEX IF NOT EXISTS idx_x402_intents_cart ON x402_payment_intents(cart_id);
CREATE INDEX IF NOT EXISTS idx_x402_intents_order ON x402_payment_intents(order_id);
CREATE INDEX IF NOT EXISTS idx_x402_intents_batch ON x402_payment_intents(batch_id);
CREATE INDEX IF NOT EXISTS idx_x402_intents_nonce ON x402_payment_intents(payer_address, nonce);
CREATE INDEX IF NOT EXISTS idx_x402_intents_valid_until ON x402_payment_intents(valid_until);
CREATE INDEX IF NOT EXISTS idx_x402_intents_idempotency ON x402_payment_intents(idempotency_key);

-- ============================================================================
-- Agent Cards - Capability advertisement for AI agents
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_cards (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,

    -- Identity & authentication
    wallet_address TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,          -- Ed25519 public key for verification

    -- Payment capabilities (JSON arrays)
    supported_networks TEXT NOT NULL,   -- ["set_chain", "base", "ethereum"]
    supported_assets TEXT NOT NULL,     -- ["usdc", "ssusd", "usdt"]

    -- A2A commerce capabilities (JSON array)
    a2a_skills TEXT,                    -- ["commerce.sell", "commerce.buy", "commerce.quote"]

    -- Trust & verification
    trust_level TEXT NOT NULL DEFAULT 'standard',  -- sandbox, standard, verified, enterprise
    verified_at TEXT,
    verification_method TEXT,

    -- Endpoint for A2A communication
    endpoint_url TEXT,
    endpoint_protocol TEXT DEFAULT 'https',  -- https, grpc, websocket

    -- Merchant/business info
    merchant_id TEXT,
    merchant_name TEXT,
    business_category TEXT,

    -- Limits & policies
    max_transaction_amount INTEGER,     -- In smallest unit (USDC cents)
    daily_volume_limit INTEGER,
    requires_kyc INTEGER DEFAULT 0,

    -- Status
    active INTEGER NOT NULL DEFAULT 1,
    suspended_at TEXT,
    suspension_reason TEXT,

    -- Metadata
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Indexes for agent cards
CREATE INDEX IF NOT EXISTS idx_agent_cards_wallet ON agent_cards(wallet_address);
CREATE INDEX IF NOT EXISTS idx_agent_cards_trust_level ON agent_cards(trust_level);
CREATE INDEX IF NOT EXISTS idx_agent_cards_active ON agent_cards(active);
CREATE INDEX IF NOT EXISTS idx_agent_cards_merchant ON agent_cards(merchant_id);

-- ============================================================================
-- A2A Commerce Quotes - Price quotes between agents
-- ============================================================================

CREATE TABLE IF NOT EXISTS a2a_quotes (
    id TEXT PRIMARY KEY,
    quote_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, accepted, rejected, expired, purchased

    -- Participants
    buyer_agent_id TEXT NOT NULL,
    seller_agent_id TEXT NOT NULL,

    -- Quote details (JSON)
    items TEXT NOT NULL,                -- JSON array of quote items

    -- Pricing
    subtotal TEXT NOT NULL,
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_amount TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Payment terms
    payment_network TEXT,
    payment_asset TEXT,

    -- Shipping (if physical goods)
    shipping_address TEXT,              -- JSON

    -- Validity
    valid_until TEXT NOT NULL,

    -- Conversion to purchase
    purchase_id TEXT,
    payment_intent_id TEXT,

    -- Metadata
    notes TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    
    FOREIGN KEY (payment_intent_id) REFERENCES x402_payment_intents(id) ON DELETE SET NULL
);

-- Indexes for A2A quotes
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_buyer ON a2a_quotes(buyer_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_seller ON a2a_quotes(seller_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_status ON a2a_quotes(status);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_valid_until ON a2a_quotes(valid_until);

-- ============================================================================
-- A2A Purchases - Completed agent-to-agent transactions
-- ============================================================================

CREATE TABLE IF NOT EXISTS a2a_purchases (
    id TEXT PRIMARY KEY,
    purchase_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'initiated',  -- initiated, payment_pending, paid, fulfilling, completed, cancelled, disputed

    -- Participants
    buyer_agent_id TEXT NOT NULL,
    seller_agent_id TEXT NOT NULL,

    -- References
    quote_id TEXT,
    cart_id TEXT,
    order_id TEXT,
    payment_intent_id TEXT,

    -- Purchase details
    items TEXT NOT NULL,                -- JSON array
    total TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',

    -- Fulfillment
    fulfillment_type TEXT,              -- shipping, digital, pickup
    tracking_info TEXT,                 -- JSON
    delivered_at TEXT,
    delivery_confirmed_at TEXT,
    delivery_confirmation_signature TEXT,

    -- Rating & feedback
    buyer_rating INTEGER,               -- 1-5
    buyer_feedback TEXT,
    seller_rating INTEGER,
    seller_feedback TEXT,

    -- Metadata
    notes TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    
    FOREIGN KEY (quote_id) REFERENCES a2a_quotes(id) ON DELETE SET NULL,
    
    FOREIGN KEY (payment_intent_id) REFERENCES x402_payment_intents(id) ON DELETE SET NULL
);

-- Indexes for A2A purchases
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_buyer ON a2a_purchases(buyer_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_seller ON a2a_purchases(seller_agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_status ON a2a_purchases(status);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_order ON a2a_purchases(order_id);
CREATE INDEX IF NOT EXISTS idx_a2a_purchases_payment ON a2a_purchases(payment_intent_id);

-- ============================================================================
-- Add x402 payment fields to carts table
-- ============================================================================

-- Add x402 payment columns to existing carts table
ALTER TABLE carts ADD COLUMN x402_payer_address TEXT;
ALTER TABLE carts ADD COLUMN x402_network TEXT;
ALTER TABLE carts ADD COLUMN x402_asset TEXT;
ALTER TABLE carts ADD COLUMN x402_intent_id TEXT;
ALTER TABLE carts ADD COLUMN x402_status TEXT;
