//! Bill of Materials (BOM) endpoints.

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
use stateset_core::ProductId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateBomComponentRequest {
    pub name: String,
    pub component_product_id: Option<String>,
    pub component_sku: Option<String>,
    /// Decimal quantity as a string.
    pub quantity: String,
    pub unit_of_measure: Option<String>,
    pub position: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateBomRequest {
    pub product_id: String,
    pub name: String,
    pub description: Option<String>,
    pub revision: Option<String>,
    pub components: Option<Vec<CreateBomComponentRequest>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct UpdateBomRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub revision: Option<String>,
    /// One of `draft`, `active`, `obsolete`.
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct BomFilterParams {
    pub product_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BomResponse {
    pub id: String,
    pub bom_number: String,
    pub product_id: String,
    pub name: String,
    pub description: Option<String>,
    pub revision: String,
    pub status: String,
    pub component_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BomListResponse {
    pub boms: Vec<BomResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BomComponentResponse {
    pub id: String,
    pub bom_id: String,
    pub name: String,
    pub component_sku: Option<String>,
    pub quantity: String,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BomComponentListResponse {
    pub components: Vec<BomComponentResponse>,
    pub total: usize,
}

fn to_resp(b: &stateset_core::BillOfMaterials) -> BomResponse {
    BomResponse {
        id: b.id.to_string(),
        bom_number: b.bom_number.clone(),
        product_id: b.product_id.to_string(),
        name: b.name.clone(),
        description: b.description.clone(),
        revision: b.revision.clone(),
        status: b.status.to_string(),
        component_count: b.components.len(),
        created_at: b.created_at.to_rfc3339(),
    }
}

fn component_to_resp(comp: &stateset_core::BomComponent) -> BomComponentResponse {
    BomComponentResponse {
        id: comp.id.to_string(),
        bom_id: comp.bom_id.to_string(),
        name: comp.name.clone(),
        component_sku: comp.component_sku.clone(),
        quantity: comp.quantity.to_string(),
        unit_of_measure: comp.unit_of_measure.clone(),
        position: comp.position.clone(),
        notes: comp.notes.clone(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn to_component_input(
    comp: CreateBomComponentRequest,
) -> Result<stateset_core::CreateBomComponent, HttpError> {
    let component_product_id = match comp.component_product_id.as_deref() {
        Some(s) => Some(parse_id::<ProductId>(s, "component_product_id")?),
        None => None,
    };
    Ok(stateset_core::CreateBomComponent {
        component_product_id,
        component_sku: comp.component_sku,
        name: comp.name,
        quantity: parse_decimal(&comp.quantity, "quantity")?,
        unit_of_measure: comp.unit_of_measure,
        position: comp.position,
        notes: comp.notes,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/boms", post(create).get(list))
        .route("/boms/{id}", get(get_one).put(update).delete(delete_one))
        .route("/boms/{id}/activate", post(activate))
        .route("/boms/{id}/components", post(add_component).get(list_components))
}

#[utoipa::path(post, operation_id = "bom_create", path = "/api/v1/boms", tag = "bom",
    request_body = CreateBomRequest,
    responses((status = 201, body = BomResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateBomRequest>,
) -> Result<(StatusCode, Json<BomResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let components = match req.components {
        Some(comps) => {
            Some(comps.into_iter().map(to_component_input).collect::<Result<Vec<_>, _>>()?)
        }
        None => None,
    };
    let input = stateset_core::CreateBom {
        product_id: parse_id::<ProductId>(&req.product_id, "product_id")?,
        name: req.name,
        description: req.description,
        revision: req.revision,
        components,
        created_by: None,
    };
    let bom = c.bom().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&bom))))
}

#[utoipa::path(get, operation_id = "bom_list", path = "/api/v1/boms", tag = "bom",
    params(BomFilterParams),
    responses((status = 200, body = BomListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<BomFilterParams>,
) -> Result<Json<BomListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = match params.product_id.as_deref() {
        Some(s) => Some(parse_id::<ProductId>(s, "product_id")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::BomFilter {
        product_id,
        status,
        search: params.search.clone(),
        ..Default::default()
    };
    let total = c.bom().count(base.clone())?;
    let filter = stateset_core::BomFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let boms = c.bom().list(filter)?;
    Ok(Json(BomListResponse { boms: boms.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "bom_get_one", path = "/api/v1/boms/{id}", tag = "bom",
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 200, body = BomResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BomResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let bom = c.bom().get(id)?.ok_or_else(|| HttpError::NotFound(format!("BOM {id} not found")))?;
    Ok(Json(to_resp(&bom)))
}

#[utoipa::path(put, operation_id = "bom_update", path = "/api/v1/boms/{id}", tag = "bom",
    request_body = UpdateBomRequest,
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 200, body = BomResponse), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateBomRequest>,
) -> Result<Json<BomResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match req.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let input = stateset_core::UpdateBom {
        name: req.name,
        description: req.description,
        revision: req.revision,
        status,
        updated_by: None,
    };
    Ok(Json(to_resp(&c.bom().update(id, input)?)))
}

#[utoipa::path(delete, operation_id = "bom_delete", path = "/api/v1/boms/{id}", tag = "bom",
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 204), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.bom().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, operation_id = "bom_activate", path = "/api/v1/boms/{id}/activate", tag = "bom",
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 200, body = BomResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn activate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BomResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.bom().activate(id)?)))
}

#[utoipa::path(post, operation_id = "bom_add_component", path = "/api/v1/boms/{id}/components", tag = "bom",
    request_body = CreateBomComponentRequest,
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 201, body = BomComponentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn add_component(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateBomComponentRequest>,
) -> Result<(StatusCode, Json<BomComponentResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let component = c.bom().add_component(id, to_component_input(req)?)?;
    Ok((StatusCode::CREATED, Json(component_to_resp(&component))))
}

#[utoipa::path(get, operation_id = "bom_list_components", path = "/api/v1/boms/{id}/components", tag = "bom",
    params(("id" = String, Path, description = "BOM ID")),
    responses((status = 200, body = BomComponentListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_components(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BomComponentListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let components = c.bom().get_components(id)?;
    Ok(Json(BomComponentListResponse {
        total: components.len(),
        components: components.iter().map(component_to_resp).collect(),
    }))
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
    async fn create_activate_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "product_id": uuid::Uuid::new_v4().to_string(),
            "name": "Widget Assembly",
            "components": [{"name": "Screw M3", "component_sku": "SCREW-M3", "quantity": "4"}]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/boms")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        assert_eq!(json["status"], "draft");
        assert_eq!(json["component_count"], 1);
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(Request::post(format!("/boms/{id}/activate")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["status"], "active");

        let resp = app
            .oneshot(Request::get(format!("/boms/{id}/components")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["total"], 1);
    }

    #[tokio::test]
    async fn list_and_update() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "product_id": uuid::Uuid::new_v4().to_string(),
            "name": "Gadget BOM"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/boms")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let id = json_body(resp).await["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(Request::get("/boms?status=draft").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json_body(resp).await["total"], 1);

        let resp = app
            .oneshot(
                Request::put(format!("/boms/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"name": "Gadget BOM v2", "revision": "B"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = json_body(resp).await;
        assert_eq!(json["name"], "Gadget BOM v2");
        assert_eq!(json["revision"], "B");
    }
}
