//! Invoice endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};

use crate::dto::{
    CreateInvoiceRequest, InvoiceFilterParams, InvoiceListResponse, InvoiceResponse,
    RecordInvoicePaymentRequest, finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use rust_decimal::Decimal;
use stateset_core::{
    CreateInvoice, CustomerId, InvoiceFilter, InvoiceStatus, InvoiceType, OrderId,
    RecordInvoicePayment,
};
use std::str::FromStr;
use uuid::Uuid;

/// Build the invoices sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/invoices", post(create_invoice).get(list_invoices))
        .route("/invoices/{id}", get(get_invoice))
        .route("/invoices/{id}/send", post(send_invoice))
        .route("/invoices/{id}/payments", post(record_invoice_payment))
}

/// `GET /api/v1/invoices/:id`
#[utoipa::path(
    get,
    path = "/api/v1/invoices/{id}",
    tag = "invoices",
    params(("id" = String, Path, description = "Invoice ID (UUID)")),
    responses(
        (status = 200, description = "Invoice details", body = InvoiceResponse),
        (status = 404, description = "Invoice not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<InvoiceResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let invoice = commerce
        .invoices()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Invoice {id} not found")))?;
    Ok(Json(InvoiceResponse::from(invoice)))
}

/// `GET /api/v1/invoices`
#[utoipa::path(
    get,
    path = "/api/v1/invoices",
    tag = "invoices",
    params(InvoiceFilterParams),
    responses(
        (status = 200, description = "List of invoices", body = InvoiceListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_invoices(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<InvoiceFilterParams>,
) -> Result<Json<InvoiceListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    // Parse filter parameters
    let customer_id = params
        .customer_id
        .map(|s| s.parse::<CustomerId>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid customer_id: {e}")))?;
    let order_id = params
        .order_id
        .map(|s| s.parse::<OrderId>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid order_id: {e}")))?;
    let status = params
        .status
        .as_deref()
        .map(InvoiceStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}. Valid values: draft, sent, viewed, paid, partially_paid, overdue, void, written_off")))?;
    let invoice_type = params
        .invoice_type
        .as_deref()
        .map(InvoiceType::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid invoice_type: {e}. Valid values: standard, credit, proforma, recurring")))?;
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
    let count_filter = InvoiceFilter {
        customer_id,
        order_id,
        status,
        invoice_type,
        overdue_only: params.overdue_only,
        from_date,
        to_date,
        due_from: None,
        due_to: None,
        min_total: None,
        max_total: None,
        min_balance: None,
        invoice_number: None,
        limit: None,
        offset: None,
    };
    let total = commerce.invoices().list(count_filter)?.len();

    // Fetch the requested page
    let filter = InvoiceFilter {
        customer_id,
        order_id,
        status,
        invoice_type,
        overdue_only: params.overdue_only,
        from_date,
        to_date,
        due_from: None,
        due_to: None,
        min_total: None,
        max_total: None,
        min_balance: None,
        invoice_number: None,
        limit: Some(overfetch_limit(limit)),
        offset: Some(offset),
    };
    let mut invoices = commerce.invoices().list(filter)?;
    let has_more = finalize_page(&mut invoices, limit);
    Ok(Json(InvoiceListResponse {
        invoices: invoices.into_iter().map(InvoiceResponse::from).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `POST /api/v1/invoices`
#[utoipa::path(
    post,
    path = "/api/v1/invoices",
    tag = "invoices",
    request_body = CreateInvoiceRequest,
    responses(
        (status = 201, description = "Invoice created", body = InvoiceResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateInvoiceRequest>,
) -> Result<(StatusCode, Json<InvoiceResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let invoice_type = req
        .invoice_type
        .as_deref()
        .map(InvoiceType::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid invoice_type: {e}. Valid values: standard, credit, proforma, recurring")))?;

    let input = CreateInvoice {
        customer_id: req.customer_id,
        order_id: req.order_id,
        invoice_type,
        payment_terms: req.payment_terms,
        currency: req.currency.map(|c| {
            c.parse().unwrap_or_default()
        }),
        notes: req.notes,
        ..Default::default()
    };
    let invoice = commerce.invoices().create(input)?;
    Ok((StatusCode::CREATED, Json(InvoiceResponse::from(invoice))))
}

/// `POST /api/v1/invoices/:id/send`
#[utoipa::path(
    post,
    path = "/api/v1/invoices/{id}/send",
    tag = "invoices",
    params(("id" = String, Path, description = "Invoice ID (UUID)")),
    responses(
        (status = 200, description = "Invoice sent", body = InvoiceResponse),
        (status = 404, description = "Invoice not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn send_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<InvoiceResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let invoice = commerce.invoices().send(id)?;
    Ok(Json(InvoiceResponse::from(invoice)))
}

/// `POST /api/v1/invoices/:id/payments`
#[utoipa::path(
    post,
    path = "/api/v1/invoices/{id}/payments",
    tag = "invoices",
    params(("id" = String, Path, description = "Invoice ID (UUID)")),
    request_body = RecordInvoicePaymentRequest,
    responses(
        (status = 200, description = "Payment recorded on invoice", body = InvoiceResponse),
        (status = 404, description = "Invoice not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn record_invoice_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<RecordInvoicePaymentRequest>,
) -> Result<Json<InvoiceResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let input = RecordInvoicePayment {
        amount: Decimal::try_from(req.amount)
            .map_err(|e| HttpError::BadRequest(format!("Invalid amount: {e}")))?,
        payment_id: None,
        payment_method: req.payment_method,
        reference: req.reference,
        notes: req.notes,
    };
    let invoice = commerce.invoices().record_payment(id, input)?;
    Ok(Json(InvoiceResponse::from(invoice)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_core::InvoiceId;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    #[tokio::test]
    async fn get_invoice_not_found() {
        let id = InvoiceId::new();
        let resp = app()
            .oneshot(Request::get(format!("/invoices/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_invoices_empty() {
        let resp =
            app().oneshot(Request::get("/invoices").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["invoices"].as_array().unwrap().is_empty());
        assert_eq!(json["has_more"], false);
    }

    #[tokio::test]
    async fn list_invoices_invalid_status_returns_400() {
        let resp = app()
            .oneshot(Request::get("/invoices?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_invoices_invalid_customer_id_returns_400() {
        let resp = app()
            .oneshot(Request::get("/invoices?customer_id=not-a-uuid").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_invoices_invalid_invoice_type_returns_400() {
        let resp = app()
            .oneshot(Request::get("/invoices?invoice_type=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_invoices_with_pagination() {
        let resp = app()
            .oneshot(Request::get("/invoices?limit=10&offset=5").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 5);
    }
}
