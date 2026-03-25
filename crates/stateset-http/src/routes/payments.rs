//! Payment endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};

use crate::dto::{
    CreatePaymentRequest, CreateRefundRequest, PaymentFilterParams, PaymentListResponse,
    PaymentResponse, finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use rust_decimal::Decimal;
use stateset_core::{
    CreatePayment, CreateRefund, CurrencyCode, CustomerId, OrderId, PaymentFilter, PaymentId,
    PaymentMethodType, PaymentTransactionStatus,
};
use std::str::FromStr;

/// Build the payments sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/payments", post(create_payment).get(list_payments))
        .route("/payments/{id}", get(get_payment))
        .route("/payments/{id}/complete", post(complete_payment))
        .route("/payments/{id}/refund", post(create_refund))
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
        limit: Some(overfetch_limit(limit)),
        offset: Some(offset),
    };
    let mut payments = commerce.payments().list(filter)?;
    let has_more = finalize_page(&mut payments, limit);
    Ok(Json(PaymentListResponse {
        payments: payments.into_iter().map(PaymentResponse::from).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `POST /api/v1/payments`
#[utoipa::path(
    post,
    path = "/api/v1/payments",
    tag = "payments",
    request_body = CreatePaymentRequest,
    responses(
        (status = 201, description = "Payment created", body = PaymentResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePaymentRequest>,
) -> Result<(StatusCode, Json<PaymentResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let currency = req
        .currency
        .as_deref()
        .map(CurrencyCode::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid currency: {e}")))?;

    let payment_method = req
        .payment_method
        .as_deref()
        .map(PaymentMethodType::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid payment_method: {e}")))?;

    let input = CreatePayment {
        order_id: Some(req.order_id),
        customer_id: req.customer_id,
        payment_method: payment_method.unwrap_or_default(),
        amount: Decimal::try_from(req.amount)
            .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?,
        currency,
        external_id: req.external_id,
        ..Default::default()
    };
    let payment = commerce.payments().create(input)?;
    Ok((StatusCode::CREATED, Json(PaymentResponse::from(payment))))
}

/// `POST /api/v1/payments/:id/complete`
#[utoipa::path(
    post,
    path = "/api/v1/payments/{id}/complete",
    tag = "payments",
    params(("id" = String, Path, description = "Payment ID (UUID)")),
    responses(
        (status = 200, description = "Payment marked as completed", body = PaymentResponse),
        (status = 404, description = "Payment not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentId>,
) -> Result<Json<PaymentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let payment = commerce.payments().mark_completed(id)?;
    Ok(Json(PaymentResponse::from(payment)))
}

/// `POST /api/v1/payments/:id/refund`
#[utoipa::path(
    post,
    path = "/api/v1/payments/{id}/refund",
    tag = "payments",
    params(("id" = String, Path, description = "Payment ID (UUID)")),
    request_body = CreateRefundRequest,
    responses(
        (status = 201, description = "Refund created", body = PaymentResponse),
        (status = 404, description = "Payment not found", body = ErrorBody),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_refund(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PaymentId>,
    Json(req): Json<CreateRefundRequest>,
) -> Result<(StatusCode, Json<PaymentResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let input = CreateRefund {
        payment_id: id,
        amount: Some(
            Decimal::try_from(req.amount)
                .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?,
        ),
        reason: req.reason,
        ..Default::default()
    };
    let _refund = commerce.payments().create_refund(input)?;
    let payment = commerce
        .payments()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Payment {id} not found")))?;
    Ok((StatusCode::CREATED, Json(PaymentResponse::from(payment))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use rust_decimal_macros::dec;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
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
        let resp =
            app().oneshot(Request::get("/payments").body(Body::empty()).unwrap()).await.unwrap();
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

    #[tokio::test]
    async fn list_payments_exact_boundary_has_more_false() {
        let (app, state) = app_with_state();

        for _ in 0..2 {
            state
                .commerce()
                .payments()
                .create(stateset_core::CreatePayment {
                    payment_method: PaymentMethodType::CreditCard,
                    amount: dec!(25.00),
                    ..Default::default()
                })
                .unwrap();
        }

        let resp = app
            .oneshot(Request::get("/payments?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["total"], 2);
        assert_eq!(json["payments"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], false);
    }
}
