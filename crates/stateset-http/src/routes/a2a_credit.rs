//! Agent credit terms endpoints for net 30/60/90 payment between trusted agents.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_a2a::credit::{CreditManager, CreditStatus, CreditTerms, PaymentTerms};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ErrorBody, HttpError};
use crate::state::AppState;

type CreditStore = Arc<RwLock<HashMap<Uuid, CreditTerms>>>;

/// Build the credit terms sub-router.
pub fn router() -> Router<AppState> {
    let store: CreditStore = Arc::new(RwLock::new(HashMap::new()));
    Router::new()
        .route("/a2a/credit", post({
            let s = store.clone();
            move |state, headers, body| create_terms(state, headers, body, s)
        }).get({
            let s = store.clone();
            move |state, headers| list_terms(state, headers, s)
        }))
        .route("/a2a/credit/{id}", get({
            let s = store.clone();
            move |state, headers, path| get_terms(state, headers, path, s)
        }))
        .route("/a2a/credit/{id}/charge", post({
            let s = store.clone();
            move |state, headers, path, body| charge_credit(state, headers, path, body, s)
        }))
        .route("/a2a/credit/{id}/payment", post({
            let s = store.clone();
            move |state, headers, path, body| record_payment(state, headers, path, body, s)
        }))
}

/// Request body for creating credit terms.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateCreditTermsRequest {
    pub creditor_agent_id: String,
    pub debtor_agent_id: String,
    pub credit_limit: f64,
    pub currency: Option<String>,
    pub payment_terms: Option<String>,
}

/// Request body for charging or paying.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreditAmountRequest {
    pub amount: f64,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
}

/// Response body for credit terms.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreditTermsResponse {
    pub id: String,
    pub creditor_agent_id: String,
    pub debtor_agent_id: String,
    pub credit_limit: String,
    pub outstanding_balance: String,
    pub available_credit: String,
    pub currency: String,
    pub payment_terms: String,
    pub status: String,
}

fn terms_to_response(t: &CreditTerms) -> CreditTermsResponse {
    CreditTermsResponse {
        id: t.id.to_string(),
        creditor_agent_id: t.creditor_agent_id.to_string(),
        debtor_agent_id: t.debtor_agent_id.to_string(),
        credit_limit: t.credit_limit.to_string(),
        outstanding_balance: t.outstanding_balance.to_string(),
        available_credit: CreditManager::available_credit(t).to_string(),
        currency: t.currency.clone(),
        payment_terms: format!("{:?}", t.payment_terms),
        status: format!("{:?}", t.status),
    }
}

/// `POST /api/v1/a2a/credit`
#[tracing::instrument(skip_all)]
async fn create_terms(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Json(req): Json<CreateCreditTermsRequest>,
    store: CreditStore,
) -> Result<(StatusCode, Json<CreditTermsResponse>), HttpError> {
    let limit = Decimal::try_from(req.credit_limit)
        .map_err(|e| HttpError::BadRequest(format!("Invalid credit_limit: {e}")))?;

    let payment_terms = match req.payment_terms.as_deref() {
        Some("net_15") => PaymentTerms::Net15,
        Some("net_30") => PaymentTerms::Net30,
        Some("net_60") => PaymentTerms::Net60,
        Some("net_90") => PaymentTerms::Net90,
        Some("prepaid") => PaymentTerms::Prepaid,
        Some(other) => return Err(HttpError::BadRequest(
            format!("Invalid payment_terms: {other}. Valid values: net_15, net_30, net_60, net_90, prepaid")
        )),
        None => PaymentTerms::Net30,
    };

    let terms = CreditManager::create_terms(
        req.creditor_agent_id,
        req.debtor_agent_id,
        limit,
        req.currency.as_deref().unwrap_or("USD"),
        payment_terms,
        "standard",
    );

    let resp = terms_to_response(&terms);
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    guard.insert(terms.id, terms);

    Ok((StatusCode::CREATED, Json(resp)))
}

/// `GET /api/v1/a2a/credit`
#[tracing::instrument(skip_all)]
async fn list_terms(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    store: CreditStore,
) -> Result<Json<Vec<CreditTermsResponse>>, HttpError> {
    let guard = store.read().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let terms: Vec<CreditTermsResponse> = guard.values().map(terms_to_response).collect();
    Ok(Json(terms))
}

/// `GET /api/v1/a2a/credit/:id`
#[tracing::instrument(skip_all)]
async fn get_terms(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: CreditStore,
) -> Result<Json<CreditTermsResponse>, HttpError> {
    let guard = store.read().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let terms = guard.get(&id).ok_or_else(|| HttpError::NotFound(format!("Credit terms {id} not found")))?;
    Ok(Json(terms_to_response(terms)))
}

/// `POST /api/v1/a2a/credit/:id/charge`
#[tracing::instrument(skip_all)]
async fn charge_credit(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreditAmountRequest>,
    store: CreditStore,
) -> Result<Json<CreditTermsResponse>, HttpError> {
    let amount = Decimal::try_from(req.amount)
        .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?;

    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let terms = guard.get_mut(&id).ok_or_else(|| HttpError::NotFound(format!("Credit terms {id} not found")))?;

    CreditManager::charge(terms, amount, req.reference_id)
        .map_err(|e| HttpError::BadRequest(format!("Charge failed: {e}")))?;

    Ok(Json(terms_to_response(terms)))
}

/// `POST /api/v1/a2a/credit/:id/payment`
#[tracing::instrument(skip_all)]
async fn record_payment(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreditAmountRequest>,
    store: CreditStore,
) -> Result<Json<CreditTermsResponse>, HttpError> {
    let amount = Decimal::try_from(req.amount)
        .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?;

    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let terms = guard.get_mut(&id).ok_or_else(|| HttpError::NotFound(format!("Credit terms {id} not found")))?;

    CreditManager::record_payment(terms, amount, req.reference_id)
        .map_err(|e| HttpError::BadRequest(format!("Payment failed: {e}")))?;

    Ok(Json(terms_to_response(terms)))
}

#[cfg(test)]
mod tests {}
