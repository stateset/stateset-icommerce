use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use stateset_core::{
    AgentFeedback, AgentFeedbackFilter, AgentFeedbackResponse, AgentIdentity, AgentIdentityFilter,
    AgentMetadataEntry, AgentValidationRequest, AgentValidationResponse, AgentValidationStatus,
    AgentWalletProofType, CreateAgentFeedback, CreateAgentFeedbackResponse, CreateAgentIdentity,
    CreateAgentValidationRequest, CreateAgentValidationResponse, FeedbackSummary,
    UpdateAgentIdentity, ValidationSummary,
};

use crate::{ApiError, ApiResult, ApiState};

#[derive(Debug, Deserialize)]
pub struct SetAgentWalletRequest {
    pub agent_wallet: String,
    pub proof_type: Option<AgentWalletProofType>,
    pub proof: Option<String>,
    pub proof_chain_id: Option<u64>,
    pub proof_deadline: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataRequest {
    pub value: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct RevokeFeedbackRequest {
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    pub feedback_index: u64,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackListQuery {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub client_addresses: Option<String>,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub include_revoked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackSummaryQuery {
    pub agent_registry: String,
    pub agent_id: String,
    pub client_addresses: String,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ValidationSummaryQuery {
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_addresses: Option<String>,
    pub tag: Option<String>,
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// ============================================================================
// Identity
// ============================================================================

pub async fn create_identity(
    State(state): State<ApiState>,
    Json(input): Json<CreateAgentIdentity>,
) -> ApiResult<Json<AgentIdentity>> {
    let identity = state.commerce.erc8004().register_identity(input)?;
    Ok(Json(identity))
}

pub async fn list_identities(
    State(state): State<ApiState>,
    Query(filter): Query<AgentIdentityFilter>,
) -> ApiResult<Json<Vec<AgentIdentity>>> {
    let identities = state.commerce.erc8004().list_identities(filter)?;
    Ok(Json(identities))
}

pub async fn get_identity(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id)): Path<(String, String)>,
) -> ApiResult<Json<AgentIdentity>> {
    let identity = state
        .commerce
        .erc8004()
        .get_identity(&agent_registry, &agent_id)?
        .ok_or_else(|| ApiError::not_found(\"agent identity not found\"))?;
    Ok(Json(identity))
}

pub async fn update_identity(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id)): Path<(String, String)>,
    Json(input): Json<UpdateAgentIdentity>,
) -> ApiResult<Json<AgentIdentity>> {
    let identity = state
        .commerce
        .erc8004()
        .update_identity(&agent_registry, &agent_id, input)?;
    Ok(Json(identity))
}

pub async fn set_agent_wallet(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id)): Path<(String, String)>,
    Json(input): Json<SetAgentWalletRequest>,
) -> ApiResult<Json<AgentIdentity>> {
    let identity = state.commerce.erc8004().set_agent_wallet(
        &agent_registry,
        &agent_id,
        &input.agent_wallet,
        input.proof_type,
        input.proof.as_deref(),
        input.proof_chain_id,
        input.proof_deadline,
    )?;
    Ok(Json(identity))
}

pub async fn clear_agent_wallet(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id)): Path<(String, String)>,
) -> ApiResult<Json<AgentIdentity>> {
    let identity = state
        .commerce
        .erc8004()
        .clear_agent_wallet(&agent_registry, &agent_id)?;
    Ok(Json(identity))
}

pub async fn set_identity_metadata(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id, metadata_key)): Path<(String, String, String)>,
    Json(input): Json<MetadataRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    state.commerce.erc8004().set_identity_metadata(
        &agent_registry,
        &agent_id,
        AgentMetadataEntry {
            metadata_key,
            metadata_value: input.value,
        },
    )?;
    Ok(Json(serde_json::json!({"status": "ok"})))
}

pub async fn get_identity_metadata(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id, metadata_key)): Path<(String, String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let value = state
        .commerce
        .erc8004()
        .get_identity_metadata(&agent_registry, &agent_id, &metadata_key)?
        .unwrap_or_default();
    Ok(Json(serde_json::json!({"value": value})))
}

pub async fn delete_identity_metadata(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id, metadata_key)): Path<(String, String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    state
        .commerce
        .erc8004()
        .delete_identity_metadata(&agent_registry, &agent_id, &metadata_key)?;
    Ok(Json(serde_json::json!({"status": "ok"})))
}

// ============================================================================
// Reputation
// ============================================================================

