//! Computed report endpoints (no persistence) — e.g. inventory aging.

use crate::error::{ErrorBody, HttpError};
use crate::state::AppState;
use axum::{Json, Router, http::StatusCode, routing::post};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::ProductId;
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AgingLayerRequest {
    pub product_id: Option<String>,
    pub sku: String,
    /// Remaining quantity as a string.
    pub quantity: String,
    /// Unit cost as a string.
    pub unit_cost: String,
    /// Received date in `YYYY-MM-DD` form.
    pub received_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct InventoryAgingRequest {
    /// As-of date (`YYYY-MM-DD`); defaults to today when omitted.
    pub as_of: Option<String>,
    pub layers: Vec<AgingLayerRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct AgingBucketResponse {
    pub label: String,
    pub min_days: i64,
    pub max_days: Option<i64>,
    pub quantity: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct InventoryAgingResponse {
    pub as_of: String,
    pub buckets: Vec<AgingBucketResponse>,
    pub total_quantity: String,
    pub total_value: String,
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_date(s: &str) -> Result<NaiveDate, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid date (expected YYYY-MM-DD): {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/reports/inventory-aging", post(inventory_aging))
        .route("/reports/sales-by-channel", post(sales_by_channel))
        .route("/reports/transaction-cogs", post(transaction_cogs))
        .route("/reports/close-the-books", post(close_the_books))
}

#[utoipa::path(post, path = "/api/v1/reports/inventory-aging", tag = "reports",
    request_body = InventoryAgingRequest,
    responses((status = 200, body = InventoryAgingResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(req))]
pub(crate) async fn inventory_aging(
    Json(req): Json<InventoryAgingRequest>,
) -> Result<(StatusCode, Json<InventoryAgingResponse>), HttpError> {
    let as_of = match req.as_of.as_deref() {
        Some(s) => parse_date(s)?,
        None => Utc::now().date_naive(),
    };
    let mut layers = Vec::with_capacity(req.layers.len());
    for l in req.layers {
        let product_id = match l.product_id.as_deref() {
            Some(s) => Some(parse_id::<ProductId>(s, "product_id")?),
            None => None,
        };
        layers.push(stateset_core::AgingCostLayer {
            product_id,
            sku: l.sku,
            quantity: parse_decimal(&l.quantity, "quantity")?,
            unit_cost: parse_decimal(&l.unit_cost, "unit_cost")?,
            received_at: parse_date(&l.received_at)?,
        });
    }
    let report = stateset_core::compute_inventory_aging(&layers, as_of);
    Ok((
        StatusCode::OK,
        Json(InventoryAgingResponse {
            as_of: report.as_of.to_string(),
            buckets: report
                .buckets
                .iter()
                .map(|b| AgingBucketResponse {
                    label: b.label.clone(),
                    min_days: b.min_days,
                    max_days: b.max_days,
                    quantity: b.quantity.to_string(),
                    value: b.value.to_string(),
                })
                .collect(),
            total_quantity: report.total_quantity.to_string(),
            total_value: report.total_value.to_string(),
        }),
    ))
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SalesRecordRequest {
    pub channel: String,
    /// Revenue as a string.
    pub revenue: String,
    /// Units as a string.
    pub units: String,
    /// Order date in `YYYY-MM-DD` form.
    pub order_date: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SalesByChannelRequest {
    /// Inclusive window start (`YYYY-MM-DD`).
    pub from: String,
    /// Inclusive window end (`YYYY-MM-DD`).
    pub to: String,
    pub records: Vec<SalesRecordRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ChannelSalesRowResponse {
    pub channel: String,
    pub order_count: u64,
    pub total_revenue: String,
    pub total_units: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct SalesByChannelResponse {
    pub from: String,
    pub to: String,
    pub rows: Vec<ChannelSalesRowResponse>,
    pub total_orders: u64,
    pub total_revenue: String,
    pub total_units: String,
}

#[utoipa::path(post, path = "/api/v1/reports/sales-by-channel", tag = "reports",
    request_body = SalesByChannelRequest,
    responses((status = 200, body = SalesByChannelResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(req))]
pub(crate) async fn sales_by_channel(
    Json(req): Json<SalesByChannelRequest>,
) -> Result<(StatusCode, Json<SalesByChannelResponse>), HttpError> {
    let from = parse_date(&req.from)?;
    let to = parse_date(&req.to)?;
    if from > to {
        return Err(HttpError::BadRequest("`from` must be on or before `to`".into()));
    }
    let mut records = Vec::with_capacity(req.records.len());
    for r in req.records {
        records.push(stateset_core::SalesRecord {
            channel: r.channel,
            revenue: parse_decimal(&r.revenue, "revenue")?,
            units: parse_decimal(&r.units, "units")?,
            order_date: parse_date(&r.order_date)?,
        });
    }
    let report = stateset_core::compute_sales_by_channel(&records, from, to);
    Ok((
        StatusCode::OK,
        Json(SalesByChannelResponse {
            from: report.from.to_string(),
            to: report.to.to_string(),
            rows: report
                .rows
                .iter()
                .map(|r| ChannelSalesRowResponse {
                    channel: r.channel.clone(),
                    order_count: r.order_count,
                    total_revenue: r.total_revenue.to_string(),
                    total_units: r.total_units.to_string(),
                })
                .collect(),
            total_orders: report.total_orders,
            total_revenue: report.total_revenue.to_string(),
            total_units: report.total_units.to_string(),
        }),
    ))
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CogsLineRequest {
    pub product_id: Option<String>,
    pub sku: String,
    pub quantity: String,
    pub revenue: String,
    pub cost: String,
    /// Transaction date (`YYYY-MM-DD`).
    pub transaction_date: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct TransactionCogsRequest {
    pub from: String,
    pub to: String,
    pub lines: Vec<CogsLineRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CogsRowResponse {
    pub sku: String,
    pub quantity: String,
    pub revenue: String,
    pub cogs: String,
    pub gross_margin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TransactionCogsResponse {
    pub from: String,
    pub to: String,
    pub rows: Vec<CogsRowResponse>,
    pub total_revenue: String,
    pub total_cogs: String,
    pub gross_margin: String,
    pub gross_margin_pct: String,
}

#[utoipa::path(post, path = "/api/v1/reports/transaction-cogs", tag = "reports",
    request_body = TransactionCogsRequest,
    responses((status = 200, body = TransactionCogsResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(req))]
pub(crate) async fn transaction_cogs(
    Json(req): Json<TransactionCogsRequest>,
) -> Result<(StatusCode, Json<TransactionCogsResponse>), HttpError> {
    let from = parse_date(&req.from)?;
    let to = parse_date(&req.to)?;
    if from > to {
        return Err(HttpError::BadRequest("`from` must be on or before `to`".into()));
    }
    let mut lines = Vec::with_capacity(req.lines.len());
    for l in req.lines {
        let product_id = match l.product_id.as_deref() {
            Some(s) => Some(parse_id::<ProductId>(s, "product_id")?),
            None => None,
        };
        lines.push(stateset_core::CogsLine {
            product_id,
            sku: l.sku,
            quantity: parse_decimal(&l.quantity, "quantity")?,
            revenue: parse_decimal(&l.revenue, "revenue")?,
            cost: parse_decimal(&l.cost, "cost")?,
            transaction_date: parse_date(&l.transaction_date)?,
        });
    }
    let report = stateset_core::compute_transaction_cogs(&lines, from, to);
    Ok((
        StatusCode::OK,
        Json(TransactionCogsResponse {
            from: report.from.to_string(),
            to: report.to.to_string(),
            rows: report
                .rows
                .iter()
                .map(|r| CogsRowResponse {
                    sku: r.sku.clone(),
                    quantity: r.quantity.to_string(),
                    revenue: r.revenue.to_string(),
                    cogs: r.cogs.to_string(),
                    gross_margin: r.gross_margin.to_string(),
                })
                .collect(),
            total_revenue: report.total_revenue.to_string(),
            total_cogs: report.total_cogs.to_string(),
            gross_margin: report.gross_margin.to_string(),
            gross_margin_pct: report.gross_margin_pct.to_string(),
        }),
    ))
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CloseBooksLineRequest {
    pub sku: String,
    pub cost: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CloseBooksInventoryRequest {
    pub sku: String,
    pub quantity: String,
    pub valuation: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CloseBooksRequest {
    #[serde(default)]
    pub lines: Vec<CloseBooksLineRequest>,
    #[serde(default)]
    pub inventory: Vec<CloseBooksInventoryRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ZeroValuationItemResponse {
    pub sku: String,
    pub quantity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CloseBooksResponse {
    pub zero_cost_skus: Vec<String>,
    pub zero_valuation_items: Vec<ZeroValuationItemResponse>,
    pub zero_cost_count: u64,
    pub zero_valuation_count: u64,
    pub is_ready: bool,
}

#[utoipa::path(post, path = "/api/v1/reports/close-the-books", tag = "reports",
    request_body = CloseBooksRequest,
    responses((status = 200, body = CloseBooksResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(req))]
pub(crate) async fn close_the_books(
    Json(req): Json<CloseBooksRequest>,
) -> Result<(StatusCode, Json<CloseBooksResponse>), HttpError> {
    let mut lines = Vec::with_capacity(req.lines.len());
    for l in req.lines {
        lines.push(stateset_core::CloseBooksLine {
            sku: l.sku,
            cost: parse_decimal(&l.cost, "cost")?,
        });
    }
    let mut inventory = Vec::with_capacity(req.inventory.len());
    for i in req.inventory {
        inventory.push(stateset_core::CloseBooksInventory {
            sku: i.sku,
            quantity: parse_decimal(&i.quantity, "quantity")?,
            valuation: parse_decimal(&i.valuation, "valuation")?,
        });
    }
    let report = stateset_core::compute_close_books_readiness(&lines, &inventory);
    Ok((
        StatusCode::OK,
        Json(CloseBooksResponse {
            zero_cost_skus: report.zero_cost_lines.iter().map(|l| l.sku.clone()).collect(),
            zero_valuation_items: report
                .zero_valuation_items
                .iter()
                .map(|i| ZeroValuationItemResponse {
                    sku: i.sku.clone(),
                    quantity: i.quantity.to_string(),
                })
                .collect(),
            zero_cost_count: report.zero_cost_count,
            zero_valuation_count: report.zero_valuation_count,
            is_ready: report.is_ready,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn inventory_aging_buckets() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "as_of": "2026-06-30",
            "layers": [
                {"sku":"fresh","quantity":"10","unit_cost":"2","received_at":"2026-06-20"},
                {"sku":"old","quantity":"2","unit_cost":"10","received_at":"2026-01-01"}
            ]
        });
        let resp = app
            .oneshot(
                Request::post("/reports/inventory-aging")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total_quantity"], "12");
        assert_eq!(json["buckets"][0]["quantity"], "10"); // 0-30
        assert_eq!(json["buckets"][3]["quantity"], "2"); // 90+
    }

    #[tokio::test]
    async fn sales_by_channel_groups() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "from": "2026-06-01",
            "to": "2026-06-30",
            "records": [
                {"channel":"shopify","revenue":"100","units":"2","order_date":"2026-06-05"},
                {"channel":"wholesale","revenue":"500","units":"20","order_date":"2026-06-15"}
            ]
        });
        let resp = app
            .oneshot(
                Request::post("/reports/sales-by-channel")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total_orders"], 2);
        assert_eq!(json["total_revenue"], "600");
        // highest revenue first
        assert_eq!(json["rows"][0]["channel"], "wholesale");
    }

    #[tokio::test]
    async fn transaction_cogs_margins() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "from": "2026-06-01",
            "to": "2026-06-30",
            "lines": [
                {"sku":"a","quantity":"2","revenue":"100","cost":"60","transaction_date":"2026-06-05"},
                {"sku":"b","quantity":"5","revenue":"500","cost":"100","transaction_date":"2026-06-15"}
            ]
        });
        let resp = app
            .oneshot(
                Request::post("/reports/transaction-cogs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total_revenue"], "600");
        assert_eq!(json["total_cogs"], "160");
        assert_eq!(json["gross_margin"], "440");
        // b has higher margin → first row
        assert_eq!(json["rows"][0]["sku"], "b");
    }

    #[tokio::test]
    async fn close_the_books_flags_issues() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "lines": [
                {"sku":"a","cost":"10"},
                {"sku":"b","cost":"0"}
            ],
            "inventory": [
                {"sku":"x","quantity":"10","valuation":"0"},
                {"sku":"y","quantity":"5","valuation":"100"}
            ]
        });
        let resp = app
            .oneshot(
                Request::post("/reports/close-the-books")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["zero_cost_count"], 1);
        assert_eq!(json["zero_valuation_count"], 1);
        assert_eq!(json["is_ready"], false);
        assert_eq!(json["zero_cost_skus"][0], "b");
        assert_eq!(json["zero_valuation_items"][0]["sku"], "x");
    }
}
