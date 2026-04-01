//! Tax calculation endpoints.

use axum::{Json, Router, extract::{Query, State}, http::{HeaderMap, StatusCode}, routing::{get, post}};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct TaxCalculateRequest {
    pub items: Vec<TaxLineItem>,
    pub shipping_address: TaxAddressDto,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct TaxLineItem {
    pub sku: Option<String>,
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub quantity: Option<i32>,
    pub tax_category: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct TaxAddressDto {
    pub country: String,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TaxCalculationResponse {
    #[schema(value_type = String)]
    pub total_tax: Decimal,
    #[schema(value_type = String)]
    pub subtotal: Decimal,
    #[schema(value_type = String)]
    pub total_with_tax: Decimal,
    pub jurisdiction: String,
    #[schema(value_type = String)]
    pub effective_rate: Decimal,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct TaxRateFilterParams {
    pub jurisdiction: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TaxRateResponse {
    pub id: String,
    pub jurisdiction_id: String,
    pub name: String,
    #[schema(value_type = String)]
    pub rate: Decimal,
    pub tax_type: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TaxRateListResponse { pub rates: Vec<TaxRateResponse>, pub total: usize }

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tax/calculate", post(calculate_tax))
        .route("/tax/rates", get(list_rates))
        .route("/tax/jurisdictions", get(list_jurisdictions))
}

#[utoipa::path(post, path = "/api/v1/tax/calculate", tag = "tax",
    request_body = TaxCalculateRequest,
    responses((status = 200, body = TaxCalculationResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn calculate_tax(
    State(state): State<AppState>, headers: HeaderMap, Json(req): Json<TaxCalculateRequest>,
) -> Result<Json<TaxCalculationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let address = stateset_core::TaxAddress {
        country: req.shipping_address.country,
        state: req.shipping_address.state,
        postal_code: req.shipping_address.postal_code,
        city: req.shipping_address.city,
    };
    let items: Vec<stateset_core::TaxCalculationRequest> = req.items.iter().map(|item| {
        stateset_core::TaxCalculationRequest {
            line_items: vec![stateset_core::models::tax::TaxLineItem {
                sku: item.sku.clone(),
                amount: item.amount,
                quantity: item.quantity.unwrap_or(1),
                product_tax_category: item.tax_category.as_deref()
                    .and_then(|c| c.parse().ok()),
            }],
            shipping_address: address.clone(),
            billing_address: None,
            shipping_amount: None,
            discount_amount: None,
            customer_tax_exempt: false,
            exemption_certificate: None,
        }
    }).collect();
    if let Some(request) = items.into_iter().next() {
        let result = c.tax().calculate(request)?;
        Ok(Json(TaxCalculationResponse {
            total_tax: result.total_tax, subtotal: result.subtotal,
            total_with_tax: result.total_with_tax,
            jurisdiction: result.jurisdiction_name.unwrap_or_default(),
            effective_rate: result.effective_rate,
        }))
    } else {
        Err(HttpError::BadRequest("At least one line item required".into()))
    }
}

#[utoipa::path(get, path = "/api/v1/tax/rates", tag = "tax", params(TaxRateFilterParams),
    responses((status = 200, body = TaxRateListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_rates(
    State(state): State<AppState>, headers: HeaderMap, Query(params): Query<TaxRateFilterParams>,
) -> Result<Json<TaxRateListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::TaxRateFilter {
        jurisdiction_id: params.jurisdiction.and_then(|j| j.parse().ok()),
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let rates = c.tax().list_rates(filter)?;
    let total = rates.len();
    Ok(Json(TaxRateListResponse {
        rates: rates.into_iter().map(|r| TaxRateResponse {
            id: r.id.to_string(), jurisdiction_id: r.jurisdiction_id.to_string(),
            name: r.name, rate: r.rate, tax_type: r.tax_type.to_string(),
            is_active: r.is_active,
        }).collect(), total,
    }))
}

#[utoipa::path(get, path = "/api/v1/tax/jurisdictions", tag = "tax",
    responses((status = 200, description = "List of tax jurisdictions")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_jurisdictions(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<serde_json::Value>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::TaxJurisdictionFilter { limit: Some(50), offset: Some(0), ..Default::default() };
    let jurisdictions = c.tax().list_jurisdictions(filter)?;
    Ok(Json(serde_json::json!({ "jurisdictions": jurisdictions, "total": jurisdictions.len() })))
}
