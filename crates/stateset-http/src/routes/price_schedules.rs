//! Price schedule endpoints (time-bounded pricing).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{PriceScheduleId, ProductId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePriceScheduleRequest {
    pub name: String,
    pub code: Option<String>,
    /// RFC3339 window start.
    pub starts_at: Option<String>,
    /// RFC3339 window end.
    pub ends_at: Option<String>,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetEntryRequest {
    pub product_id: String,
    /// Scheduled price as a string.
    pub price: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PriceScheduleFilterParams {
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ResolveParams {
    pub product_id: String,
    /// RFC3339 instant; defaults to now when omitted.
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PriceScheduleResponse {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub currency: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub is_active: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PriceScheduleListResponse {
    pub price_schedules: Vec<PriceScheduleResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PriceScheduleEntryResponse {
    pub product_id: String,
    pub price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ResolveResponse {
    pub product_id: String,
    pub price: Option<String>,
}

fn to_resp(s: &stateset_core::PriceSchedule) -> PriceScheduleResponse {
    PriceScheduleResponse {
        id: s.id.to_string(),
        name: s.name.clone(),
        code: s.code.clone(),
        currency: s.currency.to_string(),
        starts_at: s.starts_at.map(|d| d.to_rfc3339()),
        ends_at: s.ends_at.map(|d| d.to_rfc3339()),
        is_active: s.is_active,
        priority: s.priority,
    }
}

fn entry_resp(e: &stateset_core::PriceScheduleEntry) -> PriceScheduleEntryResponse {
    PriceScheduleEntryResponse { product_id: e.product_id.to_string(), price: e.price.to_string() }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_dt(s: &str) -> Result<DateTime<Utc>, HttpError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| HttpError::BadRequest(format!("invalid RFC3339 timestamp: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/price-schedules", post(create).get(list))
        .route("/price-schedules/resolve", get(resolve))
        .route("/price-schedules/{id}", get(get_one).delete(delete_one))
        .route("/price-schedules/{id}/entries", post(set_entry).get(list_entries))
}

#[utoipa::path(post, operation_id = "price_schedules_create", path = "/api/v1/price-schedules", tag = "price_schedules",
    request_body = CreatePriceScheduleRequest,
    responses((status = 201, body = PriceScheduleResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePriceScheduleRequest>,
) -> Result<(StatusCode, Json<PriceScheduleResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let starts_at = match req.starts_at.as_deref() {
        Some(s) => Some(parse_dt(s)?),
        None => None,
    };
    let ends_at = match req.ends_at.as_deref() {
        Some(s) => Some(parse_dt(s)?),
        None => None,
    };
    let input = stateset_core::CreatePriceSchedule {
        name: req.name,
        code: req.code,
        currency: None,
        starts_at,
        ends_at,
        priority: req.priority,
    };
    let s = c.price_schedules().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&s))))
}

#[utoipa::path(get, operation_id = "price_schedules_list", path = "/api/v1/price-schedules", tag = "price_schedules",
    params(PriceScheduleFilterParams),
    responses((status = 200, body = PriceScheduleListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PriceScheduleFilterParams>,
) -> Result<Json<PriceScheduleListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let total = c
        .price_schedules()
        .list(stateset_core::PriceScheduleFilter {
            is_active: params.is_active,
            ..Default::default()
        })?
        .len();
    let filter = stateset_core::PriceScheduleFilter {
        is_active: params.is_active,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let schedules = c.price_schedules().list(filter)?;
    Ok(Json(PriceScheduleListResponse {
        price_schedules: schedules.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "price_schedules_resolve", path = "/api/v1/price-schedules/resolve", tag = "price_schedules",
    params(ResolveParams),
    responses((status = 200, body = ResolveResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn resolve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ResolveParams>,
) -> Result<Json<ResolveResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = parse_id::<ProductId>(&params.product_id, "product_id")?;
    let at = match params.at.as_deref() {
        Some(s) => parse_dt(s)?,
        None => Utc::now(),
    };
    let price = c.price_schedules().resolve_price(product_id, at)?;
    Ok(Json(ResolveResponse {
        product_id: product_id.to_string(),
        price: price.map(|p| p.to_string()),
    }))
}

#[utoipa::path(get, operation_id = "price_schedules_get_one", path = "/api/v1/price-schedules/{id}", tag = "price_schedules",
    params(("id" = String, Path, description = "Price schedule ID")),
    responses((status = 200, body = PriceScheduleResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceScheduleId>,
) -> Result<Json<PriceScheduleResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .price_schedules()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Price schedule {id} not found")))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(delete, operation_id = "price_schedules_delete_one", path = "/api/v1/price-schedules/{id}", tag = "price_schedules",
    params(("id" = String, Path, description = "Price schedule ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceScheduleId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.price_schedules().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "price_schedules_set_entry", path = "/api/v1/price-schedules/{id}/entries", tag = "price_schedules",
    request_body = SetEntryRequest,
    params(("id" = String, Path, description = "Price schedule ID")),
    responses((status = 200, body = PriceScheduleEntryResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceScheduleId>,
    Json(req): Json<SetEntryRequest>,
) -> Result<Json<PriceScheduleEntryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = parse_id::<ProductId>(&req.product_id, "product_id")?;
    let price = parse_decimal(&req.price, "price")?;
    Ok(Json(entry_resp(&c.price_schedules().set_entry(id, product_id, price)?)))
}

#[utoipa::path(get, operation_id = "price_schedules_list_entries", path = "/api/v1/price-schedules/{id}/entries", tag = "price_schedules",
    params(("id" = String, Path, description = "Price schedule ID")),
    responses((status = 200, body = [PriceScheduleEntryResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceScheduleId>,
) -> Result<Json<Vec<PriceScheduleEntryResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let entries = c.price_schedules().list_entries(id)?;
    Ok(Json(entries.iter().map(entry_resp).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_entry_and_resolve() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let product = uuid::Uuid::new_v4().to_string();
        let body = serde_json::json!({
            "name": "Sale",
            "starts_at": "2026-06-01T00:00:00Z",
            "ends_at": "2026-06-30T23:59:59Z"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/price-schedules")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = json["id"].as_str().unwrap().to_string();

        let entry_body = serde_json::json!({"product_id": product, "price": "9.99"});
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/price-schedules/{id}/entries"))
                    .header("content-type", "application/json")
                    .body(Body::from(entry_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(
                Request::get(format!(
                    "/price-schedules/resolve?product_id={product}&at=2026-06-15T12:00:00Z"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["price"], "9.99");
    }
}
