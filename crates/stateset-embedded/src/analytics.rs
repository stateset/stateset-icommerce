//! Analytics and forecasting operations

use stateset_core::{
    AnalyticsQuery, CustomerMetrics, DemandForecast, FulfillmentMetrics, InventoryHealth,
    InventoryMovement, LowStockItem, OrderStatusBreakdown, ProductPerformance, Result,
    ReturnMetrics, RevenueByPeriod, RevenueForecast, SalesSummary, TimeGranularity,
    TopCustomer, TopProduct,
};
use stateset_db::Database;
use std::sync::Arc;
use rust_decimal::Decimal;

/// Analytics operations interface.
///
/// Provides sales analytics, inventory forecasting, and business intelligence.
///
/// # Example
///
/// ```rust,no_run
/// use stateset_embedded::{Commerce, AnalyticsQuery, TimePeriod};
///
/// let commerce = Commerce::new("./store.db")?;
///
/// // Get sales summary for last 30 days
/// let summary = commerce.analytics().sales_summary(
///     AnalyticsQuery::new().period(TimePeriod::Last30Days)
/// )?;
/// println!("Revenue: ${}", summary.total_revenue);
///
/// // Get top selling products
/// let top_products = commerce.analytics().top_products(
///     AnalyticsQuery::new().period(TimePeriod::ThisMonth).limit(10)
/// )?;
///
/// // Get demand forecast
/// let forecasts = commerce.analytics().demand_forecast(None, 30)?;
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct Analytics {
    db: Arc<dyn Database>,
}

impl Analytics {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Sales Analytics
    // ========================================================================

