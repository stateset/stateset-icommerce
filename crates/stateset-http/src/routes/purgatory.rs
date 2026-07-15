//! Purgatory endpoints (order ingestion staging).

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
use stateset_core::{ChannelId, ProductId, PurgatoryLineItemId, PurgatoryOrderId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct IngestLineRequest {
    pub external_sku: String,
    /// Quantity as a string.
    pub quantity: String,
    pub product_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct IngestOrderRequest {
    pub channel_id: Option<String>,
    pub external_order_id: String,
    pub external_status: Option<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    pub items: Vec<IngestLineRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct MapLineRequest {
    pub product_id: Option<String>,
    pub ignore_item: Option<bool>,
    pub non_physical: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PurgatoryFilterParams {
    pub channel_id: Option<String>,
    pub is_posted: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurgatoryLineResponse {
    pub id: String,
    pub external_sku: String,
    pub product_id: Option<String>,
    pub quantity: String,
    pub ignore_item: bool,
    pub non_physical: bool,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurgatoryOrderResponse {
    pub id: String,
    pub external_order_id: String,
    pub external_status: Option<String>,
    pub is_posted: bool,
    pub ready_to_post: bool,
    pub unresolved_count: usize,
    pub items: Vec<PurgatoryLineResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PurgatoryListResponse {
    pub orders: Vec<PurgatoryOrderResponse>,
    pub total: usize,
}

fn to_resp(o: &stateset_core::PurgatoryOrder) -> PurgatoryOrderResponse {
    PurgatoryOrderResponse {
        id: o.id.to_string(),
        external_order_id: o.external_order_id.clone(),
        external_status: o.external_status.clone(),
        is_posted: o.is_posted,
        ready_to_post: o.is_ready_to_post(),
        unresolved_count: o.unresolved_count(),
        items: o
            .items
            .iter()
            .map(|i| PurgatoryLineResponse {
                id: i.id.to_string(),
                external_sku: i.external_sku.clone(),
                product_id: i.product_id.map(|p| p.to_string()),
                quantity: i.quantity.to_string(),
                ignore_item: i.ignore_item,
                non_physical: i.non_physical,
                resolved: i.is_resolved(),
            })
            .collect(),
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
        .route("/purgatory/orders", post(ingest).get(list))
        .route("/purgatory/orders/{id}", get(get_one).delete(delete_one))
        .route("/purgatory/orders/{id}/post", post(post_order))
        .route("/purgatory/orders/{id}/lines/{line_id}", post(map_line))
}

#[utoipa::path(post, operation_id = "purgatory_ingest", path = "/api/v1/purgatory/orders", tag = "purgatory",
    request_body = IngestOrderRequest,
    responses((status = 201, body = PurgatoryOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<IngestOrderRequest>,
) -> Result<(StatusCode, Json<PurgatoryOrderResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let channel_id = match req.channel_id.as_deref() {
        Some(s) => Some(parse_id::<ChannelId>(s, "channel_id")?),
        None => None,
    };
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        let product_id = match i.product_id.as_deref() {
            Some(s) => Some(parse_id::<ProductId>(s, "product_id")?),
            None => None,
        };
        items.push(stateset_core::IngestLineItem {
            external_sku: i.external_sku,
            quantity: parse_decimal(&i.quantity, "quantity")?,
            product_id,
        });
    }
    let input = stateset_core::IngestOrder {
        channel_id,
        external_order_id: req.external_order_id,
        external_status: req.external_status,
        metadata: req.metadata,
        items,
    };
    let o = c.purgatory().ingest(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&o))))
}

#[utoipa::path(get, operation_id = "purgatory_list", path = "/api/v1/purgatory/orders", tag = "purgatory",
    params(PurgatoryFilterParams),
    responses((status = 200, body = PurgatoryListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PurgatoryFilterParams>,
) -> Result<Json<PurgatoryListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let channel_id = match params.channel_id.as_deref() {
        Some(s) => Some(parse_id::<ChannelId>(s, "channel_id")?),
        None => None,
    };
    let base = stateset_core::PurgatoryFilter {
        channel_id,
        is_posted: params.is_posted,
        ..Default::default()
    };
    let total = c.purgatory().list(base.clone())?.len();
    let filter = stateset_core::PurgatoryFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let orders = c.purgatory().list(filter)?;
    Ok(Json(PurgatoryListResponse { orders: orders.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "purgatory_get_one", path = "/api/v1/purgatory/orders/{id}", tag = "purgatory",
    params(("id" = String, Path, description = "Purgatory order ID")),
    responses((status = 200, body = PurgatoryOrderResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PurgatoryOrderId>,
) -> Result<Json<PurgatoryOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let o = c
        .purgatory()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Purgatory order {id} not found")))?;
    Ok(Json(to_resp(&o)))
}

#[utoipa::path(post, operation_id = "purgatory_post_order", path = "/api/v1/purgatory/orders/{id}/post", tag = "purgatory",
    params(("id" = String, Path, description = "Purgatory order ID")),
    responses((status = 200, body = PurgatoryOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn post_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PurgatoryOrderId>,
) -> Result<Json<PurgatoryOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.purgatory().post(id)?)))
}

#[utoipa::path(post, operation_id = "purgatory_map_line", path = "/api/v1/purgatory/orders/{id}/lines/{line_id}", tag = "purgatory",
    request_body = MapLineRequest,
    params(
        ("id" = String, Path, description = "Purgatory order ID"),
        ("line_id" = String, Path, description = "Line item ID")
    ),
    responses((status = 200, body = PurgatoryOrderResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn map_line(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, line_id)): Path<(PurgatoryOrderId, PurgatoryLineItemId)>,
    Json(req): Json<MapLineRequest>,
) -> Result<Json<PurgatoryOrderResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = match req.product_id.as_deref() {
        Some(s) => Some(parse_id::<ProductId>(s, "product_id")?),
        None => None,
    };
    let input = stateset_core::MapPurgatoryLine {
        product_id,
        ignore_item: req.ignore_item,
        non_physical: req.non_physical,
    };
    Ok(Json(to_resp(&c.purgatory().map_line(id, line_id, input)?)))
}

#[utoipa::path(delete, operation_id = "purgatory_delete_one", path = "/api/v1/purgatory/orders/{id}", tag = "purgatory",
    params(("id" = String, Path, description = "Purgatory order ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PurgatoryOrderId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.purgatory().delete(id)?;
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
    async fn ingest_map_post_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "external_order_id": "SHOP-1001",
            "external_status": "paid",
            "items": [{"external_sku": "EXT-1", "quantity": "2"}]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/purgatory/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["ready_to_post"], false);
        let id = json["id"].as_str().unwrap().to_string();
        let line_id = json["items"][0]["id"].as_str().unwrap().to_string();

        // map line to a product
        let map = serde_json::json!({"product_id": uuid::Uuid::new_v4().to_string()});
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/purgatory/orders/{id}/lines/{line_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(map.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // post
        let resp = app
            .oneshot(
                Request::post(format!("/purgatory/orders/{id}/post")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["is_posted"], true);
    }
}
