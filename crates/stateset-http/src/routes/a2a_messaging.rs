//! Agent-to-agent messaging endpoints for reliable inter-agent communication.
//!
//! Messages are durable and tenant-scoped: they live in the
//! `a2a_agent_messages` table of the tenant's database (resolved from
//! `x-tenant-id`), grouped into conversations with a per-conversation
//! sequence number, so they survive restarts and are visible to every replica.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::{
    A2AAgentMessage, A2AAgentMessageFilter, A2AAgentMessageStatus, SendA2AAgentMessage,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::a2a_credit::tenant_scope;
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

/// Build the A2A messaging sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/a2a/messages", post(send_message).get(list_messages))
        .route("/a2a/messages/{id}", get(get_message))
        .route("/a2a/messages/{id}/acknowledge", post(acknowledge_message))
        .route("/a2a/messages/{id}/fail", post(fail_message))
}

/// Request body for sending an A2A message.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SendMessageRequest {
    pub to_agent_id: String,
    pub from_agent_id: String,
    /// Free-form type, e.g. `quote_request`, `purchase_intent`, `dispute_notice`.
    pub message_type: String,
    #[schema(value_type = HashMap<String, String>)]
    pub payload: serde_json::Value,
    /// Existing conversation to append to; omitted starts a new one.
    pub conversation_id: Option<String>,
    /// Delivery attempts before the message is marked failed (default 5).
    pub max_attempts: Option<u32>,
}

/// Request body for recording a delivery failure.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct FailMessageRequest {
    pub error: String,
}

/// Query params for listing messages.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MessageFilterParams {
    pub to_agent_id: Option<String>,
    pub from_agent_id: Option<String>,
    pub conversation_id: Option<String>,
    /// `pending` (default when omitted), `delivered`, `acknowledged`, `failed`,
    /// `expired`, or `all`.
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Response body for a message.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MessageResponse {
    pub id: String,
    pub tenant_id: String,
    pub conversation_id: String,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub message_type: String,
    #[schema(value_type = HashMap<String, String>)]
    pub payload: serde_json::Value,
    pub status: String,
    pub sequence_number: u64,
    pub attempts: u32,
    pub max_attempts: u32,
    pub next_retry_at: Option<String>,
    pub acknowledged_at: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
}

fn msg_to_response(m: &A2AAgentMessage) -> MessageResponse {
    MessageResponse {
        id: m.id.to_string(),
        tenant_id: m.tenant_id.clone(),
        conversation_id: m.conversation_id.to_string(),
        from_agent_id: m.from_agent_id.to_string(),
        to_agent_id: m.to_agent_id.to_string(),
        message_type: m.message_type.clone(),
        payload: m.payload.clone(),
        status: m.status.to_string(),
        sequence_number: m.sequence_number,
        attempts: m.attempts,
        max_attempts: m.max_attempts,
        next_retry_at: m.next_retry_at.map(|d| d.to_rfc3339()),
        acknowledged_at: m.acknowledged_at.map(|d| d.to_rfc3339()),
        error: m.error.clone(),
        created_at: m.created_at.to_rfc3339(),
    }
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, HttpError> {
    value.parse().map_err(|_| HttpError::BadRequest(format!("Invalid {field} UUID")))
}

/// `POST /api/v1/a2a/messages`
#[utoipa::path(post, path = "/api/v1/a2a/messages", tag = "a2a",
    request_body = SendMessageRequest,
    responses((status = 201, body = MessageResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let conversation_id =
        req.conversation_id.as_deref().map(|s| parse_uuid(s, "conversation_id")).transpose()?;
    let message = c.x402().send_agent_message(SendA2AAgentMessage {
        tenant_id: tenant_scope(&headers),
        conversation_id,
        from_agent_id: parse_uuid(&req.from_agent_id, "from_agent_id")?,
        to_agent_id: parse_uuid(&req.to_agent_id, "to_agent_id")?,
        message_type: req.message_type,
        payload: req.payload,
        max_attempts: req.max_attempts,
    })?;
    Ok((StatusCode::CREATED, Json(msg_to_response(&message))))
}

/// `GET /api/v1/a2a/messages`
#[utoipa::path(get, path = "/api/v1/a2a/messages", tag = "a2a",
    params(MessageFilterParams),
    responses((status = 200, body = Vec<MessageResponse>)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<MessageFilterParams>,
) -> Result<Json<Vec<MessageResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        None => Some(A2AAgentMessageStatus::Pending),
        Some("all") => None,
        Some(raw) => Some(
            raw.parse::<A2AAgentMessageStatus>()
                .map_err(|_| HttpError::BadRequest(format!("Invalid status: {raw}")))?,
        ),
    };
    let messages = c.x402().list_agent_messages(A2AAgentMessageFilter {
        tenant_id: tenant_scope(&headers),
        conversation_id: params
            .conversation_id
            .as_deref()
            .map(|s| parse_uuid(s, "conversation_id"))
            .transpose()?,
        to_agent_id: params
            .to_agent_id
            .as_deref()
            .map(|s| parse_uuid(s, "to_agent_id"))
            .transpose()?,
        from_agent_id: params
            .from_agent_id
            .as_deref()
            .map(|s| parse_uuid(s, "from_agent_id"))
            .transpose()?,
        status,
        limit: params.limit,
        offset: params.offset,
    })?;
    Ok(Json(messages.iter().map(msg_to_response).collect()))
}

