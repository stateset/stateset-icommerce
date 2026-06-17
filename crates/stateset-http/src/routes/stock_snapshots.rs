//! Stock snapshot endpoints (point-in-time inventory).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{ProductId, StockSnapshotId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CaptureLineRequest {
    pub product_id: String,
    pub sku: String,
    /// On-hand quantity as a string.
    pub quantity_on_hand: String,
    /// Available quantity as a string.
    pub quantity_available: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CaptureRequest {
    pub label: Option<String>,
    pub lines: Vec<CaptureLineRequest>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SnapshotFilterParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SnapshotLineResponse {
    pub product_id: String,
    pub sku: String,
    pub quantity_on_hand: String,
    pub quantity_available: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SnapshotResponse {
    pub id: String,
    pub label: Option<String>,
    pub total_skus: u64,
    pub total_units: String,
    pub captured_at: String,
    pub lines: Vec<SnapshotLineResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SnapshotSummary {
    pub id: String,
    pub label: Option<String>,
    pub total_skus: u64,
    pub total_units: String,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SnapshotListResponse {
    pub snapshots: Vec<SnapshotSummary>,
    pub total: usize,
}

fn to_resp(s: &stateset_core::StockSnapshot) -> SnapshotResponse {
    SnapshotResponse {
        id: s.id.to_string(),
        label: s.label.clone(),
        total_skus: s.total_skus,
        total_units: s.total_units.to_string(),
        captured_at: s.captured_at.to_rfc3339(),
        lines: s
            .lines
            .iter()
            .map(|l| SnapshotLineResponse {
                product_id: l.product_id.to_string(),
                sku: l.sku.clone(),
                quantity_on_hand: l.quantity_on_hand.to_string(),
                quantity_available: l.quantity_available.to_string(),
                location: l.location.clone(),
            })
            .collect(),
    }
}

fn summary(s: &stateset_core::StockSnapshot) -> SnapshotSummary {
    SnapshotSummary {
        id: s.id.to_string(),
        label: s.label.clone(),
        total_skus: s.total_skus,
        total_units: s.total_units.to_string(),
        captured_at: s.captured_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stock-snapshots", post(capture).get(list))
        .route("/stock-snapshots/latest", get(latest))
        .route("/stock-snapshots/{id}", get(get_one).delete(delete_one))
}

#[utoipa::path(post, path = "/api/v1/stock-snapshots", tag = "stock_snapshots",
    request_body = CaptureRequest,
    responses((status = 201, body = SnapshotResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CaptureRequest>,
) -> Result<(StatusCode, Json<SnapshotResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut lines = Vec::with_capacity(req.lines.len());
    for l in req.lines {
        lines.push(stateset_core::CaptureStockLine {
            product_id: parse_id::<ProductId>(&l.product_id, "product_id")?,
            sku: l.sku,
            quantity_on_hand: parse_decimal(&l.quantity_on_hand, "quantity_on_hand")?,
            quantity_available: parse_decimal(&l.quantity_available, "quantity_available")?,
            location: l.location,
        });
    }
    let input = stateset_core::CaptureStockSnapshot { label: req.label, lines };
    let s = c.stock_snapshots().capture(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&s))))
}

#[utoipa::path(get, path = "/api/v1/stock-snapshots", tag = "stock_snapshots",
    params(SnapshotFilterParams),
    responses((status = 200, body = SnapshotListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SnapshotFilterParams>,
) -> Result<Json<SnapshotListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let total = c.stock_snapshots().list(stateset_core::StockSnapshotFilter::default())?.len();
    let filter = stateset_core::StockSnapshotFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let snapshots = c.stock_snapshots().list(filter)?;
    Ok(Json(SnapshotListResponse { snapshots: snapshots.iter().map(summary).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/stock-snapshots/latest", tag = "stock_snapshots",
    responses((status = 200, body = SnapshotResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn latest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SnapshotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .stock_snapshots()
        .latest()?
        .ok_or_else(|| HttpError::NotFound("no stock snapshots captured yet".into()))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(get, path = "/api/v1/stock-snapshots/{id}", tag = "stock_snapshots",
    params(("id" = String, Path, description = "Stock snapshot ID")),
    responses((status = 200, body = SnapshotResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StockSnapshotId>,
) -> Result<Json<SnapshotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .stock_snapshots()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Stock snapshot {id} not found")))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(delete, path = "/api/v1/stock-snapshots/{id}", tag = "stock_snapshots",
    params(("id" = String, Path, description = "Stock snapshot ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<StockSnapshotId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.stock_snapshots().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn capture_then_get() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "label": "EOM",
            "lines": [
                {"product_id": uuid::Uuid::new_v4().to_string(), "sku":"SKU-1", "quantity_on_hand":"10", "quantity_available":"8"},
                {"product_id": uuid::Uuid::new_v4().to_string(), "sku":"SKU-2", "quantity_on_hand":"5", "quantity_available":"5"}
            ]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/stock-snapshots")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total_skus"], 2);
        assert_eq!(json["total_units"], "15");
        assert_eq!(json["lines"].as_array().unwrap().len(), 2);
    }
}
