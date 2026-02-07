//! ERC-8004 Trustless Agents models
//!
//! Provides identity, reputation, and validation data structures for
//! trustless agent discovery across organizational boundaries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ERC-8004 registration file type URL (v1)
pub const ERC8004_REGISTRATION_V1: &str = "https://eips.ethereum.org/EIPS/eip-8004#registration-v1";

// =============================================================================
// Registration File Models
// =============================================================================

/// Service endpoint advertised by an agent registration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentServiceEndpoint {
    pub name: String,
    pub endpoint: String,
    pub version: Option<String>,
    pub skills: Option<Vec<String>>,
    pub domains: Option<Vec<String>>,
}

/// Registration reference entry in the registration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistrationRef {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "agentRegistry")]
    pub agent_registry: String,
}

/// Agent registration file (off-chain JSON referenced by agentURI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistrationFile {
    #[serde(rename = "type")]
    pub type_url: String,
    pub name: String,
    pub description: String,
    pub image: String,
    pub services: Vec<AgentServiceEndpoint>,
    #[serde(rename = "x402Support")]
    pub x402_support: bool,
    pub active: bool,
    pub registrations: Vec<AgentRegistrationRef>,
    #[serde(default, rename = "supportedTrust")]
    pub supported_trust: Option<Vec<String>>,
}

// =============================================================================
// Identity Registry Models
// =============================================================================

/// Type of proof used to set or update agent wallet ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWalletProofType {
    Eip712,
    Erc1271,
}

impl std::fmt::Display for AgentWalletProofType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eip712 => write!(f, "eip712"),
            Self::Erc1271 => write!(f, "erc1271"),
        }
    }
}

impl std::str::FromStr for AgentWalletProofType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eip712" | "eip_712" => Ok(Self::Eip712),
            "erc1271" | "erc_1271" => Ok(Self::Erc1271),
            _ => Err(format!("Unknown agent wallet proof type: {}", s)),
        }
    }
}

/// Identity record for an on-chain ERC-8004 agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub id: Uuid,
    pub agent_registry: String,
    pub agent_id: String,
    pub agent_uri: String,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<Uuid>,
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    pub wallet_proof_type: Option<AgentWalletProofType>,
    pub wallet_proof: Option<String>,
    pub wallet_proof_chain_id: Option<u64>,
    pub wallet_proof_deadline: Option<DateTime<Utc>>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Metadata entry for on-chain identity metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadataEntry {
    pub metadata_key: String,
    pub metadata_value: Vec<u8>,
}

/// Input for registering a new agent identity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAgentIdentity {
    pub agent_registry: String,
    pub agent_id: String,
    pub agent_uri: String,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<Uuid>,
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    pub wallet_proof_type: Option<AgentWalletProofType>,
    pub wallet_proof: Option<String>,
    pub wallet_proof_chain_id: Option<u64>,
    pub wallet_proof_deadline: Option<DateTime<Utc>>,
    pub active: Option<bool>,
}

/// Input for updating an agent identity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAgentIdentity {
    pub agent_uri: Option<String>,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<Uuid>,
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    pub wallet_proof_type: Option<AgentWalletProofType>,
    pub wallet_proof: Option<String>,
    pub wallet_proof_chain_id: Option<u64>,
    pub wallet_proof_deadline: Option<DateTime<Utc>>,
    pub active: Option<bool>,
}

/// Filter for listing agent identities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentIdentityFilter {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<Uuid>,
    pub active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// =============================================================================
// Reputation Registry Models
// =============================================================================

/// Reputation feedback submitted by a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFeedback {
    pub id: Uuid,
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    pub feedback_index: u64,
    pub value: i128,
    pub value_decimals: u8,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub feedback_hash: Option<String>,
    pub is_revoked: bool,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Input for creating new feedback.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAgentFeedback {
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    pub value: i128,
    pub value_decimals: u8,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub feedback_hash: Option<String>,
}

/// Filter for reading feedback.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentFeedbackFilter {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub client_addresses: Option<Vec<String>>,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub include_revoked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Summary for feedback aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackSummary {
    pub count: u64,
    pub summary_value: i128,
    pub summary_value_decimals: u8,
}

/// Feedback response appended by an agent or third party.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFeedbackResponse {
    pub id: Uuid,
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    pub feedback_index: u64,
    pub responder_address: String,
    pub response_uri: String,
    pub response_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Input for appending a feedback response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAgentFeedbackResponse {
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    pub feedback_index: u64,
    pub responder_address: String,
    pub response_uri: String,
    pub response_hash: Option<String>,
}

// =============================================================================
// Validation Registry Models
// =============================================================================

/// Validation request submitted by an agent owner/operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentValidationRequest {
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub request_uri: String,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a validation request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAgentValidationRequest {
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub request_uri: String,
}

/// Validation response from a validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentValidationResponse {
    pub id: Uuid,
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub response: u8,
    pub response_uri: Option<String>,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Input for recording a validation response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAgentValidationResponse {
    pub response: u8,
    pub response_uri: Option<String>,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
}

/// Latest validation status for a request hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentValidationStatus {
    pub validator_address: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub response: u8,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
    pub last_update: DateTime<Utc>,
}

/// Summary of validation responses for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub count: u64,
    pub average_response: u8,
}

/// Filter for validation summaries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentValidationFilter {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub validator_addresses: Option<Vec<String>>,
    pub tag: Option<String>,
}
