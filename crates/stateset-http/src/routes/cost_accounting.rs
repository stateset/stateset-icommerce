//! Cost accounting API. All monetary and quantity inputs are decimal strings.
use crate::{
    error::HttpError,
    middleware::FinanceActor,
    state::{AppState, tenant_id_from_headers},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stateset_core::*;
use std::result::Result;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
fn parse<T: std::str::FromStr>(value: &str, field: &str) -> Result<T, HttpError> {
    value.parse().map_err(|_| HttpError::BadRequest(format!("invalid {field}")))
}
fn optional<T: std::str::FromStr>(
    value: Option<String>,
    field: &str,
) -> Result<Option<T>, HttpError> {
    value.map(|v| parse(&v, field)).transpose()
}
fn json(value: impl Serialize) -> Result<Json<Value>, HttpError> {
    serde_json::to_value(value).map(Json).map_err(|e| HttpError::InternalError(e.to_string()))
}
fn found<T>(value: Option<T>) -> Result<T, HttpError> {
    value.ok_or_else(|| HttpError::NotFound("Cost record not found".into()))
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetCostRequest {
    pub sku: String,
    pub cost_method: Option<String>,
    pub standard_cost: Option<String>,
    pub material_cost: Option<String>,
    pub labor_cost: Option<String>,
    pub overhead_cost: Option<String>,
    pub currency: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct AverageCostRequest {
    pub quantity: String,
    pub unit_cost: String,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct LastCostRequest {
    pub unit_cost: String,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct CostLayerRequest {
    pub sku: String,
    pub quantity: String,
    pub unit_cost: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub lot_id: Option<String>,
    pub location_id: Option<i32>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct IssueLayersRequest {
    pub sku: String,
    pub quantity: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct CostAdjustmentRequest {
    pub sku: String,
    pub adjustment_type: String,
    pub new_cost: String,
    pub reason: String,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct CostRollupRequest {
    pub bom_id: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct CostVarianceRequest {
    pub sku: String,
    pub variance_type: String,
    pub standard_cost: String,
    pub actual_cost: String,
    pub quantity: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct CostTransactionRequest {
    pub sku: String,
    pub transaction_type: String,
    pub quantity: String,
    pub unit_cost: String,
    pub layer_id: Option<String>,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
}
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in=Query)]
pub(crate) struct ValuationQuery {
    pub cost_method: Option<String>,
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cost-accounting/items", post(set_cost).get(list_items))
        .route("/cost-accounting/layers", get(list_layers).post(create_layer))
        .route("/cost-accounting/transactions", get(list_transactions).post(record_transaction))
        .route("/cost-accounting/adjustments", get(list_adjustments).post(create_adjustment))
        .route("/cost-accounting/variances", get(list_variances).post(record_variance))
        .route("/cost-accounting/items/{sku}", get(get_cost))
        .route("/cost-accounting/items/{sku}/average", post(average_cost))
        .route("/cost-accounting/items/{sku}/last", post(last_cost))
        .route("/cost-accounting/layers/{id}", get(get_layer))
        .route("/cost-accounting/layers/issue-fifo", post(issue_fifo))
        .route("/cost-accounting/layers/issue-lifo", post(issue_lifo))
        .route("/cost-accounting/adjustments/{id}", get(get_adjustment))
        .route("/cost-accounting/adjustments/{id}/approve", post(approve_adjustment))
        .route("/cost-accounting/adjustments/{id}/apply", post(apply_adjustment))
        .route("/cost-accounting/adjustments/{id}/reject", post(reject_adjustment))
        .route("/cost-accounting/items/{sku}/rollup", post(calculate_rollup).get(get_rollup))
        .route("/cost-accounting/items/{sku}/summary", get(cost_summary))
        .route("/cost-accounting/items/{sku}/remaining", get(layers_remaining))
        .route("/cost-accounting/valuation", get(valuation))
        .route("/cost-accounting/total-value", get(total_value))
        .route("/cost-accounting/close-check", get(close_check))
}
#[utoipa::path(post,operation_id="cost_set_cost",path="/api/v1/cost-accounting/items",tag="cost_accounting",request_body=SetCostRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn set_cost(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetCostRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().set_item_cost(SetItemCost {
                sku: req.sku,
                cost_method: optional(req.cost_method, "cost_method")?,
                standard_cost: optional(req.standard_cost, "standard_cost")?,
                material_cost: optional(req.material_cost, "material_cost")?,
                labor_cost: optional(req.labor_cost, "labor_cost")?,
                overhead_cost: optional(req.overhead_cost, "overhead_cost")?,
                currency: optional(req.currency, "currency")?,
            })?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_list_items",path="/api/v1/cost-accounting/items",tag="cost_accounting",params(("sku"=Option<String>,Query,description="sku"),("cost_method"=Option<String>,Query,description="cost method"),("limit"=Option<u32>,Query,description="Maximum 200 records"),("offset"=Option<u32>,Query,description="offset")),responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn list_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(mut req): Query<ItemCostFilter>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            req.limit = Some(req.limit.unwrap_or(50).clamp(1, 200));
            json(c.cost_accounting().list_item_costs(req)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_list_layers",path="/api/v1/cost-accounting/layers",tag="cost_accounting",params(("sku"=Option<String>,Query,description="sku"),("source_type"=Option<String>,Query,description="source type"),("has_remaining"=Option<bool>,Query,description="has remaining"),("from_date"=Option<String>,Query,description="RFC3339 timestamp"),("to_date"=Option<String>,Query,description="RFC3339 timestamp"),("limit"=Option<u32>,Query,description="Maximum 200 records"),("offset"=Option<u32>,Query,description="offset")),responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn list_layers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(mut req): Query<CostLayerFilter>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            req.limit = Some(req.limit.unwrap_or(50).clamp(1, 200));
            json(c.cost_accounting().list_cost_layers(req)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_list_transactions",path="/api/v1/cost-accounting/transactions",tag="cost_accounting",params(("sku"=Option<String>,Query,description="sku"),("transaction_type"=Option<String>,Query,description="transaction type"),("from_date"=Option<String>,Query,description="RFC3339 timestamp"),("to_date"=Option<String>,Query,description="RFC3339 timestamp"),("limit"=Option<u32>,Query,description="Maximum 200 records"),("offset"=Option<u32>,Query,description="offset")),responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn list_transactions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(mut req): Query<CostTransactionFilter>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            req.limit = Some(req.limit.unwrap_or(50).clamp(1, 200));
            json(c.cost_accounting().list_cost_transactions(req)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_list_adjustments",path="/api/v1/cost-accounting/adjustments",tag="cost_accounting",params(("sku"=Option<String>,Query,description="sku"),("status"=Option<String>,Query,description="status"),("adjustment_type"=Option<String>,Query,description="adjustment type"),("from_date"=Option<String>,Query,description="RFC3339 timestamp"),("to_date"=Option<String>,Query,description="RFC3339 timestamp"),("limit"=Option<u32>,Query,description="Maximum 200 records"),("offset"=Option<u32>,Query,description="offset")),responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn list_adjustments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(mut req): Query<CostAdjustmentFilter>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            req.limit = Some(req.limit.unwrap_or(50).clamp(1, 200));
            json(c.cost_accounting().list_adjustments(req)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_list_variances",path="/api/v1/cost-accounting/variances",tag="cost_accounting",params(("sku"=Option<String>,Query,description="sku"),("variance_type"=Option<String>,Query,description="variance type"),("from_date"=Option<String>,Query,description="RFC3339 timestamp"),("to_date"=Option<String>,Query,description="RFC3339 timestamp"),("limit"=Option<u32>,Query,description="Maximum 200 records"),("offset"=Option<u32>,Query,description="offset")),responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn list_variances(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(mut req): Query<CostVarianceFilter>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            req.limit = Some(req.limit.unwrap_or(50).clamp(1, 200));
            json(c.cost_accounting().list_variances(req)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_get_cost",path="/api/v1/cost-accounting/items/{sku}",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn get_cost(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(found(c.cost_accounting().get_item_cost(&sku)?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_average_cost",path="/api/v1/cost-accounting/items/{sku}/average",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), request_body=AverageCostRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn average_cost(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
    Json(req): Json<AverageCostRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().update_average_cost(
                &sku,
                parse(&req.quantity, "quantity")?,
                parse(&req.unit_cost, "unit_cost")?,
            )?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_last_cost",path="/api/v1/cost-accounting/items/{sku}/last",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), request_body=LastCostRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn last_cost(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
    Json(req): Json<LastCostRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().update_last_cost(&sku, parse(&req.unit_cost, "unit_cost")?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_create_layer",path="/api/v1/cost-accounting/layers",tag="cost_accounting",request_body=CostLayerRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn create_layer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CostLayerRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().create_cost_layer(CreateCostLayer {
                sku: req.sku,
                quantity: parse(&req.quantity, "quantity")?,
                unit_cost: parse(&req.unit_cost, "unit_cost")?,
                source_type: parse(&req.source_type, "source_type")?,
                source_id: optional(req.source_id, "source_id")?,
                lot_id: optional(req.lot_id, "lot_id")?,
                location_id: req.location_id,
            })?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_get_layer",path="/api/v1/cost-accounting/layers/{id}",tag="cost_accounting",params(("id"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn get_layer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(found(c.cost_accounting().get_cost_layer(id)?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_issue_fifo",path="/api/v1/cost-accounting/layers/issue-fifo",tag="cost_accounting",request_body=IssueLayersRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn issue_fifo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<IssueLayersRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().issue_fifo(IssueCostLayers {
                sku: req.sku,
                quantity: parse(&req.quantity, "quantity")?,
                reference_type: req.reference_type,
                reference_id: optional(req.reference_id, "reference_id")?,
                notes: req.notes,
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_issue_lifo",path="/api/v1/cost-accounting/layers/issue-lifo",tag="cost_accounting",request_body=IssueLayersRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn issue_lifo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<IssueLayersRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().issue_lifo(IssueCostLayers {
                sku: req.sku,
                quantity: parse(&req.quantity, "quantity")?,
                reference_type: req.reference_type,
                reference_id: optional(req.reference_id, "reference_id")?,
                notes: req.notes,
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_create_adjustment",path="/api/v1/cost-accounting/adjustments",tag="cost_accounting",request_body=CostAdjustmentRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn create_adjustment(
    State(state): State<AppState>,
    headers: HeaderMap,
    actor: FinanceActor,
    Json(req): Json<CostAdjustmentRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().create_adjustment(CreateCostAdjustment {
                sku: req.sku,
                adjustment_type: parse(&req.adjustment_type, "adjustment_type")?,
                new_cost: parse(&req.new_cost, "new_cost")?,
                reason: req.reason,
                created_by: Some(actor.0),
            })?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_get_adjustment",path="/api/v1/cost-accounting/adjustments/{id}",tag="cost_accounting",params(("id"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn get_adjustment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(found(c.cost_accounting().get_adjustment(id)?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_approve_adjustment",path="/api/v1/cost-accounting/adjustments/{id}/approve",tag="cost_accounting",params(("id"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn approve_adjustment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    actor: FinanceActor,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().approve_adjustment(id, &actor.0)?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_apply_adjustment",path="/api/v1/cost-accounting/adjustments/{id}/apply",tag="cost_accounting",params(("id"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn apply_adjustment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| json(c.cost_accounting().apply_adjustment(id)?))
        .await
}

#[utoipa::path(post,operation_id="cost_reject_adjustment",path="/api/v1/cost-accounting/adjustments/{id}/reject",tag="cost_accounting",params(("id"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn reject_adjustment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| json(c.cost_accounting().reject_adjustment(id)?))
        .await
}

#[utoipa::path(post,operation_id="cost_record_variance",path="/api/v1/cost-accounting/variances",tag="cost_accounting",request_body=CostVarianceRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn record_variance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CostVarianceRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().record_variance(RecordCostVariance {
                sku: req.sku,
                variance_type: parse(&req.variance_type, "variance_type")?,
                standard_cost: parse(&req.standard_cost, "standard_cost")?,
                actual_cost: parse(&req.actual_cost, "actual_cost")?,
                quantity: parse(&req.quantity, "quantity")?,
                reference_type: req.reference_type,
                reference_id: optional(req.reference_id, "reference_id")?,
                notes: req.notes,
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_record_transaction",path="/api/v1/cost-accounting/transactions",tag="cost_accounting",request_body=CostTransactionRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn record_transaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CostTransactionRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().record_cost_transaction(
                &req.sku,
                parse(&req.transaction_type, "transaction_type")?,
                parse(&req.quantity, "quantity")?,
                parse(&req.unit_cost, "unit_cost")?,
                optional(req.layer_id, "layer_id")?,
                req.reference_type.as_deref(),
                optional(req.reference_id, "reference_id")?,
                req.notes.as_deref(),
            )?)
        })
        .await
}

#[utoipa::path(post,operation_id="cost_calculate_rollup",path="/api/v1/cost-accounting/items/{sku}/rollup",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), request_body=CostRollupRequest, responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn calculate_rollup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
    Json(req): Json<CostRollupRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().calculate_rollup(&sku, optional(req.bom_id, "bom_id")?)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_get_rollup",path="/api/v1/cost-accounting/items/{sku}/rollup",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn get_rollup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(found(c.cost_accounting().get_rollup(&sku)?)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_cost_summary",path="/api/v1/cost-accounting/items/{sku}/summary",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn cost_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(found(c.cost_accounting().get_sku_cost_summary(&sku)?)?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_layers_remaining",path="/api/v1/cost-accounting/items/{sku}/remaining",tag="cost_accounting",params(("sku"=String,Path,description="Cost record")), responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn layers_remaining(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().get_layers_remaining(&sku)?.to_string())
        })
        .await
}

#[utoipa::path(get,operation_id="cost_valuation",path="/api/v1/cost-accounting/valuation",tag="cost_accounting",params(ValuationQuery),responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn valuation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(req): Query<ValuationQuery>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().get_inventory_valuation(
                optional(req.cost_method, "cost_method")?.unwrap_or_default(),
            )?)
        })
        .await
}

#[utoipa::path(get,operation_id="cost_total_value",path="/api/v1/cost-accounting/total-value",tag="cost_accounting",responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn total_value(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.cost_accounting().get_total_inventory_value()?.to_string())
        })
        .await
}

#[utoipa::path(get,operation_id="cost_close_check",path="/api/v1/cost-accounting/close-check",tag="cost_accounting",responses((status=200,body=Value),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn close_check(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| json(c.cost_accounting().close_books_check()?))
        .await
}
