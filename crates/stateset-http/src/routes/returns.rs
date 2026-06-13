//! Return endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};

use crate::dto::{
    CreateReturnRequest, ReturnFilterParams, ReturnListResponse, ReturnResponse, decode_cursor,
    encode_cursor, finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{
    CreateReturn, CreateReturnItem, CustomerId, ItemCondition, OrderId, OrderItemId, ReturnFilter,
    ReturnId, ReturnReason, ReturnStatus,
};
use std::str::FromStr;

/// Build the returns sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/returns", post(create_return).get(list_returns))
        .route("/returns/{id}", get(get_return))
        .route("/returns/{id}/approve", patch(approve_return))
}

/// `POST /api/v1/returns`
#[utoipa::path(
    post,
    path = "/api/v1/returns",
    tag = "returns",
    request_body = CreateReturnRequest,
    params(
        ("Idempotency-Key" = Option<String>, Header,
            description = "Optional client-generated key. Replaying the same key with an \
                identical body returns the original response without creating a duplicate; \
                reusing it with a different body returns 422. Scoped per tenant."),
    ),
    responses(
        (status = 201, description = "Return created", body = ReturnResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
        (status = 422, description = "Idempotency-Key reused with a different body", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_return(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateReturnRequest>,
) -> Result<(axum::http::StatusCode, Json<ReturnResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let reason = ReturnReason::from_str(&req.reason)
        .map_err(|e| HttpError::BadRequest(format!("Invalid reason: {e}")))?;

    let items: Vec<CreateReturnItem> = req
        .items
        .into_iter()
        .map(|item| {
            let condition = item
                .condition
                .as_deref()
                .map(ItemCondition::from_str)
                .transpose()
                .map_err(|e| HttpError::BadRequest(format!("Invalid condition: {e}")));
            condition.map(|c| CreateReturnItem {
                order_item_id: OrderItemId::from_uuid(item.order_item_id),
                quantity: item.quantity,
                condition: c,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let input = CreateReturn {
        order_id: req.order_id,
        reason,
        reason_details: req.reason_details,
        items,
        notes: req.notes,
        ..Default::default()
    };
    let ret = commerce.returns().create(input)?;
    Ok((axum::http::StatusCode::CREATED, Json(ReturnResponse::from(ret))))
}

/// `GET /api/v1/returns/:id`
#[utoipa::path(
    get,
    path = "/api/v1/returns/{id}",
    tag = "returns",
    params(("id" = String, Path, description = "Return ID (UUID)")),
    responses(
        (status = 200, description = "Return details", body = ReturnResponse),
        (status = 404, description = "Return not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_return(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ReturnId>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let ret = commerce
        .returns()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Return {id} not found")))?;
    Ok(Json(ReturnResponse::from(ret)))
}

/// `PATCH /api/v1/returns/:id/approve`
#[utoipa::path(
    patch,
    path = "/api/v1/returns/{id}/approve",
    tag = "returns",
    params(("id" = String, Path, description = "Return ID (UUID)")),
    responses(
        (status = 200, description = "Return approved", body = ReturnResponse),
        (status = 400, description = "Return cannot be approved", body = ErrorBody),
        (status = 404, description = "Return not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn approve_return(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ReturnId>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let ret = commerce.returns().approve(id)?;
    Ok(Json(ReturnResponse::from(ret)))
}

/// `GET /api/v1/returns`
#[utoipa::path(
    get,
    path = "/api/v1/returns",
    tag = "returns",
    params(ReturnFilterParams),
    responses(
        (status = 200, description = "List of returns", body = ReturnListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_returns(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ReturnFilterParams>,
) -> Result<Json<ReturnListResponse>, HttpError> {
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
        .map(ReturnStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;
    let reason = params
        .reason
        .as_deref()
        .map(ReturnReason::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid reason: {e}")))?;
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

    // Decode cursor if provided
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };

    // Count total matching records (without pagination or cursor)
    let count_filter = ReturnFilter {
        order_id,
        customer_id,
        status,
        reason,
        from_date,
        to_date,
        limit: None,
        offset: None,
        after_cursor: None,
    };
    let total = commerce.returns().list(count_filter)?.len();

    // Fetch the requested page
    let filter = ReturnFilter {
        order_id,
        customer_id,
        status,
        reason,
        from_date,
        to_date,
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(offset) },
        after_cursor,
    };
    let mut returns = commerce.returns().list(filter)?;
    let has_more = finalize_page(&mut returns, limit);
    let next_cursor = if has_more {
        returns.last().map(|r| encode_cursor(&r.created_at.to_rfc3339(), &r.id.to_string()))
    } else {
        None
    };
    Ok(Json(ReturnListResponse {
        returns: returns.into_iter().map(ReturnResponse::from).collect(),
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

    #[tokio::test]
    async fn get_return_not_found() {
        let id = ReturnId::new();
        let resp = app()
            .oneshot(Request::get(format!("/returns/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn approve_nonexistent_return() {
        let id = ReturnId::new();
        let resp = app()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/returns/{id}/approve"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp.status().is_client_error());
    }

    #[tokio::test]
    async fn create_return_invalid_reason() {
        let body = serde_json::json!({
            "order_id": uuid::Uuid::new_v4(),
            "reason": "unicorn_dust",
            "items": []
        });
        let resp = app()
            .oneshot(
                Request::post("/returns")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_returns_empty() {
        let resp =
            app().oneshot(Request::get("/returns").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["returns"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_returns_invalid_status_returns_400() {
        let resp = app()
            .oneshot(Request::get("/returns?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_returns_invalid_order_id_returns_400() {
        let resp = app()
            .oneshot(Request::get("/returns?order_id=not-a-uuid").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_returns_invalid_reason_returns_400() {
        let resp = app()
            .oneshot(Request::get("/returns?reason=unicorn").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_returns_with_pagination() {
        let resp = app()
            .oneshot(Request::get("/returns?limit=10&offset=5").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 5);
    }
}
