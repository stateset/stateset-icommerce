//! ERC-8004 Trustless Agents operations

use stateset_core::{
    AgentFeedback, AgentFeedbackFilter, AgentFeedbackResponse, AgentIdentity, AgentIdentityFilter,
    AgentMetadataEntry, AgentValidationRequest, AgentValidationResponse, AgentValidationStatus,
    CreateAgentFeedback, CreateAgentFeedbackResponse, CreateAgentIdentity,
    CreateAgentValidationRequest, CreateAgentValidationResponse, FeedbackSummary,
    Result, UpdateAgentIdentity, ValidationSummary, AgentWalletProofType,
};
use stateset_db::Database;
use std::sync::Arc;

/// ERC-8004 operations
pub struct Erc8004 {
    db: Arc<dyn Database>,
}

impl Erc8004 {
    /// Create a new ERC-8004 operations instance
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Identity Registry
    // ========================================================================

    /// Register a new agent identity
    pub fn register_identity(&self, input: CreateAgentIdentity) -> Result<AgentIdentity> {
        self.db.agent_identities().register(input)
    }

    /// Get agent identity by registry + ID
    pub fn get_identity(&self, agent_registry: &str, agent_id: &str) -> Result<Option<AgentIdentity>> {
        self.db.agent_identities().get(agent_registry, agent_id)
    }

    /// Get agent identity by agent wallet
    pub fn get_identity_by_wallet(&self, agent_wallet: &str) -> Result<Option<AgentIdentity>> {
        self.db.agent_identities().get_by_wallet(agent_wallet)
    }

    /// Update agent identity
    pub fn update_identity(
        &self,
        agent_registry: &str,
        agent_id: &str,
        input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity> {
        self.db.agent_identities().update(agent_registry, agent_id, input)
    }

    /// Set agent wallet with optional proof data
    pub fn set_agent_wallet(
        &self,
        agent_registry: &str,
        agent_id: &str,
        agent_wallet: &str,
        proof_type: Option<AgentWalletProofType>,
        proof: Option<&str>,
        proof_chain_id: Option<u64>,
        proof_deadline: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<AgentIdentity> {
        self.db.agent_identities().set_agent_wallet(
            agent_registry,
            agent_id,
            agent_wallet,
            proof_type,
            proof,
            proof_chain_id,
            proof_deadline,
        )
    }

    /// Clear agent wallet
    pub fn clear_agent_wallet(&self, agent_registry: &str, agent_id: &str) -> Result<AgentIdentity> {
        self.db.agent_identities().clear_agent_wallet(agent_registry, agent_id)
    }

    /// List identities with filter
    pub fn list_identities(&self, filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>> {
        self.db.agent_identities().list(filter)
    }

    /// Count identities with filter
    pub fn count_identities(&self, filter: AgentIdentityFilter) -> Result<u64> {
        self.db.agent_identities().count(filter)
    }

    /// Set identity metadata entry
    pub fn set_identity_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        entry: AgentMetadataEntry,
    ) -> Result<()> {
        self.db.agent_identities().set_metadata(agent_registry, agent_id, entry)
    }

    /// Get identity metadata entry
    pub fn get_identity_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<Option<Vec<u8>>> {
        self.db
            .agent_identities()
            .get_metadata(agent_registry, agent_id, metadata_key)
    }

    /// Delete identity metadata entry
    pub fn delete_identity_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<()> {
        self.db
            .agent_identities()
            .delete_metadata(agent_registry, agent_id, metadata_key)
    }

    // ========================================================================
    // Reputation Registry
    // ========================================================================

    /// Give feedback for an agent
    pub fn give_feedback(&self, input: CreateAgentFeedback) -> Result<AgentFeedback> {
        self.db.agent_reputation().give_feedback(input)
    }

    /// Revoke feedback
    pub fn revoke_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<AgentFeedback> {
        self.db
            .agent_reputation()
            .revoke_feedback(agent_registry, agent_id, client_address, feedback_index)
    }

    /// Read a feedback entry
    pub fn read_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<Option<AgentFeedback>> {
        self.db
            .agent_reputation()
            .read_feedback(agent_registry, agent_id, client_address, feedback_index)
    }

    /// Read all feedback entries with filter
    pub fn read_all_feedback(&self, filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>> {
        self.db.agent_reputation().read_all_feedback(filter)
    }

    /// Get feedback summary
    pub fn feedback_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_addresses: Vec<String>,
        tag1: Option<String>,
        tag2: Option<String>,
    ) -> Result<FeedbackSummary> {
        self.db
            .agent_reputation()
            .get_summary(agent_registry, agent_id, client_addresses, tag1, tag2)
    }

    /// Append a response to feedback
    pub fn append_feedback_response(
        &self,
        input: CreateAgentFeedbackResponse,
    ) -> Result<AgentFeedbackResponse> {
        self.db.agent_reputation().append_response(input)
    }

    /// Count responses for a feedback entry
    pub fn feedback_response_count(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
        responders: Option<Vec<String>>,
    ) -> Result<u64> {
        self.db.agent_reputation().get_response_count(
            agent_registry,
            agent_id,
            client_address,
            feedback_index,
            responders,
        )
    }

    /// Get feedback client addresses
    pub fn feedback_clients(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>> {
        self.db.agent_reputation().get_clients(agent_registry, agent_id)
    }

    /// Get last feedback index for client/agent pair
    pub fn last_feedback_index(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
    ) -> Result<u64> {
        self.db
            .agent_reputation()
            .get_last_index(agent_registry, agent_id, client_address)
    }

    // ========================================================================
    // Validation Registry
    // ========================================================================

    /// Submit a validation request
    pub fn request_validation(
        &self,
        input: CreateAgentValidationRequest,
    ) -> Result<AgentValidationRequest> {
        self.db.agent_validation().request_validation(input)
    }

    /// Record a validation response
    pub fn respond_validation(
        &self,
        request_hash: &str,
        input: CreateAgentValidationResponse,
    ) -> Result<AgentValidationResponse> {
        self.db.agent_validation().respond_validation(request_hash, input)
    }

    /// Get latest validation status
    pub fn validation_status(&self, request_hash: &str) -> Result<Option<AgentValidationStatus>> {
        self.db.agent_validation().get_validation_status(request_hash)
    }

    /// Get validation summary for an agent
    pub fn validation_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummary> {
        self.db
            .agent_validation()
            .get_summary(agent_registry, agent_id, validator_addresses, tag)
    }

    /// Get validation request hashes for an agent
    pub fn agent_validations(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>> {
        self.db.agent_validation().get_agent_validations(agent_registry, agent_id)
    }

    /// Get validation request hashes for a validator
    pub fn validator_requests(&self, validator_address: &str) -> Result<Vec<String>> {
        self.db.agent_validation().get_validator_requests(validator_address)
    }
}
