//! Fixed asset register endpoints.

use crate::dto::{decode_cursor, encode_cursor, finalize_page, overfetch_limit};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::DepreciationMethod;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateFixedAssetRequest {
    pub asset_number: Option<String>,
    pub name: String,
    pub description: Option<String>,
    /// One of `land`, `building`, `machinery`, `equipment`, `vehicle`,
    /// `furniture_and_fixtures`, `computer_hardware`, `software`,
    /// `leasehold_improvement`, `other`.
    pub category: String,
    /// ISO date (`YYYY-MM-DD`).
    pub acquisition_date: String,
    /// Decimal cost as a string.
    pub acquisition_cost: String,
    /// Decimal salvage value as a string (default `0`).
    pub salvage_value: Option<String>,
    pub useful_life_months: u32,
    /// One of `straight_line`, `declining_balance`, `units_of_production`.
    pub depreciation_method: String,
    /// Declining-balance rate as a decimal string (required for `declining_balance`).
    pub declining_balance_rate: Option<String>,
    /// ISO date; when supplied the asset is created in service.
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub(crate) struct UpdateFixedAssetRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    /// Decimal salvage value as a string.
    pub salvage_value: Option<String>,
    pub useful_life_months: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct PlaceInServiceRequest {
    /// ISO date (`YYYY-MM-DD`).
    pub date: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct DisposeFixedAssetRequest {
    /// ISO date (`YYYY-MM-DD`).
    pub date: String,
    /// Decimal proceeds as a string.
    pub proceeds: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct WriteOffFixedAssetRequest {
    /// ISO date (`YYYY-MM-DD`).
    pub date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct PostDepreciationRequest {
    /// Number of scheduled periods to post (default 1).
    pub periods: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct FixedAssetFilterParams {
    pub category: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct FixedAssetResponse {
    pub id: String,
    pub asset_number: String,
    pub name: String,
    pub category: String,
    pub status: String,
    pub acquisition_date: String,
    pub acquisition_cost: String,
    pub salvage_value: String,
    pub useful_life_months: u32,
    pub accumulated_depreciation: String,
    pub book_value: String,
    pub currency: String,
    pub in_service_date: Option<String>,
    pub disposal_gain_loss: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct FixedAssetListResponse {
    pub fixed_assets: Vec<FixedAssetResponse>,
    pub total: usize,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct DepreciationEntryResponse {
    pub period: u32,
    pub amount: String,
    pub accumulated: String,
    pub book_value: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct DepreciationScheduleResponse {
    pub asset_id: String,
    pub total_depreciation: String,
    pub entries: Vec<DepreciationEntryResponse>,
}

fn to_resp(a: &stateset_core::FixedAsset) -> FixedAssetResponse {
    FixedAssetResponse {
        id: a.id.to_string(),
        asset_number: a.asset_number.clone(),
        name: a.name.clone(),
        category: a.category.to_string(),
        status: a.status.to_string(),
        acquisition_date: a.acquisition_date.to_string(),
        acquisition_cost: a.acquisition_cost.to_string(),
        salvage_value: a.salvage_value.to_string(),
        useful_life_months: a.useful_life_months,
        accumulated_depreciation: a.accumulated_depreciation.to_string(),
        book_value: a.book_value().to_string(),
        currency: a.currency.to_string(),
        in_service_date: a.in_service_date.map(|d| d.to_string()),
        disposal_gain_loss: a.disposal.as_ref().map(|d| d.gain_loss.to_string()),
        created_at: a.created_at.to_rfc3339(),
    }
}

fn schedule_resp(s: &stateset_core::DepreciationSchedule) -> DepreciationScheduleResponse {
    DepreciationScheduleResponse {
        asset_id: s.asset_id.to_string(),
        total_depreciation: s.total_depreciation.to_string(),
        entries: s
            .entries
            .iter()
            .map(|e| DepreciationEntryResponse {
                period: e.period,
                amount: e.amount.to_string(),
                accumulated: e.accumulated.to_string(),
                book_value: e.book_value.to_string(),
                status: e.status.to_string(),
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

fn parse_date(s: &str, what: &str) -> Result<NaiveDate, HttpError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_method(method: &str, rate: Option<&str>) -> Result<DepreciationMethod, HttpError> {
    match method {
        "straight_line" => Ok(DepreciationMethod::StraightLine),
        "units_of_production" => Ok(DepreciationMethod::UnitsOfProduction),
        "declining_balance" => {
            let rate = rate.ok_or_else(|| {
                HttpError::BadRequest(
                    "declining_balance_rate is required for declining_balance".into(),
                )
            })?;
            Ok(DepreciationMethod::DecliningBalance {
                rate: parse_decimal(rate, "declining_balance_rate")?,
            })
        }
        other => Err(HttpError::BadRequest(format!("invalid depreciation_method: {other}"))),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/fixed-assets", post(create).get(list))
        .route("/fixed-assets/{id}", get(get_one).put(update))
        .route("/fixed-assets/{id}/place-in-service", post(place_in_service))
        .route("/fixed-assets/{id}/dispose", post(dispose))
        .route("/fixed-assets/{id}/write-off", post(write_off))
        .route("/fixed-assets/{id}/schedule", post(generate_schedule).get(get_schedule))
        .route("/fixed-assets/{id}/post-depreciation", post(post_depreciation))
}

#[utoipa::path(post, operation_id = "fixed_assets_create", path = "/api/v1/fixed-assets", tag = "fixed_assets",
    request_body = CreateFixedAssetRequest,
    responses((status = 201, body = FixedAssetResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateFixedAssetRequest>,
) -> Result<(StatusCode, Json<FixedAssetResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let in_service_date = match req.in_service_date.as_deref() {
        Some(s) => Some(parse_date(s, "in_service_date")?),
        None => None,
    };
    let location_id = match req.location_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "location_id")?),
        None => None,
    };
    let input = stateset_core::CreateFixedAsset {
        asset_number: req.asset_number,
        name: req.name,
        description: req.description,
        category: parse_id(&req.category, "category")?,
        acquisition_date: parse_date(&req.acquisition_date, "acquisition_date")?,
        acquisition_cost: parse_decimal(&req.acquisition_cost, "acquisition_cost")?,
        salvage_value: match req.salvage_value.as_deref() {
            Some(s) => parse_decimal(s, "salvage_value")?,
            None => Decimal::ZERO,
        },
        useful_life_months: req.useful_life_months,
        depreciation_method: parse_method(
            &req.depreciation_method,
            req.declining_balance_rate.as_deref(),
        )?,
        in_service_date,
        location_id,
        asset_account_id: None,
        accumulated_depreciation_account_id: None,
        depreciation_expense_account_id: None,
        currency: None,
    };
    let a = c.fixed_assets().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&a))))
}

#[utoipa::path(get, operation_id = "fixed_assets_list", path = "/api/v1/fixed-assets", tag = "fixed_assets",
    params(FixedAssetFilterParams),
    responses((status = 200, body = FixedAssetListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<FixedAssetFilterParams>,
) -> Result<Json<FixedAssetListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let category = match params.category.as_deref() {
        Some(s) => Some(parse_id(s, "category")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };
    let base = stateset_core::FixedAssetFilter {
        category,
        status,
        search: params.search.clone(),
        ..Default::default()
    };
    let total = c.fixed_assets().list(base.clone())?.len();
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let filter = stateset_core::FixedAssetFilter {
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(params.offset.unwrap_or(0)) },
        after_cursor,
        ..base
    };
    let mut assets = c.fixed_assets().list(filter)?;
    let has_more = finalize_page(&mut assets, limit);
    let next_cursor = if has_more {
        assets.last().map(|a| encode_cursor(&a.created_at.to_rfc3339(), &a.id.to_string()))
    } else {
        None
    };
    Ok(Json(FixedAssetListResponse {
        fixed_assets: assets.iter().map(to_resp).collect(),
        total,
        next_cursor,
        has_more,
    }))
}

#[utoipa::path(get, operation_id = "fixed_assets_get_one", path = "/api/v1/fixed-assets/{id}", tag = "fixed_assets",
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = FixedAssetResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<FixedAssetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let a = c
        .fixed_assets()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Fixed asset {id} not found")))?;
    Ok(Json(to_resp(&a)))
}

#[utoipa::path(put, operation_id = "fixed_assets_update", path = "/api/v1/fixed-assets/{id}", tag = "fixed_assets",
    request_body = UpdateFixedAssetRequest,
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = FixedAssetResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateFixedAssetRequest>,
) -> Result<Json<FixedAssetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let category = match req.category.as_deref() {
        Some(s) => Some(parse_id(s, "category")?),
        None => None,
    };
    let salvage_value = match req.salvage_value.as_deref() {
        Some(s) => Some(parse_decimal(s, "salvage_value")?),
        None => None,
    };
    let input = stateset_core::UpdateFixedAsset {
        name: req.name,
        description: req.description,
        category,
        salvage_value,
        useful_life_months: req.useful_life_months,
        ..Default::default()
    };
    Ok(Json(to_resp(&c.fixed_assets().update(id, input)?)))
}

#[utoipa::path(post, operation_id = "fixed_assets_place_in_service", path = "/api/v1/fixed-assets/{id}/place-in-service", tag = "fixed_assets",
    request_body = PlaceInServiceRequest,
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = FixedAssetResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn place_in_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<PlaceInServiceRequest>,
) -> Result<Json<FixedAssetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let date = parse_date(&req.date, "date")?;
    Ok(Json(to_resp(&c.fixed_assets().place_in_service(id, date)?)))
}

#[utoipa::path(post, operation_id = "fixed_assets_dispose", path = "/api/v1/fixed-assets/{id}/dispose", tag = "fixed_assets",
    request_body = DisposeFixedAssetRequest,
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = FixedAssetResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn dispose(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<DisposeFixedAssetRequest>,
) -> Result<Json<FixedAssetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let date = parse_date(&req.date, "date")?;
    let proceeds = parse_decimal(&req.proceeds, "proceeds")?;
    Ok(Json(to_resp(&c.fixed_assets().dispose(id, date, proceeds, req.notes)?)))
}

#[utoipa::path(post, operation_id = "fixed_assets_write_off", path = "/api/v1/fixed-assets/{id}/write-off", tag = "fixed_assets",
    request_body = WriteOffFixedAssetRequest,
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = FixedAssetResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn write_off(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<WriteOffFixedAssetRequest>,
) -> Result<Json<FixedAssetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let date = parse_date(&req.date, "date")?;
    Ok(Json(to_resp(&c.fixed_assets().write_off(id, date, req.notes)?)))
}

#[utoipa::path(post, operation_id = "fixed_assets_generate_schedule", path = "/api/v1/fixed-assets/{id}/schedule", tag = "fixed_assets",
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = DepreciationScheduleResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn generate_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<DepreciationScheduleResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(schedule_resp(&c.fixed_assets().generate_schedule(id)?)))
}

