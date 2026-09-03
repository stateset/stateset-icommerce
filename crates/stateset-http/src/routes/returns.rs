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
    ReturnId, ReturnReason, ReturnStatus, UpdateReturn,
};
use std::str::FromStr;

/// Build the returns sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/returns", post(create_return).get(list_returns))
        .route("/returns/{id}", get(get_return).patch(update_return))
        .route("/returns/{id}/approve", patch(approve_return))
        .route("/returns/{id}/reject", patch(reject_return))
        .route("/returns/{id}/complete", patch(complete_return))
        .route("/returns/{id}/items/{item_id}/disposition", post(set_item_disposition))
}

/// Request body for `POST /api/v1/returns/{id}/items/{item_id}/disposition`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct SetReturnDispositionRequest {
    /// One of `restock`, `refurbish`, `scrap`, `return_to_vendor`, `quarantine`.
    pub disposition: String,
    /// Warehouse receiving the stock (default 1).
    pub warehouse_id: Option<i32>,
    /// Explicit target bin; otherwise the warehouse's `returns` (restock) or
    /// `quarantine` bin is used when one exists.
    pub bin_id: Option<i32>,
    pub disposition_by: Option<String>,
    /// Lot the received units belong to; restocked/quarantined units are
    /// restored to it in the same transaction.
    #[serde(default)]
    #[schema(value_type = Option<String>, format = "uuid")]
    pub lot_id: Option<uuid::Uuid>,
    /// Serial numbers physically received (count must equal the item
    /// quantity); each is marked `returned` and moved to the disposition's
    /// target status.
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    pub serial_ids: Vec<uuid::Uuid>,
}

/// A return line item with its disposition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ReturnItemResponse {
    pub id: String,
    pub return_id: String,
    pub order_item_id: String,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub condition: String,
    pub refund_amount: String,
    pub disposition: Option<String>,
    pub disposition_at: Option<String>,
    pub disposition_by: Option<String>,
    pub lot_id: Option<String>,
    pub serial_ids: Vec<String>,
}

impl From<stateset_core::ReturnItem> for ReturnItemResponse {
    fn from(i: stateset_core::ReturnItem) -> Self {
        Self {
            id: i.id.to_string(),
            return_id: i.return_id.to_string(),
            order_item_id: i.order_item_id.to_string(),
            sku: i.sku,
            name: i.name,
            quantity: i.quantity,
            condition: i.condition.to_string(),
            refund_amount: i.refund_amount.to_string(),
            disposition: i.disposition.map(|d| d.to_string()),
            disposition_at: i.disposition_at.map(|d| d.to_rfc3339()),
            disposition_by: i.disposition_by,
            lot_id: i.lot_id.map(|id| id.to_string()),
            serial_ids: i.serial_ids.iter().map(ToString::to_string).collect(),
        }
    }
}

/// `POST /api/v1/returns/:id/items/:item_id/disposition`
#[utoipa::path(
    post,
    operation_id = "return_item_set_disposition",
    path = "/api/v1/returns/{id}/items/{item_id}/disposition",
    tag = "returns",
    request_body = SetReturnDispositionRequest,
    params(
        ("id" = String, Path, description = "Return ID (UUID)"),
        ("item_id" = String, Path, description = "Return item ID (UUID)"),
    ),
    responses(
        (status = 200, description = "Disposition recorded; stock, serial and lot effects applied", body = ReturnItemResponse),
        (status = 400, description = "Invalid disposition / stock, serial or lot effect rejected", body = ErrorBody),
        (status = 403, description = "Return not yet received", body = ErrorBody),
        (status = 404, description = "Return not found", body = ErrorBody),
        (status = 409, description = "Item already dispositioned", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_item_disposition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, item_id)): Path<(ReturnId, uuid::Uuid)>,
    Json(req): Json<SetReturnDispositionRequest>,
) -> Result<Json<ReturnItemResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let disposition = stateset_core::ReturnDisposition::from_str(&req.disposition)
        .map_err(|e| HttpError::BadRequest(format!("Invalid disposition: {e}")))?;
    let item = commerce.returns().set_item_disposition(
        id,
        item_id,
        stateset_core::SetReturnDisposition {
            disposition,
            warehouse_id: req.warehouse_id,
            bin_id: req.bin_id,
            disposition_by: req.disposition_by,
            lot_id: req.lot_id,
            serial_ids: req.serial_ids,
        },
    )?;
    Ok(Json(ReturnItemResponse::from(item)))
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

/// Request body for `PATCH /api/v1/returns/{id}`.
///
/// Every field is optional; the accessor applies the same guards as the
/// dedicated action routes (legal transition, refund bounds, terminal returns
/// immutable, no rejecting/cancelling after a disposition).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct UpdateReturnRequest {
    /// Target status: `requested`, `approved`, `rejected`, `in_transit`,
    /// `received`, `inspecting`, `completed` or `cancelled`.
    pub status: Option<String>,
    pub tracking_number: Option<String>,
    /// Refund total. Non-negative and at most the sum of the line refunds.
    pub refund_amount: Option<String>,
    /// `original_payment` (the default) settles through the payments ledger on
    /// completion; anything else (store credit, exchange, …) is recorded only.
    pub refund_method: Option<String>,
    pub notes: Option<String>,
    /// Complete even though some received items have no disposition; the
    /// undispositioned units are written off.
    #[serde(default)]
    pub write_off_undispositioned: bool,
}

