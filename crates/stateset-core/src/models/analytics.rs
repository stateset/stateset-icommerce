//! Analytics and reporting models
//!
//! Provides types for sales analytics, inventory forecasting, and business intelligence.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Time Period
// ============================================================================

/// Time period for analytics queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimePeriod {
    Today,
    Yesterday,
    Last7Days,
    Last30Days,
    ThisMonth,
    LastMonth,
    ThisQuarter,
    LastQuarter,
    ThisYear,
    LastYear,
    AllTime,
    Custom,
}

impl Default for TimePeriod {
    fn default() -> Self {
        Self::Last30Days
    }
}

/// Date range for custom period queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

// ============================================================================
// Sales Analytics
// ============================================================================

/// Sales summary metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SalesSummary {
    /// Total revenue in the period
    pub total_revenue: Decimal,
    /// Number of orders
    pub order_count: u64,
    /// Average order value
    pub average_order_value: Decimal,
    /// Number of items sold
    pub items_sold: u64,
    /// Number of unique customers
    pub unique_customers: u64,
    /// Revenue change vs previous period (percentage)
    pub revenue_change_percent: Option<Decimal>,
    /// Order count change vs previous period (percentage)
    pub order_count_change_percent: Option<Decimal>,
    /// Period start
    pub period_start: Option<DateTime<Utc>>,
    /// Period end
    pub period_end: Option<DateTime<Utc>>,
}

/// Revenue breakdown by time bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueByPeriod {
    /// Time bucket label (e.g., "2024-01-15", "Week 3", "January")
    pub period: String,
    /// Revenue for this period
    pub revenue: Decimal,
    /// Order count for this period
    pub order_count: u64,
    /// Start of period
    pub period_start: DateTime<Utc>,
}

/// Granularity for time-series data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeGranularity {
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl Default for TimeGranularity {
    fn default() -> Self {
        Self::Day
    }
}

// ============================================================================
// Product Analytics
// ============================================================================

/// Top selling product
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopProduct {
    /// Product ID
    pub product_id: Option<Uuid>,
    /// SKU
    pub sku: String,
    /// Product name
    pub name: String,
    /// Total units sold
    pub units_sold: u64,
    /// Total revenue from this product
    pub revenue: Decimal,
    /// Number of orders containing this product
    pub order_count: u64,
    /// Average selling price
    pub average_price: Decimal,
}

/// Product performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductPerformance {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    /// Units sold in current period
    pub units_sold: u64,
    /// Revenue in current period
    pub revenue: Decimal,
    /// Units sold in previous period
    pub previous_units_sold: u64,
    /// Revenue in previous period
    pub previous_revenue: Decimal,
    /// Growth rate (units)
    pub units_growth_percent: Decimal,
    /// Growth rate (revenue)
    pub revenue_growth_percent: Decimal,
}

// ============================================================================
// Customer Analytics
// ============================================================================

/// Customer segment metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerMetrics {
    /// Total customers
    pub total_customers: u64,
    /// New customers in period
    pub new_customers: u64,
    /// Returning customers (ordered more than once)
    pub returning_customers: u64,
    /// Average customer lifetime value
    pub average_lifetime_value: Decimal,
    /// Average orders per customer
    pub average_orders_per_customer: Decimal,
    /// Customer retention rate
    pub retention_rate_percent: Option<Decimal>,
}

/// Top customer by spend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopCustomer {
    pub customer_id: Uuid,
    pub email: String,
    pub name: String,
    /// Total spend
    pub total_spent: Decimal,
    /// Number of orders
    pub order_count: u64,
    /// Average order value
    pub average_order_value: Decimal,
    /// First order date
    pub first_order_date: Option<DateTime<Utc>>,
    /// Last order date
    pub last_order_date: Option<DateTime<Utc>>,
}

// ============================================================================
// Inventory Analytics
// ============================================================================

/// Inventory health summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryHealth {
    /// Total SKUs tracked
    pub total_skus: u64,
    /// SKUs currently in stock
    pub in_stock_skus: u64,
    /// SKUs at low stock level
    pub low_stock_skus: u64,
    /// SKUs out of stock
    pub out_of_stock_skus: u64,
    /// Total inventory value
    pub total_value: Decimal,
    /// Inventory turnover ratio
    pub turnover_ratio: Option<Decimal>,
}

/// Low stock alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowStockItem {
    pub sku: String,
    pub name: String,
    /// Current on-hand quantity
    pub on_hand: Decimal,
    /// Allocated (reserved) quantity
    pub allocated: Decimal,
    /// Available quantity
    pub available: Decimal,
    /// Reorder point threshold
    pub reorder_point: Option<Decimal>,
    /// Average daily sales
    pub average_daily_sales: Option<Decimal>,
    /// Days of stock remaining
    pub days_of_stock: Option<Decimal>,
}