pub async fn give_feedback(
    State(state): State<ApiState>,
    Json(input): Json<CreateAgentFeedback>,
) -> ApiResult<Json<AgentFeedback>> {
    let feedback = state.commerce.erc8004().give_feedback(input)?;
    Ok(Json(feedback))
}

pub async fn revoke_feedback(
    State(state): State<ApiState>,
    Json(input): Json<RevokeFeedbackRequest>,
) -> ApiResult<Json<AgentFeedback>> {
    let feedback = state.commerce.erc8004().revoke_feedback(
        &input.agent_registry,
        &input.agent_id,
        &input.client_address,
        input.feedback_index,
    )?;
    Ok(Json(feedback))
}

pub async fn list_feedback(
    State(state): State<ApiState>,
    Query(query): Query<FeedbackListQuery>,
) -> ApiResult<Json<Vec<AgentFeedback>>> {
    let client_addresses = query.client_addresses.as_deref().map(split_csv);
    let filter = AgentFeedbackFilter {
        agent_registry: query.agent_registry,
        agent_id: query.agent_id,
        client_addresses,
        tag1: query.tag1,
        tag2: query.tag2,
        include_revoked: query.include_revoked,
        limit: query.limit,
        offset: query.offset,
    };

    let feedback = state.commerce.erc8004().read_all_feedback(filter)?;
    Ok(Json(feedback))
}

pub async fn feedback_summary(
    State(state): State<ApiState>,
    Query(query): Query<FeedbackSummaryQuery>,
) -> ApiResult<Json<FeedbackSummary>> {
    let clients = split_csv(&query.client_addresses);
    let summary = state.commerce.erc8004().feedback_summary(
        &query.agent_registry,
        &query.agent_id,
        clients,
        query.tag1,
        query.tag2,
    )?;
    Ok(Json(summary))
}

pub async fn append_feedback_response(
    State(state): State<ApiState>,
    Json(input): Json<CreateAgentFeedbackResponse>,
) -> ApiResult<Json<AgentFeedbackResponse>> {
    let response = state.commerce.erc8004().append_feedback_response(input)?;
    Ok(Json(response))
}

pub async fn feedback_clients(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<String>>> {
    let clients = state.commerce.erc8004().feedback_clients(&agent_registry, &agent_id)?;
    Ok(Json(clients))
}

pub async fn last_feedback_index(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id, client_address)): Path<(String, String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let index = state
        .commerce
        .erc8004()
        .last_feedback_index(&agent_registry, &agent_id, &client_address)?;
    Ok(Json(serde_json::json!({"feedback_index": index})))
}

// ============================================================================
// Validation
// ============================================================================

pub async fn request_validation(
    State(state): State<ApiState>,
    Json(input): Json<CreateAgentValidationRequest>,
) -> ApiResult<Json<AgentValidationRequest>> {
    let request = state.commerce.erc8004().request_validation(input)?;
    Ok(Json(request))
}

pub async fn respond_validation(
    State(state): State<ApiState>,
    Path(request_hash): Path<String>,
    Json(input): Json<CreateAgentValidationResponse>,
) -> ApiResult<Json<AgentValidationResponse>> {
    let response = state
        .commerce
        .erc8004()
        .respond_validation(&request_hash, input)?;
    Ok(Json(response))
}

pub async fn validation_status(
    State(state): State<ApiState>,
    Path(request_hash): Path<String>,
) -> ApiResult<Json<AgentValidationStatus>> {
    let status = state
        .commerce
        .erc8004()
        .validation_status(&request_hash)?
        .ok_or_else(|| ApiError::not_found(\"validation status not found\"))?;
    Ok(Json(status))
}

pub async fn validation_summary(
    State(state): State<ApiState>,
    Query(query): Query<ValidationSummaryQuery>,
) -> ApiResult<Json<ValidationSummary>> {
    let validators = query.validator_addresses.as_deref().map(split_csv);
    let summary = state.commerce.erc8004().validation_summary(
        &query.agent_registry,
        &query.agent_id,
        validators,
        query.tag,
    )?;
    Ok(Json(summary))
}

pub async fn agent_validations(
    State(state): State<ApiState>,
    Path((agent_registry, agent_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<String>>> {
    let requests = state.commerce.erc8004().agent_validations(&agent_registry, &agent_id)?;
    Ok(Json(requests))
}

pub async fn validator_requests(
    State(state): State<ApiState>,
    Path(validator_address): Path<String>,
) -> ApiResult<Json<Vec<String>>> {
    let requests = state.commerce.erc8004().validator_requests(&validator_address)?;
    Ok(Json(requests))
}
