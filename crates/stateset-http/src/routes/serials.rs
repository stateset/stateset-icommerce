//! Serial number endpoints (creation, lookup, reservation, lifecycle transitions).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateSerialRequest {
    /// Serial string; auto-generated when omitted.
    pub serial: Option<String>,
    pub sku: String,
    pub lot_id: Option<String>,
    pub lot_number: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReserveSerialRequest {
    /// Defaults to `api`.
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub reserved_by: Option<String>,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ShipSerialRequest {
    pub shipment_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReturnSerialRequest {
    pub return_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ScrapSerialRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SerialFilterParams {
    pub sku: Option<String>,
    pub serial: Option<String>,
    /// One of `in_production`, `available`, `reserved`, `shipped`, `sold`, `returned`,
    /// `in_service`, `in_warranty`, `quarantined`, `scrapped`, `recalled`, `lost`, `transferred`.
    pub status: Option<String>,
    pub lot_number: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SerialResponse {
    pub id: String,
    pub serial: String,
    pub sku: String,
    pub status: String,
    pub lot_id: Option<String>,
    pub lot_number: Option<String>,
    pub sold_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SerialListResponse {
    pub serials: Vec<SerialResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SerialReservationResponse {
    pub reservation_id: String,
    pub serial_id: String,
    pub reference_type: String,
    pub expires_at: Option<String>,
}

fn to_resp(s: &stateset_core::SerialNumber) -> SerialResponse {
    SerialResponse {
        id: s.id.to_string(),
        serial: s.serial.clone(),
        sku: s.sku.clone(),
        status: s.status.to_string(),
        lot_id: s.lot_id.map(|id| id.to_string()),
        lot_number: s.lot_number.clone(),
        sold_at: s.sold_at.map(|d| d.to_rfc3339()),
        created_at: s.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/serials", post(create).get(list))
        .route("/serials/{id}", get(get_one))
        .route("/serials/{id}/reserve", post(reserve))
        .route("/serials/reservations/{reservation_id}/release", post(release_reservation))
        .route("/serials/{id}/ship", post(ship))
        .route("/serials/{id}/return", post(mark_returned))
        .route("/serials/{id}/scrap", post(scrap))
}

#[utoipa::path(post, operation_id = "serials_create", path = "/api/v1/serials", tag = "serials",
    request_body = CreateSerialRequest,
    responses((status = 201, body = SerialResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateSerialRequest>,
) -> Result<(StatusCode, Json<SerialResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let lot_id = match req.lot_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "lot_id")?),
        None => None,
    };
    let serial = c.serials().create(stateset_core::CreateSerialNumber {
        serial: req.serial,
        sku: req.sku,
        lot_id,
        lot_number: req.lot_number,
        notes: req.notes,
        ..Default::default()
    })?;
    Ok((StatusCode::CREATED, Json(to_resp(&serial))))
}

#[utoipa::path(get, operation_id = "serials_list", path = "/api/v1/serials", tag = "serials",
    params(SerialFilterParams),
    responses((status = 200, body = SerialListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SerialFilterParams>,
) -> Result<Json<SerialListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::SerialFilter {
        sku: params.sku,
        serial: params.serial,
        status,
        lot_number: params.lot_number,
        ..Default::default()
    };
    let total = c.serials().count(base.clone())?;
    let filter = stateset_core::SerialFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let serials = c.serials().list(filter)?;
    Ok(Json(SerialListResponse { serials: serials.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "serials_get_one", path = "/api/v1/serials/{id}", tag = "serials",
    params(("id" = String, Path, description = "Serial ID")),
    responses((status = 200, body = SerialResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SerialResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let serial = c
        .serials()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Serial {id} not found")))?;
    Ok(Json(to_resp(&serial)))
}

#[utoipa::path(post, operation_id = "serials_reserve", path = "/api/v1/serials/{id}/reserve", tag = "serials",
    request_body = ReserveSerialRequest,
    params(("id" = String, Path, description = "Serial ID")),
    responses((status = 201, body = SerialReservationResponse), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn reserve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReserveSerialRequest>,
) -> Result<(StatusCode, Json<SerialReservationResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let reference_id = match req.reference_id.as_deref() {
        Some(s) => parse_id::<Uuid>(s, "reference_id")?,
        None => Uuid::nil(),
    };
    let r = c.serials().reserve(stateset_core::ReserveSerialNumber {
        serial_id: id,
        reference_type: req.reference_type.unwrap_or_else(|| "api".to_string()),
        reference_id,
        reserved_by: req.reserved_by,
        expires_in_seconds: req.expires_in_seconds,
    })?;
    Ok((
        StatusCode::CREATED,
        Json(SerialReservationResponse {
            reservation_id: r.id.to_string(),
            serial_id: r.serial_id.to_string(),
            reference_type: r.reference_type,
            expires_at: r.expires_at.map(|d| d.to_rfc3339()),
        }),
    ))
}

#[utoipa::path(post, operation_id = "serials_release_reservation",
    path = "/api/v1/serials/reservations/{reservation_id}/release", tag = "serials",
    params(("reservation_id" = String, Path, description = "Reservation ID")),
    responses((status = 204), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn release_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reservation_id): Path<Uuid>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.serials().release_reservation(reservation_id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "serials_ship", path = "/api/v1/serials/{id}/ship", tag = "serials",
    request_body = ShipSerialRequest,
    params(("id" = String, Path, description = "Serial ID")),
    responses((status = 200, body = SerialResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn ship(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ShipSerialRequest>,
) -> Result<Json<SerialResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let shipment_id = parse_id::<Uuid>(&req.shipment_id, "shipment_id")?;
    Ok(Json(to_resp(&c.serials().mark_shipped(id, shipment_id)?)))
}

#[utoipa::path(post, operation_id = "serials_return", path = "/api/v1/serials/{id}/return", tag = "serials",
    request_body = ReturnSerialRequest,
    params(("id" = String, Path, description = "Serial ID")),
    responses((status = 200, body = SerialResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn mark_returned(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReturnSerialRequest>,
) -> Result<Json<SerialResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let return_id = parse_id::<Uuid>(&req.return_id, "return_id")?;
    Ok(Json(to_resp(&c.serials().mark_returned(id, return_id)?)))
}

#[utoipa::path(post, operation_id = "serials_scrap", path = "/api/v1/serials/{id}/scrap", tag = "serials",
    request_body = ScrapSerialRequest,
    params(("id" = String, Path, description = "Serial ID")),
    responses((status = 200, body = SerialResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn scrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ScrapSerialRequest>,
) -> Result<Json<SerialResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.serials().scrap(id, &req.reason)?)))
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

    async fn post_json(
        app: &Router,
        path: &str,
        body: serde_json::Value,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn create_serial(app: &Router) -> serde_json::Value {
        let resp = post_json(
            app,
            "/serials",
            serde_json::json!({ "serial": format!("SN-{}", Uuid::new_v4()), "sku": "LAPTOP-15" }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        json_body(resp).await
    }

    #[tokio::test]
    async fn reserve_ship_return_flow() {
        let app = app();
        let serial = create_serial(&app).await;
        assert_eq!(serial["status"], "available");
        let id = serial["id"].as_str().unwrap().to_string();

        let resp = post_json(
            &app,
            &format!("/serials/{id}/reserve"),
            serde_json::json!({
                "reference_type": "order",
                "reference_id": Uuid::new_v4().to_string()
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let reservation = json_body(resp).await;
        assert_eq!(reservation["serial_id"], id);

        let resp = post_json(
            &app,
            &format!("/serials/{id}/ship"),
            serde_json::json!({ "shipment_id": Uuid::new_v4().to_string() }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "shipped");

        let resp = post_json(
            &app,
            &format!("/serials/{id}/return"),
            serde_json::json!({ "return_id": Uuid::new_v4().to_string() }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "returned");
    }

    #[tokio::test]
    async fn scrap_and_get_flow() {
        let app = app();
        let serial = create_serial(&app).await;
        let id = serial["id"].as_str().unwrap().to_string();

        let resp = post_json(
            &app,
            &format!("/serials/{id}/scrap"),
            serde_json::json!({ "reason": "damaged beyond repair" }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "scrapped");

        let resp = app
            .oneshot(Request::get(format!("/serials/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "scrapped");
    }

    #[tokio::test]
    async fn ship_refuses_scrapped_serial_with_409() {
        let app = app();
        let serial = create_serial(&app).await;
        let id = serial["id"].as_str().unwrap().to_string();

        let resp = post_json(
            &app,
            &format!("/serials/{id}/scrap"),
            serde_json::json!({ "reason": "damaged beyond repair" }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = post_json(
            &app,
            &format!("/serials/{id}/ship"),
            serde_json::json!({ "shipment_id": Uuid::new_v4().to_string() }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let body = json_body(resp).await;
        let msg = body.to_string();
        assert!(msg.contains("scrapped") && msg.contains("shipped"), "{msg}");

        let resp = app
            .oneshot(Request::get(format!("/serials/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json_body(resp).await["status"], "scrapped");
    }

    #[tokio::test]
    async fn release_after_ship_is_refused_and_serial_stays_shipped() {
        let app = app();
        let serial = create_serial(&app).await;
        let id = serial["id"].as_str().unwrap().to_string();

        let resp = post_json(
            &app,
            &format!("/serials/{id}/reserve"),
            serde_json::json!({
                "reference_type": "order",
                "reference_id": Uuid::new_v4().to_string()
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let reservation_id = json_body(resp).await["reservation_id"].as_str().unwrap().to_string();

        let resp = post_json(
            &app,
            &format!("/serials/{id}/ship"),
            serde_json::json!({ "shipment_id": Uuid::new_v4().to_string() }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = post_json(
            &app,
            &format!("/serials/reservations/{reservation_id}/release"),
            serde_json::json!({}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        let resp = app
            .oneshot(Request::get(format!("/serials/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json_body(resp).await["status"], "shipped");
    }

    #[tokio::test]
    async fn list_filters_by_status() {
        let app = app();
        create_serial(&app).await;
        let resp = app
            .oneshot(Request::get("/serials?status=available").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["serials"][0]["status"], "available");
    }
}
