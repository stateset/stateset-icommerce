# A2A Subscriptions

Recurring agent-to-agent payments for ongoing services. An agent can subscribe to another agent's service with automatic billing.

## Subscription Lifecycle

```
Active → Paused → Active (resume)
   └──→ Past Due → Active (payment retry)
   └──→ Cancelled (terminal)
```

## Create an A2A Subscription

```javascript
const subscription = await toolkit.executeTool('a2a_create_subscription', {
    subscriberAgent: 'research-agent',
    providerAgent: 'data-agent',
    planName: 'Premium Data Feed',
    interval: 'month',        // week, month, quarter, year
    intervalCount: 1,
    amount: 99.00,
    currency: 'USD',
    trialDays: 7
});
```

## Billing Intervals

| Interval | Supported Counts |
|----------|-----------------|
| `week` | 1-52 |
| `month` | 1-12 |
| `quarter` | 1-4 |
| `year` | 1 |

## Lifecycle Operations

```javascript
// Pause (e.g., agent hibernating)
await toolkit.executeTool('a2a_pause_subscription', {
    subscriptionId: sub.id
});

// Resume
await toolkit.executeTool('a2a_resume_subscription', {
    subscriptionId: sub.id
});

// Cancel
await toolkit.executeTool('a2a_cancel_subscription', {
    subscriptionId: sub.id
});
```

## Automatic Billing

The billing executor runs on a schedule and:

1. Finds subscriptions due for billing
2. Creates a payment intent for each
3. Charges the subscriber via x402
4. Records the charge in `a2a_subscription_charges`
5. On failure, moves subscription to `past_due` and retries with exponential backoff

```javascript
// Manually trigger billing (usually automated)
await toolkit.executeTool('a2a_billing_run', {
    subscriptionId: sub.id
});
```

## Billing Intervals (Exact Values)

| Interval | Days | Date Calculation |
|----------|------|-----------------|
| `weekly` | 7 | `date + 7 days` |
| `biweekly` | 14 | `date + 14 days` |
| `monthly` | 30 | `date + 1 calendar month` |
| `quarterly` | 90 | `date + 3 calendar months` |
| `annual` | 365 | `date + 1 calendar year` |

## Trial → Active Transition

When a subscription has `trialDays`, the state machine is:

```
Trial → Active (on trial expiry) → Paused / Past Due / Cancelled
```

The billing executor automatically transitions expired trials to `active` and schedules the first charge.

## Dunning Flow

When a recurring charge fails:

```
Active → Past Due (charge failed)
          ├─ Retry cycle 1: bill again at next interval
          ├─ Retry cycle 2: bill again, send dunning notification
          ├─ Retry cycle 3: bill again, final notice
          └─ maxPastDueCycles exceeded → Cancelled (auto)
```

Both subscriber and provider agents receive notifications at each retry.

## Error Handling

| Error | Cause | Resolution |
|-------|-------|------------|
| `SubscriptionNotFoundError` | Invalid subscription ID | Check `a2a_list_subscriptions` |
| `InvalidStateError` | Pause on already-paused subscription | Check current status |
| `BillingFailedError` | Payment charge failed | Subscription moves to past_due, retries automatically |
| `InvalidIntervalError` | Unsupported billing interval | Use: weekly, biweekly, monthly, quarterly, annual |

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_create_subscription` | Create a recurring subscription with interval and trial |
| `a2a_pause_subscription` | Pause billing |
| `a2a_resume_subscription` | Resume billing |
| `a2a_cancel_subscription` | Cancel subscription (terminal) |
| `a2a_list_subscriptions` | List subscriptions (filter by status, agent) |
| `a2a_get_subscription` | Get subscription details with charge history |
| `a2a_billing_run` | Manually trigger a billing cycle |
