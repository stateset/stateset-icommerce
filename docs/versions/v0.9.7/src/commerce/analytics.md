# Analytics & Forecasting

Built-in analytics for revenue reporting, demand forecasting, inventory health, and customer lifetime value.

## Sales Summary

```javascript
const summary = commerce.analytics.salesSummary();
console.log(`Total revenue: ${summary.totalRevenue}`);
console.log(`Order count: ${summary.orderCount}`);
console.log(`Average order value: ${summary.averageOrderValue}`);
```

```bash
stateset "what is my revenue this month?"
stateset "compare revenue this month vs last month"
```

## Top Products

```javascript
const topProducts = commerce.analytics.topProducts(10);
// → [{ sku: 'WIDGET-001', name: 'Premium Widget', revenue: 29990.00, unitsSold: 1000 }, ...]
```

## Top Customers

```javascript
const topCustomers = commerce.analytics.topCustomers(10);
// → [{ id: '...', email: 'alice@example.com', totalSpend: 5000.00, orderCount: 15 }, ...]
```

## Revenue Forecasting

```bash
stateset "forecast revenue for next quarter"
stateset "predict demand for WIDGET-001 over the next 30 days"
```

## Inventory Health

```bash
stateset "which products are overstocked?"
stateset "what SKUs will run out within 2 weeks at current sales velocity?"
```

## Revenue Milestones

The heartbeat monitor tracks revenue milestones:

```json
{
    "id": "revenue-milestone",
    "checker": "revenue-milestone",
    "intervalMs": 3600000,
    "enabled": true,
    "config": { "target": 100000, "period": "month" }
}
```

## Cohort Analysis

```bash
stateset "show customer retention by monthly cohort"
stateset "what is the average lifetime value for customers acquired in January?"
```

## Performance Characteristics

| Query | Typical Latency (SQLite) | Typical Latency (PostgreSQL) |
|-------|-------------------------|------------------------------|
| Sales summary | 2ms | 10ms |
| Top products (10) | 3ms | 12ms |
| Top customers (10) | 3ms | 12ms |
| Revenue forecast | 5ms | 20ms |
| Cohort analysis | 8ms | 30ms |

For large datasets (100k+ orders), use pagination and date filtering to keep queries fast.

## CLI Examples

```bash
stateset "what is my revenue this month?"
stateset "compare revenue this month vs last month"
stateset "which products are overstocked?"
stateset "what SKUs will run out within 2 weeks?"
stateset "show customer retention by monthly cohort"
stateset "what is the average lifetime value?"
stateset "forecast revenue for next quarter"
```

## Demand Forecasting

Predict future demand for specific products based on historical sales data:

```javascript
const forecast = await toolkit.executeTool('demand_forecast', {
    sku: 'WIDGET-001',
    horizon: 30,  // days
});
// → {
//     sku: 'WIDGET-001',
//     currentDailyVelocity: 12.5,
//     forecastedDemand: 375,      // total units over 30 days
//     confidence: 0.85,
//     seasonalityFactor: 1.15,    // holiday uplift
//     trendDirection: 'increasing',
//     recommendedReorderPoint: 150,
//     recommendedReorderQuantity: 500,
// }
```

The forecasting engine considers:
- **Trend**: Moving average over 30/60/90 day windows
- **Seasonality**: Day-of-week and month-of-year patterns
- **Velocity**: Units sold per day (trailing average)

## Inventory Health

Comprehensive stock health dashboard:

```javascript
const health = await toolkit.executeTool('inventory_health', {});
// → {
//     totalSkus: 850,
//     healthy: 720,
//     understocked: 45,
//     overstocked: 30,
//     outOfStock: 15,
//     deadStock: 40,           // no sales in 90+ days
//     alerts: [
//         { sku: 'WIDGET-001', issue: 'below_reorder_point', daysUntilStockout: 5 },
//         { sku: 'GADGET-OLD', issue: 'dead_stock', daysSinceLastSale: 120 },
//     ],
// }
```

## Customer Lifetime Value

```javascript
const ltv = await toolkit.executeTool('customer_ltv', {
    customerId: 'cust-123',
});
// → {
//     historicalLtv: 2450.00,
//     predictedLtv: 4800.00,     // projected over 12 months
//     avgOrderValue: 122.50,
//     purchaseFrequency: 1.8,    // orders per month
//     customerAge: 14,           // months since first order
//     churnProbability: 0.12,
// }
```

## Conversion Funnel

Track the cart-to-order conversion pipeline:

```javascript
const funnel = await toolkit.executeTool('conversion_funnel', {
    period: 'month',
});
// → {
//     cartsCreated: 1200,
//     cartsWithItems: 950,
//     checkoutsStarted: 600,
//     ordersCompleted: 480,
//     conversionRate: 0.40,       // 480/1200
//     abandonmentRate: 0.60,
//     avgTimeToConvert: '2h 15m',
// }
```

## Anomaly Detection

Revenue and volume anomalies are flagged automatically via the heartbeat monitor. Configure thresholds in heartbeat checks:

```json
{
    "id": "revenue-anomaly",
    "checker": "revenue-change",
    "intervalMs": 3600000,
    "config": {
        "threshold": 0.30,
        "period": "day",
        "direction": "drop"
    }
}
```

A 30%+ revenue drop compared to the same day last week triggers an alert.

## MCP Tools

| Tool | Description |
|------|-------------|
| `sales_summary` | Revenue and order summary for a period |
| `top_products` | Best-selling products by revenue or units |
| `top_customers` | Highest-value customers by spend or order count |
| `revenue_forecast` | Revenue projection based on historical trends |
| `demand_forecast` | Demand prediction by SKU with seasonality |
| `inventory_health` | Stock health: overstock, understock, dead stock, velocity |
| `customer_ltv` | Lifetime value (historical + predicted) per customer |
| `cohort_analysis` | Retention curves by acquisition month |
| `conversion_funnel` | Cart-to-order funnel with abandonment rates |
| `currency_analytics` | Revenue breakdown by currency |
