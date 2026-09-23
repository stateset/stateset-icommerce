//! Analytics API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Analytics API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalyticsQueryInput {
    /// Reporting window. Defaults to the last 30 days.
    #[napi(ts_type = "AnalyticsPeriod")]
    pub period: Option<String>,
    /// Bucket size for period breakdowns. Defaults to `day`.
    #[napi(ts_type = "AnalyticsGranularity")]
    pub granularity: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct SalesSummaryOutput {
    /// @deprecated Use the `totalRevenueExact` twin; float money will be removed in 2.0.
    pub total_revenue: f64,
    /// Exact base-10 total revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_revenue_exact: String,
    pub order_count: u32,
    /// @deprecated Use the `averageOrderValueExact` twin; float money will be removed in 2.0.
    pub average_order_value: f64,
    /// Exact base-10 average order value, straight from the engine's `Decimal`. Prefer this field for money.
    pub average_order_value_exact: String,
    pub items_sold: u32,
    pub unique_customers: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct RevenueByPeriodOutput {
    pub period: String,
    /// @deprecated Use the `revenueExact` twin; float money will be removed in 2.0.
    pub revenue: f64,
    /// Exact base-10 revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub revenue_exact: String,
    pub order_count: u32,
    pub period_start: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct TopProductOutput {
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub units_sold: u32,
    /// @deprecated Use the `revenueExact` twin; float money will be removed in 2.0.
    pub revenue: f64,
    /// Exact base-10 revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub revenue_exact: String,
    pub order_count: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ProductPerformanceOutput {
    pub product_id: String,
    pub sku: String,
    pub name: String,
    pub units_sold: u32,
    /// @deprecated Use the `revenueExact` twin; float money will be removed in 2.0.
    pub revenue: f64,
    /// Exact base-10 revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub revenue_exact: String,
    pub previous_units_sold: u32,
    /// @deprecated Use the `previousRevenueExact` twin; float money will be removed in 2.0.
    pub previous_revenue: f64,
    /// Exact base-10 previous revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub previous_revenue_exact: String,
    pub units_growth_percent: f64,
    pub revenue_growth_percent: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct CustomerMetricsOutput {
    pub total_customers: u32,
    pub new_customers: u32,
    pub returning_customers: u32,
    /// @deprecated Use the `averageLifetimeValueExact` twin; float money will be removed in 2.0.
    pub average_lifetime_value: f64,
    /// Exact base-10 average lifetime value, straight from the engine's `Decimal`. Prefer this field for money.
    pub average_lifetime_value_exact: String,
    pub average_orders_per_customer: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct TopCustomerOutput {
    pub customer_id: String,
    pub name: String,
    pub email: String,
    pub order_count: u32,
    /// @deprecated Use the `totalSpentExact` twin; float money will be removed in 2.0.
    pub total_spent: f64,
    /// Exact base-10 total spent, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_spent_exact: String,
    /// @deprecated Use the `averageOrderValueExact` twin; float money will be removed in 2.0.
    pub average_order_value: f64,
    /// Exact base-10 average order value, straight from the engine's `Decimal`. Prefer this field for money.
    pub average_order_value_exact: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct InventoryHealthOutput {
    pub total_skus: u32,
    pub in_stock_skus: u32,
    pub low_stock_skus: u32,
    pub out_of_stock_skus: u32,
    /// @deprecated Use the `totalValueExact` twin; float money will be removed in 2.0.
    pub total_value: f64,
    /// Exact base-10 total value, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_value_exact: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct LowStockItemOutput {
    pub sku: String,
    pub name: String,
    pub on_hand: f64,
    pub allocated: f64,
    pub available: f64,
    pub reorder_point: Option<f64>,
    pub average_daily_sales: Option<f64>,
    pub days_of_stock: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct InventoryMovementOutput {
    pub sku: String,
    pub name: String,
    pub units_sold: u32,
    pub units_received: u32,
    pub units_returned: u32,
    pub units_adjusted: i32,
    pub net_change: i32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct DemandForecastOutput {
    pub sku: String,
    pub name: String,
    pub average_daily_demand: f64,
    pub forecasted_demand: f64,
    pub confidence: f64,
    pub current_stock: f64,
    pub days_until_stockout: Option<i32>,
    pub recommended_reorder_qty: Option<f64>,
    #[napi(ts_type = "DemandTrend")]
    pub trend: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct RevenueForecastOutput {
    pub period: String,
    /// @deprecated Use the `forecastedRevenueExact` twin; float money will be removed in 2.0.
    pub forecasted_revenue: f64,
    /// Exact base-10 forecasted revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub forecasted_revenue_exact: String,
    /// @deprecated Use the `lowerBoundExact` twin; float money will be removed in 2.0.
    pub lower_bound: f64,
    /// Exact base-10 lower bound, straight from the engine's `Decimal`. Prefer this field for money.
    pub lower_bound_exact: String,
    /// @deprecated Use the `upperBoundExact` twin; float money will be removed in 2.0.
    pub upper_bound: f64,
    /// Exact base-10 upper bound, straight from the engine's `Decimal`. Prefer this field for money.
    pub upper_bound_exact: String,
    pub confidence_level: f64,
    pub based_on_periods: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct OrderStatusBreakdownOutput {
    pub pending: u32,
    pub confirmed: u32,
    pub processing: u32,
    pub shipped: u32,
    pub delivered: u32,
    pub cancelled: u32,
    pub refunded: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct FulfillmentMetricsOutput {
    pub avg_time_to_ship_hours: Option<f64>,
    pub avg_time_to_deliver_hours: Option<f64>,
    pub on_time_shipping_percent: Option<f64>,
    pub on_time_delivery_percent: Option<f64>,
    pub shipped_today: u32,
    pub awaiting_shipment: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ReturnMetricsOutput {
    pub total_returns: u32,
    pub return_rate_percent: f64,
    /// @deprecated Use the `totalRefundedExact` twin; float money will be removed in 2.0.
    pub total_refunded: f64,
    /// Exact base-10 total refunded, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_refunded_exact: String,
}

/// Analytics and forecasting API
#[napi]
pub struct Analytics {
    pub(crate) commerce: Handle,
}

/// Build the engine query from the JavaScript input, validating every field
/// even where the calling report ignores it: a misspelt `granularity` on a
/// summary is still a mistake the caller should hear about, not a silent
/// no-op.
pub(crate) fn analytics_query(
    query: Option<&AnalyticsQueryInput>,
) -> Result<stateset_embedded::AnalyticsQuery> {
    let mut q = stateset_embedded::AnalyticsQuery::new();
    if let Some(input) = query {
        if let Some(period) = input.period.as_deref() {
            q = q.period(parse_period(period)?);
        }
        if let Some(granularity) = input.granularity.as_deref() {
            q = q.granularity(parse_granularity(granularity)?);
        }
        if let Some(limit) = input.limit {
            q = q.limit(limit);
        }
    }
    Ok(q)
}

pub(crate) fn parse_period(s: &str) -> Result<stateset_embedded::TimePeriod> {
    Ok(match s.to_lowercase().as_str() {
        "today" => stateset_embedded::TimePeriod::Today,
        "yesterday" => stateset_embedded::TimePeriod::Yesterday,
        "last7days" | "last_7_days" => stateset_embedded::TimePeriod::Last7Days,
        "last30days" | "last_30_days" => stateset_embedded::TimePeriod::Last30Days,
        "this_month" | "thismonth" => stateset_embedded::TimePeriod::ThisMonth,
        "last_month" | "lastmonth" => stateset_embedded::TimePeriod::LastMonth,
        "this_quarter" | "thisquarter" => stateset_embedded::TimePeriod::ThisQuarter,
        "last_quarter" | "lastquarter" => stateset_embedded::TimePeriod::LastQuarter,
        "this_year" | "thisyear" => stateset_embedded::TimePeriod::ThisYear,
        "last_year" | "lastyear" => stateset_embedded::TimePeriod::LastYear,
        "all_time" | "alltime" | "all" => stateset_embedded::TimePeriod::AllTime,
        _ => {
            return Err(unknown_variant(
                "analytics period",
                s,
                &[
                    "today",
                    "yesterday",
                    "last_7_days",
                    "last_30_days",
                    "this_month",
                    "last_month",
                    "this_quarter",
                    "last_quarter",
                    "this_year",
                    "last_year",
                    "all_time",
                ],
            ));
        }
    })
}

pub(crate) fn parse_granularity(s: &str) -> Result<stateset_embedded::TimeGranularity> {
    Ok(match s.to_lowercase().as_str() {
        "hour" | "hourly" => stateset_embedded::TimeGranularity::Hour,
        "day" | "daily" => stateset_embedded::TimeGranularity::Day,
        "week" | "weekly" => stateset_embedded::TimeGranularity::Week,
        "month" | "monthly" => stateset_embedded::TimeGranularity::Month,
        "quarter" | "quarterly" => stateset_embedded::TimeGranularity::Quarter,
        "year" | "yearly" => stateset_embedded::TimeGranularity::Year,
        _ => {
            return Err(unknown_variant(
                "analytics granularity",
                s,
                &["hour", "day", "week", "month", "quarter", "year"],
            ));
        }
    })
}

#[napi]
impl Analytics {
    /// Get sales summary for a time period
    #[napi]
    pub async fn sales_summary(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<SalesSummaryOutput> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let summary = commerce
            .analytics()
            .sales_summary(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get sales summary", e))?;

        let (total_revenue, total_revenue_exact) =
            money_pair(summary.total_revenue, "sales total revenue")?;
        let (average_order_value, average_order_value_exact) =
            money_pair(summary.average_order_value, "sales average order value")?;
        Ok(SalesSummaryOutput {
            total_revenue,
            total_revenue_exact,
            order_count: summary.order_count as u32,
            average_order_value,
            average_order_value_exact,
            items_sold: summary.items_sold as u32,
            unique_customers: summary.unique_customers as u32,
        })
    }

    /// Get revenue broken down by time periods
    #[napi]
    pub async fn revenue_by_period(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<RevenueByPeriodOutput>> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let revenue = commerce
            .analytics()
            .revenue_by_period(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get revenue", e))?;

        revenue
            .into_iter()
            .map(|r| {
                let (revenue, revenue_exact) = money_pair(r.revenue, "period revenue")?;
                Ok(RevenueByPeriodOutput {
                    period: r.period,
                    revenue,
                    revenue_exact,
                    order_count: r.order_count as u32,
                    period_start: r.period_start.to_rfc3339(),
                })
            })
            .collect()
    }

    /// Get top selling products
    #[napi]
    pub async fn top_products(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<TopProductOutput>> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let products = commerce
            .analytics()
            .top_products(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get top products", e))?;

        products
            .into_iter()
            .map(|p| {
                let (revenue, revenue_exact) = money_pair(p.revenue, "top product revenue")?;
                Ok(TopProductOutput {
                    product_id: p.product_id.map(|id| id.to_string()),
                    sku: p.sku,
                    name: p.name,
                    units_sold: p.units_sold as u32,
                    revenue,
                    revenue_exact,
                    order_count: p.order_count as u32,
                })
            })
            .collect()
    }

    /// Get product performance with period comparison
    #[napi]
    pub async fn product_performance(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<ProductPerformanceOutput>> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let perf = commerce
            .analytics()
            .product_performance(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get product performance", e))?;

        perf.into_iter()
            .map(|p| {
                let (revenue, revenue_exact) = money_pair(p.revenue, "product revenue")?;
                let (previous_revenue, previous_revenue_exact) =
                    money_pair(p.previous_revenue, "product previous revenue")?;
                Ok(ProductPerformanceOutput {
                    product_id: p.product_id.to_string(),
                    sku: p.sku,
                    name: p.name,
                    units_sold: p.units_sold as u32,
                    revenue,
                    revenue_exact,
                    previous_units_sold: p.previous_units_sold as u32,
                    previous_revenue,
                    previous_revenue_exact,
                    units_growth_percent: to_f64_checked(
                        p.units_growth_percent,
                        "product units growth percent",
                    )?,
                    revenue_growth_percent: to_f64_checked(
                        p.revenue_growth_percent,
                        "product revenue growth percent",
                    )?,
                })
            })
            .collect()
    }

    /// Get customer metrics
    #[napi]
    pub async fn customer_metrics(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<CustomerMetricsOutput> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let metrics = commerce
            .analytics()
            .customer_metrics(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get customer metrics", e))?;

        let (average_lifetime_value, average_lifetime_value_exact) =
            money_pair(metrics.average_lifetime_value, "average customer lifetime value")?;
        Ok(CustomerMetricsOutput {
            total_customers: metrics.total_customers as u32,
            new_customers: metrics.new_customers as u32,
            returning_customers: metrics.returning_customers as u32,
            average_lifetime_value,
            average_lifetime_value_exact,
            average_orders_per_customer: to_f64_checked(
                metrics.average_orders_per_customer,
                "average orders per customer",
            )?,
        })
    }

    /// Get top customers by spend
    #[napi]
    pub async fn top_customers(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<TopCustomerOutput>> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let customers = commerce
            .analytics()
            .top_customers(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get top customers", e))?;

        customers
            .into_iter()
            .map(|c| {
                let (total_spent, total_spent_exact) =
                    money_pair(c.total_spent, "customer total spent")?;
                let (average_order_value, average_order_value_exact) =
                    money_pair(c.average_order_value, "customer average order value")?;
                Ok(TopCustomerOutput {
                    customer_id: c.customer_id.to_string(),
                    name: c.name,
                    email: c.email,
                    order_count: c.order_count as u32,
                    total_spent,
                    total_spent_exact,
                    average_order_value,
                    average_order_value_exact,
                })
            })
            .collect()
    }

    /// Get inventory health summary
    #[napi]
    pub async fn inventory_health(&self) -> Result<InventoryHealthOutput> {
        let commerce = self.commerce.get()?;

        let health = commerce
            .analytics()
            .inventory_health()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get inventory health", e))?;

        let (total_value, total_value_exact) =
            money_pair(health.total_value, "inventory total value")?;
        Ok(InventoryHealthOutput {
            total_skus: health.total_skus as u32,
            in_stock_skus: health.in_stock_skus as u32,
            low_stock_skus: health.low_stock_skus as u32,
            out_of_stock_skus: health.out_of_stock_skus as u32,
            total_value,
            total_value_exact,
        })
    }

    /// Get low stock items
    #[napi]
    pub async fn low_stock_items(&self, threshold: Option<f64>) -> Result<Vec<LowStockItemOutput>> {
        let commerce = self.commerce.get()?;

        let threshold_dec = optional_decimal_from_f64(threshold, "low stock threshold")?;

        let items = commerce
            .analytics()
            .low_stock_items(threshold_dec)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get low stock items", e))?;

        items
            .into_iter()
            .map(|i| {
                Ok(LowStockItemOutput {
                    sku: i.sku,
                    name: i.name,
                    on_hand: to_f64_checked(i.on_hand, "low stock on hand")?,
                    allocated: to_f64_checked(i.allocated, "low stock allocated")?,
                    available: to_f64_checked(i.available, "low stock available")?,
                    reorder_point: optional_to_f64_checked(
                        i.reorder_point,
                        "low stock reorder point",
                    )?,
                    average_daily_sales: optional_to_f64_checked(
                        i.average_daily_sales,
                        "low stock average daily sales",
                    )?,
                    days_of_stock: optional_to_f64_checked(
                        i.days_of_stock,
                        "low stock days of stock",
                    )?,
                })
            })
            .collect()
    }

    /// Get inventory movement summary
    #[napi]
    pub async fn inventory_movement(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<InventoryMovementOutput>> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let movements = commerce
            .analytics()
            .inventory_movement(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get inventory movement", e))?;

        Ok(movements
            .into_iter()
            .map(|m| InventoryMovementOutput {
                sku: m.sku,
                name: m.name,
                units_sold: m.units_sold as u32,
                units_received: m.units_received as u32,
                units_returned: m.units_returned as u32,
                units_adjusted: m.units_adjusted as i32,
                net_change: m.net_change as i32,
            })
            .collect())
    }

    /// Get demand forecast for inventory items
    #[napi]
    pub async fn demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: Option<u32>,
    ) -> Result<Vec<DemandForecastOutput>> {
        let commerce = self.commerce.get()?;

        let forecasts = commerce
            .analytics()
            .demand_forecast(skus, days_ahead.unwrap_or(30))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get demand forecast", e))?;

        forecasts
            .into_iter()
            .map(|f| {
                Ok(DemandForecastOutput {
                    sku: f.sku,
                    name: f.name,
                    average_daily_demand: to_f64_checked(
                        f.average_daily_demand,
                        "average daily demand",
                    )?,
                    forecasted_demand: to_f64_checked(f.forecasted_demand, "forecasted demand")?,
                    confidence: to_f64_checked(f.confidence, "demand forecast confidence")?,
                    current_stock: to_f64_checked(f.current_stock, "current stock")?,
                    days_until_stockout: f.days_until_stockout,
                    recommended_reorder_qty: optional_to_f64_checked(
                        f.recommended_reorder_qty,
                        "recommended reorder quantity",
                    )?,
                    trend: format!("{:?}", f.trend),
                })
            })
            .collect()
    }

    /// Get revenue forecast. `granularity` defaults to `month`.
    #[napi(
        ts_args_type = "periodsAhead?: number | undefined | null, granularity?: AnalyticsGranularity | undefined | null"
    )]
    pub async fn revenue_forecast(
        &self,
        periods_ahead: Option<u32>,
        granularity: Option<String>,
    ) -> Result<Vec<RevenueForecastOutput>> {
        let commerce = self.commerce.get()?;

        let gran = granularity
            .map(|g| parse_granularity(&g))
            .transpose()?
            .unwrap_or(stateset_embedded::TimeGranularity::Month);

        let forecasts = commerce
            .analytics()
            .revenue_forecast(periods_ahead.unwrap_or(3), gran)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get revenue forecast", e))?;

        forecasts
            .into_iter()
            .map(|f| {
                let (forecasted_revenue, forecasted_revenue_exact) =
                    money_pair(f.forecasted_revenue, "forecasted revenue")?;
                let (lower_bound, lower_bound_exact) =
                    money_pair(f.lower_bound, "revenue forecast lower bound")?;
                let (upper_bound, upper_bound_exact) =
                    money_pair(f.upper_bound, "revenue forecast upper bound")?;
                Ok(RevenueForecastOutput {
                    period: f.period,
                    forecasted_revenue,
                    forecasted_revenue_exact,
                    lower_bound,
                    lower_bound_exact,
                    upper_bound,
                    upper_bound_exact,
                    confidence_level: to_f64_checked(
                        f.confidence_level,
                        "revenue forecast confidence level",
                    )?,
                    based_on_periods: f.based_on_periods,
                })
            })
            .collect()
    }

    /// Get order status breakdown
    #[napi]
    pub async fn order_status_breakdown(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<OrderStatusBreakdownOutput> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let breakdown = commerce
            .analytics()
            .order_status_breakdown(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get order status breakdown", e))?;

        Ok(OrderStatusBreakdownOutput {
            pending: breakdown.pending as u32,
            confirmed: breakdown.confirmed as u32,
            processing: breakdown.processing as u32,
            shipped: breakdown.shipped as u32,
            delivered: breakdown.delivered as u32,
            cancelled: breakdown.cancelled as u32,
            refunded: breakdown.refunded as u32,
        })
    }

    /// Get fulfillment metrics
    #[napi]
    pub async fn fulfillment_metrics(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<FulfillmentMetricsOutput> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let metrics = commerce
            .analytics()
            .fulfillment_metrics(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get fulfillment metrics", e))?;

        Ok(FulfillmentMetricsOutput {
            avg_time_to_ship_hours: optional_to_f64_checked(
                metrics.avg_time_to_ship_hours,
                "average time to ship hours",
            )?,
            avg_time_to_deliver_hours: optional_to_f64_checked(
                metrics.avg_time_to_deliver_hours,
                "average time to deliver hours",
            )?,
            on_time_shipping_percent: optional_to_f64_checked(
                metrics.on_time_shipping_percent,
                "on-time shipping percent",
            )?,
            on_time_delivery_percent: optional_to_f64_checked(
                metrics.on_time_delivery_percent,
                "on-time delivery percent",
            )?,
            shipped_today: metrics.shipped_today as u32,
            awaiting_shipment: metrics.awaiting_shipment as u32,
        })
    }

    /// Get return metrics
    #[napi]
    pub async fn return_metrics(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<ReturnMetricsOutput> {
        let commerce = self.commerce.get()?;

        let q = analytics_query(query.as_ref())?;

        let metrics = commerce
            .analytics()
            .return_metrics(q)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get return metrics", e))?;

        let (total_refunded, total_refunded_exact) =
            money_pair(metrics.total_refunded, "total refunded")?;
        Ok(ReturnMetricsOutput {
            total_returns: metrics.total_returns as u32,
            return_rate_percent: to_f64_checked(
                metrics.return_rate_percent,
                "return rate percent",
            )?,
            total_refunded,
            total_refunded_exact,
        })
    }
}
