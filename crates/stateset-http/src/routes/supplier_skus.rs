//! Supplier SKU endpoints (per-supplier SKU / unit-cost overrides).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{ProductId, SupplierSkuId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateSupplierSkuRequest {
    pub product_id: String,
    pub supplier_id: String,
    pub sku: String,
    /// Optional unit cost as a string (e.g. `"9.99"`).
    pub unit_cost: Option<String>,
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SupplierSkuFilterParams {
    pub supplier_id: Option<String>,
    pub product_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SupplierSkuResponse {
    pub id: String,
    pub product_id: String,
    pub supplier_id: String,
    pub sku: String,
    pub unit_cost: Option<String>,
    pub currency: String,
    pub is_preferred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SupplierSkuListResponse {
    pub supplier_skus: Vec<SupplierSkuResponse>,
    pub total: usize,
}

fn to_resp(s: &stateset_core::SupplierSku) -> SupplierSkuResponse {
    SupplierSkuResponse {
        id: s.id.to_string(),
        product_id: s.product_id.to_string(),
        supplier_id: s.supplier_id.to_string(),
        sku: s.sku.clone(),
        unit_cost: s.unit_cost.map(|c| c.to_string()),
        currency: s.currency.to_string(),
        is_preferred: s.is_preferred,
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal_opt(s: Option<&str>) -> Result<Option<Decimal>, HttpError> {
    match s {
        Some(v) => {
            v.parse().map(Some).map_err(|_| HttpError::BadRequest(format!("invalid decimal: {v}")))
        }
        None => Ok(None),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/supplier-skus", post(create).get(list))
        .route("/supplier-skus/{id}", axum::routing::get(get_one).delete(delete_one))
}

#[utoipa::path(post, path = "/api/v1/supplier-skus", tag = "supplier_skus",
    request_body = CreateSupplierSkuRequest,
    responses((status = 201, body = SupplierSkuResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateSupplierSkuRequest>,
) -> Result<(StatusCode, Json<SupplierSkuResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateSupplierSku {
        product_id: parse_id::<ProductId>(&req.product_id, "product_id")?,
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        sku: req.sku,
        unit_cost: parse_decimal_opt(req.unit_cost.as_deref())?,
        currency: None,
        min_order_qty: parse_decimal_opt(req.min_order_qty.as_deref())?,
        lead_time_days: req.lead_time_days,
    };
    let s = c.supplier_skus().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&s))))
}

#[utoipa::path(get, path = "/api/v1/supplier-skus", tag = "supplier_skus",
    params(SupplierSkuFilterParams),
    responses((status = 200, body = SupplierSkuListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SupplierSkuFilterParams>,
) -> Result<Json<SupplierSkuListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let supplier_id = match params.supplier_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "supplier_id")?),
        None => None,
    };
    let product_id = match params.product_id.as_deref() {
        Some(s) => Some(parse_id::<ProductId>(s, "product_id")?),
        None => None,
    };
    let total = c
        .supplier_skus()
        .list(stateset_core::SupplierSkuFilter { supplier_id, product_id, ..Default::default() })?
        .len();
    let filter = stateset_core::SupplierSkuFilter {
        supplier_id,
        product_id,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let skus = c.supplier_skus().list(filter)?;
    Ok(Json(SupplierSkuListResponse { supplier_skus: skus.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/supplier-skus/{id}", tag = "supplier_skus",
    params(("id" = String, Path, description = "Supplier SKU ID")),
    responses((status = 200, body = SupplierSkuResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SupplierSkuId>,
) -> Result<Json<SupplierSkuResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c
        .supplier_skus()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Supplier SKU {id} not found")))?;
    Ok(Json(to_resp(&s)))
}

#[utoipa::path(delete, path = "/api/v1/supplier-skus/{id}", tag = "supplier_skus",
    params(("id" = String, Path, description = "Supplier SKU ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<SupplierSkuId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.supplier_skus().delete(id)?;
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
    async fn create_and_list_supplier_sku() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "product_id": uuid::Uuid::new_v4().to_string(),
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "sku": "ACME-1",
            "unit_cost": "9.99"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/supplier-skus")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp =
            app.oneshot(Request::get("/supplier-skus").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"], 1);
    }
}
