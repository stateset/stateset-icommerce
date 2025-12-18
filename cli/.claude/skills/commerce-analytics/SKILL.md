---
name: commerce-analytics
description: Use when analyzing sales performance, customer metrics, inventory health, or generating forecasts.
---

# Commerce Analytics Skill

Domain knowledge for business intelligence, sales analytics, customer insights, and demand forecasting.

## Analytics Concepts

### Time Periods

| Period | Description | Use Case |
|--------|-------------|----------|
| `today` | Current calendar day | Real-time monitoring |
| `last7days` | Rolling 7 days | Weekly trends |
| `last30days` | Rolling 30 days | Monthly performance |
| `this_month` | Current calendar month | Month-to-date |
| `last_month` | Previous calendar month | Month comparison |
| `this_year` | Current calendar year | Year-to-date |
| `all_time` | All historical data | Lifetime value |

### Key Metrics

| Metric | Formula | Meaning |
|--------|---------|---------|
| AOV | Revenue / Orders | Average Order Value |
| LTV | Sum(Revenue per Customer) | Customer Lifetime Value |
| Return Rate | Returns / Orders * 100 | Percentage returned |
| Conversion | Orders / Visitors | Purchase rate |

## Sales Summary Structure

```javascript
{
  totalRevenue: 45230.00,      // Sum of all order totals
  orderCount: 156,             // Number of orders
  averageOrderValue: 290.00,   // Revenue / Orders
  itemsSold: 423,              // Sum of quantities
  uniqueCustomers: 89          // Distinct customers
}
```

### Interpreting Sales Data

- **Revenue up, Orders down** = Higher AOV (upselling working)
- **Revenue down, Orders up** = Lower AOV (discounting impact)
- **New customers high** = Marketing working
- **Returning customers high** = Retention strong

## Product Performance

### Top Products Output

```javascript
{
  sku: "WIDGET-001",
  name: "Premium Widget",
  unitsSold: 78,
  revenue: 15600.00,
  orderCount: 45
}
```

### Product Metrics to Watch

| Metric | High Value | Low Value |
|--------|------------|-----------|
| Units Sold | Popular item | Consider discontinuing |
| Revenue | High-value product | May need pricing review |
| Order Count | Frequently bought | Bundle opportunity |

### Product Performance Ratios

```
Revenue per Unit = Revenue / Units Sold
Units per Order = Units Sold / Order Count
```

## Customer Analytics

### Customer Metrics Output

```javascript
{
  totalCustomers: 1250,           // All customers
  newCustomers: 45,               // New in period
  returningCustomers: 234,        // Repeat buyers
  averageLifetimeValue: 450.00,   // Average total spend
  averageOrdersPerCustomer: 2.3   // Order frequency
}
```

### Customer Segmentation

| Segment | Definition | Action |
|---------|------------|--------|
| New | First purchase | Welcome, onboarding |
| Active | Purchased in 90 days | Maintain engagement |
| At Risk | No purchase 90-180 days | Re-engagement campaign |
| Lapsed | No purchase 180+ days | Win-back campaign |
| VIP | Top 10% by LTV | Premium treatment |

### Top Customers Output

```javascript
{
  customerId: "uuid",
  name: "John Doe",
  email: "john@example.com",
  orderCount: 15,
  totalSpent: 2340.00,
  averageOrderValue: 156.00
}
```

## Inventory Health

### Health Overview Output

```javascript
{
  totalSkus: 150,           // All products
  inStockSkus: 120,         // Available > 0
  lowStockSkus: 18,         // Below reorder point
  outOfStockSkus: 12,       // Available = 0
  totalValue: 125000.00     // Sum of inventory value
}
```

### Stock Status Levels

| Level | Condition | Priority |
|-------|-----------|----------|
| In Stock | Available > reorder point | Normal |
| Low Stock | Available <= reorder point | Monitor |
| Critical | Available <= reorder point/2 | Urgent |
| Out of Stock | Available = 0 | Emergency |

### Low Stock Item Output

```javascript
{
  sku: "WIDGET-001",
  name: "Premium Widget",
  onHand: 15,
  allocated: 5,
  available: 10,
  reorderPoint: 20,
  averageDailySales: 3.5,
  daysOfStock: 2.8    // available / averageDailySales
}
```

## Demand Forecasting

### Forecast Output

