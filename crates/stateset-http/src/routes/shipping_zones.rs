//! Shipping zone endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::ShippingZoneId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateShippingZoneRequest {
    pub name: String,
    pub countries: Vec<String>,
    pub regions: Option<Vec<String>>,
    pub postal_codes: Option<Vec<String>>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ShippingZoneFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ShippingZoneResponse {
    pub id: String,
    pub name: String,
    pub countries: Vec<String>,
    pub is_active: bool,
    pub priority: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ShippingZoneListResponse {
    pub zones: Vec<ShippingZoneResponse>,
    pub total: usize,
}

fn zone_to_resp(z: &stateset_core::ShippingZone) -> ShippingZoneResponse {
    ShippingZoneResponse {
        id: z.id.to_string(),
        name: z.name.clone(),
        countries: z.countries.clone(),
        is_active: z.is_active,
        priority: z.priority,
        created_at: z.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/shipping-zones", post(create_zone).get(list_zones))
        .route("/shipping-zones/{id}", get(get_zone).delete(delete_zone))
}

#[utoipa::path(post, path = "/api/v1/shipping-zones", tag = "shipping",
    request_body = CreateShippingZoneRequest,
    responses((status = 201, body = ShippingZoneResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateShippingZoneRequest>,
) -> Result<(StatusCode, Json<ShippingZoneResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateShippingZone {
        name: req.name,
        countries: req.countries,
        regions: req.regions.unwrap_or_default(),
        postal_codes: req.postal_codes.unwrap_or_default(),
        priority: req.priority,
    };
    let z = c.shipping_zones().create(input)?;
    Ok((StatusCode::CREATED, Json(zone_to_resp(&z))))
}

#[utoipa::path(get, path = "/api/v1/shipping-zones", tag = "shipping",
    params(ShippingZoneFilterParams),
    responses((status = 200, body = ShippingZoneListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_zones(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ShippingZoneFilterParams>,
) -> Result<Json<ShippingZoneListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;

    // Count the full matching set before applying the page window so that
    // `total` reflects the number of matching zones, not the page size.
    let count_filter = stateset_core::ShippingZoneFilter::default();
    let total = c.shipping_zones().list(count_filter)?.len();

    let filter = stateset_core::ShippingZoneFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let zones = c.shipping_zones().list(filter)?;
    Ok(Json(ShippingZoneListResponse { zones: zones.iter().map(zone_to_resp).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/shipping-zones/{id}", tag = "shipping",
    params(("id" = String, Path, description = "Shipping zone ID")),
    responses((status = 200, body = ShippingZoneResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ShippingZoneId>,
) -> Result<Json<ShippingZoneResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let z = c
        .shipping_zones()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Zone {id} not found")))?;
    Ok(Json(zone_to_resp(&z)))
}

#[utoipa::path(delete, path = "/api/v1/shipping-zones/{id}", tag = "shipping",
    params(("id" = String, Path, description = "Shipping zone ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ShippingZoneId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.shipping_zones().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
    }

    /// Create a shipping zone via the embedded commerce backend.
    ///
    /// The `shipping_zones` table is provisioned by SQLite migration
    /// `037_commerce_entities`, so creation must succeed against the live
    /// in-memory schema (it previously failed because the table was only
    /// created in the PostgreSQL backend and in `#[cfg(test)]` blocks).
    fn create_zone(state: &AppState, name: &str) {
        state
            .commerce()
            .shipping_zones()
            .create(stateset_core::CreateShippingZone {
                name: name.to_string(),
                countries: vec!["US".to_string()],
                regions: Vec::new(),
                postal_codes: Vec::new(),
                priority: None,
            })
            .expect("create shipping zone against live SQLite schema");
    }

    #[tokio::test]
    async fn list_zones_reports_total_before_pagination() {
        let (app, state) = app_with_state();

        // Create more zones than the requested page size.
        let created = 5u32;
        for i in 0..created {
            create_zone(&state, &format!("zone-{i}"));
        }

        let resp = app
            .oneshot(Request::get("/shipping-zones?limit=2&offset=0").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // `total` must reflect the full matching set, not the clamped page size.
        assert_eq!(json["total"], created);
        assert_eq!(json["zones"].as_array().unwrap().len(), usize::min(2, created as usize));
    }
}
