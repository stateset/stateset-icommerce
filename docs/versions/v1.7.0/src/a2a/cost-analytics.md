# Cost Analytics & Forecasting

The cost analytics engine is an in-memory ledger that tracks spend, earnings, margins, and cost anomalies across agent-to-agent commerce. Every payment, settlement, escrow fund, subscription billing, and split payment is recorded with metadata so agents can make informed economic decisions.

## Recording Economic Events

Every monetary transaction is logged with direction and operation type:

```javascript
const analytics = createCostAnalytics();

analytics.record({
    agentAddress: '0xBuyer',
    counterparty: '0xSeller',
    direction: 'spend',           // 'spend' or 'earn'
    amount: 100,
    operation: 'quote_payment',
    sagaId: 'saga-001',           // optional: link to saga
    timestamp: new Date().toISOString(),
});
```

### Operation Types

| Operation | Description |
|-----------|-------------|
| `quote_payment` | Payment for an accepted quote |
| `escrow_fund` | Funds deposited into escrow |
| `escrow_release` | Escrow funds released to recipient |
| `escrow_refund` | Escrow funds returned to sender |
| `subscription_billing` | Recurring subscription charge |
| `split_payment` | Multi-party split distribution |
| `settlement` | On-chain settlement |
| `platform_fee` | Platform/protocol fee deduction |
| `refund` | Refund issued |

## Spend Summary

Get an overview of an agent's economic activity:

```javascript
const summary = analytics.getAgentSpendSummary('0xBuyer');
// → {
//     totalSpent: 4500.00,
//     totalEarned: 1200.00,
//     netMargin: -3300.00,
//     avgTransactionSize: 112.50,
//     transactionCount: 40,
//     firstTransaction: '2026-01-10T...',
//     lastTransaction: '2026-03-16T...',
// }
```

## Counterparty Breakdown

See where money flows per counterparty:

```javascript
const breakdown = analytics.getCounterpartyBreakdown('0xBuyer');
// → [
//     { counterparty: '0xSeller-A', spent: 2800, earned: 0, count: 18 },
//     { counterparty: '0xSeller-B', spent: 1200, earned: 500, count: 15 },
//     { counterparty: '0xSeller-C', spent: 500, earned: 700, count: 7 },
// ]
```

## Operation Breakdown

Understand spending by operation type:

```javascript
const ops = analytics.getOperationBreakdown('0xBuyer');
// → {
//     quote_payment: { total: 3200, count: 25 },
//     subscription_billing: { total: 594, count: 6 },
//     escrow_fund: { total: 500, count: 3 },
//     platform_fee: { total: 206, count: 40 },
// }
```

## Daily Spend Trends

Track spending patterns over time:

```javascript
const trend = analytics.getDailySpendTrend('0xBuyer', { days: 30 });
// → [
//     { date: '2026-03-16', spent: 150, earned: 45, net: -105, count: 4 },
//     { date: '2026-03-15', spent: 200, earned: 0, net: -200, count: 3 },
//     ...
// ]
```

## Anomaly Detection

Automatically flag unusual transactions:

```javascript
const anomalies = analytics.detectAnomalies('0xBuyer');
// → [
//     {
//         type: 'large_transaction',
//         description: 'Transaction 3x above average',
//         transaction: { amount: 1500, counterparty: '0xNew', operation: 'quote_payment' },
//         threshold: 337.50,   // 3x the average of 112.50
//     },
//     {
//         type: 'daily_spike',
//         description: 'Daily spend 2x above average',
//         date: '2026-03-14',
//         dailySpend: 800,
//         dailyAvg: 150,
//     }
// ]
```

Detection rules:
- **Large transaction**: Any single transaction > 3x the agent's average transaction size
- **Daily spike**: Any day where total spend > 2x the agent's daily average

## Escrow Metrics

Track escrow performance and hold times:

```javascript
const escrow = analytics.getEscrowMetrics('0xBuyer');
// → {
//     totalFunded: 5000,
//     totalReleased: 4200,
//     totalRefunded: 800,
//     releaseRate: 0.84,
//     refundRate: 0.16,
//     avgHoldTimeMs: 172800000,   // ~2 days average
// }
```

## Margin Analysis

Per-counterparty profitability:

```javascript
const margins = analytics.getMarginAnalysis('0xBuyer');
// → [
//     { counterparty: '0xSeller-C', spent: 500, earned: 700, margin: 200, marginPct: 0.40 },
//     { counterparty: '0xSeller-B', spent: 1200, earned: 500, margin: -700, marginPct: -0.58 },
//     ...
// ]
```

## Budget Forecasting

Project when an agent's budget will be exhausted:

```javascript
const forecast = analytics.getBudgetForecast('0xBuyer', 1000); // $1000 remaining budget
// → {
//     dailyAvgSpend: 150.00,
//     daysRemaining: 6,
//     exhaustionDate: '2026-03-22',
//     confidence: 'medium',       // based on variance
// }
```

## Top Spenders

Cross-agent spend ranking (useful for platform operators):

```javascript
const top = analytics.getTopSpenders({ limit: 10 });
// → [
//     { agent: '0xBuyer-1', totalSpent: 12000, transactionCount: 85 },
//     { agent: '0xBuyer-2', totalSpent: 8500, transactionCount: 62 },
//     ...
// ]
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_cost_record` | Record a spend/earn event |
| `a2a_cost_summary` | Get agent spend summary |
| `a2a_cost_counterparty` | Counterparty spend breakdown |
| `a2a_cost_operations` | Breakdown by operation type |
| `a2a_cost_trend` | Daily spend trend |
| `a2a_cost_anomalies` | Detect spending anomalies |
| `a2a_cost_escrow_metrics` | Escrow hold times and release rates |
| `a2a_cost_margins` | Per-counterparty margin analysis |
| `a2a_cost_forecast` | Budget exhaustion forecast |
| `a2a_cost_top_spenders` | Cross-agent spend ranking |
