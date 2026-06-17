//! Integration mapping endpoints (external↔internal value translation).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::IntegrationMappingId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateIntegrationMappingRequest {
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
    pub internal_value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct BulkCreateRequest {
    pub mappings: Vec<CreateIntegrationMappingRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateIntegrationMappingRequest {
    pub internal_value: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct IntegrationMappingFilterParams {
    pub integration: Option<String>,
    pub mapping_group: Option<String>,
    pub field_name: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ResolveParams {
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct IntegrationMappingResponse {
    pub id: String,
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
    pub internal_value: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct IntegrationMappingListResponse {
    pub mappings: Vec<IntegrationMappingResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ResolveResponse {
    pub internal_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BulkResultResponse {
    pub affected: u64,
}

fn to_resp(m: &stateset_core::IntegrationMapping) -> IntegrationMappingResponse {
    IntegrationMappingResponse {
        id: m.id.to_string(),
        integration: m.integration.clone(),
        mapping_group: m.mapping_group.clone(),
        field_name: m.field_name.clone(),
        external_value: m.external_value.clone(),
        internal_value: m.internal_value.clone(),
        is_active: m.is_active,
    }
}

fn to_input(r: CreateIntegrationMappingRequest) -> stateset_core::CreateIntegrationMapping {
    stateset_core::CreateIntegrationMapping {
        integration: r.integration,
        mapping_group: r.mapping_group,
        field_name: r.field_name,
        external_value: r.external_value,
        internal_value: r.internal_value,
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/integration-mappings", post(create).get(list))
        .route("/integration-mappings/bulk", post(bulk_create))
        .route("/integration-mappings/resolve", get(resolve))
        .route("/integration-mappings/{id}", get(get_one).put(update).delete(delete_one))
}

#[utoipa::path(post, path = "/api/v1/integration-mappings", tag = "integration_mappings",
    request_body = CreateIntegrationMappingRequest,
    responses((status = 201, body = IntegrationMappingResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateIntegrationMappingRequest>,
) -> Result<(StatusCode, Json<IntegrationMappingResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let m = c.integration_mappings().create(to_input(req))?;
    Ok((StatusCode::CREATED, Json(to_resp(&m))))
}

#[utoipa::path(post, path = "/api/v1/integration-mappings/bulk", tag = "integration_mappings",
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
    let items = req.mappings.into_iter().map(to_input).collect();
    let affected = c.integration_mappings().bulk_upsert(items)?;
    Ok(Json(BulkResultResponse { affected }))
}

#[utoipa::path(get, path = "/api/v1/integration-mappings", tag = "integration_mappings",
    params(IntegrationMappingFilterParams),
    responses((status = 200, body = IntegrationMappingListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<IntegrationMappingFilterParams>,
) -> Result<Json<IntegrationMappingListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let base = stateset_core::IntegrationMappingFilter {
        integration: params.integration.clone(),
        mapping_group: params.mapping_group.clone(),
        field_name: params.field_name.clone(),
        is_active: params.is_active,
        ..Default::default()
    };
    let total = c.integration_mappings().list(base.clone())?.len();
    let filter = stateset_core::IntegrationMappingFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 500)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let mappings = c.integration_mappings().list(filter)?;
    Ok(Json(IntegrationMappingListResponse {
        mappings: mappings.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/integration-mappings/resolve", tag = "integration_mappings",
    params(ResolveParams),
    responses((status = 200, body = ResolveResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn resolve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ResolveParams>,
) -> Result<Json<ResolveResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let lookup = stateset_core::MappingLookup {
        integration: params.integration,
        mapping_group: params.mapping_group,
        field_name: params.field_name,
        external_value: params.external_value,
    };
    let internal_value = c.integration_mappings().resolve(&lookup)?;
    Ok(Json(ResolveResponse { internal_value }))
}

#[utoipa::path(get, path = "/api/v1/integration-mappings/{id}", tag = "integration_mappings",
    params(("id" = String, Path, description = "Integration mapping ID")),
    responses((status = 200, body = IntegrationMappingResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IntegrationMappingId>,
) -> Result<Json<IntegrationMappingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let m = c
        .integration_mappings()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Integration mapping {id} not found")))?;
    Ok(Json(to_resp(&m)))
}

#[utoipa::path(put, path = "/api/v1/integration-mappings/{id}", tag = "integration_mappings",
    request_body = UpdateIntegrationMappingRequest,
    params(("id" = String, Path, description = "Integration mapping ID")),
    responses((status = 200, body = IntegrationMappingResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IntegrationMappingId>,
    Json(req): Json<UpdateIntegrationMappingRequest>,
) -> Result<Json<IntegrationMappingResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::UpdateIntegrationMapping {
        internal_value: req.internal_value,
        is_active: req.is_active,
    };
    Ok(Json(to_resp(&c.integration_mappings().update(id, input)?)))
}

#[utoipa::path(delete, path = "/api/v1/integration-mappings/{id}", tag = "integration_mappings",
    params(("id" = String, Path, description = "Integration mapping ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IntegrationMappingId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.integration_mappings().delete(id)?;
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
    async fn create_then_resolve() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "integration": "shopify",
            "mapping_group": "carrier",
            "field_name": "carrier_code",
            "external_value": "USPS Ground",
            "internal_value": "usps"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/integration-mappings")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = app
            .oneshot(
                Request::get("/integration-mappings/resolve?integration=shopify&mapping_group=carrier&field_name=carrier_code&external_value=USPS%20Ground")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["internal_value"], "usps");
    }
}
