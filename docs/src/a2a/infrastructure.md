# A2A Infrastructure

Behind the core A2A protocol sit six infrastructure modules that make autonomous agent commerce production-grade: saga orchestration, distributed tracing, idempotency, handshake negotiation, cost analytics, and a rules engine.

## Saga Orchestration

Multi-step commerce flows with automatic compensation on failure. If step 3 of a 5-step flow fails, steps 1 and 2 are automatically rolled back in reverse order.

### Saga Lifecycle

```
PENDING → RUNNING → COMPLETED
                  → FAILED → COMPENSATING → COMPENSATED
```

### Step Lifecycle

```
PENDING → RUNNING → COMPLETED
                  → FAILED → COMPENSATING → COMPENSATED
                                           → COMPENSATION_FAILED
                  → TIMED_OUT
                  → SKIPPED
```

### Defining a Saga

Each saga is a sequence of steps, each with an `execute` function and a `compensate` (rollback) function:

```javascript
const PURCHASE_SAGA = {
    name: 'purchase',
    steps: [
        {
            name: 'reserve_inventory',
            execute: async (ctx) => commerce.inventory.reserve(ctx.sku, ctx.quantity),
            compensate: async (ctx, result) => commerce.inventory.release(result.id)
        },
        {
            name: 'create_payment',
            execute: async (ctx) => toolkit.executeTool('x402_create_payment_intent', ctx.payment),
            compensate: async (_ctx, result) => cancelPendingIntent(result.intentId)
        },
        {
            name: 'create_order',
            execute: async (ctx) => commerce.orders.create(ctx.order),
            compensate: async (ctx, result) => commerce.orders.cancel(result.id)
        }
    ]
};
```

### Executing a Saga

```javascript
const result = await orchestrator.execute(PURCHASE_SAGA, {
    sku: 'WIDGET-001',
    quantity: 2,
    payment: { fromAgent: 'buyer', toAgent: 'seller', amount: 59.98 },
    order: { customerId: 'cust-001', items: [...] }
});
// → { sagaId: '...', status: 'completed', steps: [...] }
```

If payment creation fails (step 2), inventory reservation (step 1) is automatically compensated.

### Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `stepTimeoutMs` | 30,000 | Max time per step before timeout |
| `retryAttempts` | 0 | Retry count per step on failure |
| `idempotencyKey` | auto | Re-executing with same key skips completed steps |

## Distributed Tracing

W3C Trace Context-compatible tracing for multi-agent transactions. Tracks latency, error rates, and throughput across agent boundaries.

### Creating Spans

```javascript
const tracing = createTracingService({ maxSpans: 5000 });

// Manual span
const span = tracing.startSpan('process_order', { kind: 'server' });
span.setAttribute('orderId', 'ord-123');
try {
    await processOrder();
    span.setStatus('ok');
} catch (err) {
    span.setStatus('error');
    span.addEvent('error', { message: err.message });
} finally {
    span.end();
}

// Convenience wrapper
const result = await tracing.withSpan('checkout', async () => {
    return await commerce.carts.checkout(cartId);
});
```

### Context Propagation

Inject trace context into outgoing HTTP headers for cross-agent tracing:

```javascript
const headers = {};
tracing.inject(headers);
// headers → { traceparent: '00-{traceId}-{spanId}-01', tracestate: '...' }
```

### Metrics

```javascript
const metrics = tracing.getMetrics();
// → { p50: 12, p95: 45, p99: 120, errorRate: 0.02, throughput: 150 }
```

## Idempotency Guard

Prevents duplicate execution of operations — critical for AI agents that retry on timeout or network failure.

```javascript
const guard = createIdempotencyGuard({
    ttlMs: 86_400_000,   // 24-hour cache
    maxSize: 10_000       // LRU eviction
});

// First call executes the function
const result1 = await guard.execute('payment-abc-123', async () => {
    return await toolkit.executeTool('a2a_pay', { amount: 100 });
});

// Second call with same key returns cached result (no re-execution)
const result2 = await guard.execute('payment-abc-123', async () => {
    return await toolkit.executeTool('a2a_pay', { amount: 100 });
});
// result1 === result2, payment executed only once

// Metrics
guard.getMetrics();
// → { hits: 1, misses: 1, size: 1, evictions: 0 }
```

Concurrent callers with the same key wait for the first execution to complete rather than executing in parallel.

## Handshake Protocol

Before two agents transact, they exchange capability manifests to negotiate compatible networks, assets, and features.

```javascript
const handshake = createHandshakeService({
    agentId: 'my-agent',
    supportedNetworks: ['set_chain', 'base', 'ethereum'],
    supportedAssets: ['USDC', 'USDT', 'ssUSD'],
    features: { escrow: true, subscriptions: true, splits: true },
    maxTransactionAmount: 50000,
    preferredFinality: 'final',
    webhookEndpoint: 'https://my-agent.example.com/webhooks',
    publicKey: 'ed25519:abc123...'
});

const result = handshake.initiateHandshake(theirCapabilities);
// → {
//     compatible: true,
//     bestNetwork: 'base',        // Highest priority shared network
//     negotiatedAssets: ['USDC'],  // Shared asset list
//     sharedFeatures: ['escrow', 'subscriptions']
// }
```

