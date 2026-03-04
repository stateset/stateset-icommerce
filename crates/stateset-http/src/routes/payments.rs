//! Payment endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};

use crate::dto::{PaymentFilterParams, PaymentListResponse, PaymentResponse};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use rust_decimal::Decimal;
use stateset_core::{
    CustomerId, OrderId, PaymentFilter, PaymentId, PaymentMethodType, PaymentTransactionStatus,
};
use std::str::FromStr;

/// Build the payments sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/payments", get(list_payments))
        .route("/payments/{id}", get(get_payment))
}

/// `GET /api/v1/payments/:id`
#[utoipa::path(
    get,
    path = "/api/v1/payments/{id}",
    tag = "payments",
    params(("id" = String, Path, description = "Payment ID (UUID)")),
    responses(
        (status = 200, description = "Payment details", body = PaymentResponse),
        (status = 404, description = "Payment not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentId>,
) -> Result<Json<PaymentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let payment = commerce
        .payments()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Payment {id} not found")))?;
    Ok(Json(PaymentResponse::from(payment)))
}

/// `GET /api/v1/payments`
#[utoipa::path(
    get,
    path = "/api/v1/payments",
    tag = "payments",
    params(PaymentFilterParams),
    responses(
        (status = 200, description = "List of payments", body = PaymentListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_payments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaymentFilterParams>,
) -> Result<Json<PaymentListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    // Parse filter parameters
    let order_id = params
        .order_id
        .map(|s| s.parse::<OrderId>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid order_id: {e}")))?;
    let customer_id = params
        .customer_id
        .map(|s| s.parse::<CustomerId>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid customer_id: {e}")))?;
    let status = params
        .status
        .as_deref()
        .map(PaymentTransactionStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;
    let payment_method = params
        .payment_method
        .as_deref()
        .map(PaymentMethodType::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid payment_method: {e}")))?;
    let min_amount = params
        .min_amount
        .map(|s| s.parse::<Decimal>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid min_amount: {e}")))?;
    let max_amount = params
        .max_amount
        .map(|s| s.parse::<Decimal>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid max_amount: {e}")))?;
    let from_date = params
        .from_date
        .map(|s| s.parse())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid from_date: {e}")))?;
    let to_date = params
        .to_date
        .map(|s| s.parse())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid to_date: {e}")))?;

    // Count total matching records (without pagination)
    let count_filter = PaymentFilter {
        order_id,
        invoice_id: None,
        customer_id,
        status,
        payment_method,
        processor: params.processor.clone(),
        currency: None,
        min_amount,
        max_amount,
        from_date,
        to_date,
        limit: None,
        offset: None,
    };
    let total = commerce.payments().list(count_filter)?.len();

    // Fetch the requested page
    let filter = PaymentFilter {
        order_id,
        invoice_id: None,
        customer_id,
        status,
        payment_method,
        processor: params.processor,
        currency: None,
        min_amount,
        max_amount,
        from_date,
        to_date,
        limit: Some(limit),
        offset: Some(offset),
    };
    let payments = commerce.payments().list(filter)?;
    let has_more = payments.len() == limit as usize;
    Ok(Json(PaymentListResponse {
        payments: payments.into_iter().map(PaymentResponse::from).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    #[tokio::test]
    async fn get_payment_not_found() {
        let id = PaymentId::new();
        let resp = app()
            .oneshot(Request::get(format!("/payments/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_payments_empty() {
        let resp = app()
            .oneshot(Request::get("/payments").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["payments"].as_array().unwrap().is_empty());
        assert_eq!(json["has_more"], false);
    }

    #[tokio::test]
    async fn list_payments_invalid_status_returns_400() {
        let resp = app()
            .oneshot(Request::get("/payments?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_payments_invalid_order_id_returns_400() {
        let resp = app()
            .oneshot(Request::get("/payments?order_id=not-a-uuid").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_payments_invalid_amount_returns_400() {
        let resp = app()
            .oneshot(Request::get("/payments?min_amount=abc").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_payments_with_pagination() {
        let resp = app()
            .oneshot(Request::get("/payments?limit=10&offset=5").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 5);
    }
}