```javascript
{
  sku: "WIDGET-001",
  name: "Premium Widget",
  averageDailyDemand: 3.5,     // Historical average
  forecastedDemand: 105,       // Next 30 days
  confidence: 0.7,             // 70% confidence
  currentStock: 45,
  daysUntilStockout: 12,       // stock / daily demand
  recommendedReorderQty: 105,  // 30 days supply
  trend: "Rising"              // Rising, Falling, Stable
}
```

### Demand Trends

| Trend | Pattern | Action |
|-------|---------|--------|
| Rising | Demand increasing | Order more |
| Stable | Consistent demand | Maintain levels |
| Falling | Demand decreasing | Reduce orders |

### Reorder Recommendations

When `daysUntilStockout < leadTime`:
- Order immediately
- Quantity = `forecastedDemand + safetyStock`

## Revenue Forecasting

### Revenue Forecast Output

```javascript
{
  period: "Period +1",          // Future period
  forecastedRevenue: 48000.00,  // Point estimate
  lowerBound: 40800.00,         // 80% confidence lower
  upperBound: 55200.00,         // 80% confidence upper
  confidenceLevel: 0.8,         // 80%
  basedOnPeriods: 12            // Historical periods used
}
```

### Forecast Granularity

| Granularity | Best For | Accuracy |
|-------------|----------|----------|
| Day | Short-term planning | Higher variance |
| Week | Operational planning | Moderate |
| Month | Strategic planning | More stable |

### Interpreting Confidence Intervals

```
Forecasted: $48,000
Lower (80%): $40,800
Upper (80%): $55,200

There's an 80% chance revenue will fall between
$40,800 and $55,200
```

## Order Status Breakdown

### Status Breakdown Output

```javascript
{
  pending: 12,      // Awaiting confirmation
  confirmed: 8,     // Confirmed, not processing
  processing: 15,   // Being prepared
  shipped: 45,      // In transit
  delivered: 120,   // Completed
  cancelled: 5,     // Cancelled by customer/merchant
  refunded: 3       // Refunded
}
```

### Operational Health Indicators

| Ratio | Formula | Good Target |
|-------|---------|-------------|
| Completion Rate | Delivered / Total | > 95% |
| Cancellation Rate | Cancelled / Total | < 3% |
| Fulfillment Rate | Shipped / (Pending+Confirmed+Processing) | Improving |

## Return Metrics

### Return Metrics Output

```javascript
{
  totalReturns: 23,
  returnRatePercent: 4.5,
  totalRefunded: 3450.00
}
```

### Return Rate Benchmarks

| Rate | Assessment | Action |
|------|------------|--------|
| < 3% | Excellent | Maintain quality |
| 3-5% | Average | Monitor reasons |
| 5-10% | High | Investigate causes |
| > 10% | Critical | Quality review |

### Common Return Reasons

1. **Defective** - Quality control issue
2. **Wrong item** - Fulfillment error
3. **Not as described** - Listing accuracy
4. **Changed mind** - Normal behavior
5. **Better price found** - Competitive pricing
6. **Damaged** - Shipping issue

## Analytics Workflows

### Weekly Business Review

```
1. get_sales_summary(period: "last7days")
2. get_top_products(period: "last7days", limit: 5)
3. get_order_status_breakdown(period: "last7days")
4. get_return_metrics(period: "last7days")
```

### Monthly Planning

```
1. get_sales_summary(period: "last_month")
2. get_customer_metrics(period: "last_month")
3. get_revenue_forecast(periodsAhead: 3, granularity: "month")
4. get_demand_forecast(daysAhead: 30)
```

### Inventory Review

```
1. get_inventory_health()
2. get_low_stock_items(threshold: 20)
3. get_demand_forecast(skus: [critical_skus], daysAhead: 14)
```

## Best Practices

1. **Compare periods** - Always show context vs prior period
2. **Focus on trends** - Single data points are less meaningful
3. **Segment analysis** - Break down by product, customer, region
4. **Action-oriented** - Every insight should suggest an action
5. **Set benchmarks** - Define what "good" looks like for your business
6. **Regular cadence** - Schedule analytics reviews weekly/monthly

## Common Questions and Answers

### "Is this a good month?"
Compare to:
- Same month last year (seasonality)
- Previous month (trend)
- Monthly average (benchmark)

### "Which products should I reorder?"
Look for:
- daysUntilStockout < 14
- trend = "Rising"
- High revenue contribution

### "Are customers coming back?"
Check:
- returningCustomers / (totalCustomers - newCustomers)
- Average orders per customer
- Customer lifetime value trend

### "Is my pricing right?"
Analyze:
- AOV trends
- Return rate
- Revenue per unit sold