#[utoipa::path(get, operation_id = "fixed_assets_get_schedule", path = "/api/v1/fixed-assets/{id}/schedule", tag = "fixed_assets",
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = DepreciationScheduleResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<DepreciationScheduleResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .fixed_assets()
        .get_schedule(id)?
        .ok_or_else(|| HttpError::NotFound(format!("No schedule generated for asset {id}")))?;
    Ok(Json(schedule_resp(&s)))
}

#[utoipa::path(post, operation_id = "fixed_assets_post_depreciation", path = "/api/v1/fixed-assets/{id}/post-depreciation", tag = "fixed_assets",
    request_body = PostDepreciationRequest,
    params(("id" = String, Path, description = "Fixed asset ID")),
    responses((status = 200, body = FixedAssetResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn post_depreciation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<PostDepreciationRequest>,
) -> Result<Json<FixedAssetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.fixed_assets().post_depreciation(id, req.periods.unwrap_or(1))?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    async fn json_of(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn create_place_in_service_schedule_and_post_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "name": "Forklift",
            "category": "machinery",
            "acquisition_date": "2026-01-01",
            "acquisition_cost": "10000",
            "salvage_value": "1000",
            "useful_life_months": 36,
            "depreciation_method": "straight_line"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/fixed-assets")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_of(resp).await;
        assert_eq!(json["status"], "draft");
        assert_eq!(json["book_value"], "10000");
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/fixed-assets/{id}/place-in-service"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"date": "2026-02-01"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["status"], "in_service");

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/fixed-assets/{id}/schedule")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["total_depreciation"], "9000");
        assert_eq!(json["entries"].as_array().unwrap().len(), 36);

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/fixed-assets/{id}/post-depreciation"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"periods": 2}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["accumulated_depreciation"], "500");
        assert_eq!(json["book_value"], "9500");

        let resp = app
            .oneshot(
                Request::post(format!("/fixed-assets/{id}/dispose"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"date": "2026-06-30", "proceeds": "9700"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["status"], "disposed");
        assert_eq!(json["disposal_gain_loss"], "200");
    }

    #[tokio::test]
    async fn invalid_transition_returns_conflict() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "name": "Laptop",
            "category": "computer_hardware",
            "acquisition_date": "2026-01-01",
            "acquisition_cost": "2000",
            "useful_life_months": 24,
            "depreciation_method": "straight_line"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/fixed-assets")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = json_of(resp).await["id"].as_str().unwrap().to_string();
        // Draft assets cannot be disposed.
        let resp = app
            .oneshot(
                Request::post(format!("/fixed-assets/{id}/dispose"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"date": "2026-02-01", "proceeds": "1"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn list_fixed_assets_has_more_and_next_cursor() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        for i in 0..3 {
            let _ = i;
            let body = serde_json::json!({
                "name": "Cursor Asset",
                "category": "machinery",
                "acquisition_date": "2026-01-01",
                "acquisition_cost": "1000",
                "useful_life_months": 12,
                "depreciation_method": "straight_line"
            });
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/fixed-assets")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        let resp = app
            .clone()
            .oneshot(Request::get("/fixed-assets?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["fixed_assets"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().expect("next_cursor").to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/fixed-assets?limit=2&after={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["fixed_assets"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none() || json["next_cursor"].is_null());
    }

    #[tokio::test]
    async fn list_fixed_assets_invalid_cursor_returns_400() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .oneshot(Request::get("/fixed-assets?after=!!!invalid!!!").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