### Network Priority

When multiple shared networks exist, the handshake selects by priority:

```
set_chain > base > arbitrum > solana > ethereum
```

### Asset Priority

```
USDC > USDT > ssUSD > DAI
```

If no compatible network or asset exists, the handshake returns `compatible: false` and the transaction is not attempted.

## Cost Analytics

In-memory ledger tracking spend, earnings, and margins across A2A commerce. Enables agents to make budget-aware decisions.

```javascript
const analytics = createCostAnalytics();

// Record a transaction
analytics.record({
    agentAddress: 'my-agent',
    counterparty: 'data-agent',
    direction: 'outbound',     // outbound = spending, inbound = earning
    amount: 0.02,
    operation: 'quote_payment',
    sagaId: 'saga-123'
});

// Spending summary
const summary = analytics.getAgentSpendSummary('my-agent');
// → { totalSpent: 4.50, totalEarned: 12.00, netMargin: 7.50,
//     avgTransactionSize: 0.02, transactionCount: 225 }

// Budget forecast
const forecast = analytics.getBudgetForecast('my-agent', 5.00);
// → { dailyAvgSpend: 0.45, daysRemaining: 1.1, exhaustionDate: '2026-03-17T12:00:00Z' }
```

### Tracked Operations

| Operation | Direction | Description |
|-----------|-----------|-------------|
| `quote_payment` | outbound | Paying for an accepted quote |
| `escrow_fund` | outbound | Depositing into escrow |
| `escrow_release` | inbound | Receiving escrow release |
| `escrow_refund` | inbound | Receiving escrow refund |
| `subscription_billing` | outbound | Recurring subscription charge |
| `split_payment` | both | Multi-party split distribution |
| `settlement` | both | On-chain settlement |
| `platform_fee` | outbound | Platform fee deduction |
| `refund` | inbound | Receiving a refund |

## Rules Engine

Declarative "if X then Y" rules for programmable agent guardrails. Higher priority rules evaluate first; the first `block` action halts execution.

```javascript
const engine = createRulesEngine();

// Add a rule: require escrow for high-value transactions
engine.addRule({
    name: 'high-value-escrow',
    description: 'Transactions over $500 require escrow',
    agentAddress: 'my-agent',
    condition: { field: 'amount', operator: 'gte', value: 500 },
    action: { type: 'require_escrow' },
    priority: 90,
    enabled: true
});

// Add a rule: block transactions with low-reputation agents
engine.addRule({
    name: 'reputation-gate',
    condition: { field: 'counterparty_reputation', operator: 'lt', value: 2.0 },
    action: { type: 'block' },
    priority: 100,     // Evaluated before escrow rule
    enabled: true
});

// Evaluate
const decision = engine.evaluate({
    amount: 1000,
    counterparty_reputation: 1.5
});
// → { allowed: false, appliedRules: ['reputation-gate'],
//     explanation: 'Blocked: counterparty reputation 1.5 < 2.0' }
```

### Condition Operators

| Operator | Description |
|----------|-------------|
| `eq` | Equal |
| `neq` | Not equal |
| `gt`, `gte` | Greater than / or equal |
| `lt`, `lte` | Less than / or equal |
| `in` | Value in array |
| `not_in` | Value not in array |
| `contains` | String/array contains |
| `matches` | Regex match |

## Agent Memory

Counterparty learning engine that builds profiles from past interactions and makes risk-aware recommendations.

```javascript
const memory = createAgentMemory();

// Record an interaction
memory.recordInteraction({
    agentAddress: 'my-agent',
    counterpartyAddress: 'seller-agent',
    interactionType: 'payment_sent',
    outcome: 'success',
    amount: 450.00,
    responseTimeMs: 2500
});

// Query counterparty profile
const profile = memory.getCounterpartyProfile('my-agent', 'seller-agent');
// → {
//     totalInteractions: 15,
//     successRate: 0.93,
//     reliabilityScore: 4.2,
//     avgResponseTimeMs: 3100,
//     recentOutcomes: ['success', 'success', 'timeout', 'success', ...]
// }

// Get recommendation
const rec = memory.getRecommendation('my-agent', 'seller-agent', 'payment_sent');
// → { recommended: true, confidence: 0.87, reason: 'High success rate (93%), good reliability' }
```

### Interaction Types

| Type | Description |
|------|-------------|
| `quote_received` | Received a quote from counterparty |
| `quote_sent` | Sent a quote to counterparty |
| `payment_sent` | Sent payment |
| `payment_received` | Received payment |
| `negotiation` | Negotiation round |
| `dispute` | Dispute filed |
| `fulfillment` | Service delivered |
| `rating` | Reputation feedback |

### Risk Thresholds

| Constant | Value | Description |
|----------|-------|-------------|
| `TIMELY_RESPONSE_MS` | 10,000 | Response under this = "timely" |
| `RECENT_WINDOW` | 10 | Number of recent interactions for trend analysis |
| `HIGH_RISK_THRESHOLD` | 0.2 | Dispute/failure rate above this = "high risk" |
