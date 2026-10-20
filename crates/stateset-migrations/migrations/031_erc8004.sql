-- ERC-8004 Trustless Agents (Identity, Reputation, Validation)

-- ============================================================================
-- Agent Identities (on-chain registry mirror)
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_identities (
    id TEXT PRIMARY KEY,
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    agent_uri TEXT NOT NULL,
    agent_wallet TEXT,
    owner_address TEXT,
    agent_card_id TEXT,
    registration TEXT,
    registration_hash TEXT,
    wallet_proof_type TEXT,
    wallet_proof TEXT,
    wallet_proof_chain_id INTEGER,
    wallet_proof_deadline TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    UNIQUE(agent_registry, agent_id),
    FOREIGN KEY (agent_card_id) REFERENCES agent_cards(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_identities_registry
    ON agent_identities(agent_registry);
CREATE INDEX IF NOT EXISTS idx_agent_identities_wallet
    ON agent_identities(agent_wallet);
CREATE INDEX IF NOT EXISTS idx_agent_identities_owner
    ON agent_identities(owner_address);
CREATE INDEX IF NOT EXISTS idx_agent_identities_card
    ON agent_identities(agent_card_id);
CREATE INDEX IF NOT EXISTS idx_agent_identities_active
    ON agent_identities(active);

-- ============================================================================
-- Agent Identity Metadata
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_identity_metadata (
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    metadata_key TEXT NOT NULL,
    metadata_value BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    PRIMARY KEY (agent_registry, agent_id, metadata_key)
);

CREATE INDEX IF NOT EXISTS idx_agent_identity_metadata_registry
    ON agent_identity_metadata(agent_registry);
CREATE INDEX IF NOT EXISTS idx_agent_identity_metadata_key
    ON agent_identity_metadata(metadata_key);

-- ============================================================================
-- Reputation Feedback
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_feedback (
    id TEXT PRIMARY KEY,
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    client_address TEXT NOT NULL,
    feedback_index INTEGER NOT NULL,
    value INTEGER NOT NULL,
    value_decimals INTEGER NOT NULL,
    tag1 TEXT,
    tag2 TEXT,
    endpoint TEXT,
    feedback_uri TEXT,
    feedback_hash TEXT,
    is_revoked INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    revoked_at TEXT,

    UNIQUE(agent_registry, agent_id, client_address, feedback_index)
);

CREATE INDEX IF NOT EXISTS idx_agent_feedback_agent
    ON agent_feedback(agent_registry, agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_feedback_client
    ON agent_feedback(client_address);
CREATE INDEX IF NOT EXISTS idx_agent_feedback_tag1
    ON agent_feedback(tag1);
CREATE INDEX IF NOT EXISTS idx_agent_feedback_tag2
    ON agent_feedback(tag2);
CREATE INDEX IF NOT EXISTS idx_agent_feedback_revoked
    ON agent_feedback(is_revoked);

-- ============================================================================
-- Feedback Responses
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_feedback_responses (
    id TEXT PRIMARY KEY,
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    client_address TEXT NOT NULL,
    feedback_index INTEGER NOT NULL,
    responder_address TEXT NOT NULL,
    response_uri TEXT NOT NULL,
    response_hash TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_feedback_responses_feedback
    ON agent_feedback_responses(agent_registry, agent_id, client_address, feedback_index);
CREATE INDEX IF NOT EXISTS idx_agent_feedback_responses_responder
    ON agent_feedback_responses(responder_address);

-- ============================================================================
-- Validation Requests
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_validation_requests (
    request_hash TEXT PRIMARY KEY,
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    validator_address TEXT NOT NULL,
    request_uri TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_validation_requests_agent
    ON agent_validation_requests(agent_registry, agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_validation_requests_validator
    ON agent_validation_requests(validator_address);

-- ============================================================================
-- Validation Responses
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent_validation_responses (
    id TEXT PRIMARY KEY,
    request_hash TEXT NOT NULL,
    agent_registry TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    validator_address TEXT NOT NULL,
    response INTEGER NOT NULL,
    response_uri TEXT,
    response_hash TEXT,
    tag TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_validation_responses_request
    ON agent_validation_responses(request_hash);
CREATE INDEX IF NOT EXISTS idx_agent_validation_responses_agent
    ON agent_validation_responses(agent_registry, agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_validation_responses_validator
    ON agent_validation_responses(validator_address);
CREATE INDEX IF NOT EXISTS idx_agent_validation_responses_tag
    ON agent_validation_responses(tag);
