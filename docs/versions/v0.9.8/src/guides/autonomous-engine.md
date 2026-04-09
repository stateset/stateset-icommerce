# Autonomous Engine

The autonomous engine enables self-governing commerce operations — scheduled billing, automatic dispute resolution, SLA enforcement, and proactive monitoring — without human intervention.

Note: some interfaces on this page are runtime-specific operational surfaces. Verify the named tools against your actual runtime before depending on them in unattended automation.

## Components

| Component | Module | Description |
|-----------|--------|-------------|
| Billing Executor | `billing-executor.js` | Automatic recurring charge processing |
| Dispute Resolver | `dispute-resolver.js` | Rule-based dispute auto-resolution |
| SLA Enforcer | `sla.js` | Compliance checking and auto-penalties |
| Marketplace Auto-Award | `marketplace.js` | Automatic RFQ winner selection |
| Health Monitor | `health.js` | Live/ready/health endpoints |
| Rate Limiter | `rate-limiter.js` | Request throttling per agent |
| Tick Optimizer | `tick-optimizer.js` | Heartbeat timing optimization |

## Billing Executor

Automatically processes recurring subscription charges.

### Billing Intervals

| Interval | Days | Calculation |
|----------|------|-------------|
| `weekly` | 7 | `date + 7 days` |
| `biweekly` | 14 | `date + 14 days` |
| `monthly` | 30 | `date + 1 month` (calendar-aware) |
| `quarterly` | 90 | `date + 3 months` |
| `annual` | 365 | `date + 1 year` |

### Billing Cycle

1. Finds subscriptions where `next_billing_date <= now`
2. Transitions expired trials to `active`
3. Creates an x402 payment intent for the subscription amount
4. Executes the A2A payment
5. Records the charge in `a2a_subscription_charges`
6. Computes the next billing date
7. On failure: moves subscription to `past_due`, increments `past_due_cycles`
8. On `past_due_cycles > maxPastDueCycles`: auto-cancels the subscription

### Dunning

Past-due subscriptions trigger dunning notifications at each billing cycle. The subscriber and provider both receive notifications.

```javascript
// Initialize billing executor
const executor = createBillingExecutor(store, a2aService, notifications, {
    intervalMs: 60_000,     // check every minute
    maxPastDueCycles: 3,    // cancel after 3 failed cycles
});
executor.start();

// Check status
const status = await toolkit.executeTool('a2a_billing_metrics', {});
// → { running: true, totalTicks: 142, chargesProcessed: 87, failedCharges: 3 }

// Manually run a billing cycle
await toolkit.executeTool('a2a_billing_tick', {});
```

## Dispute Auto-Resolution

Resolves disputes based on deadlines and rule-based arbitration.

### Dispute Timeline

| Phase | Duration | Trigger |
|-------|----------|---------|
| `filed` | 24 hours | Dispute opened |
| `evidence_period` | 72 hours | Auto-transition after 24h, or first evidence submitted |
| `under_review` | 7 days | Auto-transition after evidence deadline |
| `resolved` / `escalated` | — | Arbitration applied or escalated |

### Arbitration Rules

| Reason | Condition | Resolution |
|--------|-----------|------------|
| `non_delivery` | No `delivery_proof` evidence from seller | Full refund |
| `poor_quality` | Seller reputation < 2.5 | Full refund |
| `overcharged` | > 20% price discrepancy from quote | Partial refund (market delta) |
| Other | Amount ≤ `autoResolveThreshold` | Split 50/50 |
| Other | Amount > `autoResolveThreshold` | Escalated for manual review |

### Configuration

```javascript
const resolver = createDisputeResolver(store, disputeService, escrowService, notifications, {
    autoResolveThreshold: 1000,  // auto-resolve disputes up to $1,000
    intervalMs: 300_000,          // check every 5 minutes
});
resolver.start();

// Check resolver metrics
const status = await toolkit.executeTool('a2a_dispute_resolver_metrics', {});
// → { running: true, totalTicks: 58, autoResolutions: 12, autoEscalations: 2 }

// Manually run one resolver cycle
await toolkit.executeTool('a2a_dispute_resolver_tick', {});
```

