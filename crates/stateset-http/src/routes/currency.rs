//! Currency and exchange rate endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetExchangeRateRequest {
    pub base_currency: String,
    pub quote_currency: String,
    #[schema(value_type = String)]
    pub rate: Decimal,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ConvertCurrencyRequest {
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub base_currency: String,
    pub quote_currency: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct RateFilterParams {
    pub base_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ExchangeRateResponse {
    pub base_currency: String,
    pub quote_currency: String,
    #[schema(value_type = String)]
    pub rate: Decimal,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ExchangeRateListResponse {
    pub rates: Vec<ExchangeRateResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ConversionResponse {
    #[schema(value_type = String)]
    pub original_amount: Decimal,
    pub base_currency: String,
    #[schema(value_type = String)]
    pub converted_amount: Decimal,
    pub quote_currency: String,
    #[schema(value_type = String)]
    pub rate: Decimal,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/currencies/rates", get(list_rates).post(set_rate))
        .route("/currencies/convert", post(convert_currency))
}

#[utoipa::path(get, path = "/api/v1/currencies/rates", tag = "currency",
    params(RateFilterParams),
    responses((status = 200, body = ExchangeRateListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_rates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<RateFilterParams>,
) -> Result<Json<ExchangeRateListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let filter = stateset_core::ExchangeRateFilter {
        base_currency: params.base_currency.and_then(|s| s.parse().ok()),
        ..Default::default()
    };
    let rates = c.currency().list_rates(filter)?;
    let total = rates.len();
    Ok(Json(ExchangeRateListResponse {
        rates: rates
            .into_iter()
            .map(|r| ExchangeRateResponse {
                base_currency: r.base_currency.code().to_string(),
                quote_currency: r.quote_currency.code().to_string(),
                rate: r.rate,
                updated_at: r.updated_at.to_rfc3339(),
            })
            .collect(),
        total,
    }))
}

#[utoipa::path(post, path = "/api/v1/currencies/rates", tag = "currency",
    request_body = SetExchangeRateRequest,
    responses((status = 201, body = ExchangeRateResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_rate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetExchangeRateRequest>,
) -> Result<(StatusCode, Json<ExchangeRateResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let from =
        stateset_core::Currency::from_str(&req.base_currency).map_err(HttpError::BadRequest)?;
    let to =
        stateset_core::Currency::from_str(&req.quote_currency).map_err(HttpError::BadRequest)?;
    let input = stateset_core::SetExchangeRate {
        base_currency: from,
        quote_currency: to,
        rate: req.rate,
        source: None,
    };
    let r = c.currency().set_rate(input)?;
    Ok((
        StatusCode::CREATED,
        Json(ExchangeRateResponse {
            base_currency: r.base_currency.code().to_string(),
            quote_currency: r.quote_currency.code().to_string(),
            rate: r.rate,
            updated_at: r.updated_at.to_rfc3339(),
        }),
    ))
}

#[utoipa::path(post, path = "/api/v1/currencies/convert", tag = "currency",
    request_body = ConvertCurrencyRequest,
    responses((status = 200, body = ConversionResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn convert_currency(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ConvertCurrencyRequest>,
) -> Result<Json<ConversionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let from =
        stateset_core::Currency::from_str(&req.base_currency).map_err(HttpError::BadRequest)?;
    let to =
        stateset_core::Currency::from_str(&req.quote_currency).map_err(HttpError::BadRequest)?;
    let input = stateset_core::ConvertCurrency { amount: req.amount, from, to };
    let result = c.currency().convert(input)?;
    Ok(Json(ConversionResponse {
        original_amount: result.original_amount,
        base_currency: result.original_currency.code().to_string(),
        converted_amount: result.converted_amount,
        quote_currency: result.target_currency.code().to_string(),
        rate: result.rate,
    }))
}
