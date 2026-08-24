//! Analytics and vector-search repositories.

use super::*;

/// Analytics repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AnalyticsRepository: Send + Sync {
    // Sales analytics
    /// Get sales summary for a time period
    fn get_sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary>;

    /// Get revenue broken down by time periods
    fn get_revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>>;

    /// Get top selling products
    fn get_top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>>;

    /// Get product performance with period comparison
    fn get_product_performance(&self, query: AnalyticsQuery) -> Result<Vec<ProductPerformance>>;

    // Customer analytics
    /// Get customer metrics
    fn get_customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics>;

    /// Get top customers by spend
    fn get_top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>>;

    // Inventory analytics
    /// Get inventory health summary
    fn get_inventory_health(&self) -> Result<InventoryHealth>;

    /// Get low stock items
    fn get_low_stock_items(
        &self,
        threshold: Option<rust_decimal::Decimal>,
    ) -> Result<Vec<LowStockItem>>;

    /// Get inventory movement summary
    fn get_inventory_movement(&self, query: AnalyticsQuery) -> Result<Vec<InventoryMovement>>;

    // Order analytics
    /// Get order status breakdown
    fn get_order_status_breakdown(&self, query: AnalyticsQuery) -> Result<OrderStatusBreakdown>;

    /// Get fulfillment metrics
    fn get_fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics>;

    // Return analytics
    /// Get return metrics
    fn get_return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics>;

    // Forecasting
    /// Get demand forecast for SKUs
    fn get_demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>>;

    /// Get revenue forecast
    fn get_revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>>;

    // === Batch Operations ===

    /// Get multiple sales summaries for different queries
    fn get_sales_summary_batch(&self, queries: Vec<AnalyticsQuery>) -> Result<Vec<SalesSummary>>;
}

// ============================================================================
// Vector Search Repository
// ============================================================================

/// Vector search repository trait for semantic similarity search
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait VectorRepository: Send + Sync {
    /// Store embedding for an entity
    fn store_embedding(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        embedding: &[f32],
        text_hash: &str,
        model: &str,
    ) -> Result<()>;

    /// Search similar products by embedding vector
    fn search_products(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>>;

    /// Search similar customers by embedding vector
    fn search_customers(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Customer>>>;

    /// Search similar orders by embedding vector
    fn search_orders(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Order>>>;

    /// Search similar inventory items by embedding vector
    fn search_inventory(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<InventoryItem>>>;

    /// Delete embedding for an entity
    fn delete_embedding(&self, entity_type: EntityType, entity_id: &str) -> Result<()>;

    /// Check if entity has an embedding stored
    fn has_embedding(&self, entity_type: EntityType, entity_id: &str) -> Result<bool>;

    /// Get embedding metadata for an entity
    fn get_embedding_metadata(
        &self,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Result<Option<EmbeddingMetadata>>;

    /// Get embedding statistics
    fn get_stats(&self) -> Result<EmbeddingStats>;

    /// Delete all embeddings for an entity type
    fn clear_embeddings(&self, entity_type: EntityType) -> Result<u64>;
}