/// `GET /api/v1/a2a/messages/:id`
#[utoipa::path(get, path = "/api/v1/a2a/messages/{id}", tag = "a2a",
    params(("id" = String, Path, description = "Message ID (UUID)")),
    responses((status = 200, body = MessageResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn get_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let message = c
        .x402()
        .get_agent_message(&tenant_scope(&headers), id)?
        .ok_or_else(|| HttpError::NotFound(format!("Message {id} not found")))?;
    Ok(Json(msg_to_response(&message)))
}

/// `POST /api/v1/a2a/messages/:id/acknowledge`
#[utoipa::path(post, path = "/api/v1/a2a/messages/{id}/acknowledge", tag = "a2a",
    params(("id" = String, Path, description = "Message ID (UUID)")),
    responses((status = 200, body = MessageResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn acknowledge_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let message =
        c.x402().acknowledge_agent_message(&tenant_scope(&headers), id).map_err(|e| match e {
            stateset_core::CommerceError::NotFound => {
                HttpError::NotFound(format!("Message {id} not found"))
            }
            other => HttpError::from(other),
        })?;
    Ok(Json(msg_to_response(&message)))
}

/// `POST /api/v1/a2a/messages/:id/fail`
#[utoipa::path(post, path = "/api/v1/a2a/messages/{id}/fail", tag = "a2a",
    params(("id" = String, Path, description = "Message ID (UUID)")),
    request_body = FailMessageRequest,
    responses((status = 200, body = MessageResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn fail_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<FailMessageRequest>,
) -> Result<Json<MessageResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let message = c.x402().fail_agent_message(&tenant_scope(&headers), id, &req.error).map_err(
        |e| match e {
            stateset_core::CommerceError::NotFound => {
                HttpError::NotFound(format!("Message {id} not found"))
            }
            other => HttpError::from(other),
        },
    )?;
    Ok(Json(msg_to_response(&message)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_message_request_accepts_arbitrary_json_payload() {
        let req: SendMessageRequest = serde_json::from_str(
            r#"{"to_agent_id":"a","from_agent_id":"b","message_type":"quote_request",
                "payload":{"sku":"X","qty":2,"nested":{"ok":true}}}"#,
        )
        .expect("request");
        assert_eq!(req.payload["nested"]["ok"], serde_json::Value::Bool(true));
        assert!(req.conversation_id.is_none());
    }
}
