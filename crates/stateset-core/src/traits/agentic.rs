//! Agentic commerce repositories: x402 payment intents and credits, ERC-8004 agent identity / reputation / validation, agent cards, and A2A commerce.

use super::*;

// ============================================================================
// X402 Payment Intent Repository
// ============================================================================

/// X402 Payment Intent repository trait for off-chain payment signing
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait X402PaymentIntentRepository: Send + Sync {
    /// Create a new x402 payment intent
    fn create(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent>;

    /// Get payment intent by ID
    fn get(&self, id: Uuid) -> Result<Option<X402PaymentIntent>>;

    /// Get payment intent by idempotency key
    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<X402PaymentIntent>>;

    /// Sign a payment intent (records signature and public key)
    fn sign(&self, id: Uuid, input: SignX402PaymentIntent) -> Result<X402PaymentIntent>;

    /// Mark intent as sequenced (submitted to sequencer)
    fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent>;

    /// Mark intent as settled (confirmed on-chain)
    fn mark_settled(&self, id: Uuid, tx_hash: &str, block_number: u64)
    -> Result<X402PaymentIntent>;

    /// Mark intent as failed
    fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent>;

    /// Mark intent as expired
    fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent>;

    /// Cancel a payment intent (only if not yet sequenced)
    fn cancel(&self, id: Uuid) -> Result<X402PaymentIntent>;

    /// Get payment intents for a cart
    fn for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>>;

    /// Get payment intents for an order
    fn for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>>;

    /// Get the next nonce for a payer address
    fn get_next_nonce(&self, payer_address: &str) -> Result<u64>;

    /// List payment intents with filter
    fn list(&self, filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>>;

    /// Count payment intents matching filter
    fn count(&self, filter: X402PaymentIntentFilter) -> Result<u64>;

    /// Expire all intents that have passed their `valid_until` timestamp
    fn expire_stale_intents(&self) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple payment intents - partial success allowed
    fn create_batch(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<BatchResult<X402PaymentIntent>>;

    /// Create multiple payment intents - atomic (all-or-nothing)
    fn create_batch_atomic(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<Vec<X402PaymentIntent>>;

    /// Get multiple payment intents by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>>;
}

// ============================================================================
// X402 Credit Repository (Metered Billing)
// ============================================================================

/// X402 credit ledger repository for prepaid balances and metered usage.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait X402CreditRepository: Send + Sync {
    /// Get a credit account for payer/asset/network
    fn get_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>>;

    /// Get or create a credit account (balance default = 0)
    fn get_or_create_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount>;

    /// Get current balance for payer/asset/network
    fn get_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64>;

    /// Apply a credit or debit adjustment
    fn adjust_balance(&self, input: X402CreditAdjustment) -> Result<X402CreditTransaction>;

    /// List credit transactions with optional filter
    fn list_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>>;
}

// ============================================================================
// Agent Card Repository
// ============================================================================

/// Agent Card repository trait for AI agent identity and capabilities
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AgentCardRepository: Send + Sync {
    /// Create a new agent card
    fn create(&self, input: CreateAgentCard) -> Result<AgentCard>;

    /// Get agent card by ID
    fn get(&self, id: Uuid) -> Result<Option<AgentCard>>;

    /// Get agent card by wallet address
    fn get_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>>;

    /// Update an agent card
    fn update(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard>;

    /// Delete an agent card
    fn delete(&self, id: Uuid) -> Result<()>;

    /// List agent cards with filter
    fn list(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>>;

    /// Count agent cards matching filter
    fn count(&self, filter: AgentCardFilter) -> Result<u64>;

    /// Verify an agent card (set trust level and verification info)
    fn verify(&self, id: Uuid, trust_level: TrustLevel, method: &str) -> Result<AgentCard>;

    /// Suspend an agent card
    fn suspend(&self, id: Uuid, reason: &str) -> Result<AgentCard>;

    /// Reactivate a suspended agent card
    fn reactivate(&self, id: Uuid) -> Result<AgentCard>;

    /// Discover agents with specific capabilities
    fn discover(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>>;

    // === Batch Operations ===

    /// Create multiple agent cards - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateAgentCard>) -> Result<BatchResult<AgentCard>>;

    /// Create multiple agent cards - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateAgentCard>) -> Result<Vec<AgentCard>>;

    /// Get multiple agent cards by ID
    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<AgentCard>>;
}

// ============================================================================
// ERC-8004 Agent Identity Repository
// ============================================================================

/// Agent identity registry repository (ERC-8004)
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AgentIdentityRepository: Send + Sync {
    /// Register a new agent identity
    fn register(&self, input: CreateAgentIdentity) -> Result<AgentIdentity>;

    /// Get identity by agent registry and agent ID
    fn get(&self, agent_registry: &str, agent_id: &str) -> Result<Option<AgentIdentity>>;

    /// Get identity by agent wallet address
    fn get_by_wallet(&self, agent_wallet: &str) -> Result<Option<AgentIdentity>>;

    /// Update agent identity
    fn update(
        &self,
        agent_registry: &str,
        agent_id: &str,
        input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity>;

    /// Set or update agent wallet with proof metadata
    #[allow(clippy::too_many_arguments)]
    fn set_agent_wallet(
        &self,
        agent_registry: &str,
        agent_id: &str,
        agent_wallet: &str,
        proof_type: Option<AgentWalletProofType>,
        proof: Option<&str>,
        proof_chain_id: Option<u64>,
        proof_deadline: Option<DateTime<Utc>>,
    ) -> Result<AgentIdentity>;

    /// Clear agent wallet
    fn clear_agent_wallet(&self, agent_registry: &str, agent_id: &str) -> Result<AgentIdentity>;

    /// List identities with optional filtering
    fn list(&self, filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>>;

    /// Count identities matching filter
    fn count(&self, filter: AgentIdentityFilter) -> Result<u64>;

    /// Set identity metadata entry
    fn set_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        entry: AgentMetadataEntry,
    ) -> Result<()>;

    /// Get identity metadata entry
    fn get_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<Option<Vec<u8>>>;

    /// Delete identity metadata entry
    fn delete_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<()>;
}

// ============================================================================
// ERC-8004 Reputation Registry
// ============================================================================

/// Reputation feedback registry repository (ERC-8004)
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AgentReputationRepository: Send + Sync {
    /// Submit feedback for an agent
    fn give_feedback(&self, input: CreateAgentFeedback) -> Result<AgentFeedback>;

    /// Revoke previously submitted feedback
    fn revoke_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<AgentFeedback>;

    /// Read a specific feedback entry
    fn read_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<Option<AgentFeedback>>;

    /// Read feedback entries with filters
    fn read_all_feedback(&self, filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>>;

    /// Get feedback summary for an agent (filtered by client addresses + tags)
    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_addresses: Vec<String>,
        tag1: Option<String>,
        tag2: Option<String>,
    ) -> Result<FeedbackSummary>;

    /// Append a response to a feedback entry
    fn append_response(&self, input: CreateAgentFeedbackResponse) -> Result<AgentFeedbackResponse>;

    /// Count responses for a feedback entry
    fn get_response_count(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
        responders: Option<Vec<String>>,
    ) -> Result<u64>;

    /// List client addresses that have provided feedback
    fn get_clients(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>>;

    /// Get last feedback index for a client/agent pair
    fn get_last_index(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
    ) -> Result<u64>;
}

// ============================================================================
// ERC-8004 Validation Registry
// ============================================================================

/// Validation registry repository (ERC-8004)
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AgentValidationRepository: Send + Sync {
    /// Submit a validation request
    fn request_validation(
        &self,
        input: CreateAgentValidationRequest,
    ) -> Result<AgentValidationRequest>;

    /// Record a validation response for a request hash
    fn respond_validation(
        &self,
        request_hash: &str,
        input: CreateAgentValidationResponse,
    ) -> Result<AgentValidationResponse>;

    /// Get latest validation status for a request hash
    fn get_validation_status(&self, request_hash: &str) -> Result<Option<AgentValidationStatus>>;

    /// Get validation summary for an agent
    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummary>;

    /// Get all request hashes for an agent
    fn get_agent_validations(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>>;

    /// Get all request hashes for a validator
    fn get_validator_requests(&self, validator_address: &str) -> Result<Vec<String>>;
}

// ============================================================================
// A2A Commerce Repository
// ============================================================================

/// A2A (Agent-to-Agent) Commerce repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait A2ACommerceRepository: Send + Sync {
    // Quote operations
    /// Create a new quote
    fn create_quote(&self, input: CreateA2AQuote) -> Result<SkillQuote>;

    /// Get quote by ID
    fn get_quote(&self, id: Uuid) -> Result<Option<SkillQuote>>;

    /// Get quote by quote number
    fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<SkillQuote>>;

    /// Update quote status
    fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<SkillQuote>;

    /// List quotes with filter
    fn list_quotes(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>>;

    /// Count quotes matching filter
    fn count_quotes(&self, filter: SkillQuoteFilter) -> Result<u64>;

    // Purchase operations
    /// Create a new purchase
    fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase>;

    /// Get purchase by ID
    fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>>;

    /// Get purchase by purchase number
    fn get_purchase_by_number(&self, purchase_number: &str) -> Result<Option<A2APurchase>>;

    /// Update purchase status
    fn update_purchase_status(&self, id: Uuid, status: PurchaseStatus) -> Result<A2APurchase>;

    /// Link purchase to order
    fn link_purchase_to_order(&self, purchase_id: Uuid, order_id: Uuid) -> Result<A2APurchase>;

    /// Confirm delivery
    fn confirm_delivery(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase>;

    /// List purchases with filter
    fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>>;

    /// Count purchases matching filter
    fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64>;
}
