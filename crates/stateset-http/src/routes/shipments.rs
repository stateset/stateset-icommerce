//! Shipment endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};

use crate::dto::{
    CreateShipmentRequest, ShipmentFilterParams, ShipmentListResponse, ShipmentResponse,
    finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{
    CreateShipment, OrderId, ShipmentFilter, ShipmentId, ShipmentStatus, ShippingCarrier,
};
use std::str::FromStr;

/// Build the shipments sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/shipments", post(create_shipment).get(list_shipments))
        .route("/shipments/{id}", get(get_shipment))
        .route("/shipments/{id}/deliver", post(deliver_shipment))
}

/// `GET /api/v1/shipments/:id`
#[utoipa::path(
    get,
    path = "/api/v1/shipments/{id}",
    tag = "shipments",
    params(("id" = String, Path, description = "Shipment ID (UUID)")),
    responses(
        (status = 200, description = "Shipment details", body = ShipmentResponse),
        (status = 404, description = "Shipment not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_shipment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ShipmentId>,
) -> Result<Json<ShipmentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let shipment = commerce
        .shipments()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Shipment {id} not found")))?;
    Ok(Json(ShipmentResponse::from(shipment)))
}

/// `GET /api/v1/shipments`
#[utoipa::path(
    get,
    path = "/api/v1/shipments",
    tag = "shipments",
    params(ShipmentFilterParams),
    responses(
        (status = 200, description = "List of shipments", body = ShipmentListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_shipments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ShipmentFilterParams>,
) -> Result<Json<ShipmentListResponse>, HttpError> {
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
    let status = params
        .status
        .as_deref()
        .map(ShipmentStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}. Valid values: pending, processing, shipped, in_transit, delivered, failed, cancelled")))?;
    let carrier = params
        .carrier
        .as_deref()
        .map(ShippingCarrier::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid carrier: {e}. Valid values: fedex, ups, usps, dhl, other")))?;

    // Count total matching records (without pagination)
    let count_filter = ShipmentFilter {
        order_id,
        status,
        carrier,
        tracking_number: params.tracking_number.clone(),
        limit: None,
        offset: None,
    };
    let total = commerce.shipments().list(count_filter)?.len();

    // Fetch the requested page
    let filter = ShipmentFilter {
        order_id,
        status,
        carrier,
        tracking_number: params.tracking_number,
        limit: Some(overfetch_limit(limit)),
        offset: Some(offset),
    };
    let mut shipments = commerce.shipments().list(filter)?;
    let has_more = finalize_page(&mut shipments, limit);
    Ok(Json(ShipmentListResponse {
        shipments: shipments.into_iter().map(ShipmentResponse::from).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `POST /api/v1/shipments`
#[utoipa::path(
    post,
    path = "/api/v1/shipments",
    tag = "shipments",
    request_body = CreateShipmentRequest,
    responses(
        (status = 201, description = "Shipment created", body = ShipmentResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_shipment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateShipmentRequest>,
) -> Result<(StatusCode, Json<ShipmentResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let carrier = req
        .carrier
        .as_deref()
        .map(ShippingCarrier::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid carrier: {e}. Valid values: fedex, ups, usps, dhl, other")))?;

    let input = CreateShipment {
        order_id: req.order_id,
        carrier,
        tracking_number: req.tracking_number,
        recipient_name: req.recipient_name.unwrap_or_default(),
        notes: req.notes,
        ..Default::default()
    };
    let shipment = commerce.shipments().create(input)?;
    Ok((StatusCode::CREATED, Json(ShipmentResponse::from(shipment))))
}

/// `POST /api/v1/shipments/:id/deliver`
#[utoipa::path(
    post,
    path = "/api/v1/shipments/{id}/deliver",
    tag = "shipments",
    params(("id" = String, Path, description = "Shipment ID (UUID)")),
    responses(
        (status = 200, description = "Shipment marked as delivered", body = ShipmentResponse),
        (status = 404, description = "Shipment not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn deliver_shipment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ShipmentId>,
) -> Result<Json<ShipmentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let shipment = commerce.shipments().mark_delivered(id)?;
    Ok(Json(ShipmentResponse::from(shipment)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    #[tokio::test]
    async fn get_shipment_not_found() {
        let id = ShipmentId::new();
        let resp = app()
            .oneshot(Request::get(format!("/shipments/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_shipments_empty() {
        let resp =
            app().oneshot(Request::get("/shipments").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["shipments"].as_array().unwrap().is_empty());
        assert_eq!(json["has_more"], false);
    }

    #[tokio::test]
    async fn list_shipments_invalid_status_returns_400() {
        let resp = app()
            .oneshot(Request::get("/shipments?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_shipments_invalid_order_id_returns_400() {
        let resp = app()
            .oneshot(Request::get("/shipments?order_id=not-a-uuid").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_shipments_invalid_carrier_returns_400() {
        let resp = app()
            .oneshot(Request::get("/shipments?carrier=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_shipments_with_pagination() {
        let resp = app()
            .oneshot(Request::get("/shipments?limit=10&offset=5").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 5);
    }
}