Both parties receive notifications at each transition (filed → evidence_period → under_review → resolved).

## SLA Enforcement

Monitor service level agreements and auto-penalize violations:

```javascript
// Enforce penalties for one service
const compliance = await toolkit.executeTool('a2a_sla_enforce', {
    serviceId: 'uptime-99'
});

// Run a full SLA enforcement sweep
await toolkit.executeTool('a2a_sla_enforce_all', {});
```

## Marketplace Auto-Award

Automatically select RFQ winners based on scoring:

```javascript
await toolkit.executeTool('a2a_marketplace_auto_award', {});

// Or run the broader maintenance tick
await toolkit.executeTool('a2a_marketplace_maintenance', {});
```

## Event-Driven Automation

The autonomous engine reacts to commerce events:

| Event | Automatic Action |
|-------|-----------------|
| `order.created` | Reserve inventory, initiate payment |
| `payment.failed` | Retry with backoff, notify customer agent |
| `inventory.low` | Trigger procurement via supplier agents |
| `subscription.past_due` | Dunning sequence, escalation |
| `dispute.deadline_passed` | Auto-resolve in filer's favor |
| `sla.violated` | Calculate and apply penalties |

## Health Checks

The health service provides three probe types for production deployments:

### Full Health Check (`/health`)

Tests all dependencies and subsystems:

```javascript
const health = await toolkit.executeTool('a2a_health_check', {});
// → {
//     status: 'healthy',
//     timestamp: '2026-03-16T10:30:45Z',
//     uptime: 86400000,
//     checks: {
//         database: { status: 'ok', latencyMs: 2 },
//         sequencer: { status: 'ok', latencyMs: 45 },
//         billingExecutor: { status: 'running', totalTicks: 142, lastTickAt: '...' },
//         disputeResolver: { status: 'running', totalTicks: 58, lastTickAt: '...' }
//     }
// }
```

### Liveness Probe (`/live` or `/livez`)

Returns immediately if the process is alive. Used by Kubernetes `livenessProbe`.
This probe is exposed as HTTP only; there is no dedicated MCP wrapper:

```json
{ "status": "alive", "timestamp": "2026-03-16T10:30:45Z" }
```

### Readiness Probe (`/ready` or `/readyz`)

Tests database connectivity. Returns `503` if the database is unreachable. Used by Kubernetes `readinessProbe`:

```javascript
const readiness = await toolkit.executeTool('a2a_readiness', {});
// → { status: 'ready', timestamp: '2026-03-16T10:30:45Z' }
```

## Rate Limiter

The rate limiter throttles requests per agent to prevent abuse:

```javascript
const metrics = await toolkit.executeTool('a2a_rate_limit_metrics', {});
// → { requests: 450, limit: 1000, remaining: 550, resetAt: '...' }
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_billing_metrics` | Billing executor health and metrics |
| `a2a_billing_tick` | Trigger one billing cycle |
| `a2a_dispute_resolver_metrics` | Dispute resolver health and metrics |
| `a2a_dispute_resolver_tick` | Run one dispute-resolution cycle |
| `a2a_sla_enforce` | Apply SLA penalties for one service |
| `a2a_sla_enforce_all` | Run SLA enforcement across all services |
| `a2a_marketplace_auto_award` | Auto-award an RFQ based on scoring |
| `a2a_marketplace_maintenance` | Run auto-award, expiry, and cleanup |
| `a2a_health_check` | Full health check (DB + sequencer + subsystems) |
| `a2a_readiness` | Readiness probe (DB connectivity) |
| `a2a_rate_limit_metrics` | Rate limiter stats per agent |
