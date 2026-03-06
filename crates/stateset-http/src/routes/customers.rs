//! Customer endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};

use crate::dto::{
    CreateCustomerRequest, CustomerFilterParams, CustomerListResponse, CustomerResponse,
    decode_cursor, encode_cursor, finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{CreateCustomer, CustomerFilter, CustomerId, CustomerStatus};
use std::str::FromStr;

/// Build the customers sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/customers", post(create_customer).get(list_customers))
        .route("/customers/{id}", get(get_customer))
}

/// `POST /api/v1/customers`
#[utoipa::path(
    post,
    path = "/api/v1/customers",
    tag = "customers",
    request_body = CreateCustomerRequest,
    responses(
        (status = 201, description = "Customer created", body = CustomerResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_customer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCustomerRequest>,
) -> Result<(axum::http::StatusCode, Json<CustomerResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let input = CreateCustomer {
        email: req.email,
        first_name: req.first_name,
        last_name: req.last_name,
        phone: req.phone,
        accepts_marketing: req.accepts_marketing,
        tags: req.tags,
        metadata: req.metadata,
    };
    let customer = commerce.customers().create(input)?;
    Ok((axum::http::StatusCode::CREATED, Json(CustomerResponse::from(customer))))
}

/// `GET /api/v1/customers/:id`
#[utoipa::path(
    get,
    path = "/api/v1/customers/{id}",
    tag = "customers",
    params(("id" = String, Path, description = "Customer ID (UUID)")),
    responses(
        (status = 200, description = "Customer details", body = CustomerResponse),
        (status = 404, description = "Customer not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_customer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CustomerId>,
) -> Result<Json<CustomerResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let customer = commerce
        .customers()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Customer {id} not found")))?;
    Ok(Json(CustomerResponse::from(customer)))
}

/// `GET /api/v1/customers`
#[utoipa::path(
    get,
    path = "/api/v1/customers",
    tag = "customers",
    params(CustomerFilterParams),
    responses(
        (status = 200, description = "List of customers", body = CustomerListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_customers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CustomerFilterParams>,
) -> Result<Json<CustomerListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    // Parse filter parameters
    let status = params
        .status
        .as_deref()
        .map(CustomerStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;

    // Decode cursor if provided
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };

    // Count total matching records (without pagination or cursor)
    let count_filter = CustomerFilter {
        email: params.email.clone(),
        status,
        tag: params.tag.clone(),
        accepts_marketing: params.accepts_marketing,
        limit: None,
        offset: None,
        after_cursor: None,
    };
    let total = commerce.customers().list(count_filter)?.len();

    // Fetch the requested page
    let filter = CustomerFilter {
        email: params.email,
        status,
        tag: params.tag,
        accepts_marketing: params.accepts_marketing,
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(offset) },
        after_cursor,
    };
    let mut customers = commerce.customers().list(filter)?;
    let has_more = finalize_page(&mut customers, limit);
    let next_cursor = if has_more {
        customers.last().map(|c| encode_cursor(&c.created_at.to_rfc3339(), &c.id.to_string()))
    } else {
        None
    };
    Ok(Json(CustomerListResponse {
        customers: customers.into_iter().map(CustomerResponse::from).collect(),
        total,
        limit,
        offset,
        next_cursor,
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

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
    }

    #[tokio::test]
    async fn create_customer_returns_201() {
        let body = serde_json::json!({
            "email": "alice@example.com",
            "first_name": "Alice",
            "last_name": "Smith"
        });
        let resp = app()
            .oneshot(
                Request::post("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["email"], "alice@example.com");
    }

    #[tokio::test]
    async fn get_customer_not_found() {
        let id = CustomerId::new();
        let resp = app()
            .oneshot(Request::get(format!("/customers/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_customers_empty() {
        let resp =
            app().oneshot(Request::get("/customers").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn create_and_get_customer() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state.clone());

        // Create
        let body = serde_json::json!({
            "email": "bob@example.com",
            "first_name": "Bob",
            "last_name": "Jones"
        });
        let resp = app
            .oneshot(
                Request::post("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let created: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        let id = created["id"].as_str().unwrap();

        // Get
        let app2 = router().with_state(state);
        let resp = app2
            .oneshot(Request::get(format!("/customers/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let fetched: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(fetched["email"], "bob@example.com");
    }

    #[tokio::test]
    async fn list_customers_reports_total_before_pagination() {
        let (app, state) = app_with_state();

        for i in 0..2 {
            state
                .commerce()
                .customers()
                .create(stateset_core::CreateCustomer {
                    email: format!("paging-customer-{i}@example.com"),
                    first_name: "Paging".into(),
                    last_name: "Customer".into(),
                    ..Default::default()
                })
                .unwrap();
        }

        let resp = app
            .oneshot(Request::get("/customers?limit=1&offset=0").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["customers"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_customers_filter_by_email() {
        let (app, state) = app_with_state();

        state
            .commerce()
            .customers()
            .create(stateset_core::CreateCustomer {
                email: "alice@filter.com".into(),
                first_name: "Alice".into(),
                last_name: "Filter".into(),
                ..Default::default()
            })
            .unwrap();
        state
            .commerce()
            .customers()
            .create(stateset_core::CreateCustomer {
                email: "bob@filter.com".into(),
                first_name: "Bob".into(),
                last_name: "Filter".into(),
                ..Default::default()
            })
            .unwrap();

        let resp = app
            .oneshot(
                Request::get("/customers?email=alice%40filter.com").body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["customers"][0]["email"], "alice@filter.com");
    }

    #[tokio::test]
    async fn list_customers_invalid_status_returns_400() {
        let resp = app()
            .oneshot(Request::get("/customers?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_customers_filter_by_accepts_marketing() {
        let (app, state) = app_with_state();

        state
            .commerce()
            .customers()
            .create(stateset_core::CreateCustomer {
                email: "opt-in@test.com".into(),
                first_name: "Opt".into(),
                last_name: "In".into(),
                accepts_marketing: Some(true),
                ..Default::default()
            })
            .unwrap();
        state
            .commerce()
            .customers()
            .create(stateset_core::CreateCustomer {
                email: "opt-out@test.com".into(),
                first_name: "Opt".into(),
                last_name: "Out".into(),
                accepts_marketing: Some(false),
                ..Default::default()
            })
            .unwrap();

        let resp = app
            .oneshot(Request::get("/customers?accepts_marketing=true").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["customers"][0]["email"], "opt-in@test.com");
    }
}