    /// Get sales summary for a time period.
    ///
    /// Returns total revenue, order count, average order value, and more.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let summary = commerce.analytics().sales_summary(
    ///     AnalyticsQuery::new().period(TimePeriod::Last30Days)
    /// )?;
    /// println!("Revenue: ${}", summary.total_revenue);
    /// println!("Orders: {}", summary.order_count);
    /// println!("AOV: ${}", summary.average_order_value);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary> {
        self.db.analytics().get_sales_summary(query)
    }

    /// Get revenue broken down by time periods.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let revenue = commerce.analytics().revenue_by_period(
    ///     AnalyticsQuery::new()
    ///         .period(TimePeriod::Last30Days)
    ///         .granularity(TimeGranularity::Day)
    /// )?;
    /// for day in revenue {
    ///     println!("{}: ${}", day.period, day.revenue);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>> {
        self.db.analytics().get_revenue_by_period(query)
    }

    // ========================================================================
    // Product Analytics
    // ========================================================================

    /// Get top selling products.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let top = commerce.analytics().top_products(
    ///     AnalyticsQuery::new()
    ///         .period(TimePeriod::ThisMonth)
    ///         .limit(10)
    /// )?;
    /// for product in top {
    ///     println!("{}: {} units, ${}", product.name, product.units_sold, product.revenue);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        self.db.analytics().get_top_products(query)
    }

    /// Get product performance with period comparison.
    pub fn product_performance(&self, query: AnalyticsQuery) -> Result<Vec<ProductPerformance>> {
        self.db.analytics().get_product_performance(query)
    }

    // ========================================================================
    // Customer Analytics
    // ========================================================================

    /// Get customer metrics.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let metrics = commerce.analytics().customer_metrics(
    ///     AnalyticsQuery::new().period(TimePeriod::ThisMonth)
    /// )?;
    /// println!("Total customers: {}", metrics.total_customers);
    /// println!("New this month: {}", metrics.new_customers);
    /// println!("Avg LTV: ${}", metrics.average_lifetime_value);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics> {
        self.db.analytics().get_customer_metrics(query)
    }

    /// Get top customers by spend.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let top = commerce.analytics().top_customers(
    ///     AnalyticsQuery::new().period(TimePeriod::AllTime).limit(10)
    /// )?;
    /// for customer in top {
    ///     println!("{}: {} orders, ${}", customer.name, customer.order_count, customer.total_spent);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>> {
        self.db.analytics().get_top_customers(query)
    }

    // ========================================================================
    // Inventory Analytics
    // ========================================================================

    /// Get inventory health summary.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let health = commerce.analytics().inventory_health()?;
    /// println!("Total SKUs: {}", health.total_skus);
    /// println!("Low stock: {}", health.low_stock_skus);
    /// println!("Out of stock: {}", health.out_of_stock_skus);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn inventory_health(&self) -> Result<InventoryHealth> {
        self.db.analytics().get_inventory_health()
    }

    /// Get low stock items.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use rust_decimal_macros::dec;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let low_stock = commerce.analytics().low_stock_items(Some(dec!(20)))?;
    /// for item in low_stock {
    ///     println!("{}: {} available", item.sku, item.available);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn low_stock_items(&self, threshold: Option<Decimal>) -> Result<Vec<LowStockItem>> {
        self.db.analytics().get_low_stock_items(threshold)
    }

    /// Get inventory movement summary.
    pub fn inventory_movement(&self, query: AnalyticsQuery) -> Result<Vec<InventoryMovement>> {
        self.db.analytics().get_inventory_movement(query)
    }

    // ========================================================================
    // Order Analytics
    // ========================================================================

    /// Get order status breakdown.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let breakdown = commerce.analytics().order_status_breakdown(
    ///     AnalyticsQuery::new().period(TimePeriod::Last30Days)
    /// )?;
    /// println!("Pending: {}", breakdown.pending);
    /// println!("Shipped: {}", breakdown.shipped);
    /// println!("Delivered: {}", breakdown.delivered);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn order_status_breakdown(&self, query: AnalyticsQuery) -> Result<OrderStatusBreakdown> {
        self.db.analytics().get_order_status_breakdown(query)
    }

    /// Get fulfillment metrics.
    pub fn fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics> {
        self.db.analytics().get_fulfillment_metrics(query)
    }

    // ========================================================================
    // Return Analytics
    // ========================================================================

    /// Get return metrics.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let metrics = commerce.analytics().return_metrics(
    ///     AnalyticsQuery::new().period(TimePeriod::ThisMonth)
    /// )?;
    /// println!("Returns: {}", metrics.total_returns);
    /// println!("Return rate: {}%", metrics.return_rate_percent);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics> {
        self.db.analytics().get_return_metrics(query)
    }

    // ========================================================================
    // Forecasting
    // ========================================================================

    /// Get demand forecast for inventory items.
    ///
    /// Predicts future demand based on historical sales data.
    ///
    /// # Arguments
    ///
    /// * `skus` - Optional list of SKUs to forecast. If None, forecasts all items.
    /// * `days_ahead` - Number of days to forecast ahead.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // Forecast for all items
    /// let forecasts = commerce.analytics().demand_forecast(None, 30)?;
    /// for f in forecasts {
    ///     println!("{}: {} units/day, {} days until stockout",
    ///         f.sku,
    ///         f.average_daily_demand,
    ///         f.days_until_stockout.unwrap_or(999)
    ///     );
    /// }
    ///
    /// // Forecast for specific SKUs
    /// let forecasts = commerce.analytics().demand_forecast(
    ///     Some(vec!["SKU-001".to_string(), "SKU-002".to_string()]),
    ///     14
    /// )?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        self.db.analytics().get_demand_forecast(skus, days_ahead)
    }

    /// Get revenue forecast.
    ///
    /// Predicts future revenue based on historical trends.
    ///
    /// # Arguments
    ///
    /// * `periods_ahead` - Number of periods to forecast.
    /// * `granularity` - Time granularity (Day, Week, Month).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let forecasts = commerce.analytics().revenue_forecast(3, TimeGranularity::Month)?;
    /// for f in forecasts {
    ///     println!("{}: ${} (${} - ${})",
    ///         f.period,
    ///         f.forecasted_revenue,
    ///         f.lower_bound,
    ///         f.upper_bound
    ///     );
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
        self.db.analytics().get_revenue_forecast(periods_ahead, granularity)
    }
}
