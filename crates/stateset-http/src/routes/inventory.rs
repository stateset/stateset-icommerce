//! Inventory endpoints: stock levels, adjustments, **reservations**, and the
//! operator-facing stock sweep.
//!
//! Reservations are the engine's oversell guard, but until now they were
//! reachable only from `stateset-embedded` — this router exposed no way to
//! take, read, release or confirm a hold over HTTP, and no way to reclaim
//! expired ones. `/inventory/reservations*` and `/inventory/sweeps/run` close
//! that gap.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::dto::{
    InventoryAdjustRequest, InventoryFilterParams, InventoryItemResponse, InventoryListResponse,
    InventoryResponse, finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use crate::sweeps::{SweepConfig, SweepRunReport, run_sweeps_now};
use stateset_core::{InventoryFilter, InventoryReservation};

/// Default cap on reservations expired by one `POST /inventory/reservations/expire`.
const DEFAULT_EXPIRE_LIMIT: u32 = 500;
/// Hard cap on that limit, so one request cannot hold a huge transaction.
const MAX_EXPIRE_LIMIT: u32 = 5_000;

/// Build the inventory sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/inventory", get(list_inventory))
        // Static segments are registered before `/inventory/{sku}` so a SKU
        // can never shadow them (axum/matchit prefers the static match).
        .route("/inventory/reservations", get(list_reservations))
        .route("/inventory/reservations/expire", post(expire_reservations))
        .route("/inventory/reservations/{reservation_id}", get(get_reservation))
        .route("/inventory/reservations/{reservation_id}/release", post(release_reservation))
        .route("/inventory/reservations/{reservation_id}/confirm", post(confirm_reservation))
        .route("/inventory/sweeps/run", post(run_sweeps))
        .route("/inventory/{sku}", get(get_stock))
        .route("/inventory/{sku}/adjust", post(adjust_stock))
        .route("/inventory/{sku}/reservations", post(create_reservation))
}

/// One inventory reservation.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReservationResponse {
    /// Reservation id.
    pub id: Uuid,
    /// Inventory item the hold is against.
    pub item_id: i64,
    /// Location the units are held at.
    pub location_id: i32,
    /// Units held.
    #[schema(value_type = String)]
    pub quantity: Decimal,
    /// `pending`, `confirmed`, `allocated`, `released`, `expired` or `cancelled`.
    pub status: String,
    /// What the hold is for (`order`, `cart`, `backorder`, …).
    pub reference_type: String,
    /// Id of that thing.
    pub reference_id: String,
    /// When the hold lapses, if it is time-boxed.
    pub expires_at: Option<DateTime<Utc>>,
    /// When the hold was taken.
    pub created_at: DateTime<Utc>,
}

impl From<InventoryReservation> for ReservationResponse {
    fn from(reservation: InventoryReservation) -> Self {
        Self {
            id: reservation.id,
            item_id: reservation.item_id,
            location_id: reservation.location_id,
            quantity: reservation.quantity,
            status: reservation.status.to_string(),
            reference_type: reservation.reference_type,
            reference_id: reservation.reference_id,
            expires_at: reservation.expires_at,
            created_at: reservation.created_at,
        }
    }
}

/// Take a hold on stock.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateReservationRequest {
    /// Units to hold. Must be positive and available.
    #[schema(value_type = String)]
    pub quantity: Decimal,
    /// What the hold is for (`order`, `cart`, …).
    pub reference_type: String,
    /// Id of that thing.
    pub reference_id: String,
    /// Time-box the hold; omit for an open-ended one.
    pub expires_in_seconds: Option<i64>,
}

/// Filter for `GET /inventory/reservations`.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ReservationFilterParams {
    /// What the holds are for (`order`, `cart`, …).
    pub reference_type: String,
    /// Id of that thing.
    pub reference_id: String,
}

/// A page of reservations.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReservationListResponse {
    /// The reservations, oldest first.
    pub items: Vec<ReservationResponse>,
    /// How many were returned.
    pub total: usize,
}

/// How many expired holds to reclaim in one call.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ExpireReservationsRequest {
    /// Maximum reservations to expire (default 500, capped at 5000).
    pub limit: Option<u32>,
}

/// What an expiry sweep reclaimed.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExpireReservationsResponse {
    /// Reservations flipped to `expired`, their units back in
    /// `quantity_available`.
    pub expired: u64,
    /// Whether the batch came back full, i.e. more may be waiting.
    pub has_more: bool,
}

