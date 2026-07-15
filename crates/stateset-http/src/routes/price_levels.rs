//! Price level endpoints (B2B pricing tiers).

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
use stateset_core::{PriceLevelId, ProductId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreatePriceLevelRequest {
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    /// One of `none`, `percentage_discount`, `percentage_markup`.
    pub adjustment_type: Option<String>,
    /// Percentage value as a string (e.g. `"10"`).
    pub adjustment_value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdatePriceLevelRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub adjustment_type: Option<String>,
    pub adjustment_value: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetEntryRequest {
    pub product_id: String,
    /// Fixed price as a string.
    pub price: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PriceLevelFilterParams {
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PriceLevelResponse {
    pub id: String,
    pub name: String,
    pub code: String,
    pub adjustment_type: String,
    pub adjustment_value: String,
    pub currency: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PriceLevelListResponse {
    pub price_levels: Vec<PriceLevelResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PriceLevelEntryResponse {
    pub product_id: String,
    pub price: String,
}

fn to_resp(l: &stateset_core::PriceLevel) -> PriceLevelResponse {
    PriceLevelResponse {
        id: l.id.to_string(),
        name: l.name.clone(),
        code: l.code.clone(),
        adjustment_type: l.adjustment_type.to_string(),
        adjustment_value: l.adjustment_value.to_string(),
        currency: l.currency.to_string(),
        is_active: l.is_active,
    }
}

fn entry_resp(e: &stateset_core::PriceLevelEntry) -> PriceLevelEntryResponse {
    PriceLevelEntryResponse { product_id: e.product_id.to_string(), price: e.price.to_string() }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/price-levels", post(create).get(list))
        .route("/price-levels/{id}", get(get_one).put(update).delete(delete_one))
        .route("/price-levels/{id}/entries", post(set_entry).get(list_entries))
}

#[utoipa::path(post, operation_id = "price_levels_create", path = "/api/v1/price-levels", tag = "price_levels",
    request_body = CreatePriceLevelRequest,
    responses((status = 201, body = PriceLevelResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePriceLevelRequest>,
) -> Result<(StatusCode, Json<PriceLevelResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let adjustment_type = match req.adjustment_type.as_deref() {
        Some(s) => parse_id(s, "adjustment_type")?,
        None => stateset_core::PriceAdjustmentType::default(),
    };
    let adjustment_value = match req.adjustment_value.as_deref() {
        Some(s) => parse_decimal(s, "adjustment_value")?,
        None => Decimal::ZERO,
    };
    let input = stateset_core::CreatePriceLevel {
        name: req.name,
        code: req.code,
        description: req.description,
        adjustment_type,
        adjustment_value,
        currency: None,
    };
    let l = c.price_levels().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&l))))
}

#[utoipa::path(get, operation_id = "price_levels_list", path = "/api/v1/price-levels", tag = "price_levels",
    params(PriceLevelFilterParams),
    responses((status = 200, body = PriceLevelListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PriceLevelFilterParams>,
) -> Result<Json<PriceLevelListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let total = c
        .price_levels()
        .list(stateset_core::PriceLevelFilter {
            is_active: params.is_active,
            ..Default::default()
        })?
        .len();
    let filter = stateset_core::PriceLevelFilter {
        is_active: params.is_active,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let levels = c.price_levels().list(filter)?;
    Ok(Json(PriceLevelListResponse { price_levels: levels.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "price_levels_get_one", path = "/api/v1/price-levels/{id}", tag = "price_levels",
    params(("id" = String, Path, description = "Price level ID")),
    responses((status = 200, body = PriceLevelResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceLevelId>,
) -> Result<Json<PriceLevelResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let l = c
        .price_levels()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Price level {id} not found")))?;
    Ok(Json(to_resp(&l)))
}

#[utoipa::path(put, operation_id = "price_levels_update", path = "/api/v1/price-levels/{id}", tag = "price_levels",
    request_body = UpdatePriceLevelRequest,
    params(("id" = String, Path, description = "Price level ID")),
    responses((status = 200, body = PriceLevelResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceLevelId>,
    Json(req): Json<UpdatePriceLevelRequest>,
) -> Result<Json<PriceLevelResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let adjustment_type = match req.adjustment_type.as_deref() {
        Some(s) => Some(parse_id(s, "adjustment_type")?),
        None => None,
    };
    let adjustment_value = match req.adjustment_value.as_deref() {
        Some(s) => Some(parse_decimal(s, "adjustment_value")?),
        None => None,
    };
    let input = stateset_core::UpdatePriceLevel {
        name: req.name,
        description: req.description,
        adjustment_type,
        adjustment_value,
        is_active: req.is_active,
    };
    Ok(Json(to_resp(&c.price_levels().update(id, input)?)))
}

#[utoipa::path(delete, operation_id = "price_levels_delete_one", path = "/api/v1/price-levels/{id}", tag = "price_levels",
    params(("id" = String, Path, description = "Price level ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceLevelId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.price_levels().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "price_levels_set_entry", path = "/api/v1/price-levels/{id}/entries", tag = "price_levels",
    request_body = SetEntryRequest,
    params(("id" = String, Path, description = "Price level ID")),
    responses((status = 200, body = PriceLevelEntryResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceLevelId>,
    Json(req): Json<SetEntryRequest>,
) -> Result<Json<PriceLevelEntryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = parse_id::<ProductId>(&req.product_id, "product_id")?;
    let price = parse_decimal(&req.price, "price")?;
    Ok(Json(entry_resp(&c.price_levels().set_entry(id, product_id, price)?)))
}

#[utoipa::path(get, operation_id = "price_levels_list_entries", path = "/api/v1/price-levels/{id}/entries", tag = "price_levels",
    params(("id" = String, Path, description = "Price level ID")),
    responses((status = 200, body = [PriceLevelEntryResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PriceLevelId>,
) -> Result<Json<Vec<PriceLevelEntryResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let entries = c.price_levels().list_entries(id)?;
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
    async fn create_then_set_entry() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "name": "Wholesale",
            "code": "WHOLESALE",
            "adjustment_type": "percentage_discount",
            "adjustment_value": "10"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/price-levels")
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

        let entry_body = serde_json::json!({
            "product_id": uuid::Uuid::new_v4().to_string(),
            "price": "42.00"
        });
        let resp = app
            .oneshot(
                Request::post(format!("/price-levels/{id}/entries"))
                    .header("content-type", "application/json")
                    .body(Body::from(entry_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["price"], "42.00");
    }
}