/// Inventory movement summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryMovement {
    pub sku: String,
    pub name: String,
    /// Units sold
    pub units_sold: u64,
    /// Units received (from POs)
    pub units_received: u64,
    /// Units returned
    pub units_returned: u64,
    /// Units adjusted (manual)
    pub units_adjusted: i64,
    /// Net change
    pub net_change: i64,
}

// ============================================================================
// Forecasting
// ============================================================================

/// Demand forecast for a SKU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandForecast {
    pub sku: String,
    pub name: String,
    /// Historical average daily demand
    pub average_daily_demand: Decimal,
    /// Forecasted demand for next period
    pub forecasted_demand: Decimal,
    /// Confidence level (0-1)
    pub confidence: Decimal,
    /// Current stock
    pub current_stock: Decimal,
    /// Days until stockout (if no reorder)
    pub days_until_stockout: Option<i32>,
    /// Recommended reorder quantity
    pub recommended_reorder_qty: Option<Decimal>,
    /// Recommended reorder date
    pub recommended_reorder_date: Option<DateTime<Utc>>,
    /// Trend direction
    pub trend: Trend,
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trend {
    Rising,
    Stable,
    Falling,
}

impl Default for Trend {
    fn default() -> Self {
        Self::Stable
    }
}

/// Revenue forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueForecast {
    /// Period being forecasted
    pub period: String,
    /// Forecasted revenue
    pub forecasted_revenue: Decimal,
    /// Lower bound (confidence interval)
    pub lower_bound: Decimal,
    /// Upper bound (confidence interval)
    pub upper_bound: Decimal,
    /// Confidence level (e.g., 0.95 for 95%)
    pub confidence_level: Decimal,
    /// Based on historical data from
    pub based_on_periods: u32,
}

// ============================================================================
// Order Analytics
// ============================================================================

/// Order status breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderStatusBreakdown {
    pub pending: u64,
    pub confirmed: u64,
    pub processing: u64,
    pub shipped: u64,
    pub delivered: u64,
    pub cancelled: u64,
    pub refunded: u64,
    pub total: u64,
}

/// Order fulfillment metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentMetrics {
    /// Average time from order to shipped (in hours)
    pub avg_time_to_ship_hours: Option<Decimal>,
    /// Average time from shipped to delivered (in hours)
    pub avg_time_to_deliver_hours: Option<Decimal>,
    /// Percentage of orders shipped on time
    pub on_time_shipping_percent: Option<Decimal>,
    /// Percentage of orders delivered on time
    pub on_time_delivery_percent: Option<Decimal>,
    /// Orders shipped today
    pub shipped_today: u64,
    /// Orders awaiting shipment
    pub awaiting_shipment: u64,
}

// ============================================================================
// Return Analytics
// ============================================================================

/// Return metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReturnMetrics {
    /// Total returns in period
    pub total_returns: u64,
    /// Return rate (returns / orders)
    pub return_rate_percent: Decimal,
    /// Total refund amount
    pub total_refunded: Decimal,
    /// Returns by reason breakdown
    pub by_reason: Vec<ReturnReasonCount>,
    /// Most returned products
    pub top_returned_products: Vec<TopReturnedProduct>,
}

/// Return count by reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnReasonCount {
    pub reason: String,
    pub count: u64,
    pub percentage: Decimal,
}

/// Product with high return rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopReturnedProduct {
    pub sku: String,
    pub name: String,
    pub units_returned: u64,
    pub units_sold: u64,
    pub return_rate_percent: Decimal,
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Common analytics query parameters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    /// Time period preset
    pub period: Option<TimePeriod>,
    /// Custom date range (used when period is Custom)
    pub date_range: Option<DateRange>,
    /// Time granularity for time-series data
    pub granularity: Option<TimeGranularity>,
    /// Number of results to return (for top-N queries)
    pub limit: Option<u32>,
    /// Compare to previous period
    pub compare_previous: Option<bool>,
}

impl AnalyticsQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn period(mut self, period: TimePeriod) -> Self {
        self.period = Some(period);
        self
    }

    pub fn date_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.period = Some(TimePeriod::Custom);
        self.date_range = Some(DateRange {
            start: Some(start),
            end: Some(end),
        });
        self
    }

    pub fn granularity(mut self, granularity: TimeGranularity) -> Self {
        self.granularity = Some(granularity);
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn compare_previous(mut self, compare: bool) -> Self {
        self.compare_previous = Some(compare);
        self
    }
}
