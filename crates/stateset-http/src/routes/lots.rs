//! Lot/batch tracking endpoints (creation, inventory operations, quarantine, expiry).

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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateLotRequest {
    /// Lot number; auto-generated when omitted.
    pub lot_number: Option<String>,
    pub sku: String,
    /// Decimal quantity as a string.
    pub quantity: String,
    /// RFC 3339 timestamp.
    pub production_date: Option<String>,
    /// RFC 3339 timestamp.
    pub expiration_date: Option<String>,
    pub supplier_lot: Option<String>,
    pub supplier_id: Option<String>,
    /// Decimal unit cost as a string.
    pub cost_per_unit: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ConsumeLotRequest {
    /// Decimal quantity as a string.
    pub quantity: String,
    /// Defaults to `api`.
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub performed_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReserveLotRequest {
    /// Decimal quantity as a string.
    pub quantity: String,
    /// Defaults to `api`.
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct QuarantineLotRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct LotFilterParams {
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    /// One of `active`, `quarantine`, `expired`, `consumed`, `on_hold`, `recalled`, `scrapped`.
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ExpiringLotParams {
    /// Window in days; defaults to 30.
    pub days: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LotResponse {
    pub id: String,
    pub lot_number: String,
    pub sku: String,
    pub status: String,
    pub quantity_produced: String,
    pub quantity_remaining: String,
    pub quantity_reserved: String,
    pub quantity_available: String,
    pub production_date: String,
    pub expiration_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LotListResponse {
    pub lots: Vec<LotResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LotTransactionResponse {
    pub id: String,
    pub lot_id: String,
    pub transaction_type: String,
    pub quantity: String,
    pub reference_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LotReservationResponse {
    pub reservation_id: String,
}

fn to_resp(l: &stateset_core::Lot) -> LotResponse {
    LotResponse {
        id: l.id.to_string(),
        lot_number: l.lot_number.clone(),
        sku: l.sku.clone(),
        status: l.status.to_string(),
        quantity_produced: l.quantity_produced.to_string(),
        quantity_remaining: l.quantity_remaining.to_string(),
        quantity_reserved: l.quantity_reserved.to_string(),
        quantity_available: l.quantity_available().to_string(),
        production_date: l.production_date.to_rfc3339(),
        expiration_date: l.expiration_date.map(|d| d.to_rfc3339()),
        created_at: l.created_at.to_rfc3339(),
    }
}

fn tx_resp(t: &stateset_core::LotTransaction) -> LotTransactionResponse {
    LotTransactionResponse {
        id: t.id.to_string(),
        lot_id: t.lot_id.to_string(),
        transaction_type: t.transaction_type.to_string(),
        quantity: t.quantity.to_string(),
        reference_type: t.reference_type.clone(),
        created_at: t.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_datetime(s: &str, what: &str) -> Result<DateTime<Utc>, HttpError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn opt_datetime(s: Option<&str>, what: &str) -> Result<Option<DateTime<Utc>>, HttpError> {
    s.map(|v| parse_datetime(v, what)).transpose()
}

fn reference(
    reference_type: Option<String>,
    reference_id: Option<&str>,
) -> Result<(String, Uuid), HttpError> {
    let rid = match reference_id {
        Some(s) => parse_id::<Uuid>(s, "reference_id")?,
        None => Uuid::nil(),
    };
    Ok((reference_type.unwrap_or_else(|| "api".to_string()), rid))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lots", post(create).get(list))
        .route("/lots/expiring", get(expiring))
        .route("/lots/{id}", get(get_one))
        .route("/lots/{id}/consume", post(consume))
        .route("/lots/{id}/reserve", post(reserve))
        .route("/lots/reservations/{reservation_id}/release", post(release_reservation))
        .route("/lots/{id}/quarantine", post(quarantine))
        .route("/lots/{id}/release-quarantine", post(release_quarantine))
}

#[utoipa::path(post, operation_id = "lots_create", path = "/api/v1/lots", tag = "lots",
    request_body = CreateLotRequest,
    responses((status = 201, body = LotResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateLotRequest>,
) -> Result<(StatusCode, Json<LotResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let supplier_id = match req.supplier_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "supplier_id")?),
        None => None,
    };
    let cost_per_unit = match req.cost_per_unit.as_deref() {
        Some(s) => Some(parse_decimal(s, "cost_per_unit")?),
        None => None,
    };
    let input = stateset_core::CreateLot {
        lot_number: req.lot_number,
        sku: req.sku,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        production_date: opt_datetime(req.production_date.as_deref(), "production_date")?,
        expiration_date: opt_datetime(req.expiration_date.as_deref(), "expiration_date")?,
        supplier_lot: req.supplier_lot,
        supplier_id,
        cost_per_unit,
        notes: req.notes,
        ..Default::default()
    };
    let lot = c.lots().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&lot))))
}

#[utoipa::path(get, operation_id = "lots_list", path = "/api/v1/lots", tag = "lots",
    params(LotFilterParams),
    responses((status = 200, body = LotListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<LotFilterParams>,
) -> Result<Json<LotListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::LotFilter {
        sku: params.sku,
        lot_number: params.lot_number,
        status,
        ..Default::default()
    };
    let total = c.lots().count(base.clone())?;
    let filter = stateset_core::LotFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let lots = c.lots().list(filter)?;
    Ok(Json(LotListResponse { lots: lots.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "lots_expiring", path = "/api/v1/lots/expiring", tag = "lots",
    params(ExpiringLotParams),
    responses((status = 200, body = LotListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn expiring(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ExpiringLotParams>,
) -> Result<Json<LotListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let lots = c.lots().get_expiring_lots(params.days.unwrap_or(30))?;
    let total = lots.len() as u64;
    Ok(Json(LotListResponse { lots: lots.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "lots_get_one", path = "/api/v1/lots/{id}", tag = "lots",
    params(("id" = String, Path, description = "Lot ID")),
    responses((status = 200, body = LotResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<LotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let lot =
        c.lots().get(id)?.ok_or_else(|| HttpError::NotFound(format!("Lot {id} not found")))?;
    Ok(Json(to_resp(&lot)))
}

#[utoipa::path(post, operation_id = "lots_consume", path = "/api/v1/lots/{id}/consume", tag = "lots",
    request_body = ConsumeLotRequest,
    params(("id" = String, Path, description = "Lot ID")),
    responses((status = 200, body = LotTransactionResponse), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn consume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ConsumeLotRequest>,
) -> Result<Json<LotTransactionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let (reference_type, reference_id) =
        reference(req.reference_type, req.reference_id.as_deref())?;
    let tx = c.lots().consume(stateset_core::ConsumeLot {
        lot_id: id,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        reference_type,
        reference_id,
        performed_by: req.performed_by,
        ..Default::default()
    })?;
    Ok(Json(tx_resp(&tx)))
}

#[utoipa::path(post, operation_id = "lots_reserve", path = "/api/v1/lots/{id}/reserve", tag = "lots",
    request_body = ReserveLotRequest,
    params(("id" = String, Path, description = "Lot ID")),
    responses((status = 201, body = LotReservationResponse), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn reserve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReserveLotRequest>,
) -> Result<(StatusCode, Json<LotReservationResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let (reference_type, reference_id) =
        reference(req.reference_type, req.reference_id.as_deref())?;
    let reservation_id = c.lots().reserve(stateset_core::ReserveLot {
        lot_id: id,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        reference_type,
        reference_id,
        expires_in_seconds: req.expires_in_seconds,
    })?;
    Ok((
        StatusCode::CREATED,
        Json(LotReservationResponse { reservation_id: reservation_id.to_string() }),
    ))
}

#[utoipa::path(post, operation_id = "lots_release_reservation",
    path = "/api/v1/lots/reservations/{reservation_id}/release", tag = "lots",
    params(("reservation_id" = String, Path, description = "Reservation ID")),
    responses((status = 204), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn release_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reservation_id): Path<Uuid>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.lots().release_reservation(reservation_id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "lots_quarantine", path = "/api/v1/lots/{id}/quarantine", tag = "lots",
    request_body = QuarantineLotRequest,
    params(("id" = String, Path, description = "Lot ID")),
    responses((status = 200, body = LotResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn quarantine(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<QuarantineLotRequest>,
) -> Result<Json<LotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.lots().quarantine(id, &req.reason)?)))
}

#[utoipa::path(post, operation_id = "lots_release_quarantine",
    path = "/api/v1/lots/{id}/release-quarantine", tag = "lots",
    params(("id" = String, Path, description = "Lot ID")),
    responses((status = 200, body = LotResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn release_quarantine(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<LotResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.lots().release_quarantine(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        router().with_state(state)
    }

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn create_lot(app: &Router, quantity: &str) -> serde_json::Value {
        let body = serde_json::json!({ "sku": "WIDGET-001", "quantity": quantity });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/lots")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        json_body(resp).await
    }

    #[tokio::test]
    async fn create_consume_and_get_flow() {
        let app = app();
        let lot = create_lot(&app, "100").await;
        assert_eq!(lot["quantity_remaining"], "100");
        assert_eq!(lot["status"], "active");
        let id = lot["id"].as_str().unwrap().to_string();

        let body = serde_json::json!({
            "quantity": "25",
            "reference_type": "work_order",
            "reference_id": Uuid::new_v4().to_string()
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/lots/{id}/consume"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let tx = json_body(resp).await;
        assert_eq!(tx["transaction_type"], "consumed");

        let resp = app
            .oneshot(Request::get(format!("/lots/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let lot = json_body(resp).await;
        assert_eq!(lot["quantity_remaining"], "75");
    }

    #[tokio::test]
    async fn quarantine_and_release_flow() {
        let app = app();
        let lot = create_lot(&app, "10").await;
        let id = lot["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/lots/{id}/quarantine"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"reason": "QA hold"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "quarantine");

        let resp = app
            .oneshot(
                Request::post(format!("/lots/{id}/release-quarantine"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "active");
    }

    #[tokio::test]
    async fn invalid_quantity_is_bad_request() {
        let app = app();
        let body = serde_json::json!({ "sku": "WIDGET-001", "quantity": "not-a-number" });
        let resp = app
            .oneshot(
                Request::post("/lots")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
