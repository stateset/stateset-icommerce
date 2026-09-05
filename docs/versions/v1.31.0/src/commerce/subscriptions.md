# Subscriptions & Billing

Subscription management handles recurring billing with plans, trial periods, billing intervals, and lifecycle management.

## Subscription Lifecycle

```
Active → Paused → Active (resume)
   └──→ Past Due → Active (payment retry)
   └──→ Cancelled (terminal)
```

## Plans

Create subscription plans that define pricing and billing intervals:

```javascript
const plan = commerce.subscriptions.createPlan({
    code: 'PREMIUM',
    name: 'Premium Plan',
    interval: 'month',     // week, month, quarter, year
    intervalCount: 1,
    price: 19.99,
    currency: 'USD',
    trialDays: 14           // Optional trial period
});
```

## Subscribe a Customer

```javascript
const subscription = commerce.subscriptions.subscribe(customer.id, plan.id);
```

## Lifecycle Operations

```javascript
// Pause (e.g., customer going on vacation)
commerce.subscriptions.pause(subscription.id);

// Resume
commerce.subscriptions.resume(subscription.id);

// Cancel
commerce.subscriptions.cancel(subscription.id);
```

## Billing Intervals

| Interval | Description |
|----------|-------------|
| `week` | Weekly billing |
| `month` | Monthly billing |
| `quarter` | Every 3 months |
| `year` | Annual billing |

Custom intervals use `intervalCount`:
- Every 2 weeks: `interval: 'week', intervalCount: 2`
- Every 6 months: `interval: 'month', intervalCount: 6`

## Dunning Management

When a recurring payment fails, the subscription moves to `past_due` status. The billing executor retries with exponential backoff:

1. Immediate retry
2. Retry after 1 day
3. Retry after 3 days
4. Move to cancelled if all retries fail

## Subscription Events

| Event | Trigger |
|-------|---------|
| `subscription.created` | New subscription started |
| `subscription.charged` | Recurring charge successful |
| `subscription.paused` | Subscription paused by customer |
| `subscription.resumed` | Subscription resumed |
| `subscription.past_due` | Charge failed, moved to past-due |
| `subscription.cancelled` | Subscription cancelled (terminal) |

## Churn Analysis

```bash
stateset "what is our subscription churn rate this month?"
stateset "list customers with past-due subscriptions"
stateset "compare MRR this month vs last month"
```

The heartbeat monitor can check for subscription churn automatically:

```json
{
    "id": "subscription-churn",
    "checker": "subscription-churn",
    "intervalMs": 86400000,
    "enabled": true
}
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_subscription_plans` | List all plans |
| `create_subscription_plan` | Create a new plan |
| `subscribe_customer` | Create a subscription |
| `pause_subscription` | Pause billing |
| `resume_subscription` | Resume billing |
| `cancel_subscription` | Cancel subscription |
| `list_subscriptions` | List all subscriptions |
| `get_subscription` | Get subscription details |

For agent-to-agent recurring payments, see [A2A Subscriptions](../a2a/subscriptions.md).
