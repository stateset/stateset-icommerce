//! Negotiation endpoints for autonomous agent-to-agent price negotiation.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_a2a::negotiation::NegotiationEngine;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::HttpError;
use crate::state::AppState;

/// In-memory negotiation store (production would use DB via V9 tables).
type NegotiationStore = Arc<RwLock<HashMap<Uuid, stateset_a2a::negotiation::Negotiation>>>;

/// Build the negotiations sub-router.
pub fn router() -> Router<AppState> {
    let store: NegotiationStore = Arc::new(RwLock::new(HashMap::new()));
    Router::new()
        .route(
            "/negotiations",
            post({
                let store = store.clone();
                move |state, headers, body| create_negotiation(state, headers, body, store)
            }),
        )
        .route(
            "/negotiations/{id}",
            get({
                let store = store.clone();
                move |state, headers, path| get_negotiation(state, headers, path, store)
            }),
        )
        .route(
            "/negotiations/{id}/counter-offer",
            post({
                let store = store.clone();
                move |state, headers, path, body| counter_offer(state, headers, path, body, store)
            }),
        )
        .route(
            "/negotiations/{id}/accept",
            post({
                let store = store.clone();
                move |state, headers, path| accept_negotiation(state, headers, path, store)
            }),
        )
        .route(
            "/negotiations/{id}/reject",
            post(move |state, headers, path| reject_negotiation(state, headers, path, store)),
        )
}

/// Request body for creating a negotiation.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateNegotiationRequest {
    pub buyer_agent_id: String,
    pub seller_agent_id: String,
    pub initial_offer: f64,
    pub currency: Option<String>,
    pub max_rounds: Option<u32>,
    pub auto_accept_below: Option<f64>,
    pub auto_reject_above: Option<f64>,
    pub expires_in_hours: Option<u32>,
}

/// Request body for a counter-offer.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CounterOfferRequest {
    pub from_agent_id: String,
    pub amount: f64,
    pub message: Option<String>,
}

/// Response body for a negotiation.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NegotiationResponse {
    pub id: String,
    pub buyer_agent_id: String,
    pub seller_agent_id: String,
    pub status: String,
    pub current_offer: f64,
    pub currency: String,
    pub rounds: u32,
    pub max_rounds: u32,
    pub offer_count: usize,
    pub created_at: String,
}

fn neg_to_response(n: &stateset_a2a::negotiation::Negotiation) -> NegotiationResponse {
    NegotiationResponse {
        id: n.id.to_string(),
        buyer_agent_id: n.buyer_agent_id.clone(),
        seller_agent_id: n.seller_agent_id.clone(),
        status: n.status.to_string(),
        current_offer: n.current_offer.to_string().parse().unwrap_or(0.0),
        currency: n.currency.clone(),
        rounds: n.rounds,
        max_rounds: n.max_rounds,
        offer_count: n.offers.len(),
        created_at: n.created_at.to_rfc3339(),
    }
}

/// `POST /api/v1/negotiations`
#[tracing::instrument(skip_all)]
async fn create_negotiation(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Json(req): Json<CreateNegotiationRequest>,
    store: NegotiationStore,
) -> Result<(StatusCode, Json<NegotiationResponse>), HttpError> {
    let amount = Decimal::try_from(req.initial_offer)
        .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?;
    let auto_accept = req.auto_accept_below.map(|v| Decimal::try_from(v).unwrap_or_default());
    let auto_reject = req.auto_reject_above.map(|v| Decimal::try_from(v).unwrap_or_default());
    let expires = Utc::now() + Duration::hours(i64::from(req.expires_in_hours.unwrap_or(24)));

    let neg = NegotiationEngine::create(
        req.buyer_agent_id,
        req.seller_agent_id,
        amount,
        req.currency.as_deref().unwrap_or("USD"),
        req.max_rounds.unwrap_or(10),
        expires,
        auto_accept,
        auto_reject,
    );

    let resp = neg_to_response(&neg);
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    guard.insert(neg.id, neg);

    Ok((StatusCode::CREATED, Json(resp)))
}

/// `GET /api/v1/negotiations/:id`
#[tracing::instrument(skip_all)]
async fn get_negotiation(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let guard = store.read().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let neg =
        guard.get(&id).ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;
    Ok(Json(neg_to_response(neg)))
}

/// `POST /api/v1/negotiations/:id/counter-offer`
#[tracing::instrument(skip_all)]
async fn counter_offer(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CounterOfferRequest>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let amount = Decimal::try_from(req.amount)
        .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?;

    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let neg = guard
        .remove(&id)
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;

    let neg = NegotiationEngine::counter_offer(neg, req.from_agent_id, amount, req.message)
        .map_err(|e| HttpError::BadRequest(format!("Counter-offer failed: {e}")))?;

    let resp = neg_to_response(&neg);
    guard.insert(neg.id, neg);
    Ok(Json(resp))
}

/// `POST /api/v1/negotiations/:id/accept`
#[tracing::instrument(skip_all)]
async fn accept_negotiation(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let neg = guard
        .remove(&id)
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;

    let neg = NegotiationEngine::accept(neg)
        .map_err(|e| HttpError::BadRequest(format!("Accept failed: {e}")))?;

    let resp = neg_to_response(&neg);
    guard.insert(neg.id, neg);
    Ok(Json(resp))
}

/// `POST /api/v1/negotiations/:id/reject`
#[tracing::instrument(skip_all)]
async fn reject_negotiation(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let neg = guard
        .remove(&id)
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;

    let neg = NegotiationEngine::reject(neg, None)
        .map_err(|e| HttpError::BadRequest(format!("Reject failed: {e}")))?;

    let resp = neg_to_response(&neg);
    guard.insert(neg.id, neg);
    Ok(Json(resp))
}

#[cfg(test)]
mod tests {}
