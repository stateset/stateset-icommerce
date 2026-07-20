//! Fulfillment endpoints (waves, pick tasks, pack tasks, ship tasks).

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
use stateset_core::{FulfillmentId, OrderId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response bodies
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWaveRequest {
    pub warehouse_id: i32,
    /// Order IDs (UUIDs) to include in the wave.
    pub order_ids: Vec<String>,
    pub priority: Option<i32>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AssignTaskRequest {
    pub assigned_to: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CompletePickRequest {
    /// Decimal quantity picked as a string.
    pub quantity_picked: String,
    /// Decimal quantity short as a string.
    pub quantity_short: Option<String>,
    pub short_reason: Option<String>,
    pub lot_id: Option<String>,
    pub serial_number: Option<String>,
    pub completed_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AddCartonRequest {
    /// One of `box`, `envelope`, `tube`, `pallet`, `custom`.
    pub package_type: Option<String>,
    /// Decimal weight (kg) as a string.
    pub weight_kg: Option<String>,
    pub length_cm: Option<String>,
    pub width_cm: Option<String>,
    pub height_cm: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CompleteShipRequest {
    pub tracking_number: String,
    /// Decimal shipping cost as a string.
    pub shipping_cost: Option<String>,
    pub shipped_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct WaveFilterParams {
    pub warehouse_id: Option<i32>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PickTaskFilterParams {
    pub warehouse_id: Option<i32>,
    pub wave_id: Option<String>,
    pub order_id: Option<String>,
    pub status: Option<String>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PackTaskFilterParams {
    pub order_id: Option<String>,
    pub status: Option<String>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ShipTaskFilterParams {
    pub order_id: Option<String>,
    pub status: Option<String>,
    pub carrier: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WaveResponse {
    pub id: String,
    pub wave_number: String,
    pub warehouse_id: i32,
    pub status: String,
    pub order_count: i32,
    pub pick_count: i32,
    pub completed_pick_count: i32,
    pub priority: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WaveListResponse {
    pub waves: Vec<WaveResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PickTaskResponse {
    pub id: String,
    pub wave_id: Option<String>,
    pub order_id: String,
    pub warehouse_id: i32,
    pub status: String,
    pub sku: String,
    pub source_location_id: i32,
    pub quantity_requested: String,
    pub quantity_picked: String,
    pub quantity_short: String,
    pub assigned_to: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PickTaskListResponse {
    pub picks: Vec<PickTaskResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PackTaskResponse {
    pub id: String,
    pub order_id: String,
    pub status: String,
    pub carton_count: i32,
    pub total_weight_kg: Option<String>,
    pub assigned_to: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PackTaskListResponse {
    pub packs: Vec<PackTaskResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CartonResponse {
    pub id: String,
    pub pack_task_id: String,
    pub carton_number: String,
    pub package_type: String,
    pub weight_kg: Option<String>,
    pub tracking_number: Option<String>,
    pub label_printed: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CartonListResponse {
    pub cartons: Vec<CartonResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ShipTaskResponse {
    pub id: String,
    pub order_id: String,
    pub shipment_id: String,
    pub pack_task_id: String,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub shipping_cost: Option<String>,
    pub assigned_to: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ShipTaskListResponse {
    pub ships: Vec<ShipTaskResponse>,
    pub total: u64,
}

// ============================================================================
// Conversions / helpers
// ============================================================================

fn wave_resp(w: &stateset_core::Wave) -> WaveResponse {
    WaveResponse {
        id: w.id.to_string(),
        wave_number: w.wave_number.clone(),
        warehouse_id: w.warehouse_id,
        status: w.status.to_string(),
        order_count: w.order_count,
        pick_count: w.pick_count,
        completed_pick_count: w.completed_pick_count,
        priority: w.priority,
        created_at: w.created_at.to_rfc3339(),
    }
}

fn pick_resp(p: &stateset_core::PickTask) -> PickTaskResponse {
    PickTaskResponse {
        id: p.id.to_string(),
        wave_id: p.wave_id.map(|w| w.to_string()),
        order_id: p.order_id.to_string(),
        warehouse_id: p.warehouse_id,
        status: p.status.to_string(),
        sku: p.sku.clone(),
        source_location_id: p.source_location_id,
        quantity_requested: p.quantity_requested.to_string(),
        quantity_picked: p.quantity_picked.to_string(),
        quantity_short: p.quantity_short.to_string(),
        assigned_to: p.assigned_to.clone(),
        created_at: p.created_at.to_rfc3339(),
    }
}

fn pack_resp(p: &stateset_core::PackTask) -> PackTaskResponse {
    PackTaskResponse {
        id: p.id.to_string(),
        order_id: p.order_id.to_string(),
        status: p.status.to_string(),
        carton_count: p.carton_count,
        total_weight_kg: p.total_weight_kg.map(|w| w.to_string()),
        assigned_to: p.assigned_to.clone(),
        created_at: p.created_at.to_rfc3339(),
    }
}

fn carton_resp(c: &stateset_core::Carton) -> CartonResponse {
    CartonResponse {
        id: c.id.to_string(),
        pack_task_id: c.pack_task_id.to_string(),
        carton_number: c.carton_number.clone(),
        package_type: c.package_type.to_string(),
        weight_kg: c.weight_kg.map(|w| w.to_string()),
        tracking_number: c.tracking_number.clone(),
        label_printed: c.label_printed,
        created_at: c.created_at.to_rfc3339(),
    }
}

fn ship_resp(s: &stateset_core::ShipTask) -> ShipTaskResponse {
    ShipTaskResponse {
        id: s.id.to_string(),
        order_id: s.order_id.to_string(),
        shipment_id: s.shipment_id.to_string(),
        pack_task_id: s.pack_task_id.to_string(),
        status: s.status.to_string(),
        carrier: s.carrier.clone(),
        tracking_number: s.tracking_number.clone(),
        shipping_cost: s.shipping_cost.map(|c| c.to_string()),
        assigned_to: s.assigned_to.clone(),
        created_at: s.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_opt_decimal(s: Option<&str>, what: &str) -> Result<Option<Decimal>, HttpError> {
    s.map(|v| parse_decimal(v, what)).transpose()
}

fn parse_opt<T: std::str::FromStr>(s: Option<&str>, what: &str) -> Result<Option<T>, HttpError> {
    s.map(|v| parse_id::<T>(v, what)).transpose()
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/fulfillment/waves", post(create_wave).get(list_waves))
        .route("/fulfillment/waves/{id}", get(get_wave))
        .route("/fulfillment/waves/{id}/release", post(release_wave))
        .route("/fulfillment/picks", get(list_picks))
        .route("/fulfillment/picks/{id}/assign", post(assign_pick))
        .route("/fulfillment/picks/{id}/complete", post(complete_pick))
        .route("/fulfillment/packs", get(list_packs))
        .route("/fulfillment/packs/{id}/complete", post(complete_pack))
        .route("/fulfillment/packs/{id}/cartons", post(add_carton).get(list_cartons))
        .route("/fulfillment/ships", get(list_ships))
        .route("/fulfillment/ships/{id}/complete", post(complete_ship))
}

// ============================================================================
// Wave handlers
// ============================================================================

#[utoipa::path(post, operation_id = "fulfillment_wave_create", path = "/api/v1/fulfillment/waves", tag = "fulfillment",
    request_body = CreateWaveRequest,
    responses((status = 201, body = WaveResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_wave(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateWaveRequest>,
) -> Result<(StatusCode, Json<WaveResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut order_ids = Vec::with_capacity(req.order_ids.len());
    for id in &req.order_ids {
        order_ids.push(parse_id::<OrderId>(id, "order_id")?);
    }
    let w = c.fulfillment().create_wave(stateset_core::CreateWave {
        warehouse_id: req.warehouse_id,
        order_ids,
        priority: req.priority,
        notes: req.notes,
        created_by: req.created_by,
    })?;
    Ok((StatusCode::CREATED, Json(wave_resp(&w))))
}

#[utoipa::path(get, operation_id = "fulfillment_wave_list", path = "/api/v1/fulfillment/waves", tag = "fulfillment",
    params(WaveFilterParams),
    responses((status = 200, body = WaveListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_waves(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WaveFilterParams>,
) -> Result<Json<WaveListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = parse_opt(params.status.as_deref(), "status")?;
    let base = stateset_core::WaveFilter {
        warehouse_id: params.warehouse_id,
        status,
        ..Default::default()
    };
    let total = c.fulfillment().count_waves(base.clone())?;
    let waves = c.fulfillment().list_waves(stateset_core::WaveFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(WaveListResponse { waves: waves.iter().map(wave_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "fulfillment_wave_get_one", path = "/api/v1/fulfillment/waves/{id}", tag = "fulfillment",
    params(("id" = String, Path, description = "Wave ID")),
    responses((status = 200, body = WaveResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_wave(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<FulfillmentId>,
) -> Result<Json<WaveResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let w = c
        .fulfillment()
        .get_wave(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Wave {id} not found")))?;
    Ok(Json(wave_resp(&w)))
}

#[utoipa::path(post, operation_id = "fulfillment_wave_release", path = "/api/v1/fulfillment/waves/{id}/release", tag = "fulfillment",
    params(("id" = String, Path, description = "Wave ID")),
    responses((status = 200, body = WaveResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn release_wave(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<FulfillmentId>,
) -> Result<Json<WaveResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(wave_resp(&c.fulfillment().release_wave(id)?)))
}

// ============================================================================
// Pick handlers
// ============================================================================

#[utoipa::path(get, operation_id = "fulfillment_pick_list", path = "/api/v1/fulfillment/picks", tag = "fulfillment",
    params(PickTaskFilterParams),
    responses((status = 200, body = PickTaskListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_picks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PickTaskFilterParams>,
) -> Result<Json<PickTaskListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::PickTaskFilter {
        warehouse_id: params.warehouse_id,
        wave_id: parse_opt(params.wave_id.as_deref(), "wave_id")?,
        order_id: parse_opt(params.order_id.as_deref(), "order_id")?,
        status: parse_opt(params.status.as_deref(), "status")?,
        assigned_to: params.assigned_to.clone(),
        ..Default::default()
    };
    let total = c.fulfillment().count_picks(base.clone())?;
    let picks = c.fulfillment().list_picks(stateset_core::PickTaskFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(PickTaskListResponse { picks: picks.iter().map(pick_resp).collect(), total }))
}

#[utoipa::path(post, operation_id = "fulfillment_pick_assign", path = "/api/v1/fulfillment/picks/{id}/assign", tag = "fulfillment",
    request_body = AssignTaskRequest,
    params(("id" = String, Path, description = "Pick task ID")),
    responses((status = 200, body = PickTaskResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn assign_pick(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignTaskRequest>,
) -> Result<Json<PickTaskResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(pick_resp(&c.fulfillment().assign_pick(id, &req.assigned_to)?)))
}

#[utoipa::path(post, operation_id = "fulfillment_pick_complete", path = "/api/v1/fulfillment/picks/{id}/complete", tag = "fulfillment",
    request_body = CompletePickRequest,
    params(("id" = String, Path, description = "Pick task ID")),
    responses((status = 200, body = PickTaskResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn complete_pick(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CompletePickRequest>,
) -> Result<Json<PickTaskResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let pick = c.fulfillment().complete_pick(stateset_core::CompletePick {
        pick_id: id,
        quantity_picked: parse_decimal(&req.quantity_picked, "quantity_picked")?,
        quantity_short: parse_opt_decimal(req.quantity_short.as_deref(), "quantity_short")?,
        short_reason: req.short_reason,
        lot_id: parse_opt(req.lot_id.as_deref(), "lot_id")?,
        serial_number: req.serial_number,
        completed_by: req.completed_by,
    })?;
    Ok(Json(pick_resp(&pick)))
}

// ============================================================================
// Pack handlers
// ============================================================================

#[utoipa::path(get, operation_id = "fulfillment_pack_list", path = "/api/v1/fulfillment/packs", tag = "fulfillment",
    params(PackTaskFilterParams),
    responses((status = 200, body = PackTaskListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_packs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PackTaskFilterParams>,
) -> Result<Json<PackTaskListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::PackTaskFilter {
        order_id: parse_opt(params.order_id.as_deref(), "order_id")?,
        status: parse_opt(params.status.as_deref(), "status")?,
        assigned_to: params.assigned_to.clone(),
        ..Default::default()
    };
    let total = c.fulfillment().count_packs(base.clone())?;
    let packs = c.fulfillment().list_packs(stateset_core::PackTaskFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(PackTaskListResponse { packs: packs.iter().map(pack_resp).collect(), total }))
}

#[utoipa::path(post, operation_id = "fulfillment_pack_complete", path = "/api/v1/fulfillment/packs/{id}/complete", tag = "fulfillment",
    params(("id" = String, Path, description = "Pack task ID")),
    responses((status = 200, body = PackTaskResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete_pack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PackTaskResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(pack_resp(&c.fulfillment().complete_pack(id)?)))
}

#[utoipa::path(post, operation_id = "fulfillment_carton_add", path = "/api/v1/fulfillment/packs/{id}/cartons", tag = "fulfillment",
    request_body = AddCartonRequest,
    params(("id" = String, Path, description = "Pack task ID")),
    responses((status = 201, body = CartonResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn add_carton(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<AddCartonRequest>,
) -> Result<(StatusCode, Json<CartonResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let package_type = match req.package_type.as_deref() {
        Some(s) => parse_id(s, "package_type")?,
        None => stateset_core::PackageType::default(),
    };
    let carton = c.fulfillment().add_carton(stateset_core::AddCarton {
        pack_task_id: id,
        package_type,
        weight_kg: parse_opt_decimal(req.weight_kg.as_deref(), "weight_kg")?,
        length_cm: parse_opt_decimal(req.length_cm.as_deref(), "length_cm")?,
        width_cm: parse_opt_decimal(req.width_cm.as_deref(), "width_cm")?,
        height_cm: parse_opt_decimal(req.height_cm.as_deref(), "height_cm")?,
    })?;
    Ok((StatusCode::CREATED, Json(carton_resp(&carton))))
}

#[utoipa::path(get, operation_id = "fulfillment_carton_list", path = "/api/v1/fulfillment/packs/{id}/cartons", tag = "fulfillment",
    params(("id" = String, Path, description = "Pack task ID")),
    responses((status = 200, body = CartonListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_cartons(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<CartonListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let cartons = c.fulfillment().get_cartons(id)?;
    Ok(Json(CartonListResponse {
        total: cartons.len(),
        cartons: cartons.iter().map(carton_resp).collect(),
    }))
}

// ============================================================================
// Ship handlers
// ============================================================================

#[utoipa::path(get, operation_id = "fulfillment_ship_list", path = "/api/v1/fulfillment/ships", tag = "fulfillment",
    params(ShipTaskFilterParams),
    responses((status = 200, body = ShipTaskListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_ships(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ShipTaskFilterParams>,
) -> Result<Json<ShipTaskListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::ShipTaskFilter {
        order_id: parse_opt(params.order_id.as_deref(), "order_id")?,
        status: parse_opt(params.status.as_deref(), "status")?,
        carrier: params.carrier.clone(),
        ..Default::default()
    };
    let total = c.fulfillment().count_ships(base.clone())?;
    let ships = c.fulfillment().list_ships(stateset_core::ShipTaskFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(ShipTaskListResponse { ships: ships.iter().map(ship_resp).collect(), total }))
}

#[utoipa::path(post, operation_id = "fulfillment_ship_complete", path = "/api/v1/fulfillment/ships/{id}/complete", tag = "fulfillment",
    request_body = CompleteShipRequest,
    params(("id" = String, Path, description = "Ship task ID")),
    responses((status = 200, body = ShipTaskResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn complete_ship(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteShipRequest>,
) -> Result<Json<ShipTaskResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let ship = c.fulfillment().complete_ship(stateset_core::CompleteShip {
        ship_task_id: id,
        tracking_number: req.tracking_number,
        shipping_cost: parse_opt_decimal(req.shipping_cost.as_deref(), "shipping_cost")?,
        shipped_by: req.shipped_by,
    })?;
    Ok(Json(ship_resp(&ship)))
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
    async fn create_and_release_wave_flow() {
        let commerce = Commerce::new(":memory:").expect("in-memory Commerce");
        // Waves have a FOREIGN KEY to warehouses; seed one first.
        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: "WH-F".into(),
                name: "Fulfillment WH".into(),
                ..Default::default()
            })
            .expect("seed warehouse");
        let state = AppState::new(commerce);
        let app = router().with_state(state);

        let body = serde_json::json!({
            "warehouse_id": warehouse.id,
            "order_ids": [],
            "priority": 2,
            "notes": "test wave"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/fulfillment/waves")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let wave = json_of(resp).await;
        assert_eq!(wave["status"], "draft");
        let id = wave["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/fulfillment/waves/{id}/release"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let wave = json_of(resp).await;
        assert_eq!(wave["status"], "released");

        // Wave shows up in list
        let resp = app
            .oneshot(
                Request::get(format!("/fulfillment/waves?warehouse_id={}", warehouse.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = json_of(resp).await;
        assert_eq!(list["total"], 1);
    }

    #[tokio::test]
    async fn unknown_wave_is_not_found() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let id = uuid::Uuid::new_v4();
        let resp = app
            .oneshot(Request::get(format!("/fulfillment/waves/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
