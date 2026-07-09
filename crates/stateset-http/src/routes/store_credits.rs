//! Store credit endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{CurrencyCode, StoreCreditId, StoreCreditReason};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateStoreCreditRequest {
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: stateset_core::CustomerId,
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub currency: Option<String>,
    pub reason: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct StoreCreditFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status: Option<String>,
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct StoreCreditResponse {
    pub id: String,
    pub customer_id: String,
    #[schema(value_type = String)]
    pub original_balance: Decimal,
    #[schema(value_type = String)]
    pub current_balance: Decimal,
    pub currency: String,
    pub status: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct StoreCreditListResponse {
    pub store_credits: Vec<StoreCreditResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AdjustStoreCreditRequest {
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub note: Option<String>,
}

/// Request body for `POST /api/v1/store-credits/{id}/apply`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ApplyStoreCreditRequest {
    /// Amount to debit from the credit. Must be positive.
    #[schema(value_type = String)]
    pub amount: Decimal,
    /// Optional reference (e.g. order ID) recorded on the transaction.
    pub reference_id: Option<String>,
}

/// A store credit ledger transaction.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct StoreCreditTransactionResponse {
    pub id: String,
    pub store_credit_id: String,
    #[schema(value_type = String)]
    pub amount: Decimal,
    #[schema(value_type = String)]
    pub balance_after: Decimal,
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub created_at: String,
}

fn sc_to_response(sc: &stateset_core::StoreCredit) -> StoreCreditResponse {
    StoreCreditResponse {
        id: sc.id.to_string(),
        customer_id: sc.customer_id.to_string(),
        original_balance: sc.original_balance,
        current_balance: sc.current_balance,
        currency: sc.currency.as_str().to_string(),
        status: sc.status.to_string(),
        reason: sc.reason.to_string(),
        created_at: sc.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/store-credits", post(create_store_credit).get(list_store_credits))
        .route("/store-credits/{id}", get(get_store_credit))
        .route("/store-credits/{id}/adjust", post(adjust_store_credit))
        .route("/store-credits/{id}/apply", post(apply_store_credit))
}

#[utoipa::path(post, path = "/api/v1/store-credits", tag = "store_credits",
    request_body = CreateStoreCreditRequest,
    responses((status = 201, body = StoreCreditResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStoreCreditRequest>,
) -> Result<(StatusCode, Json<StoreCreditResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let currency = CurrencyCode::from_str(req.currency.as_deref().unwrap_or("USD"))
        .map_err(|e| HttpError::BadRequest(format!("Invalid currency: {e}")))?;
    let reason = StoreCreditReason::from_str(req.reason.as_deref().unwrap_or("manual"))
        .unwrap_or(StoreCreditReason::Manual);
    let input = stateset_core::CreateStoreCredit {
        customer_id: req.customer_id,
        amount: req.amount,
        currency,
        reason,
        reference_id: None,
        note: req.note,
        expires_at: None,
    };
    let sc = commerce.store_credits().create(input)?;
    Ok((StatusCode::CREATED, Json(sc_to_response(&sc))))
}

#[utoipa::path(get, path = "/api/v1/store-credits", tag = "store_credits",
    params(StoreCreditFilterParams),
    responses((status = 200, body = StoreCreditListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_store_credits(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<StoreCreditFilterParams>,
) -> Result<Json<StoreCreditListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let filter = stateset_core::StoreCreditFilter {
        customer_id: params.customer_id.and_then(|c| c.parse().ok()),
        status: params.status.and_then(|s| s.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let credits = commerce.store_credits().list(filter)?;
    let total = credits.len();
    Ok(Json(StoreCreditListResponse {
        store_credits: credits.iter().map(sc_to_response).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/store-credits/{id}", tag = "store_credits",
    params(("id" = String, Path, description = "Store credit ID")),
    responses((status = 200, body = StoreCreditResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StoreCreditId>,
) -> Result<Json<StoreCreditResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let sc = commerce
        .store_credits()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Store credit {id} not found")))?;
    Ok(Json(sc_to_response(&sc)))
}

#[utoipa::path(post, path = "/api/v1/store-credits/{id}/adjust", tag = "store_credits",
    params(("id" = String, Path, description = "Store credit ID")),
    request_body = AdjustStoreCreditRequest,
    responses((status = 200, body = StoreCreditResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn adjust_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StoreCreditId>,
    Json(req): Json<AdjustStoreCreditRequest>,
) -> Result<Json<StoreCreditResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let input =
        stateset_core::AdjustStoreCredit { amount: req.amount, note: req.note, reference_id: None };
    let sc = commerce.store_credits().adjust(id, input)?;
    Ok(Json(sc_to_response(&sc)))
}

/// `POST /api/v1/store-credits/{id}/apply` — debit a store credit (e.g.
/// against an order). Rejected for non-positive amounts, non-active or
/// expired credits, and insufficient balance.
#[utoipa::path(post, path = "/api/v1/store-credits/{id}/apply", tag = "store_credits",
    params(("id" = String, Path, description = "Store credit ID (UUID)")),
    request_body = ApplyStoreCreditRequest,
    responses(
        (status = 200, body = StoreCreditTransactionResponse),
        (status = 422, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn apply_store_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StoreCreditId>,
    Json(req): Json<ApplyStoreCreditRequest>,
) -> Result<Json<StoreCreditTransactionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let txn = commerce.store_credits().apply(id, req.amount, req.reference_id)?;
    Ok(Json(StoreCreditTransactionResponse {
        id: txn.id.to_string(),
        store_credit_id: txn.store_credit_id.to_string(),
        amount: txn.amount,
        balance_after: txn.balance_after,
        transaction_type: txn.transaction_type.to_string(),
        reference_id: txn.reference_id,
        created_at: txn.created_at.to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        router().with_state(state)
    }

    async fn post_json(
        app: &Router,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let resp = app
            .clone()
            .oneshot(
                Request::post(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    #[tokio::test]
    async fn apply_debits_and_guards() {
        let app = app();
        let (status, sc) = post_json(
            &app,
            "/store-credits",
            serde_json::json!({
                "customer_id": uuid::Uuid::new_v4().to_string(),
                "amount": "50.00"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = sc["id"].as_str().unwrap().to_string();

        let (status, txn) = post_json(
            &app,
            &format!("/store-credits/{id}/apply"),
            serde_json::json!({"amount": "30.00", "reference_id": "ORD-9"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(txn["balance_after"], "20.00");
        assert_eq!(txn["amount"], "-30.00");

        // Non-positive and overdraft applies are rejected.
        let (status, _) = post_json(
            &app,
            &format!("/store-credits/{id}/apply"),
            serde_json::json!({"amount": "-1.00"}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

        let (status, _) = post_json(
            &app,
            &format!("/store-credits/{id}/apply"),
            serde_json::json!({"amount": "99.00"}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
