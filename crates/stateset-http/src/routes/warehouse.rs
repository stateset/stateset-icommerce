//! Warehouse and location endpoints (warehouses, locations, location inventory).

use crate::dto::{decode_cursor, encode_cursor, finalize_page, overfetch_limit};
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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Request / response bodies
// ============================================================================

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub(crate) struct WarehouseAddressBody {
    pub street1: Option<String>,
    pub street2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWarehouseRequest {
    pub code: String,
    pub name: String,
    /// One of `distribution`, `manufacturing`, `retail`, `third_party`, `consignment`, `returns`.
    pub warehouse_type: Option<String>,
    pub address: Option<WarehouseAddressBody>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateWarehouseRequest {
    pub name: Option<String>,
    pub warehouse_type: Option<String>,
    pub address: Option<WarehouseAddressBody>,
    pub timezone: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateLocationRequest {
    pub warehouse_id: i32,
    pub code: Option<String>,
    /// One of `bulk`, `pick`, `staging`, `receiving`, `shipping`, `quarantine`,
    /// `returns`, `production`, `packing`, `cross_dock`.
    pub location_type: Option<String>,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub level: Option<String>,
    pub bin: Option<String>,
    /// Decimal maximum weight (kg) as a string.
    pub max_weight_kg: Option<String>,
    /// Decimal maximum volume (m3) as a string.
    pub max_volume_m3: Option<String>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AdjustInventoryRequest {
    pub location_id: i32,
    pub sku: String,
    pub lot_id: Option<String>,
    /// Signed decimal quantity as a string (positive adds, negative removes).
    pub quantity: String,
    pub reason: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub performed_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct MoveInventoryRequest {
    pub from_location_id: i32,
    pub to_location_id: i32,
    pub sku: String,
    pub lot_id: Option<String>,
    /// Decimal quantity as a string.
    pub quantity: String,
    pub reason: Option<String>,
    pub performed_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct WarehouseFilterParams {
    pub warehouse_type: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct LocationFilterParams {
    pub warehouse_id: Option<i32>,
    pub location_type: Option<String>,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WarehouseResponse {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub warehouse_type: String,
    pub city: Option<String>,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WarehouseListResponse {
    pub warehouses: Vec<WarehouseResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LocationResponse {
    pub id: i32,
    pub warehouse_id: i32,
    pub code: String,
    pub location_type: String,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub level: Option<String>,
    pub bin: Option<String>,
    pub is_pickable: bool,
    pub is_receivable: bool,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LocationListResponse {
    pub locations: Vec<LocationResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LocationInventoryResponse {
    pub location_id: i32,
    pub sku: String,
    pub lot_id: Option<String>,
    pub quantity_on_hand: String,
    pub quantity_reserved: String,
    pub quantity_available: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LocationInventoryListResponse {
    pub inventory: Vec<LocationInventoryResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct MovementResponse {
    pub id: String,
    pub movement_type: String,
    pub from_location_id: Option<i32>,
    pub to_location_id: Option<i32>,
    pub sku: String,
    pub quantity: String,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateCycleCountLineRequest {
    pub sku: String,
    pub lot_id: Option<String>,
    /// Decimal expected quantity as a string.
    pub expected_quantity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateCycleCountRequest {
    pub warehouse_id: i32,
    pub location_id: Option<i32>,
    /// RFC3339 timestamp for the scheduled count date.
    pub scheduled_date: Option<String>,
    pub counted_by: Option<String>,
    pub lines: Vec<CreateCycleCountLineRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct RecordCycleCountRequest {
    pub counts: Vec<RecordCycleCountLineRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct RecordCycleCountLineRequest {
    pub sku: String,
    pub lot_id: Option<String>,
    /// Decimal counted quantity as a string.
    pub counted_quantity: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CycleCountFilterParams {
    pub warehouse_id: Option<i32>,
    pub location_id: Option<i32>,
    /// One of `draft`, `in_progress`, `completed`, `cancelled`.
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CycleCountLineResponse {
    pub id: String,
    pub sku: String,
    pub lot_id: Option<String>,
    pub expected_quantity: String,
    pub counted_quantity: Option<String>,
    pub variance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CycleCountResponse {
    pub id: String,
    pub warehouse_id: i32,
    pub location_id: Option<i32>,
    pub status: String,
    pub scheduled_date: Option<String>,
    pub counted_by: Option<String>,
    pub lines: Vec<CycleCountLineResponse>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CycleCountListResponse {
    pub cycle_counts: Vec<CycleCountResponse>,
    pub total: usize,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

// ============================================================================
// Conversions / helpers
// ============================================================================

fn warehouse_resp(w: &stateset_core::Warehouse) -> WarehouseResponse {
    WarehouseResponse {
        id: w.id,
        code: w.code.clone(),
        name: w.name.clone(),
        warehouse_type: w.warehouse_type.to_string(),
        city: Some(w.address.city.clone()).filter(|c| !c.is_empty()),
        country: Some(w.address.country.clone()).filter(|c| !c.is_empty()),
        timezone: w.timezone.clone(),
        is_active: w.is_active,
        created_at: w.created_at.to_rfc3339(),
    }
}

fn location_resp(l: &stateset_core::Location) -> LocationResponse {
    LocationResponse {
        id: l.id,
        warehouse_id: l.warehouse_id,
        code: l.code.clone(),
        location_type: l.location_type.to_string(),
        zone: l.zone.clone(),
        aisle: l.aisle.clone(),
        rack: l.rack.clone(),
        level: l.level.clone(),
        bin: l.bin.clone(),
        is_pickable: l.is_pickable,
        is_receivable: l.is_receivable,
        is_active: l.is_active,
        created_at: l.created_at.to_rfc3339(),
    }
}

fn inventory_resp(i: &stateset_core::LocationInventory) -> LocationInventoryResponse {
    LocationInventoryResponse {
        location_id: i.location_id,
        sku: i.sku.clone(),
        lot_id: i.lot_id.map(|l| l.to_string()),
        quantity_on_hand: i.quantity_on_hand.to_string(),
        quantity_reserved: i.quantity_reserved.to_string(),
        quantity_available: i.quantity_available.to_string(),
        updated_at: i.updated_at.to_rfc3339(),
    }
}

fn movement_resp(m: &stateset_core::LocationMovement) -> MovementResponse {
    MovementResponse {
        id: m.id.to_string(),
        movement_type: m.movement_type.to_string(),
        from_location_id: m.from_location_id,
        to_location_id: m.to_location_id,
        sku: m.sku.clone(),
        quantity: m.quantity.to_string(),
        reason: m.reason.clone(),
        created_at: m.created_at.to_rfc3339(),
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

fn parse_opt_uuid(s: Option<&str>, what: &str) -> Result<Option<Uuid>, HttpError> {
    s.map(|v| parse_id::<Uuid>(v, what)).transpose()
}

fn to_address(a: Option<WarehouseAddressBody>) -> stateset_core::WarehouseAddress {
    let a = a.unwrap_or_default();
    stateset_core::WarehouseAddress {
        street1: a.street1.unwrap_or_default(),
        street2: a.street2,
        city: a.city.unwrap_or_default(),
        state: a.state.unwrap_or_default(),
        postal_code: a.postal_code.unwrap_or_default(),
        country: a.country.unwrap_or_default(),
        phone: a.phone,
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/warehouses", post(create_warehouse).get(list_warehouses))
        .route(
            "/warehouses/{id}",
            get(get_warehouse).put(update_warehouse).delete(delete_warehouse),
        )
        .route("/warehouse-locations", post(create_location).get(list_locations))
        .route("/warehouse-locations/{id}", get(get_location))
        .route("/warehouse-locations/{id}/inventory", get(get_location_inventory))
        .route("/warehouse-inventory/adjust", post(adjust_inventory))
        .route("/warehouse-inventory/move", post(move_inventory))
        .route("/cycle-counts", post(create_cycle_count).get(list_cycle_counts))
        .route("/cycle-counts/{id}", get(get_cycle_count))
        .route("/cycle-counts/{id}/start", post(start_cycle_count))
        .route("/cycle-counts/{id}/counts", post(record_cycle_counts))
        .route("/cycle-counts/{id}/complete", post(complete_cycle_count))
        .route("/cycle-counts/{id}/cancel", post(cancel_cycle_count))
}

// ============================================================================
// Warehouse handlers
// ============================================================================

#[utoipa::path(post, operation_id = "warehouse_create", path = "/api/v1/warehouses", tag = "warehouse",
    request_body = CreateWarehouseRequest,
    responses((status = 201, body = WarehouseResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_warehouse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateWarehouseRequest>,
) -> Result<(StatusCode, Json<WarehouseResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let warehouse_type = match req.warehouse_type.as_deref() {
        Some(s) => parse_id(s, "warehouse_type")?,
        None => stateset_core::WarehouseType::default(),
    };
    let w = c.warehouse().create_warehouse(stateset_core::CreateWarehouse {
        code: req.code,
        name: req.name,
        warehouse_type,
        address: to_address(req.address),
        timezone: req.timezone,
    })?;
    Ok((StatusCode::CREATED, Json(warehouse_resp(&w))))
}

#[utoipa::path(get, operation_id = "warehouse_list", path = "/api/v1/warehouses", tag = "warehouse",
    params(WarehouseFilterParams),
    responses((status = 200, body = WarehouseListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_warehouses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WarehouseFilterParams>,
) -> Result<Json<WarehouseListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let warehouse_type = match params.warehouse_type.as_deref() {
        Some(s) => Some(parse_id(s, "warehouse_type")?),
        None => None,
    };
    let base = stateset_core::WarehouseFilter {
        warehouse_type,
        is_active: params.is_active,
        limit: None,
        offset: None,
    };
    let total = c.warehouse().count_warehouses(base.clone())?;
    let warehouses = c.warehouse().list_warehouses(stateset_core::WarehouseFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(WarehouseListResponse {
        warehouses: warehouses.iter().map(warehouse_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "warehouse_get_one", path = "/api/v1/warehouses/{id}", tag = "warehouse",
    params(("id" = i32, Path, description = "Warehouse ID")),
    responses((status = 200, body = WarehouseResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_warehouse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<WarehouseResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let w = c
        .warehouse()
        .get_warehouse(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Warehouse {id} not found")))?;
    Ok(Json(warehouse_resp(&w)))
}

#[utoipa::path(put, operation_id = "warehouse_update", path = "/api/v1/warehouses/{id}", tag = "warehouse",
    request_body = UpdateWarehouseRequest,
    params(("id" = i32, Path, description = "Warehouse ID")),
    responses((status = 200, body = WarehouseResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update_warehouse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(req): Json<UpdateWarehouseRequest>,
) -> Result<Json<WarehouseResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let warehouse_type = match req.warehouse_type.as_deref() {
        Some(s) => Some(parse_id(s, "warehouse_type")?),
        None => None,
    };
    let w = c.warehouse().update_warehouse(
        id,
        stateset_core::UpdateWarehouse {
            name: req.name,
            warehouse_type,
            address: req.address.map(|a| to_address(Some(a))),
            timezone: req.timezone,
            is_active: req.is_active,
        },
    )?;
    Ok(Json(warehouse_resp(&w)))
}

#[utoipa::path(delete, operation_id = "warehouse_delete", path = "/api/v1/warehouses/{id}", tag = "warehouse",
    params(("id" = i32, Path, description = "Warehouse ID")),
    responses((status = 204), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_warehouse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.warehouse().delete_warehouse(id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Location handlers
// ============================================================================

#[utoipa::path(post, operation_id = "warehouse_location_create", path = "/api/v1/warehouse-locations", tag = "warehouse",
    request_body = CreateLocationRequest,
    responses((status = 201, body = LocationResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_location(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateLocationRequest>,
) -> Result<(StatusCode, Json<LocationResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let location_type = match req.location_type.as_deref() {
        Some(s) => parse_id(s, "location_type")?,
        None => stateset_core::LocationType::default(),
    };
    let l = c.warehouse().create_location(stateset_core::CreateLocation {
        warehouse_id: req.warehouse_id,
        code: req.code,
        location_type,
        zone: req.zone,
        aisle: req.aisle,
        rack: req.rack,
        level: req.level,
        bin: req.bin,
        max_weight_kg: parse_opt_decimal(req.max_weight_kg.as_deref(), "max_weight_kg")?,
        max_volume_m3: parse_opt_decimal(req.max_volume_m3.as_deref(), "max_volume_m3")?,
        is_pickable: req.is_pickable,
        is_receivable: req.is_receivable,
    })?;
    Ok((StatusCode::CREATED, Json(location_resp(&l))))
}

#[utoipa::path(get, operation_id = "warehouse_location_list", path = "/api/v1/warehouse-locations", tag = "warehouse",
    params(LocationFilterParams),
    responses((status = 200, body = LocationListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_locations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<LocationFilterParams>,
) -> Result<Json<LocationListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let location_type = match params.location_type.as_deref() {
        Some(s) => Some(parse_id(s, "location_type")?),
        None => None,
    };
    let base = stateset_core::LocationFilter {
        warehouse_id: params.warehouse_id,
        location_type,
        zone: params.zone.clone(),
        aisle: params.aisle.clone(),
        is_pickable: params.is_pickable,
        is_receivable: params.is_receivable,
        is_active: params.is_active,
        has_capacity: None,
        limit: None,
        offset: None,
    };
    let total = c.warehouse().count_locations(base.clone())?;
    let locations = c.warehouse().list_locations(stateset_core::LocationFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    })?;
    Ok(Json(LocationListResponse {
        locations: locations.iter().map(location_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "warehouse_location_get_one", path = "/api/v1/warehouse-locations/{id}", tag = "warehouse",
    params(("id" = i32, Path, description = "Location ID")),
    responses((status = 200, body = LocationResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_location(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<LocationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let l = c
        .warehouse()
        .get_location(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Location {id} not found")))?;
    Ok(Json(location_resp(&l)))
}

#[utoipa::path(get, operation_id = "warehouse_location_inventory", path = "/api/v1/warehouse-locations/{id}/inventory", tag = "warehouse",
    params(("id" = i32, Path, description = "Location ID")),
    responses((status = 200, body = LocationInventoryListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_location_inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<LocationInventoryListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let inventory = c.warehouse().get_location_inventory(id)?;
    Ok(Json(LocationInventoryListResponse {
        total: inventory.len(),
        inventory: inventory.iter().map(inventory_resp).collect(),
    }))
}

// ============================================================================
// Inventory handlers
// ============================================================================

#[utoipa::path(post, operation_id = "warehouse_inventory_adjust", path = "/api/v1/warehouse-inventory/adjust", tag = "warehouse",
    request_body = AdjustInventoryRequest,
    responses((status = 200, body = LocationInventoryResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn adjust_inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AdjustInventoryRequest>,
) -> Result<Json<LocationInventoryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let inv = c.warehouse().adjust_inventory(stateset_core::AdjustLocationInventory {
        location_id: req.location_id,
        sku: req.sku,
        lot_id: parse_opt_uuid(req.lot_id.as_deref(), "lot_id")?,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        reason: req.reason,
        reference_type: req.reference_type,
        reference_id: parse_opt_uuid(req.reference_id.as_deref(), "reference_id")?,
        performed_by: req.performed_by,
    })?;
    Ok(Json(inventory_resp(&inv)))
}

#[utoipa::path(post, operation_id = "warehouse_inventory_move", path = "/api/v1/warehouse-inventory/move", tag = "warehouse",
    request_body = MoveInventoryRequest,
    responses((status = 200, body = MovementResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn move_inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<MoveInventoryRequest>,
) -> Result<Json<MovementResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let m = c.warehouse().move_inventory(stateset_core::MoveInventory {
        from_location_id: req.from_location_id,
        to_location_id: req.to_location_id,
        sku: req.sku,
        lot_id: parse_opt_uuid(req.lot_id.as_deref(), "lot_id")?,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        reason: req.reason,
        performed_by: req.performed_by,
    })?;
    Ok(Json(movement_resp(&m)))
}

// ============================================================================
// Cycle count handlers
// ============================================================================

fn cycle_count_resp(cc: &stateset_core::CycleCount) -> CycleCountResponse {
    CycleCountResponse {
        id: cc.id.to_string(),
        warehouse_id: cc.warehouse_id,
        location_id: cc.location_id,
        status: cc.status.to_string(),
        scheduled_date: cc.scheduled_date.map(|d| d.to_rfc3339()),
        counted_by: cc.counted_by.clone(),
        lines: cc
            .lines
            .iter()
            .map(|l| CycleCountLineResponse {
                id: l.id.to_string(),
                sku: l.sku.clone(),
                lot_id: l.lot_id.map(|x| x.to_string()),
                expected_quantity: l.expected_quantity.to_string(),
                counted_quantity: l.counted_quantity.map(|q| q.to_string()),
                variance: l.variance.map(|v| v.to_string()),
            })
            .collect(),
        created_at: cc.created_at.to_rfc3339(),
        updated_at: cc.updated_at.to_rfc3339(),
        completed_at: cc.completed_at.map(|d| d.to_rfc3339()),
    }
}

fn parse_opt_datetime(
    s: Option<&str>,
    what: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, HttpError> {
    s.map(|v| {
        chrono::DateTime::parse_from_rfc3339(v)
            .map(|d| d.with_timezone(&chrono::Utc))
            .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {v}")))
    })
    .transpose()
}

#[utoipa::path(post, operation_id = "cycle_count_create", path = "/api/v1/cycle-counts", tag = "warehouse",
    request_body = CreateCycleCountRequest,
    responses((status = 201, body = CycleCountResponse), (status = 400, body = ErrorBody), (status = 422, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_cycle_count(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCycleCountRequest>,
) -> Result<(StatusCode, Json<CycleCountResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let lines = req
        .lines
        .iter()
        .map(|l| {
            Ok(stateset_core::CreateCycleCountLine {
                sku: l.sku.clone(),
                lot_id: parse_opt_uuid(l.lot_id.as_deref(), "lot_id")?,
                expected_quantity: parse_decimal(&l.expected_quantity, "expected_quantity")?,
            })
        })
        .collect::<Result<Vec<_>, HttpError>>()?;
    let cc = c.warehouse().create_cycle_count(stateset_core::CreateCycleCount {
        warehouse_id: req.warehouse_id,
        location_id: req.location_id,
        scheduled_date: parse_opt_datetime(req.scheduled_date.as_deref(), "scheduled_date")?,
        counted_by: req.counted_by,
        lines,
    })?;
    Ok((StatusCode::CREATED, Json(cycle_count_resp(&cc))))
}

#[utoipa::path(get, operation_id = "cycle_count_list", path = "/api/v1/cycle-counts", tag = "warehouse",
    params(CycleCountFilterParams),
    responses((status = 200, body = CycleCountListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_cycle_counts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CycleCountFilterParams>,
) -> Result<Json<CycleCountListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
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
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let mut cycle_counts = c.warehouse().list_cycle_counts(stateset_core::CycleCountFilter {
        warehouse_id: params.warehouse_id,
        location_id: params.location_id,
        status,
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(params.offset.unwrap_or(0)) },
        after_cursor,
    })?;
    let has_more = finalize_page(&mut cycle_counts, limit);
    let next_cursor = if has_more {
        cycle_counts.last().map(|cc| encode_cursor(&cc.created_at.to_rfc3339(), &cc.id.to_string()))
    } else {
        None
    };
    Ok(Json(CycleCountListResponse {
        total: cycle_counts.len(),
        cycle_counts: cycle_counts.iter().map(cycle_count_resp).collect(),
        next_cursor,
        has_more,
    }))
}

#[utoipa::path(get, operation_id = "cycle_count_get_one", path = "/api/v1/cycle-counts/{id}", tag = "warehouse",
    params(("id" = String, Path, description = "Cycle count ID")),
    responses((status = 200, body = CycleCountResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_cycle_count(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CycleCountResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let id: Uuid = parse_id(&id, "cycle count id")?;
    let cc = c
        .warehouse()
        .get_cycle_count(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Cycle count {id} not found")))?;
    Ok(Json(cycle_count_resp(&cc)))
}

#[utoipa::path(post, operation_id = "cycle_count_start", path = "/api/v1/cycle-counts/{id}/start", tag = "warehouse",
    params(("id" = String, Path, description = "Cycle count ID")),
    responses((status = 200, body = CycleCountResponse), (status = 422, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn start_cycle_count(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CycleCountResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let id: Uuid = parse_id(&id, "cycle count id")?;
    let cc = c.warehouse().start_cycle_count(id)?;
    Ok(Json(cycle_count_resp(&cc)))
}

#[utoipa::path(post, operation_id = "cycle_count_record_counts", path = "/api/v1/cycle-counts/{id}/counts", tag = "warehouse",
    request_body = RecordCycleCountRequest,
    params(("id" = String, Path, description = "Cycle count ID")),
    responses((status = 200, body = CycleCountResponse), (status = 422, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn record_cycle_counts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RecordCycleCountRequest>,
) -> Result<Json<CycleCountResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let id: Uuid = parse_id(&id, "cycle count id")?;
    let counts = req
        .counts
        .iter()
        .map(|l| {
            Ok(stateset_core::RecordCycleCountLine {
                sku: l.sku.clone(),
                lot_id: parse_opt_uuid(l.lot_id.as_deref(), "lot_id")?,
                counted_quantity: parse_decimal(&l.counted_quantity, "counted_quantity")?,
            })
        })
        .collect::<Result<Vec<_>, HttpError>>()?;
    let cc = c.warehouse().record_cycle_counts(id, counts)?;
    Ok(Json(cycle_count_resp(&cc)))
}

#[utoipa::path(post, operation_id = "cycle_count_complete", path = "/api/v1/cycle-counts/{id}/complete", tag = "warehouse",
    params(("id" = String, Path, description = "Cycle count ID")),
    responses((status = 200, body = CycleCountResponse), (status = 422, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete_cycle_count(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CycleCountResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let id: Uuid = parse_id(&id, "cycle count id")?;
    let cc = c.warehouse().complete_cycle_count(id)?;
    Ok(Json(cycle_count_resp(&cc)))
}

#[utoipa::path(post, operation_id = "cycle_count_cancel", path = "/api/v1/cycle-counts/{id}/cancel", tag = "warehouse",
    params(("id" = String, Path, description = "Cycle count ID")),
    responses((status = 200, body = CycleCountResponse), (status = 422, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel_cycle_count(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CycleCountResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let id: Uuid = parse_id(&id, "cycle count id")?;
    let cc = c.warehouse().cancel_cycle_count(id)?;
    Ok(Json(cycle_count_resp(&cc)))
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
    async fn warehouse_location_inventory_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);

        // Create warehouse
        let body = serde_json::json!({
            "code": "WH-TEST",
            "name": "Test Warehouse",
            "warehouse_type": "distribution",
            "address": {"street1": "1 Main St", "city": "Newark", "state": "NJ",
                        "postal_code": "07102", "country": "US"}
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/warehouses")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let wh = json_of(resp).await;
        let warehouse_id = wh["id"].as_i64().unwrap();
        assert_eq!(wh["code"], "WH-TEST");

        // Create location
        let body = serde_json::json!({
            "warehouse_id": warehouse_id,
            "location_type": "pick",
            "zone": "A", "aisle": "01", "rack": "02", "bin": "03"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/warehouse-locations")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let loc = json_of(resp).await;
        let location_id = loc["id"].as_i64().unwrap();

        // Adjust inventory into the location (lot-less: regression check for the
        // former NOT NULL failure on first-time lot-less inserts).
        let body = serde_json::json!({
            "location_id": location_id,
            "sku": "SKU-1",
            "quantity": "25",
            "reason": "initial stock"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/warehouse-inventory/adjust")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let inv = json_of(resp).await;
        assert_eq!(inv["quantity_on_hand"], "25");

        // Location inventory reflects the adjustment
        let resp = app
            .oneshot(
                Request::get(format!("/warehouse-locations/{location_id}/inventory"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = json_of(resp).await;
        assert_eq!(list["total"], 1);
        assert_eq!(list["inventory"][0]["sku"], "SKU-1");
    }

    #[tokio::test]
    async fn cycle_count_full_flow_over_http() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);

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

        // Warehouse + location + stock
        let wh = json_of(
            post_json(&app, "/warehouses", serde_json::json!({"code": "WH-CC", "name": "CC"}))
                .await,
        )
        .await;
        let warehouse_id = wh["id"].as_i64().unwrap();
        let loc = json_of(
            post_json(
                &app,
                "/warehouse-locations",
                serde_json::json!({"warehouse_id": warehouse_id, "code": "L1"}),
            )
            .await,
        )
        .await;
        let location_id = loc["id"].as_i64().unwrap();
        post_json(
            &app,
            "/warehouse-inventory/adjust",
            serde_json::json!({"location_id": location_id, "sku": "SKU-CC",
                               "quantity": "20", "reason": "seed"}),
        )
        .await;

        // Create cycle count
        let resp = post_json(
            &app,
            "/cycle-counts",
            serde_json::json!({
                "warehouse_id": warehouse_id,
                "location_id": location_id,
                "counted_by": "counter",
                "lines": [{"sku": "SKU-CC", "expected_quantity": "20"}]
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let cc = json_of(resp).await;
        assert_eq!(cc["status"], "draft");
        let id = cc["id"].as_str().unwrap().to_string();

        // Start
        let resp =
            post_json(&app, &format!("/cycle-counts/{id}/start"), serde_json::json!({})).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_of(resp).await["status"], "in_progress");

        // Record counts (over-count by 3)
        let resp = post_json(
            &app,
            &format!("/cycle-counts/{id}/counts"),
            serde_json::json!({"counts": [{"sku": "SKU-CC", "counted_quantity": "23"}]}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let cc = json_of(resp).await;
        assert_eq!(cc["lines"][0]["variance"], "3");

        // Complete
        let resp =
            post_json(&app, &format!("/cycle-counts/{id}/complete"), serde_json::json!({})).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let cc = json_of(resp).await;
        assert_eq!(cc["status"], "completed");

        // Inventory adjusted to the counted quantity
        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/warehouse-locations/{location_id}/inventory"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let inv = json_of(resp).await;
        assert_eq!(inv["inventory"][0]["quantity_on_hand"], "23");

        // Get + list see the completed count
        let resp = app
            .clone()
            .oneshot(Request::get(format!("/cycle-counts/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = app
            .clone()
            .oneshot(Request::get("/cycle-counts?status=completed").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let list = json_of(resp).await;
        assert_eq!(list["total"], 1);
    }

    #[tokio::test]
    async fn cycle_count_invalid_transitions_are_rejected() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);

        let wh = json_of(
            app.clone()
                .oneshot(
                    Request::post("/warehouses")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({"code": "WH-B", "name": "B"}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        let resp = app
            .clone()
            .oneshot(
                Request::post("/cycle-counts")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "warehouse_id": wh["id"],
                            "lines": [{"sku": "S", "expected_quantity": "1"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let id = json_of(resp).await["id"].as_str().unwrap().to_string();

        // Completing a draft count is a 422 (invalid state transition).
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/cycle-counts/{id}/complete")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // Unknown ID is a 404.
        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/cycle-counts/{}/start", uuid::Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // Empty lines rejected at validation.
        let resp = app
            .oneshot(
                Request::post("/cycle-counts")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"warehouse_id": wh["id"], "lines": []}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn bad_warehouse_type_is_rejected() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "code": "WH-X", "name": "X", "warehouse_type": "spaceship"
        });
        let resp = app
            .oneshot(
                Request::post("/warehouses")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_cycle_counts_has_more_and_next_cursor() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "code": "WH-CUR",
            "name": "Cursor Warehouse",
            "warehouse_type": "distribution",
            "address": {"street1": "1 Main St", "city": "Newark", "state": "NJ",
                        "postal_code": "07102", "country": "US"}
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/warehouses")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let warehouse_id = json_of(resp).await["id"].as_i64().unwrap();
        for i in 0..3 {
            let _ = i;
            let body = serde_json::json!({
                "warehouse_id": warehouse_id,
                "lines": [{"sku": "CUR-SKU", "expected_quantity": "1"}]
            });
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/cycle-counts")
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
            .oneshot(Request::get("/cycle-counts?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["cycle_counts"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().expect("next_cursor").to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/cycle-counts?limit=2&after={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_of(resp).await;
        assert_eq!(json["cycle_counts"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none() || json["next_cursor"].is_null());
    }

    #[tokio::test]
    async fn list_cycle_counts_invalid_cursor_returns_400() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .oneshot(Request::get("/cycle-counts?after=!!!invalid!!!").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