/// `GET /api/v1/inventory/:sku`
#[utoipa::path(
    get,
    path = "/api/v1/inventory/{sku}",
    tag = "inventory",
    params(("sku" = String, Path, description = "Product SKU")),
    responses(
        (status = 200, description = "Stock levels", body = InventoryResponse),
        (status = 404, description = "SKU not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_stock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Result<Json<InventoryResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let stock = commerce
        .inventory()
        .get_stock(&sku)?
        .ok_or_else(|| HttpError::NotFound(format!("Inventory item {sku} not found")))?;
    Ok(Json(InventoryResponse::from(stock)))
}

/// `POST /api/v1/inventory/:sku/adjust`
#[utoipa::path(
    post,
    path = "/api/v1/inventory/{sku}/adjust",
    tag = "inventory",
    params(("sku" = String, Path, description = "Product SKU")),
    request_body = InventoryAdjustRequest,
    responses(
        (status = 200, description = "Stock adjusted", body = InventoryResponse),
        (status = 404, description = "SKU not found", body = ErrorBody),
        (status = 422, description = "Validation error", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn adjust_stock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
    Json(req): Json<InventoryAdjustRequest>,
) -> Result<Json<InventoryResponse>, HttpError> {
    if req.location_id.is_some() {
        return Err(HttpError::ValidationError(
            "location_id is not supported by /inventory/:sku/adjust; omit location_id".to_string(),
        ));
    }

    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    // Perform the adjustment
    commerce.inventory().adjust(&sku, req.quantity, &req.reason)?;

    // Fetch updated stock levels
    let stock = commerce
        .inventory()
        .get_stock(&sku)?
        .ok_or_else(|| HttpError::NotFound(format!("Inventory item {sku} not found")))?;
    Ok(Json(InventoryResponse::from(stock)))
}

/// `GET /api/v1/inventory`
#[utoipa::path(
    get,
    path = "/api/v1/inventory",
    tag = "inventory",
    params(InventoryFilterParams),
    responses(
        (status = 200, description = "List of inventory items", body = InventoryListResponse),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<InventoryFilterParams>,
) -> Result<Json<InventoryListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    // Count total matching records (without pagination)
    let count_filter = InventoryFilter {
        sku: params.sku.clone(),
        location_id: None,
        below_reorder_point: params.below_reorder_point,
        is_active: params.is_active,
        limit: None,
        offset: None,
    };
    let total = commerce.inventory().list(count_filter)?.len();

    // Fetch the requested page
    let filter = InventoryFilter {
        sku: params.sku,
        location_id: None,
        below_reorder_point: params.below_reorder_point,
        is_active: params.is_active,
        limit: Some(overfetch_limit(limit)),
        offset: Some(offset),
    };
    let mut items = commerce.inventory().list(filter)?;
    let has_more = finalize_page(&mut items, limit);
    Ok(Json(InventoryListResponse {
        items: items.into_iter().map(InventoryItemResponse::from).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

// ---------------------------------------------------------------------------
// Reservations
// ---------------------------------------------------------------------------

/// `POST /api/v1/inventory/:sku/reservations`
#[utoipa::path(
    post,
    path = "/api/v1/inventory/{sku}/reservations",
    tag = "inventory",
    params(("sku" = String, Path, description = "Product SKU")),
    request_body = CreateReservationRequest,
    responses(
        (status = 201, description = "Stock held", body = ReservationResponse),
        (status = 404, description = "SKU not found", body = ErrorBody),
        (status = 409, description = "Insufficient stock", body = ErrorBody),
        (status = 422, description = "Validation error", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
    Json(req): Json<CreateReservationRequest>,
) -> Result<(axum::http::StatusCode, Json<ReservationResponse>), HttpError> {
    if req.reference_type.trim().is_empty() || req.reference_id.trim().is_empty() {
        return Err(HttpError::ValidationError(
            "reference_type and reference_id are required".to_string(),
        ));
    }
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let reservation = commerce.inventory().reserve(
        &sku,
        req.quantity,
        req.reference_type.trim(),
        req.reference_id.trim(),
        req.expires_in_seconds,
    )?;
    Ok((axum::http::StatusCode::CREATED, Json(ReservationResponse::from(reservation))))
}

/// `GET /api/v1/inventory/reservations/:reservation_id`
#[utoipa::path(
    get,
    path = "/api/v1/inventory/reservations/{reservation_id}",
    tag = "inventory",
    params(("reservation_id" = String, Path, description = "Reservation id")),
    responses(
        (status = 200, description = "The reservation", body = ReservationResponse),
        (status = 404, description = "Reservation not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reservation_id): Path<Uuid>,
) -> Result<Json<ReservationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let reservation = commerce
        .inventory()
        .get_reservation(reservation_id)?
        .ok_or_else(|| HttpError::NotFound(format!("Reservation {reservation_id} not found")))?;
    Ok(Json(ReservationResponse::from(reservation)))
}

/// `GET /api/v1/inventory/reservations?reference_type=order&reference_id=...`
#[utoipa::path(
    get,
    path = "/api/v1/inventory/reservations",
    tag = "inventory",
    params(ReservationFilterParams),
    responses(
        (status = 200, description = "Reservations for the reference", body = ReservationListResponse),
        (status = 422, description = "Validation error", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_reservations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ReservationFilterParams>,
) -> Result<Json<ReservationListResponse>, HttpError> {
    if params.reference_type.trim().is_empty() || params.reference_id.trim().is_empty() {
        return Err(HttpError::ValidationError(
            "reference_type and reference_id are required".to_string(),
        ));
    }
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let items: Vec<ReservationResponse> = commerce
        .inventory()
        .list_reservations_by_reference(params.reference_type.trim(), params.reference_id.trim())?
        .into_iter()
        .map(ReservationResponse::from)
        .collect();
    Ok(Json(ReservationListResponse { total: items.len(), items }))
}

/// `POST /api/v1/inventory/reservations/:reservation_id/release`
#[utoipa::path(
    post,
    path = "/api/v1/inventory/reservations/{reservation_id}/release",
    tag = "inventory",
    params(("reservation_id" = String, Path, description = "Reservation id")),
    responses(
        (status = 200, description = "Hold released (idempotent)", body = ReservationResponse),
        (status = 404, description = "Reservation not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn release_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reservation_id): Path<Uuid>,
) -> Result<Json<ReservationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    commerce.inventory().release_reservation(reservation_id)?;
    let reservation = commerce
        .inventory()
        .get_reservation(reservation_id)?
        .ok_or_else(|| HttpError::NotFound(format!("Reservation {reservation_id} not found")))?;
    Ok(Json(ReservationResponse::from(reservation)))
}

/// `POST /api/v1/inventory/reservations/:reservation_id/confirm`
#[utoipa::path(
    post,
    path = "/api/v1/inventory/reservations/{reservation_id}/confirm",
    tag = "inventory",
    params(("reservation_id" = String, Path, description = "Reservation id")),
    responses(
        (status = 200, description = "Hold confirmed (idempotent)", body = ReservationResponse),
        (status = 404, description = "Reservation not found", body = ErrorBody),
        (status = 409, description = "The hold already expired", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn confirm_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reservation_id): Path<Uuid>,
) -> Result<Json<ReservationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    commerce.inventory().confirm_reservation(reservation_id)?;
    let reservation = commerce
        .inventory()
        .get_reservation(reservation_id)?
        .ok_or_else(|| HttpError::NotFound(format!("Reservation {reservation_id} not found")))?;
    Ok(Json(ReservationResponse::from(reservation)))
}

/// `POST /api/v1/inventory/reservations/expire`
///
/// Reclaim expired inventory holds without waiting for the background sweep.
#[utoipa::path(
    post,
    path = "/api/v1/inventory/reservations/expire",
    tag = "inventory",
    request_body = ExpireReservationsRequest,
    responses(
        (status = 200, description = "What the sweep reclaimed", body = ExpireReservationsResponse),
        (status = 422, description = "Validation error", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn expire_reservations(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Option<Json<ExpireReservationsRequest>>,
) -> Result<Json<ExpireReservationsResponse>, HttpError> {
    let limit = req
        .and_then(|Json(body)| body.limit)
        .unwrap_or(DEFAULT_EXPIRE_LIMIT)
        .clamp(1, MAX_EXPIRE_LIMIT);
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let expired = commerce.inventory().expire_reservations(Utc::now(), limit)?;
    Ok(Json(ExpireReservationsResponse { expired, has_more: expired >= u64::from(limit) }))
}

/// `POST /api/v1/inventory/sweeps/run`
///
/// Run both engine sweeps — stock holds (inventory reservations + backorder
/// allocations) and traceability (lot expiry, lot and serial reservations) —
/// once, right now, and report what each reclaimed. The same jobs run on the
/// server's background scheduler; this is the operator's manual trigger.
#[utoipa::path(
    post,
    path = "/api/v1/inventory/sweeps/run",
    tag = "inventory",
    responses(
        (status = 200, description = "What each sweep reclaimed", body = SweepRunReport),
        (status = 500, description = "The sweep runner could not start", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn run_sweeps(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SweepRunReport>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    Ok(Json(run_sweeps_now(commerce, SweepConfig::default()).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use rust_decimal_macros::dec;
    use stateset_core::CreateInventoryItem;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
    }

    fn app() -> Router {
        let (router, _) = app_with_state();
        router
    }

    #[tokio::test]
    async fn get_stock_not_found() {
        let resp = app()
            .oneshot(Request::get("/inventory/NONEXISTENT").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_and_get_stock() {
        let (app, state) = app_with_state();

        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "WIDGET-001".into(),
                name: "Widget".into(),
                initial_quantity: Some(dec!(100)),
                ..Default::default()
            })
            .unwrap();

        let resp = app
            .oneshot(Request::get("/inventory/WIDGET-001").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["sku"], "WIDGET-001");
    }

    #[tokio::test]
    async fn adjust_stock_works() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));

        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "ADJ-001".into(),
                name: "Adjustable Widget".into(),
                initial_quantity: Some(dec!(50)),
                ..Default::default()
            })
            .unwrap();

        let app = router().with_state(state);

        let body = serde_json::json!({
            "quantity": "-10",
            "reason": "Damaged stock removal"
        });
        let resp = app
            .oneshot(
                Request::post("/inventory/ADJ-001/adjust")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json["sku"], "ADJ-001");
    }

    #[tokio::test]
    async fn adjust_stock_rejects_location_id() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "LOC-001".into(),
                name: "Location Item".into(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .unwrap();

        let app = router().with_state(state);

        let body = serde_json::json!({
            "quantity": "-1",
            "reason": "manual",
            "location_id": 42
        });
        let resp = app
            .oneshot(
                Request::post("/inventory/LOC-001/adjust")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json["error"]["code"], "validation_error");
    }

    // -----------------------------------------------------------------
    // Reservations + sweeps
    // -----------------------------------------------------------------

    fn seed_stock(state: &AppState, sku: &str, qty: rust_decimal::Decimal) {
        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: sku.into(),
                name: format!("Item {sku}"),
                initial_quantity: Some(qty),
                ..Default::default()
            })
            .expect("create item");
    }

    async fn json_of(resp: axum::response::Response) -> serde_json::Value {
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn post(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::post(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn reserve_read_release_round_trip() {
        let (_, state) = app_with_state();
        seed_stock(&state, "RES-001", dec!(10));

        let resp = router()
            .with_state(state.clone())
            .oneshot(post(
                "/inventory/RES-001/reservations",
                serde_json::json!({
                    "quantity": "4",
                    "reference_type": "cart",
                    "reference_id": "cart-1",
                    "expires_in_seconds": 3600
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let created = json_of(resp).await;
        let id = created["id"].as_str().expect("id").to_owned();
        assert_eq!(created["status"], "pending");
        assert_eq!(
            state.commerce().inventory().get_stock("RES-001").unwrap().unwrap().total_available,
            dec!(6)
        );

        // The hold is readable back...
        let resp = router()
            .with_state(state.clone())
            .oneshot(
                Request::get(format!("/inventory/reservations/{id}")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["reference_id"], "cart-1");

        // ... listable by reference ...
        let resp = router()
            .with_state(state.clone())
            .oneshot(
                Request::get("/inventory/reservations?reference_type=cart&reference_id=cart-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["total"], 1);

        // ... and releasable, which hands the units back.
        let resp = router()
            .with_state(state.clone())
            .oneshot(post(&format!("/inventory/reservations/{id}/release"), serde_json::json!({})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["status"], "released");
        assert_eq!(
            state.commerce().inventory().get_stock("RES-001").unwrap().unwrap().total_available,
            dec!(10)
        );
    }

    #[tokio::test]
    async fn confirm_reservation_keeps_the_units_allocated() {
        let (_, state) = app_with_state();
        seed_stock(&state, "RES-002", dec!(5));
        let reservation =
            state.commerce().inventory().reserve("RES-002", dec!(2), "order", "o1", None).unwrap();

        let resp = router()
            .with_state(state.clone())
            .oneshot(post(
                &format!("/inventory/reservations/{}/confirm", reservation.id),
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["status"], "confirmed");
        let stock = state.commerce().inventory().get_stock("RES-002").unwrap().unwrap();
        assert_eq!((stock.total_allocated, stock.total_available), (dec!(2), dec!(3)));
    }

    #[tokio::test]
    async fn reserving_more_than_available_is_rejected() {
        let (_, state) = app_with_state();
        seed_stock(&state, "RES-003", dec!(1));
        let resp = router()
            .with_state(state)
            .oneshot(post(
                "/inventory/RES-003/reservations",
                serde_json::json!({
                    "quantity": "5",
                    "reference_type": "cart",
                    "reference_id": "cart-9"
                }),
            ))
            .await
            .unwrap();
        assert_ne!(resp.status(), StatusCode::CREATED);
        assert!(resp.status().is_client_error(), "got {}", resp.status());
    }

    #[tokio::test]
    async fn get_reservation_not_found() {
        let resp = app()
            .oneshot(
                Request::get(format!("/inventory/reservations/{}", uuid::Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_sku_cannot_shadow_the_reservations_routes() {
        // `/inventory/reservations` must reach the list handler, not
        // `get_stock("reservations")`.
        let resp = app()
            .oneshot(
                Request::get("/inventory/reservations?reference_type=cart&reference_id=x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["total"], 0);
    }

    #[tokio::test]
    async fn expire_endpoint_reclaims_an_idle_expired_hold() {
        let (_, state) = app_with_state();
        seed_stock(&state, "RES-EXP", dec!(10));
        state.commerce().inventory().reserve("RES-EXP", dec!(4), "cart", "c1", Some(1)).unwrap();
        assert_eq!(
            state.commerce().inventory().get_stock("RES-EXP").unwrap().unwrap().total_allocated,
            dec!(4)
        );
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let resp = router()
            .with_state(state.clone())
            .oneshot(post("/inventory/reservations/expire", serde_json::json!({ "limit": 100 })))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_of(resp).await;
        assert_eq!(body["expired"], 1);
        assert_eq!(body["has_more"], false);
        assert_eq!(
            state.commerce().inventory().get_stock("RES-EXP").unwrap().unwrap().total_available,
            dec!(10)
        );
    }

    #[tokio::test]
    async fn sweep_endpoint_reports_what_each_sweep_reclaimed() {
        let (_, state) = app_with_state();
        seed_stock(&state, "RES-SWEEP", dec!(10));
        state.commerce().inventory().reserve("RES-SWEEP", dec!(3), "cart", "c1", Some(1)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let resp = router()
            .with_state(state.clone())
            .oneshot(post("/inventory/sweeps/run", serde_json::json!({})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_of(resp).await;
        let sweeps = body["sweeps"].as_array().expect("sweeps");
        assert_eq!(sweeps.len(), 2, "both engine sweeps must run: {body}");
        let reservation = sweeps
            .iter()
            .find(|s| s["job"] == "reservation_sweep")
            .expect("the stock sweep must be reported");
        assert_eq!(reservation["ok"], true, "{body}");
        assert_eq!(reservation["reclaimed"]["inventory_reservations_expired"], 1);
        let traceability = sweeps
            .iter()
            .find(|s| s["job"] == "traceability_sweep")
            .expect("the traceability sweep must be reported");
        assert_eq!(traceability["ok"], true, "{body}");
        assert!(traceability["reclaimed"]["lots_expired"].is_number());

        assert_eq!(
            state.commerce().inventory().get_stock("RES-SWEEP").unwrap().unwrap().total_available,
            dec!(10)
        );
    }

    #[tokio::test]
    async fn list_inventory_empty() {
        let resp =
            app().oneshot(Request::get("/inventory").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["items"].as_array().unwrap().is_empty());
        assert_eq!(json["has_more"], false);
    }

    #[tokio::test]
    async fn list_inventory_with_items() {
        let (_, state) = app_with_state();

        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "LIST-001".into(),
                name: "List Item 1".into(),
                initial_quantity: Some(dec!(50)),
                ..Default::default()
            })
            .unwrap();
        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "LIST-002".into(),
                name: "List Item 2".into(),
                initial_quantity: Some(dec!(25)),
                ..Default::default()
            })
            .unwrap();

        let app = router().with_state(state);
        let resp =
            app.oneshot(Request::get("/inventory").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn list_inventory_with_pagination() {
        let resp = app()
            .oneshot(Request::get("/inventory?limit=10&offset=5").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 5);
    }
}
