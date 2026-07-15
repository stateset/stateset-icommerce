//! Integration field-mapping endpoints (field-path mappings).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::IntegrationFieldMappingId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateFieldMappingRequest {
    pub integration_account: String,
    pub mapping_group: String,
    pub source_field: String,
    pub destination_field: String,
    pub template: Option<String>,
    /// One of `none`, `uppercase`, `lowercase`, `trim`.
    pub transform: Option<String>,
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct BulkCreateRequest {
    pub mappings: Vec<CreateFieldMappingRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct BulkDeleteRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateFieldMappingRequest {
    pub destination_field: Option<String>,
    pub template: Option<String>,
    pub transform: Option<String>,
    pub fallback: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct FieldMappingFilterParams {
    pub integration_account: Option<String>,
    pub mapping_group: Option<String>,
    pub source_field: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct GroupsParams {
    pub integration_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct FieldMappingResponse {
    pub id: String,
    pub integration_account: String,
    pub mapping_group: String,
    pub source_field: String,
    pub destination_field: String,
    pub template: Option<String>,
    pub transform: String,
    pub fallback: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct FieldMappingListResponse {
    pub mappings: Vec<FieldMappingResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BulkResultResponse {
    pub affected: u64,
}

fn to_resp(m: &stateset_core::IntegrationFieldMapping) -> FieldMappingResponse {
    FieldMappingResponse {
        id: m.id.to_string(),
        integration_account: m.integration_account.clone(),
        mapping_group: m.mapping_group.clone(),
        source_field: m.source_field.clone(),
        destination_field: m.destination_field.clone(),
        template: m.template.clone(),
        transform: m.transform.to_string(),
        fallback: m.fallback.clone(),
        is_active: m.is_active,
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn to_input(
    r: CreateFieldMappingRequest,
) -> Result<stateset_core::CreateIntegrationFieldMapping, HttpError> {
    let transform = match r.transform.as_deref() {
        Some(s) => parse_id(s, "transform")?,
        None => stateset_core::FieldTransform::default(),
    };
    Ok(stateset_core::CreateIntegrationFieldMapping {
        integration_account: r.integration_account,
        mapping_group: r.mapping_group,
        source_field: r.source_field,
        destination_field: r.destination_field,
        template: r.template,
        transform,
        fallback: r.fallback,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/integration-field-mappings", post(create).get(list))
        .route("/integration-field-mappings/bulk", post(bulk_create).delete(bulk_delete))
        .route("/integration-field-mappings/groups", get(groups))
        .route("/integration-field-mappings/{id}", get(get_one).put(update).delete(delete_one))
}

#[utoipa::path(post, operation_id = "integration_field_mappings_create", path = "/api/v1/integration-field-mappings", tag = "integration_field_mappings",
    request_body = CreateFieldMappingRequest,
    responses((status = 201, body = FieldMappingResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateFieldMappingRequest>,
) -> Result<(StatusCode, Json<FieldMappingResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let m = c.integration_field_mappings().create(to_input(req)?)?;
    Ok((StatusCode::CREATED, Json(to_resp(&m))))
}

#[utoipa::path(post, operation_id = "integration_field_mappings_bulk_create", path = "/api/v1/integration-field-mappings/bulk", tag = "integration_field_mappings",
    request_body = BulkCreateRequest,
    responses((status = 200, body = BulkResultResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn bulk_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BulkCreateRequest>,
) -> Result<Json<BulkResultResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut items = Vec::with_capacity(req.mappings.len());
    for m in req.mappings {
        items.push(to_input(m)?);
    }
    let affected = c.integration_field_mappings().bulk_create(items)?;
    Ok(Json(BulkResultResponse { affected }))
}

#[utoipa::path(delete, operation_id = "integration_field_mappings_bulk_delete", path = "/api/v1/integration-field-mappings/bulk", tag = "integration_field_mappings",
    request_body = BulkDeleteRequest,
    responses((status = 200, body = BulkResultResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn bulk_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BulkDeleteRequest>,
) -> Result<Json<BulkResultResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let mut ids = Vec::with_capacity(req.ids.len());
    for id in req.ids {
        ids.push(parse_id::<IntegrationFieldMappingId>(&id, "id")?);
    }
    let affected = c.integration_field_mappings().bulk_delete(ids)?;
    Ok(Json(BulkResultResponse { affected }))
}

#[utoipa::path(get, operation_id = "integration_field_mappings_list", path = "/api/v1/integration-field-mappings", tag = "integration_field_mappings",
    params(FieldMappingFilterParams),
    responses((status = 200, body = FieldMappingListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<FieldMappingFilterParams>,
) -> Result<Json<FieldMappingListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::IntegrationFieldMappingFilter {
        integration_account: params.integration_account.clone(),
        mapping_group: params.mapping_group.clone(),
        source_field: params.source_field.clone(),
        is_active: params.is_active,
        ..Default::default()
    };
    let total = c.integration_field_mappings().list(base.clone())?.len();
    let filter = stateset_core::IntegrationFieldMappingFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 500)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let mappings = c.integration_field_mappings().list(filter)?;
    Ok(Json(FieldMappingListResponse { mappings: mappings.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "integration_field_mappings_groups", path = "/api/v1/integration-field-mappings/groups", tag = "integration_field_mappings",
    params(GroupsParams),
    responses((status = 200, body = [String])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<GroupsParams>,
) -> Result<Json<Vec<String>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(c.integration_field_mappings().distinct_groups(&params.integration_account)?))
}

#[utoipa::path(get, operation_id = "integration_field_mappings_get_one", path = "/api/v1/integration-field-mappings/{id}", tag = "integration_field_mappings",
    params(("id" = String, Path, description = "Field mapping ID")),
    responses((status = 200, body = FieldMappingResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IntegrationFieldMappingId>,
) -> Result<Json<FieldMappingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let m = c
        .integration_field_mappings()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Field mapping {id} not found")))?;
    Ok(Json(to_resp(&m)))
}

#[utoipa::path(put, operation_id = "integration_field_mappings_update", path = "/api/v1/integration-field-mappings/{id}", tag = "integration_field_mappings",
    request_body = UpdateFieldMappingRequest,
    params(("id" = String, Path, description = "Field mapping ID")),
    responses((status = 200, body = FieldMappingResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IntegrationFieldMappingId>,
    Json(req): Json<UpdateFieldMappingRequest>,
) -> Result<Json<FieldMappingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let transform = match req.transform.as_deref() {
        Some(s) => Some(parse_id(s, "transform")?),
        None => None,
    };
    let input = stateset_core::UpdateIntegrationFieldMapping {
        destination_field: req.destination_field,
        template: req.template,
        transform,
        fallback: req.fallback,
        is_active: req.is_active,
    };
    Ok(Json(to_resp(&c.integration_field_mappings().update(id, input)?)))
}

#[utoipa::path(delete, operation_id = "integration_field_mappings_delete_one", path = "/api/v1/integration-field-mappings/{id}", tag = "integration_field_mappings",
    params(("id" = String, Path, description = "Field mapping ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IntegrationFieldMappingId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.integration_field_mappings().delete(id)?;
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
    async fn bulk_create_then_groups() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({"mappings": [
            {"integration_account":"acct-1","mapping_group":"order","source_field":"a","destination_field":"x"},
            {"integration_account":"acct-1","mapping_group":"shipment","source_field":"b","destination_field":"y","transform":"uppercase"}
        ]});
        let resp = app
            .clone()
            .oneshot(
                Request::post("/integration-field-mappings/bulk")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["affected"], 2);

        let resp = app
            .oneshot(
                Request::get("/integration-field-mappings/groups?integration_account=acct-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 2);
    }
}
