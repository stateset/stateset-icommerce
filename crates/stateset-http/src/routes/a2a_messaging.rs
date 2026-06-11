//! Agent-to-agent messaging endpoints for reliable inter-agent communication.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use stateset_a2a::messaging::{A2AMessage, MessageQueue, MessageStatus, MessageType};
use std::sync::{Arc, RwLock};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{ErrorBody, HttpError};
use crate::state::AppState;

type MsgStore = Arc<RwLock<MessageQueue>>;

/// Build the A2A messaging sub-router.
pub fn router() -> Router<AppState> {
    let store: MsgStore = Arc::new(RwLock::new(MessageQueue::new()));
    Router::new()
        .route(
            "/a2a/messages",
            post({
                let s = store.clone();
                move |state, headers, body| send_message(state, headers, body, s)
            })
            .get({
                let s = store.clone();
                move |state, headers, query| list_messages(state, headers, query, s)
            }),
        )
        .route(
            "/a2a/messages/{id}/acknowledge",
            post({
                let s = store;
                move |state, headers, path| acknowledge_message(state, headers, path, s)
            }),
        )
}

/// Request body for sending an A2A message.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SendMessageRequest {
    pub to_agent_id: String,
    pub from_agent_id: String,
    pub message_type: String,
    #[schema(value_type = HashMap<String, String>)]
    pub payload: serde_json::Value,
    pub conversation_id: Option<String>,
}

/// Query params for listing messages.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MessageFilterParams {
    pub to_agent_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
}

/// Response body for a message.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MessageResponse {
    pub id: String,
    pub conversation_id: String,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub message_type: String,
    pub status: String,
    pub sequence_number: u64,
    pub attempts: u32,
    pub created_at: String,
}

fn msg_to_response(m: &A2AMessage) -> MessageResponse {
    MessageResponse {
        id: m.id.to_string(),
        conversation_id: m.conversation_id.to_string(),
        from_agent_id: m.from_agent_id.to_string(),
        to_agent_id: m.to_agent_id.to_string(),
        message_type: format!("{:?}", m.message_type),
        status: format!("{:?}", m.status),
        sequence_number: m.sequence_number,
        attempts: m.attempts,
        created_at: m.created_at.to_rfc3339(),
    }
}

/// `POST /api/v1/a2a/messages`
#[utoipa::path(post, path = "/api/v1/a2a/messages", tag = "a2a",
    request_body = SendMessageRequest,
    responses((status = 201, body = MessageResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn send_message(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Json(req): Json<SendMessageRequest>,
    store: MsgStore,
) -> Result<(StatusCode, Json<MessageResponse>), HttpError> {
    let msg_type = match req.message_type.as_str() {
        "quote_request" => MessageType::QuoteRequest,
        "quote_response" => MessageType::QuoteResponse,
        "counter_offer" => MessageType::CounterOffer,
        "purchase_intent" => MessageType::PurchaseIntent,
        "delivery_notification" => MessageType::DeliveryNotification,
        "dispute_notice" => MessageType::DisputeNotice,
        other => MessageType::Custom(other.to_string()),
    };

    let conversation_id =
        req.conversation_id.and_then(|s| s.parse().ok()).unwrap_or_else(Uuid::new_v4);

    let msg = A2AMessage {
        id: Uuid::new_v4(),
        conversation_id,
        from_agent_id: req
            .from_agent_id
            .parse()
            .map_err(|_| HttpError::BadRequest("Invalid from_agent_id UUID".into()))?,
        to_agent_id: req
            .to_agent_id
            .parse()
            .map_err(|_| HttpError::BadRequest("Invalid to_agent_id UUID".into()))?,
        message_type: msg_type,
        payload: serde_json::to_string(&req.payload).unwrap_or_default(),
        status: MessageStatus::Pending,
        sequence_number: 0,
        attempts: 0,
        max_attempts: 5,
        next_retry_at: None,
        acknowledged_at: None,
        error: None,
        created_at: Utc::now(),
    };

    let resp = msg_to_response(&msg);
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    guard.enqueue(msg);

    Ok((StatusCode::CREATED, Json(resp)))
}

/// `GET /api/v1/a2a/messages`
#[utoipa::path(get, path = "/api/v1/a2a/messages", tag = "a2a",
    params(MessageFilterParams),
    responses((status = 200, body = Vec<MessageResponse>)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn list_messages(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Query(params): Query<MessageFilterParams>,
    store: MsgStore,
) -> Result<Json<Vec<MessageResponse>>, HttpError> {
    let guard = store.read().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let pending = guard.get_pending();
    let limit = params.limit.unwrap_or(50) as usize;
    let messages: Vec<MessageResponse> = pending.iter().take(limit).map(msg_to_response).collect();
    Ok(Json(messages))
}

/// `POST /api/v1/a2a/messages/:id/acknowledge`
#[utoipa::path(post, path = "/api/v1/a2a/messages/{id}/acknowledge", tag = "a2a",
    params(("id" = String, Path, description = "Message ID (UUID)")),
    responses((status = 200, description = "Message acknowledged"), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn acknowledge_message(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: MsgStore,
) -> Result<Json<serde_json::Value>, HttpError> {
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    guard.acknowledge(id).map_err(|e| HttpError::NotFound(e.to_string()))?;
    Ok(Json(serde_json::json!({ "status": "acknowledged", "id": id.to_string() })))
}

#[cfg(test)]
mod tests {}