impl TryFrom<UpdateReturnRequest> for UpdateReturn {
    type Error = HttpError;

    fn try_from(req: UpdateReturnRequest) -> Result<Self, Self::Error> {
        let status = req
            .status
            .as_deref()
            .map(ReturnStatus::from_str)
            .transpose()
            .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;
        let refund_amount = req
            .refund_amount
            .as_deref()
            .map(str::parse::<rust_decimal::Decimal>)
            .transpose()
            .map_err(|e| HttpError::BadRequest(format!("Invalid refund_amount: {e}")))?;
        Ok(Self {
            status,
            tracking_number: req.tracking_number,
            refund_amount,
            refund_method: req.refund_method,
            notes: req.notes,
            write_off_undispositioned: req.write_off_undispositioned,
        })
    }
}

/// Request body for `PATCH /api/v1/returns/{id}/reject`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RejectReturnRequest {
    /// Why the return was rejected; recorded in the return's notes.
    #[serde(default)]
    pub reason: String,
}

/// Request body for `PATCH /api/v1/returns/{id}/complete`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct CompleteReturnRequest {
    /// Complete even though some received items have no disposition. Off by
    /// default so received goods cannot silently vanish.
    #[serde(default)]
    pub write_off_undispositioned: bool,
}

/// `PATCH /api/v1/returns/:id`
///
/// The general-purpose update: set the tracking number, the refund amount and
/// method, notes, or drive the status machine. Reaching `completed` here
/// settles the refund against the order's captured payments in the same
/// transaction (unless `refund_method` settles out of band).
#[utoipa::path(
    patch,
    operation_id = "return_update",
    path = "/api/v1/returns/{id}",
    tag = "returns",
    request_body = UpdateReturnRequest,
    params(("id" = String, Path, description = "Return ID (UUID)")),
    responses(
        (status = 200, description = "Return updated", body = ReturnResponse),
        (status = 400, description = "Invalid field or illegal status transition", body = ErrorBody),
        (status = 403, description = "Terminal return, or completion with undispositioned items", body = ErrorBody),
        (status = 404, description = "Return not found", body = ErrorBody),
        (status = 409, description = "Rejecting or cancelling after a disposition", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update_return(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ReturnId>,
    Json(req): Json<UpdateReturnRequest>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let input = UpdateReturn::try_from(req)?;
    let ret = commerce.returns().update(id, input)?;
    Ok(Json(ReturnResponse::from(ret)))
}

/// `PATCH /api/v1/returns/:id/reject`
///
/// Refused once any item has been dispositioned: the goods have either
/// re-entered stock or been destroyed, and a rejected return releases its
/// claim on the order line.
#[utoipa::path(
    patch,
    operation_id = "return_reject",
    path = "/api/v1/returns/{id}/reject",
    tag = "returns",
    request_body(content = Option<RejectReturnRequest>, description = "Optional rejection reason"),
    params(("id" = String, Path, description = "Return ID (UUID)")),
    responses(
        (status = 200, description = "Return rejected", body = ReturnResponse),
        (status = 400, description = "Return cannot be rejected from its current status", body = ErrorBody),
        (status = 404, description = "Return not found", body = ErrorBody),
        (status = 409, description = "An item has already been dispositioned", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, body))]
pub(crate) async fn reject_return(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ReturnId>,
    body: Option<Json<RejectReturnRequest>>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let RejectReturnRequest { reason } = body.map(|Json(b)| b).unwrap_or_default();
    let ret = commerce.returns().reject(id, &reason)?;
    Ok(Json(ReturnResponse::from(ret)))
}

/// `PATCH /api/v1/returns/:id/complete`
///
/// Settles the refund against the order's captured payments in the same
/// transaction as the status write. Requires every item to be dispositioned
/// unless the body sets `write_off_undispositioned`.
#[utoipa::path(
    patch,
    operation_id = "return_complete",
    path = "/api/v1/returns/{id}/complete",
    tag = "returns",
    request_body(content = Option<CompleteReturnRequest>, description = "Optional write-off of undispositioned items"),
    params(("id" = String, Path, description = "Return ID (UUID)")),
    responses(
        (status = 200, description = "Return completed and refund settled", body = ReturnResponse),
        (status = 400, description = "Return cannot be completed from its current status", body = ErrorBody),
        (status = 403, description = "Items are still undispositioned", body = ErrorBody),
        (status = 404, description = "Return not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, body))]
pub(crate) async fn complete_return(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ReturnId>,
    body: Option<Json<CompleteReturnRequest>>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let CompleteReturnRequest { write_off_undispositioned } =
        body.map(|Json(b)| b).unwrap_or_default();
    let ret = if write_off_undispositioned {
        commerce.returns().update(
            id,
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                write_off_undispositioned: true,
                ..Default::default()
            },
        )?
    } else {
        commerce.returns().complete(id)?
    };
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

    /// Drive a whole return over HTTP: create → approve → in transit →
    /// received → disposition → complete, and check that reject/complete/update
    /// are reachable and carry the accessor's guards.
    async fn json(resp: axum::response::Response) -> serde_json::Value {
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn patch_req(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("PATCH")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    /// A shipped single-line order, returned as (`order_id`, `order_item_id`).
    fn shipped_order(commerce: &Commerce) -> (stateset_core::OrderId, uuid::Uuid) {
        use stateset_core::{
            CreateCustomer, CreateOrder, CreateOrderItem, CreateProduct, OrderStatus, UpdateOrder,
        };
        let unique = uuid::Uuid::new_v4().simple().to_string();
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: format!("http-ret-{unique}@example.com"),
                first_name: "Ret".into(),
                last_name: "Urn".into(),
                ..Default::default()
            })
            .unwrap();
        let product = commerce
            .products()
            .create(CreateProduct { name: format!("Widget {unique}"), ..Default::default() })
            .unwrap();
        let order = commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                items: vec![CreateOrderItem {
                    product_id: product.id,
                    sku: format!("SKU-{unique}"),
                    name: "Widget".into(),
                    quantity: 2,
                    unit_price: rust_decimal_macros::dec!(10),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
            commerce
                .orders()
                .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
                .unwrap();
        }
        let order = commerce.orders().get(order.id).unwrap().unwrap();
        let item_id = order.items[0].id.into_uuid();
        (order.id, item_id)
    }

    async fn create_return_over_http(
        app: &Router,
        order_id: stateset_core::OrderId,
        item_id: uuid::Uuid,
        quantity: i32,
    ) -> serde_json::Value {
        let body = serde_json::json!({
            "order_id": order_id.into_uuid(),
            "reason": "damaged",
            "items": [{ "order_item_id": item_id, "quantity": quantity }],
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/returns")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        json(resp).await
    }

    #[tokio::test]
    async fn update_reject_and_complete_are_reachable_over_http() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let commerce = state.commerce_for_tenant(None).expect("commerce");
        let app = router().with_state(state);
        let (order_id, item_id) = shipped_order(&commerce);

        // Reject a fresh request through the dedicated route.
        let doomed = create_return_over_http(&app, order_id, item_id, 1).await;
        let resp = app
            .clone()
            .oneshot(patch_req(
                &format!("/returns/{}/reject", doomed["id"].as_str().unwrap()),
                serde_json::json!({ "reason": "not our product" }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let rejected = json(resp).await;
        assert_eq!(rejected["status"], "rejected");

        // Walk a second return to completion through the generic update route.
        let ret = create_return_over_http(&app, order_id, item_id, 2).await;
        let id = ret["id"].as_str().unwrap().to_string();
        let resp = app
            .clone()
            .oneshot(patch_req(
                &format!("/returns/{id}/approve"),
                serde_json::Value::Object(serde_json::Map::new()),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        for status in ["in_transit", "received"] {
            let resp = app
                .clone()
                .oneshot(patch_req(
                    &format!("/returns/{id}"),
                    serde_json::json!({ "status": status }),
                ))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "transition to {status}");
        }
        // refund_method is settable over the API, so settlement is reachable.
        let resp = app
            .clone()
            .oneshot(patch_req(
                &format!("/returns/{id}"),
                serde_json::json!({ "refund_method": "store_credit", "refund_amount": "20" }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Completing with an undispositioned item is refused ...
        let resp = app
            .clone()
            .oneshot(patch_req(&format!("/returns/{id}/complete"), serde_json::json!({})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // ... until the caller explicitly writes the units off.
        let resp = app
            .clone()
            .oneshot(patch_req(
                &format!("/returns/{id}/complete"),
                serde_json::json!({ "write_off_undispositioned": true }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let completed = json(resp).await;
        assert_eq!(completed["status"], "completed");
        assert_eq!(completed["refund_amount"], "20");
    }

    #[tokio::test]
    async fn update_return_rejects_invalid_status_and_refund_amount() {
        let id = ReturnId::new();
        for body in [
            serde_json::json!({ "status": "teleported" }),
            serde_json::json!({ "refund_amount": "not-a-number" }),
        ] {
            let resp = app().oneshot(patch_req(&format!("/returns/{id}"), body)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn reject_and_complete_on_a_missing_return_are_client_errors() {
        let id = ReturnId::new();
        for uri in [format!("/returns/{id}/reject"), format!("/returns/{id}/complete")] {
            let resp = app().oneshot(patch_req(&uri, serde_json::json!({}))).await.unwrap();
            assert!(resp.status().is_client_error(), "{uri}: {}", resp.status());
        }
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
