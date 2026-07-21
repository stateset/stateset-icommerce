//! Quality control endpoints (inspections, NCRs, quality holds).

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

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateInspectionItemRequest {
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    /// Decimal quantity as a string.
    pub quantity_to_inspect: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateInspectionRequest {
    /// One of `receiving`, `in_process`, `final`, `random`.
    pub inspection_type: String,
    pub reference_type: String,
    pub reference_id: String,
    pub inspector_id: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub items: Vec<CreateInspectionItemRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct RecordInspectionResultRequest {
    pub item_id: String,
    /// Decimal quantity as a string.
    pub quantity_passed: String,
    /// Decimal quantity as a string.
    pub quantity_failed: String,
    /// One of `pass`, `fail`, `conditional_pass`, `pending` (per `InspectionResult`).
    pub result: String,
    #[serde(default)]
    pub defect_codes: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct InspectionFilterParams {
    pub inspection_type: Option<String>,
    pub status: Option<String>,
    pub reference_type: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Cursor for keyset pagination (opaque token from `next_cursor`).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct InspectionResponse {
    pub id: String,
    pub inspection_number: String,
    pub inspection_type: String,
    pub reference_type: String,
    pub reference_id: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct InspectionListResponse {
    pub inspections: Vec<InspectionResponse>,
    pub total: u64,
    /// Opaque cursor for fetching the next page (keyset pagination).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether more results are available after this page.
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateNcrRequest {
    /// One of `inspection`, `customer_complaint`, `internal`, `supplier`, `audit` (per `NonConformanceSource`).
    pub source: String,
    /// One of `critical`, `major`, `minor` (per `Severity`).
    pub severity: String,
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    /// Decimal quantity as a string.
    pub quantity_affected: String,
    pub description: String,
    pub inspection_id: Option<String>,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct DispositionNcrRequest {
    /// One of the `Disposition` values, e.g. `use_as_is`, `rework`, `scrap`, `return_to_supplier`.
    pub disposition: String,
    /// Decimal quantity as a string.
    pub disposition_quantity: Option<String>,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct NcrFilterParams {
    pub source: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub sku: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct NcrResponse {
    pub id: String,
    pub ncr_number: String,
    pub source: String,
    pub severity: String,
    pub status: String,
    pub sku: String,
    pub quantity_affected: String,
    pub description: String,
    pub disposition: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct NcrListResponse {
    pub ncrs: Vec<NcrResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateQualityHoldRequest {
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    /// Decimal quantity as a string.
    pub quantity: String,
    pub reason: String,
    /// One of the `HoldType` values, e.g. `quality_inspection`, `damage`, `recall`.
    pub hold_type: String,
    pub ncr_id: Option<String>,
    pub inspection_id: Option<String>,
    pub placed_by: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReleaseQualityHoldRequest {
    pub released_by: String,
    pub release_notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct QualityHoldFilterParams {
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    pub hold_type: Option<String>,
    pub active_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct QualityHoldResponse {
    pub id: String,
    pub sku: String,
    pub lot_number: Option<String>,
    pub quantity_held: String,
    pub reason: String,
    pub hold_type: String,
    pub placed_by: String,
    pub released_by: Option<String>,
    pub released_at: Option<String>,
    pub placed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct QualityHoldListResponse {
    pub holds: Vec<QualityHoldResponse>,
    pub total: usize,
}

fn inspection_to_resp(i: &stateset_core::Inspection) -> InspectionResponse {
    InspectionResponse {
        id: i.id.to_string(),
        inspection_number: i.inspection_number.clone(),
        inspection_type: i.inspection_type.to_string(),
        reference_type: i.reference_type.clone(),
        reference_id: i.reference_id.to_string(),
        status: i.status.to_string(),
        notes: i.notes.clone(),
        created_at: i.created_at.to_rfc3339(),
    }
}

fn ncr_to_resp(n: &stateset_core::NonConformance) -> NcrResponse {
    NcrResponse {
        id: n.id.to_string(),
        ncr_number: n.ncr_number.clone(),
        source: n.source.to_string(),
        severity: n.severity.to_string(),
        status: n.status.to_string(),
        sku: n.sku.clone(),
        quantity_affected: n.quantity_affected.to_string(),
        description: n.description.clone(),
        disposition: n.disposition.map(|d| d.to_string()),
        created_at: n.created_at.to_rfc3339(),
    }
}

fn hold_to_resp(h: &stateset_core::QualityHold) -> QualityHoldResponse {
    QualityHoldResponse {
        id: h.id.to_string(),
        sku: h.sku.clone(),
        lot_number: h.lot_number.clone(),
        quantity_held: h.quantity_held.to_string(),
        reason: h.reason.clone(),
        hold_type: h.hold_type.to_string(),
        placed_by: h.placed_by.clone(),
        released_by: h.released_by.clone(),
        released_at: h.released_at.map(|t| t.to_rfc3339()),
        placed_at: h.placed_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_opt_id<T: std::str::FromStr>(s: Option<&str>, what: &str) -> Result<Option<T>, HttpError> {
    s.map(|v| parse_id(v, what)).transpose()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/quality/inspections", post(create_inspection).get(list_inspections))
        .route("/quality/inspections/{id}", get(get_inspection))
        .route("/quality/inspections/{id}/start", post(start_inspection))
        .route("/quality/inspections/{id}/results", post(record_inspection_result))
        .route("/quality/inspections/{id}/complete", post(complete_inspection))
        .route("/quality/ncrs", post(create_ncr).get(list_ncrs))
        .route("/quality/ncrs/{id}", get(get_ncr))
        .route("/quality/ncrs/{id}/disposition", post(disposition_ncr))
        .route("/quality/ncrs/{id}/close", post(close_ncr))
        .route("/quality/holds", post(create_hold).get(list_holds))
        .route("/quality/holds/{id}", get(get_hold))
        .route("/quality/holds/{id}/release", post(release_hold))
}

#[utoipa::path(post, operation_id = "quality_create_inspection", path = "/api/v1/quality/inspections", tag = "quality",
    request_body = CreateInspectionRequest,
    responses((status = 201, body = InspectionResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_inspection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateInspectionRequest>,
) -> Result<(StatusCode, Json<InspectionResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        items.push(stateset_core::CreateInspectionItem {
            sku: i.sku,
            lot_number: i.lot_number,
            serial_number: i.serial_number,
            quantity_to_inspect: parse_decimal(&i.quantity_to_inspect, "quantity_to_inspect")?,
        });
    }
    let input = stateset_core::CreateInspection {
        inspection_type: parse_id(&req.inspection_type, "inspection_type")?,
        reference_type: req.reference_type,
        reference_id: parse_id::<Uuid>(&req.reference_id, "reference_id")?,
        inspector_id: req.inspector_id,
        scheduled_at: None,
        notes: req.notes,
        items,
    };
    let inspection = c.quality().create_inspection(input)?;
    Ok((StatusCode::CREATED, Json(inspection_to_resp(&inspection))))
}

#[utoipa::path(get, operation_id = "quality_list_inspections", path = "/api/v1/quality/inspections", tag = "quality",
    params(InspectionFilterParams),
    responses((status = 200, body = InspectionListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_inspections(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<InspectionFilterParams>,
) -> Result<Json<InspectionListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let inspection_type = match params.inspection_type.as_deref() {
        Some(s) => Some(parse_id(s, "inspection_type")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::InspectionFilter {
        inspection_type,
        status,
        reference_type: params.reference_type.clone(),
        ..Default::default()
    };
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };
    let total = c.quality().count_inspections(base.clone())?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let filter = stateset_core::InspectionFilter {
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(params.offset.unwrap_or(0)) },
        after_cursor,
        ..base
    };
    let mut inspections = c.quality().list_inspections(filter)?;
    let has_more = finalize_page(&mut inspections, limit);
    let next_cursor = if has_more {
        inspections.last().map(|i| encode_cursor(&i.created_at.to_rfc3339(), &i.id.to_string()))
    } else {
        None
    };
    Ok(Json(InspectionListResponse {
        inspections: inspections.iter().map(inspection_to_resp).collect(),
        total,
        next_cursor,
        has_more,
    }))
}

#[utoipa::path(get, operation_id = "quality_get_inspection", path = "/api/v1/quality/inspections/{id}", tag = "quality",
    params(("id" = String, Path, description = "Inspection ID")),
    responses((status = 200, body = InspectionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_inspection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<InspectionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let inspection = c
        .quality()
        .get_inspection(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Inspection {id} not found")))?;
    Ok(Json(inspection_to_resp(&inspection)))
}

#[utoipa::path(post, operation_id = "quality_start_inspection", path = "/api/v1/quality/inspections/{id}/start", tag = "quality",
    params(("id" = String, Path, description = "Inspection ID")),
    responses((status = 200, body = InspectionResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn start_inspection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<InspectionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(inspection_to_resp(&c.quality().start_inspection(id)?)))
}

#[utoipa::path(post, operation_id = "quality_record_inspection_result", path = "/api/v1/quality/inspections/{id}/results", tag = "quality",
    request_body = RecordInspectionResultRequest,
    params(("id" = String, Path, description = "Inspection ID")),
    responses((status = 200, body = InspectionResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn record_inspection_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<RecordInspectionResultRequest>,
) -> Result<Json<InspectionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::RecordInspectionResult {
        item_id: parse_id::<Uuid>(&req.item_id, "item_id")?,
        quantity_passed: parse_decimal(&req.quantity_passed, "quantity_passed")?,
        quantity_failed: parse_decimal(&req.quantity_failed, "quantity_failed")?,
        result: parse_id(&req.result, "result")?,
        defect_codes: req.defect_codes,
        measurements: None,
        notes: req.notes,
    };
    c.quality().record_inspection_result(input)?;
    let inspection = c
        .quality()
        .get_inspection(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Inspection {id} not found")))?;
    Ok(Json(inspection_to_resp(&inspection)))
}

#[utoipa::path(post, operation_id = "quality_complete_inspection", path = "/api/v1/quality/inspections/{id}/complete", tag = "quality",
    params(("id" = String, Path, description = "Inspection ID")),
    responses((status = 200, body = InspectionResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn complete_inspection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<InspectionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(inspection_to_resp(&c.quality().complete_inspection(id)?)))
}

#[utoipa::path(post, operation_id = "quality_create_ncr", path = "/api/v1/quality/ncrs", tag = "quality",
    request_body = CreateNcrRequest,
    responses((status = 201, body = NcrResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_ncr(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateNcrRequest>,
) -> Result<(StatusCode, Json<NcrResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateNonConformance {
        inspection_id: parse_opt_id::<Uuid>(req.inspection_id.as_deref(), "inspection_id")?,
        source: parse_id(&req.source, "source")?,
        severity: parse_id(&req.severity, "severity")?,
        sku: req.sku,
        lot_number: req.lot_number,
        serial_number: req.serial_number,
        quantity_affected: parse_decimal(&req.quantity_affected, "quantity_affected")?,
        description: req.description,
        assigned_to: req.assigned_to,
    };
    let ncr = c.quality().create_ncr(input)?;
    Ok((StatusCode::CREATED, Json(ncr_to_resp(&ncr))))
}

#[utoipa::path(get, operation_id = "quality_list_ncrs", path = "/api/v1/quality/ncrs", tag = "quality",
    params(NcrFilterParams),
    responses((status = 200, body = NcrListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_ncrs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<NcrFilterParams>,
) -> Result<Json<NcrListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let source = match params.source.as_deref() {
        Some(s) => Some(parse_id(s, "source")?),
        None => None,
    };
    let severity = match params.severity.as_deref() {
        Some(s) => Some(parse_id(s, "severity")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::NonConformanceFilter {
        source,
        severity,
        status,
        sku: params.sku.clone(),
        ..Default::default()
    };
    let total = c.quality().count_ncrs(base.clone())?;
    let filter = stateset_core::NonConformanceFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let ncrs = c.quality().list_ncrs(filter)?;
    Ok(Json(NcrListResponse { ncrs: ncrs.iter().map(ncr_to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "quality_get_ncr", path = "/api/v1/quality/ncrs/{id}", tag = "quality",
    params(("id" = String, Path, description = "NCR ID")),
    responses((status = 200, body = NcrResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_ncr(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<NcrResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let ncr = c
        .quality()
        .get_ncr(id)?
        .ok_or_else(|| HttpError::NotFound(format!("NCR {id} not found")))?;
    Ok(Json(ncr_to_resp(&ncr)))
}

#[utoipa::path(post, operation_id = "quality_disposition_ncr", path = "/api/v1/quality/ncrs/{id}/disposition", tag = "quality",
    request_body = DispositionNcrRequest,
    params(("id" = String, Path, description = "NCR ID")),
    responses((status = 200, body = NcrResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn disposition_ncr(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<DispositionNcrRequest>,
) -> Result<Json<NcrResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let disposition_quantity = match req.disposition_quantity.as_deref() {
        Some(s) => Some(parse_decimal(s, "disposition_quantity")?),
        None => None,
    };
    let input = stateset_core::UpdateNonConformance {
        disposition: Some(parse_id(&req.disposition, "disposition")?),
        disposition_quantity,
        root_cause: req.root_cause,
        corrective_action: req.corrective_action,
        preventive_action: req.preventive_action,
        ..Default::default()
    };
    Ok(Json(ncr_to_resp(&c.quality().update_ncr(id, input)?)))
}

#[utoipa::path(post, operation_id = "quality_close_ncr", path = "/api/v1/quality/ncrs/{id}/close", tag = "quality",
    params(("id" = String, Path, description = "NCR ID")),
    responses((status = 200, body = NcrResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn close_ncr(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<NcrResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(ncr_to_resp(&c.quality().close_ncr(id)?)))
}

#[utoipa::path(post, operation_id = "quality_create_hold", path = "/api/v1/quality/holds", tag = "quality",
    request_body = CreateQualityHoldRequest,
    responses((status = 201, body = QualityHoldResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_hold(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateQualityHoldRequest>,
) -> Result<(StatusCode, Json<QualityHoldResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateQualityHold {
        sku: req.sku,
        lot_number: req.lot_number,
        serial_number: req.serial_number,
        location_id: None,
        quantity: parse_decimal(&req.quantity, "quantity")?,
        reason: req.reason,
        hold_type: parse_id(&req.hold_type, "hold_type")?,
        ncr_id: parse_opt_id::<Uuid>(req.ncr_id.as_deref(), "ncr_id")?,
        inspection_id: parse_opt_id::<Uuid>(req.inspection_id.as_deref(), "inspection_id")?,
        placed_by: req.placed_by,
        expires_at: None,
    };
    let hold = c.quality().create_hold(input)?;
    Ok((StatusCode::CREATED, Json(hold_to_resp(&hold))))
}

#[utoipa::path(get, operation_id = "quality_list_holds", path = "/api/v1/quality/holds", tag = "quality",
    params(QualityHoldFilterParams),
    responses((status = 200, body = QualityHoldListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_holds(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<QualityHoldFilterParams>,
) -> Result<Json<QualityHoldListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let hold_type = match params.hold_type.as_deref() {
        Some(s) => Some(parse_id(s, "hold_type")?),
        None => None,
    };
    let base = stateset_core::QualityHoldFilter {
        sku: params.sku.clone(),
        lot_number: params.lot_number.clone(),
        hold_type,
        active_only: params.active_only,
        ..Default::default()
    };
    let total = c.quality().list_holds(base.clone())?.len();
    let filter = stateset_core::QualityHoldFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let holds = c.quality().list_holds(filter)?;
    Ok(Json(QualityHoldListResponse { holds: holds.iter().map(hold_to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "quality_get_hold", path = "/api/v1/quality/holds/{id}", tag = "quality",
    params(("id" = String, Path, description = "Quality hold ID")),
    responses((status = 200, body = QualityHoldResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_hold(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<QualityHoldResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let hold = c
        .quality()
        .get_hold(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Quality hold {id} not found")))?;
    Ok(Json(hold_to_resp(&hold)))
}

#[utoipa::path(post, operation_id = "quality_release_hold", path = "/api/v1/quality/holds/{id}/release", tag = "quality",
    request_body = ReleaseQualityHoldRequest,
    params(("id" = String, Path, description = "Quality hold ID")),
    responses((status = 200, body = QualityHoldResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn release_hold(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReleaseQualityHoldRequest>,
) -> Result<Json<QualityHoldResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::ReleaseQualityHold {
        released_by: req.released_by,
        release_notes: req.release_notes,
    };
    Ok(Json(hold_to_resp(&c.quality().release_hold(id, input)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn inspection_create_and_get() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "inspection_type": "receiving",
            "reference_type": "purchase_order",
            "reference_id": uuid::Uuid::new_v4().to_string(),
            "items": [{"sku": "SKU-001", "quantity_to_inspect": "10"}]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/quality/inspections")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "pending");
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::get(format!("/quality/inspections/{id}")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["inspection_type"], "receiving");
    }

    #[tokio::test]
    async fn ncr_create_disposition_close_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "source": "inspection",
            "severity": "major",
            "sku": "SKU-001",
            "quantity_affected": "5",
            "description": "Out of spec"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/quality/ncrs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/quality/ncrs/{id}/disposition"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"disposition": "scrap", "disposition_quantity": "5"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["disposition"], "scrap");

        let resp = app
            .oneshot(
                Request::post(format!("/quality/ncrs/{id}/close")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "closed");
    }

    #[tokio::test]
    async fn hold_create_and_release() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "sku": "SKU-001",
            "quantity": "50",
            "reason": "Pending inspection",
            "hold_type": "quality_inspection",
            "placed_by": "QA Team"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/quality/holds")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        assert_eq!(json["quantity_held"], "50");
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::post(format!("/quality/holds/{id}/release"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"released_by": "QA Manager"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["released_by"], "QA Manager");
    }

    #[tokio::test]
    async fn list_inspections_has_more_and_next_cursor() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        for i in 0..3 {
            let _ = i;
            let body = serde_json::json!({
                "inspection_type": "receiving",
                "reference_type": "purchase_order",
                "reference_id": uuid::Uuid::new_v4().to_string(),
                "items": [{"sku": "SKU-CUR", "quantity_to_inspect": "1"}]
            });
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/quality/inspections")
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
            .oneshot(Request::get("/quality/inspections?limit=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["inspections"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().expect("next_cursor").to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::get(format!("/quality/inspections?limit=2&after={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["inspections"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
        assert!(json.get("next_cursor").is_none() || json["next_cursor"].is_null());
    }

    #[tokio::test]
    async fn list_inspections_invalid_cursor_returns_400() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .oneshot(
                Request::get("/quality/inspections?after=!!!invalid!!!")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
