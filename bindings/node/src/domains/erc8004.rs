//! ERC-8004 Trustless Agents (identity / reputation / validation).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// ERC-8004 Trustless Agents (identity / reputation / validation)
// ============================================================================

pub(crate) fn parse_wallet_proof_type(s: &str) -> Result<stateset_core::AgentWalletProofType> {
    s.parse::<stateset_core::AgentWalletProofType>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid agent wallet proof type: {s}")))
}

pub(crate) fn parse_i128_str(s: &str, field: &str) -> Result<i128> {
    s.parse::<i128>().map_err(|_| {
        coded(ErrCode::Validation, format!("Invalid {field}: expected integer string"))
    })
}

pub(crate) fn parse_u8_field(value: u32, field: &str) -> Result<u8> {
    u8::try_from(value)
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid {field}: out of range")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentIdentityInput {
    pub agent_registry: String,
    pub agent_id: String,
    pub agent_uri: String,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    /// JSON-encoded registration document
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    /// Snake-case proof type
    #[napi(ts_type = "AgentWalletProofTypeInput")]
    pub wallet_proof_type: Option<String>,
    pub wallet_proof: Option<String>,
    /// Chain id as a decimal string
    pub wallet_proof_chain_id: Option<String>,
    /// RFC3339 timestamp
    pub wallet_proof_deadline: Option<String>,
    pub active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateAgentIdentityInput {
    pub agent_uri: Option<String>,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    /// JSON-encoded registration document
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    /// Snake-case proof type
    #[napi(ts_type = "AgentWalletProofTypeInput")]
    pub wallet_proof_type: Option<String>,
    pub wallet_proof: Option<String>,
    /// Chain id as a decimal string
    pub wallet_proof_chain_id: Option<String>,
    /// RFC3339 timestamp
    pub wallet_proof_deadline: Option<String>,
    pub active: Option<bool>,
}

/// Optional on-chain proof data accompanying a wallet binding.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AgentWalletProofInput {
    /// Snake-case proof type
    #[napi(ts_type = "AgentWalletProofTypeInput")]
    pub proof_type: Option<String>,
    pub proof: Option<String>,
    /// Chain id as a decimal string
    pub proof_chain_id: Option<String>,
    /// RFC3339 timestamp
    pub proof_deadline: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentIdentityFilterInput {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentIdentityOutput {
    pub id: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub agent_uri: String,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    /// JSON-encoded registration document
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    /// Snake-case proof type
    #[napi(ts_type = "AgentWalletProofType")]
    pub wallet_proof_type: Option<String>,
    pub wallet_proof: Option<String>,
    /// Chain id as a decimal string
    pub wallet_proof_chain_id: Option<String>,
    pub wallet_proof_deadline: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::AgentIdentity> for AgentIdentityOutput {
    fn from(i: stateset_core::AgentIdentity) -> Self {
        Self {
            id: i.id.to_string(),
            agent_registry: i.agent_registry,
            agent_id: i.agent_id,
            agent_uri: i.agent_uri,
            agent_wallet: i.agent_wallet,
            owner_address: i.owner_address,
            agent_card_id: i.agent_card_id.map(|id| id.to_string()),
            registration: i.registration,
            registration_hash: i.registration_hash,
            wallet_proof_type: i.wallet_proof_type.map(|t| t.to_string()),
            wallet_proof: i.wallet_proof,
            wallet_proof_chain_id: i.wallet_proof_chain_id.map(|c| c.to_string()),
            wallet_proof_deadline: i.wallet_proof_deadline.map(|d| d.to_rfc3339()),
            active: i.active,
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentFeedbackInput {
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    /// Signed integer value as a decimal string
    pub value: String,
    /// Number of decimal places encoded in `value`
    pub value_decimals: u32,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub feedback_hash: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentFeedbackFilterInput {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub client_addresses: Option<Vec<String>>,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub include_revoked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentFeedbackOutput {
    pub id: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    /// Feedback index as a decimal string
    pub feedback_index: String,
    /// Signed integer value as a decimal string
    pub value: String,
    pub value_decimals: u32,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub feedback_hash: Option<String>,
    pub is_revoked: bool,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

impl From<stateset_core::AgentFeedback> for AgentFeedbackOutput {
    fn from(f: stateset_core::AgentFeedback) -> Self {
        Self {
            id: f.id.to_string(),
            agent_registry: f.agent_registry,
            agent_id: f.agent_id,
            client_address: f.client_address,
            feedback_index: f.feedback_index.to_string(),
            value: f.value.to_string(),
            value_decimals: u32::from(f.value_decimals),
            tag1: f.tag1,
            tag2: f.tag2,
            endpoint: f.endpoint,
            feedback_uri: f.feedback_uri,
            feedback_hash: f.feedback_hash,
            is_revoked: f.is_revoked,
            created_at: f.created_at.to_rfc3339(),
            revoked_at: f.revoked_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FeedbackSummaryOutput {
    /// Count as a decimal string
    pub count: String,
    /// Aggregate value as a decimal string
    pub summary_value: String,
    pub summary_value_decimals: u32,
}

impl From<stateset_core::FeedbackSummary> for FeedbackSummaryOutput {
    fn from(s: stateset_core::FeedbackSummary) -> Self {
        Self {
            count: s.count.to_string(),
            summary_value: s.summary_value.to_string(),
            summary_value_decimals: u32::from(s.summary_value_decimals),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentValidationRequestInput {
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub request_uri: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentValidationRequestOutput {
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub request_uri: String,
    pub created_at: String,
}

impl From<stateset_core::AgentValidationRequest> for AgentValidationRequestOutput {
    fn from(r: stateset_core::AgentValidationRequest) -> Self {
        Self {
            request_hash: r.request_hash,
            agent_registry: r.agent_registry,
            agent_id: r.agent_id,
            validator_address: r.validator_address,
            request_uri: r.request_uri,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentValidationResponseInput {
    /// Validation score (0-100)
    pub response: u32,
    pub response_uri: Option<String>,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentValidationResponseOutput {
    pub id: String,
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub response: u32,
    pub response_uri: Option<String>,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::AgentValidationResponse> for AgentValidationResponseOutput {
    fn from(r: stateset_core::AgentValidationResponse) -> Self {
        Self {
            id: r.id.to_string(),
            request_hash: r.request_hash,
            agent_registry: r.agent_registry,
            agent_id: r.agent_id,
            validator_address: r.validator_address,
            response: u32::from(r.response),
            response_uri: r.response_uri,
            response_hash: r.response_hash,
            tag: r.tag,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentValidationStatusOutput {
    pub validator_address: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub response: u32,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
    pub last_update: String,
}

impl From<stateset_core::AgentValidationStatus> for AgentValidationStatusOutput {
    fn from(s: stateset_core::AgentValidationStatus) -> Self {
        Self {
            validator_address: s.validator_address,
            agent_registry: s.agent_registry,
            agent_id: s.agent_id,
            response: u32::from(s.response),
            response_hash: s.response_hash,
            tag: s.tag,
            last_update: s.last_update.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ValidationSummaryOutput {
    /// Count as a decimal string
    pub count: String,
    pub average_response: u32,
}

impl From<stateset_core::ValidationSummary> for ValidationSummaryOutput {
    fn from(s: stateset_core::ValidationSummary) -> Self {
        Self { count: s.count.to_string(), average_response: u32::from(s.average_response) }
    }
}

#[napi]
pub struct Erc8004 {
    pub(crate) commerce: Handle,
}

#[napi]
impl Erc8004 {
    // ---- Identity registry ----

    /// Register a new agent identity.
    #[napi]
    pub async fn register_identity(
        &self,
        input: CreateAgentIdentityInput,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.get()?;
        let identity = commerce
            .erc8004()
            .register_identity(stateset_core::CreateAgentIdentity {
                agent_registry: input.agent_registry,
                agent_id: input.agent_id,
                agent_uri: input.agent_uri,
                agent_wallet: input.agent_wallet,
                owner_address: input.owner_address,
                agent_card_id: parse_optional_uuid(input.agent_card_id, "agent_card_id")?,
                registration: input.registration,
                registration_hash: input.registration_hash,
                wallet_proof_type: input
                    .wallet_proof_type
                    .as_deref()
                    .map(parse_wallet_proof_type)
                    .transpose()?,
                wallet_proof: input.wallet_proof,
                wallet_proof_chain_id: input
                    .wallet_proof_chain_id
                    .as_deref()
                    .map(|c| parse_u64_str(c, "wallet_proof_chain_id"))
                    .transpose()?,
                wallet_proof_deadline: parse_rfc3339_opt(
                    input.wallet_proof_deadline,
                    "wallet_proof_deadline",
                )?,
                active: input.active,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to register agent identity", e))?;
        Ok(identity.into())
    }

    #[napi]
    pub async fn get_identity(
        &self,
        agent_registry: String,
        agent_id: String,
    ) -> Result<Option<AgentIdentityOutput>> {
        let commerce = self.commerce.get()?;
        let identity = commerce
            .erc8004()
            .get_identity(&agent_registry, &agent_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get agent identity", e))?;
        Ok(identity.map(Into::into))
    }

    #[napi]
    pub async fn get_identity_by_wallet(
        &self,
        agent_wallet: String,
    ) -> Result<Option<AgentIdentityOutput>> {
        let commerce = self.commerce.get()?;
        let identity = commerce
            .erc8004()
            .get_identity_by_wallet(&agent_wallet)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get agent identity by wallet", e))?;
        Ok(identity.map(Into::into))
    }

    #[napi]
    pub async fn update_identity(
        &self,
        agent_registry: String,
        agent_id: String,
        input: UpdateAgentIdentityInput,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.get()?;
        let identity = commerce
            .erc8004()
            .update_identity(
                &agent_registry,
                &agent_id,
                stateset_core::UpdateAgentIdentity {
                    agent_uri: input.agent_uri,
                    agent_wallet: input.agent_wallet,
                    owner_address: input.owner_address,
                    agent_card_id: parse_optional_uuid(input.agent_card_id, "agent_card_id")?,
                    registration: input.registration,
                    registration_hash: input.registration_hash,
                    wallet_proof_type: input
                        .wallet_proof_type
                        .as_deref()
                        .map(parse_wallet_proof_type)
                        .transpose()?,
                    wallet_proof: input.wallet_proof,
                    wallet_proof_chain_id: input
                        .wallet_proof_chain_id
                        .as_deref()
                        .map(|c| parse_u64_str(c, "wallet_proof_chain_id"))
                        .transpose()?,
                    wallet_proof_deadline: parse_rfc3339_opt(
                        input.wallet_proof_deadline,
                        "wallet_proof_deadline",
                    )?,
                    active: input.active,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update agent identity", e))?;
        Ok(identity.into())
    }

    /// Bind a wallet to an agent identity, with optional on-chain proof data.
    #[napi]
    pub async fn set_agent_wallet(
        &self,
        agent_registry: String,
        agent_id: String,
        agent_wallet: String,
        proof: Option<AgentWalletProofInput>,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.get()?;
        let proof = proof.unwrap_or_default();
        let identity = commerce
            .erc8004()
            .set_agent_wallet(
                &agent_registry,
                &agent_id,
                &agent_wallet,
                proof.proof_type.as_deref().map(parse_wallet_proof_type).transpose()?,
                proof.proof.as_deref(),
                proof
                    .proof_chain_id
                    .as_deref()
                    .map(|c| parse_u64_str(c, "proof_chain_id"))
                    .transpose()?,
                parse_rfc3339_opt(proof.proof_deadline, "proof_deadline")?,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set agent wallet", e))?;
        Ok(identity.into())
    }

    /// Clear the wallet binding on an agent identity.
    #[napi]
    pub async fn clear_agent_wallet(
        &self,
        agent_registry: String,
        agent_id: String,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.get()?;
        let identity = commerce
            .erc8004()
            .clear_agent_wallet(&agent_registry, &agent_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to clear agent wallet", e))?;
        Ok(identity.into())
    }

    #[napi]
    pub async fn list_identities(
        &self,
        filter: Option<AgentIdentityFilterInput>,
    ) -> Result<Vec<AgentIdentityOutput>> {
        let commerce = self.commerce.get()?;
        let filter = build_agent_identity_filter(filter)?;
        let identities = commerce
            .erc8004()
            .list_identities(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list agent identities", e))?;
        Ok(identities.into_iter().map(Into::into).collect())
    }

    /// Count identities matching a filter (returned as a decimal string).
    #[napi]
    pub async fn count_identities(
        &self,
        filter: Option<AgentIdentityFilterInput>,
    ) -> Result<String> {
        let commerce = self.commerce.get()?;
        let filter = build_agent_identity_filter(filter)?;
        let count = commerce
            .erc8004()
            .count_identities(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count agent identities", e))?;
        Ok(count.to_string())
    }

    // ---- Reputation registry ----

    /// Give feedback about an agent.
    #[napi]
    pub async fn give_feedback(
        &self,
        input: CreateAgentFeedbackInput,
    ) -> Result<AgentFeedbackOutput> {
        let commerce = self.commerce.get()?;
        let feedback = commerce
            .erc8004()
            .give_feedback(stateset_core::CreateAgentFeedback {
                agent_registry: input.agent_registry,
                agent_id: input.agent_id,
                client_address: input.client_address,
                value: parse_i128_str(&input.value, "value")?,
                value_decimals: parse_u8_field(input.value_decimals, "value_decimals")?,
                tag1: input.tag1,
                tag2: input.tag2,
                endpoint: input.endpoint,
                feedback_uri: input.feedback_uri,
                feedback_hash: input.feedback_hash,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to give agent feedback", e))?;
        Ok(feedback.into())
    }

    /// Revoke a previously given feedback entry.
    #[napi]
    pub async fn revoke_feedback(
        &self,
        agent_registry: String,
        agent_id: String,
        client_address: String,
        feedback_index: String,
    ) -> Result<AgentFeedbackOutput> {
        let commerce = self.commerce.get()?;
        let feedback = commerce
            .erc8004()
            .revoke_feedback(
                &agent_registry,
                &agent_id,
                &client_address,
                parse_u64_str(&feedback_index, "feedback_index")?,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to revoke agent feedback", e))?;
        Ok(feedback.into())
    }

    #[napi]
    pub async fn read_feedback(
        &self,
        agent_registry: String,
        agent_id: String,
        client_address: String,
        feedback_index: String,
    ) -> Result<Option<AgentFeedbackOutput>> {
        let commerce = self.commerce.get()?;
        let feedback = commerce
            .erc8004()
            .read_feedback(
                &agent_registry,
                &agent_id,
                &client_address,
                parse_u64_str(&feedback_index, "feedback_index")?,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to read agent feedback", e))?;
        Ok(feedback.map(Into::into))
    }

    #[napi]
    pub async fn read_all_feedback(
        &self,
        filter: Option<AgentFeedbackFilterInput>,
    ) -> Result<Vec<AgentFeedbackOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::AgentFeedbackFilter::default, |f| {
            stateset_core::AgentFeedbackFilter {
                agent_registry: f.agent_registry,
                agent_id: f.agent_id,
                client_addresses: f.client_addresses,
                tag1: f.tag1,
                tag2: f.tag2,
                include_revoked: f.include_revoked,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let feedback = commerce
            .erc8004()
            .read_all_feedback(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to read agent feedback", e))?;
        Ok(feedback.into_iter().map(Into::into).collect())
    }

    /// Aggregate feedback summary for an agent.
    #[napi]
    pub async fn feedback_summary(
        &self,
        agent_registry: String,
        agent_id: String,
        client_addresses: Option<Vec<String>>,
        tag1: Option<String>,
        tag2: Option<String>,
    ) -> Result<FeedbackSummaryOutput> {
        let commerce = self.commerce.get()?;
        let summary = commerce
            .erc8004()
            .feedback_summary(
                &agent_registry,
                &agent_id,
                client_addresses.unwrap_or_default(),
                tag1,
                tag2,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get feedback summary", e))?;
        Ok(summary.into())
    }

    // ---- Validation registry ----

    /// Submit a validation request for an agent.
    #[napi]
    pub async fn request_validation(
        &self,
        input: CreateAgentValidationRequestInput,
    ) -> Result<AgentValidationRequestOutput> {
        let commerce = self.commerce.get()?;
        let request = commerce
            .erc8004()
            .request_validation(stateset_core::CreateAgentValidationRequest {
                request_hash: input.request_hash,
                agent_registry: input.agent_registry,
                agent_id: input.agent_id,
                validator_address: input.validator_address,
                request_uri: input.request_uri,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to request validation", e))?;
        Ok(request.into())
    }

    /// Record a validator's response to a validation request.
    #[napi]
    pub async fn respond_validation(
        &self,
        request_hash: String,
        input: CreateAgentValidationResponseInput,
    ) -> Result<AgentValidationResponseOutput> {
        let commerce = self.commerce.get()?;
        let response = commerce
            .erc8004()
            .respond_validation(
                &request_hash,
                stateset_core::CreateAgentValidationResponse {
                    response: parse_u8_field(input.response, "response")?,
                    response_uri: input.response_uri,
                    response_hash: input.response_hash,
                    tag: input.tag,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to respond to validation", e))?;
        Ok(response.into())
    }

    #[napi]
    pub async fn validation_status(
        &self,
        request_hash: String,
    ) -> Result<Option<AgentValidationStatusOutput>> {
        let commerce = self.commerce.get()?;
        let status = commerce
            .erc8004()
            .validation_status(&request_hash)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get validation status", e))?;
        Ok(status.map(Into::into))
    }

    /// Aggregate validation summary for an agent.
    #[napi]
    pub async fn validation_summary(
        &self,
        agent_registry: String,
        agent_id: String,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummaryOutput> {
        let commerce = self.commerce.get()?;
        let summary = commerce
            .erc8004()
            .validation_summary(&agent_registry, &agent_id, validator_addresses, tag)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get validation summary", e))?;
        Ok(summary.into())
    }
}

pub(crate) fn build_agent_identity_filter(
    filter: Option<AgentIdentityFilterInput>,
) -> Result<stateset_core::AgentIdentityFilter> {
    filter.map_or_else(
        || Ok(stateset_core::AgentIdentityFilter::default()),
        |f| -> Result<stateset_core::AgentIdentityFilter> {
            Ok(stateset_core::AgentIdentityFilter {
                agent_registry: f.agent_registry,
                agent_id: f.agent_id,
                agent_wallet: f.agent_wallet,
                owner_address: f.owner_address,
                agent_card_id: parse_optional_uuid(f.agent_card_id, "agent_card_id")?,
                active: f.active,
                limit: f.limit,
                offset: f.offset,
            })
        },
    )
}

// ---------------------------------------------------------------------------
// Maintenance (backup / restore / structured export & import)
// ---------------------------------------------------------------------------

/// Manifest written alongside a database backup.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct BackupManifestOutput {
    pub manifest_version: u32,
    pub schema_version: String,
    pub migration_count: u32,
    pub engine_version: String,
    pub created_at: String,
    pub source_path: String,
    pub size_bytes: i64,
    pub checksum: String,
}

/// Result of a database backup.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct BackupReportOutput {
    pub backup_path: String,
    pub manifest_path: String,
    pub manifest: BackupManifestOutput,
}

/// Options controlling a restore.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RestoreOptionsInput {
    pub overwrite: Option<bool>,
    pub skip_checksum: Option<bool>,
    pub allow_newer_schema: Option<bool>,
}

/// Result of a database restore.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RestoreReportOutput {
    pub target_path: String,
    pub schema_version: String,
    pub size_bytes: i64,
    pub checksum_verified: bool,
    pub replaced_existing: bool,
}

/// Per-domain record count.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct DomainCountOutput {
    pub domain: String,
    pub count: u32,
}

/// Options controlling a structured export.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ExportOptionsInput {
    pub domains: Option<Vec<String>>,
    pub page_size: Option<u32>,
    pub pretty: Option<bool>,
}

/// Result of a structured export.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ExportReportOutput {
    pub counts: Vec<DomainCountOutput>,
    pub total: u32,
}

/// Options controlling a structured import.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ImportOptionsInput {
    pub domains: Option<Vec<String>>,
    /// `skip` (default) or `fail`.
    #[napi(ts_type = "ImportConflictPolicy")]
    pub on_conflict: Option<String>,
    pub dry_run: Option<bool>,
}

/// Result of a structured import.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ImportReportOutput {
    pub created: Vec<DomainCountOutput>,
    pub skipped: Vec<DomainCountOutput>,
    pub unsupported_domains: Vec<String>,
    pub total_created: u32,
}

/// Domains that structured export/import can cover.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct PortableDomainsOutput {
    pub exportable: Vec<String>,
    pub importable: Vec<String>,
}

pub(crate) fn domain_counts(counts: Vec<(String, usize)>) -> Vec<DomainCountOutput> {
    counts
        .into_iter()
        .map(|(domain, count)| DomainCountOutput { domain, count: count as u32 })
        .collect()
}

#[napi]
pub struct Maintenance {
    pub(crate) commerce: Handle,
}

#[napi]
impl Maintenance {
    /// Whether file-level backup and restore are available on this instance.
    #[napi]
    pub async fn supports_backup(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.maintenance().supports_backup())
    }

    /// Alias of `supportsBackup`, matching the other accessor modules.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.maintenance().supports_backup())
    }

    /// Take a consistent backup to `backupPath`, writing a sidecar manifest.
    #[napi]
    pub async fn backup(&self, backup_path: String) -> Result<BackupReportOutput> {
        let commerce = self.commerce.get()?;
        let report = commerce
            .maintenance()
            .backup_to(&backup_path)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to back up database", e))?;
        let m = report.manifest;
        Ok(BackupReportOutput {
            backup_path: report.backup_path.display().to_string(),
            manifest_path: report.manifest_path.display().to_string(),
            manifest: BackupManifestOutput {
                manifest_version: m.manifest_version,
                schema_version: m.schema_version,
                migration_count: m.migration_count as u32,
                engine_version: m.engine_version,
                created_at: m.created_at.to_rfc3339(),
                source_path: m.source_path,
                size_bytes: m.size_bytes as i64,
                checksum: m.checksum,
            },
        })
    }

    /// Alias of `backup`.
    #[napi]
    pub async fn backup_to(&self, backup_path: String) -> Result<BackupReportOutput> {
        self.backup(backup_path).await
    }

    /// Restore a backup to `targetPath`.
    #[napi]
    pub async fn restore(
        &self,
        backup_path: String,
        target_path: String,
        options: Option<RestoreOptionsInput>,
    ) -> Result<RestoreReportOutput> {
        let commerce = self.commerce.get()?;
        let opts = options.unwrap_or(RestoreOptionsInput {
            overwrite: None,
            skip_checksum: None,
            allow_newer_schema: None,
        });
        let restore_options = stateset_embedded::maintenance::RestoreOptions {
            overwrite: opts.overwrite.unwrap_or(false),
            skip_checksum: opts.skip_checksum.unwrap_or(false),
            allow_newer_schema: opts.allow_newer_schema.unwrap_or(false),
        };
        let report = commerce
            .maintenance()
            .restore_from(&backup_path, &target_path, &restore_options)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to restore database", e))?;
        Ok(RestoreReportOutput {
            target_path: report.target_path.display().to_string(),
            schema_version: report.schema_version,
            size_bytes: report.size_bytes as i64,
            checksum_verified: report.checksum_verified,
            replaced_existing: report.replaced_existing,
        })
    }

    /// Alias of `restore`.
    #[napi]
    pub async fn restore_from(
        &self,
        backup_path: String,
        target_path: String,
        options: Option<RestoreOptionsInput>,
    ) -> Result<RestoreReportOutput> {
        self.restore(backup_path, target_path, options).await
    }

    /// Write a structured JSON export to `path`.
    #[napi]
    pub async fn export(
        &self,
        path: String,
        options: Option<ExportOptionsInput>,
    ) -> Result<ExportReportOutput> {
        let commerce = self.commerce.get()?;
        let mut export_options = stateset_embedded::maintenance::ExportOptions::default();
        if let Some(o) = options {
            if let Some(domains) = o.domains {
                export_options.domains = domains;
            }
            if let Some(page_size) = o.page_size.filter(|p| *p > 0) {
                export_options.page_size = page_size;
            }
            if let Some(pretty) = o.pretty {
                export_options.pretty = pretty;
            }
        }
        let report = commerce
            .maintenance()
            .export_to_file_with(&path, &export_options)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to export data", e))?;
        Ok(ExportReportOutput { total: report.total as u32, counts: domain_counts(report.counts) })
    }

    /// Alias of `export`.
    #[napi]
    pub async fn export_to_file(
        &self,
        path: String,
        options: Option<ExportOptionsInput>,
    ) -> Result<ExportReportOutput> {
        self.export(path, options).await
    }

    /// Read a structured JSON export from `path` and replay it.
    #[napi]
    pub async fn import(
        &self,
        path: String,
        options: Option<ImportOptionsInput>,
    ) -> Result<ImportReportOutput> {
        let commerce = self.commerce.get()?;
        let mut import_options = stateset_embedded::maintenance::ImportOptions::default();
        if let Some(o) = options {
            if let Some(domains) = o.domains {
                import_options.domains = domains;
            }
            if let Some(policy) = o.on_conflict {
                import_options.on_conflict = match policy.as_str() {
                    "skip" => stateset_embedded::maintenance::ConflictPolicy::Skip,
                    "fail" => stateset_embedded::maintenance::ConflictPolicy::Fail,
                    other => {
                        return Err(coded(
                            ErrCode::Validation,
                            format!("Invalid onConflict '{}': expected 'skip' or 'fail'", other),
                        ));
                    }
                };
            }
            if let Some(dry_run) = o.dry_run {
                import_options.dry_run = dry_run;
            }
        }
        let report = commerce
            .maintenance()
            .import_from_file(&path, &import_options)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to import data", e))?;
        Ok(ImportReportOutput {
            created: domain_counts(report.created),
            skipped: domain_counts(report.skipped),
            unsupported_domains: report.unsupported_domains,
            total_created: report.total_created as u32,
        })
    }

    /// Alias of `import`.
    #[napi]
    pub async fn import_from_file(
        &self,
        path: String,
        options: Option<ImportOptionsInput>,
    ) -> Result<ImportReportOutput> {
        self.import(path, options).await
    }

    /// Domains the structured export covers, in export order.
    #[napi]
    pub async fn exportable_domains(&self) -> Result<Vec<String>> {
        let commerce = self.commerce.get()?;
        Ok(commerce.maintenance().exportable_domains().into_iter().map(ToOwned::to_owned).collect())
    }

    /// Domains the structured import can write.
    #[napi]
    pub async fn importable_domains(&self) -> Result<Vec<String>> {
        let commerce = self.commerce.get()?;
        Ok(commerce.maintenance().importable_domains().into_iter().map(ToOwned::to_owned).collect())
    }

    /// Both portable domain lists in one call.
    #[napi]
    pub async fn list_portable_domains(&self) -> Result<PortableDomainsOutput> {
        let commerce = self.commerce.get()?;
        let maintenance = commerce.maintenance();
        Ok(PortableDomainsOutput {
            exportable: maintenance
                .exportable_domains()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            importable: maintenance
                .importable_domains()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
        })
    }
}
