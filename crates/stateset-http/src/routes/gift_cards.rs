//! Gift card endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

// ============================================================================
// DTOs
// ============================================================================

/// Request body for `POST /api/v1/gift-cards`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateGiftCardRequest {
    /// Initial balance.
    pub initial_balance: f64,
    /// ISO currency code (default: USD).
    pub currency: Option<String>,
    /// Optional recipient email.
    pub recipient_email: Option<String>,
    /// Optional gift message.
    pub message: Option<String>,
    /// Optional expiration date (RFC 3339).
    pub expires_at: Option<String>,
}

/// Query parameters for `GET /api/v1/gift-cards` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct GiftCardFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by gift card status (active, disabled, expired, depleted).
    pub status: Option<String>,
}

impl GiftCardFilterParams {
    /// Default page size.
    const DEFAULT_LIMIT: u32 = 50;
    /// Maximum allowed page size.
    const MAX_LIMIT: u32 = 200;

    /// Resolved limit with bounds checking.
    #[must_use]
    pub(crate) fn resolved_limit(&self) -> u32 {
        self.limit.unwrap_or(Self::DEFAULT_LIMIT).clamp(1, Self::MAX_LIMIT)
    }

    /// Resolved offset.
    #[must_use]
    pub(crate) fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Response body for a single gift card.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct GiftCardResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: String,
    pub code: String,
    #[schema(value_type = String)]
    pub initial_balance: Decimal,
    #[schema(value_type = String)]
    pub current_balance: Decimal,
    pub currency: String,
    pub status: String,
    pub recipient_email: Option<String>,
    pub message: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for `POST /api/v1/gift-cards/{id}/charge` and `/refund`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct GiftCardAmountRequest {
    /// Amount to charge (debit) or refund (credit). Must be positive.
    #[schema(value_type = String)]
    pub amount: Decimal,
    /// Optional reference (e.g. order ID) recorded on the transaction.
    pub reference_id: Option<String>,
}

/// A gift card ledger transaction.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct GiftCardTransactionResponse {
    pub id: String,
    pub gift_card_id: String,
    #[schema(value_type = String)]
    pub amount: Decimal,
    #[schema(value_type = String)]
    pub balance_after: Decimal,
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/gift-cards` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct GiftCardListResponse {
    pub gift_cards: Vec<GiftCardResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Router
// ============================================================================

/// Build the gift-cards sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gift-cards", post(create_gift_card).get(list_gift_cards))
        .route("/gift-cards/{id}", get(get_gift_card))
        .route("/gift-cards/{id}/charge", post(charge_gift_card))
        .route("/gift-cards/{id}/refund", post(refund_gift_card))
        .route("/gift-cards/{id}/disable", post(disable_gift_card))
}

// ============================================================================
// Handlers
// ============================================================================

/// `POST /api/v1/gift-cards`
#[utoipa::path(
    post,
    path = "/api/v1/gift-cards",
    tag = "gift_cards",
    request_body = CreateGiftCardRequest,
    responses(
        (status = 201, description = "Gift card created", body = GiftCardResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_gift_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGiftCardRequest>,
) -> Result<(StatusCode, Json<GiftCardResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    if req.initial_balance <= 0.0 {
        return Err(HttpError::BadRequest("initial_balance must be positive".to_string()));
    }

    let currency = req
        .currency
        .map(|s| s.parse::<stateset_primitives::CurrencyCode>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid currency: {e}")))?
        .unwrap_or(stateset_primitives::CurrencyCode::USD);

    let expires_at = req
        .expires_at
        .map(|s| {
            s.parse::<chrono::DateTime<Utc>>()
                .map_err(|e| HttpError::BadRequest(format!("Invalid expires_at: {e}")))
        })
        .transpose()?;

    let gift_card = commerce.gift_cards().create(stateset_core::CreateGiftCard {
        code: None,
        initial_balance: Decimal::try_from(req.initial_balance)
            .map_err(|e| HttpError::BadRequest(format!("Invalid initial_balance: {e}")))?,
        currency,
        recipient_email: req.recipient_email,
        sender_name: None,
        message: req.message,
        expires_at,
    })?;

    Ok((StatusCode::CREATED, Json(gift_card_to_response(gift_card))))
}

/// `GET /api/v1/gift-cards/{id}`
#[utoipa::path(
    get,
    path = "/api/v1/gift-cards/{id}",
    tag = "gift_cards",
    params(("id" = String, Path, description = "Gift card ID (UUID)")),
    responses(
        (status = 200, description = "Gift card details", body = GiftCardResponse),
        (status = 404, description = "Gift card not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_gift_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::GiftCardId>,
) -> Result<Json<GiftCardResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let gift_card = commerce
        .gift_cards()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Gift card {id} not found")))?;
    Ok(Json(gift_card_to_response(gift_card)))
}

/// `GET /api/v1/gift-cards`
#[utoipa::path(
    get,
    path = "/api/v1/gift-cards",
    tag = "gift_cards",
    params(GiftCardFilterParams),
    responses(
        (status = 200, description = "List of gift cards", body = GiftCardListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_gift_cards(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<GiftCardFilterParams>,
) -> Result<Json<GiftCardListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    let status = params
        .status
        .map(|s| s.parse::<stateset_core::GiftCardStatus>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;

    // Count total matching records (without pagination)
    let count_filter =
        stateset_core::GiftCardFilter { status, code: None, limit: None, offset: None };
    let total = commerce.gift_cards().list(count_filter)?.len();

    // Fetch the requested page
    let filter = stateset_core::GiftCardFilter {
        status,
        code: None,
        limit: Some(limit.saturating_add(1)),
        offset: Some(offset),
    };
    let mut gift_cards = commerce.gift_cards().list(filter)?;
    let has_more = gift_cards.len() > limit as usize;
    if has_more {
        gift_cards.truncate(limit as usize);
    }

    Ok(Json(GiftCardListResponse {
        gift_cards: gift_cards.into_iter().map(gift_card_to_response).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `POST /api/v1/gift-cards/{id}/charge`
#[utoipa::path(
    post,
    path = "/api/v1/gift-cards/{id}/charge",
    tag = "gift_cards",
    params(("id" = String, Path, description = "Gift card ID (UUID)")),
    request_body = GiftCardAmountRequest,
    responses(
        (status = 200, description = "Gift card charged", body = GiftCardTransactionResponse),
        (status = 422, description = "Invalid amount, inactive or expired card, or insufficient balance", body = ErrorBody),
        (status = 404, description = "Gift card not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn charge_gift_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::GiftCardId>,
    Json(req): Json<GiftCardAmountRequest>,
) -> Result<Json<GiftCardTransactionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let txn = commerce.gift_cards().charge(id, req.amount, req.reference_id)?;
    Ok(Json(transaction_to_response(txn)))
}

/// `POST /api/v1/gift-cards/{id}/refund`
#[utoipa::path(
    post,
    path = "/api/v1/gift-cards/{id}/refund",
    tag = "gift_cards",
    params(("id" = String, Path, description = "Gift card ID (UUID)")),
    request_body = GiftCardAmountRequest,
    responses(
        (status = 200, description = "Gift card refunded", body = GiftCardTransactionResponse),
        (status = 422, description = "Invalid amount or disabled card", body = ErrorBody),
        (status = 404, description = "Gift card not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn refund_gift_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::GiftCardId>,
    Json(req): Json<GiftCardAmountRequest>,
) -> Result<Json<GiftCardTransactionResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let txn = commerce.gift_cards().refund(id, req.amount, req.reference_id)?;
    Ok(Json(transaction_to_response(txn)))
}

/// `POST /api/v1/gift-cards/{id}/disable`
#[utoipa::path(
    post,
    path = "/api/v1/gift-cards/{id}/disable",
    tag = "gift_cards",
    params(("id" = String, Path, description = "Gift card ID (UUID)")),
    responses(
        (status = 200, description = "Gift card disabled", body = GiftCardResponse),
        (status = 404, description = "Gift card not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn disable_gift_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::GiftCardId>,
) -> Result<Json<GiftCardResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let gift_card = commerce.gift_cards().disable(id)?;
    Ok(Json(gift_card_to_response(gift_card)))
}

// ============================================================================
// Helpers
// ============================================================================

fn transaction_to_response(txn: stateset_core::GiftCardTransaction) -> GiftCardTransactionResponse {
    GiftCardTransactionResponse {
        id: txn.id.to_string(),
        gift_card_id: txn.gift_card_id.to_string(),
        amount: txn.amount,
        balance_after: txn.balance_after,
        transaction_type: txn.transaction_type.to_string(),
        reference_id: txn.reference_id,
        created_at: txn.created_at,
    }
}

fn gift_card_to_response(gc: stateset_core::GiftCard) -> GiftCardResponse {
    GiftCardResponse {
        id: gc.id.to_string(),
        code: gc.code,
        initial_balance: gc.initial_balance,
        current_balance: gc.current_balance,
        currency: gc.currency.to_string(),
        status: gc.status.to_string(),
        recipient_email: gc.recipient_email,
        message: gc.message,
        expires_at: gc.expires_at,
        created_at: gc.created_at,
        updated_at: gc.updated_at,
    }
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
    async fn charge_and_refund_roundtrip() {
        let app = app();
        let (status, card) =
            post_json(&app, "/gift-cards", serde_json::json!({"initial_balance": 50.0})).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = card["id"].as_str().unwrap().to_string();

        let (status, txn) = post_json(
            &app,
            &format!("/gift-cards/{id}/charge"),
            serde_json::json!({"amount": "30.00", "reference_id": "ORD-1"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(txn["balance_after"], "20.00");
        assert_eq!(txn["transaction_type"], "charge");

        let (status, txn) = post_json(
            &app,
            &format!("/gift-cards/{id}/refund"),
            serde_json::json!({"amount": "5.00"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(txn["balance_after"], "25.00");
    }

    #[tokio::test]
    async fn charge_rejects_negative_and_overdraft() {
        let app = app();
        let (_, card) =
            post_json(&app, "/gift-cards", serde_json::json!({"initial_balance": 10.0})).await;
        let id = card["id"].as_str().unwrap().to_string();

        let (status, _) = post_json(
            &app,
            &format!("/gift-cards/{id}/charge"),
            serde_json::json!({"amount": "-5.00"}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

        let (status, _) = post_json(
            &app,
            &format!("/gift-cards/{id}/charge"),
            serde_json::json!({"amount": "99.00"}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
