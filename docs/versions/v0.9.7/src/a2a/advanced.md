# A2A Advanced Features

Beyond the core protocol primitives, the A2A layer includes advanced capabilities for autonomous agent behavior: negotiation strategies, DAG-based workflows, inter-agent messaging, fan-out coordination, simulation, and checkpointing.

## Negotiation Strategies

Agents don't hard-code pricing logic. Instead, they select from pluggable negotiation strategies that evaluate incoming quotes and decide how to respond.

### Built-in Strategies

| Strategy | Behavior | Best For |
|----------|----------|----------|
| `AlwaysAccept` | Accepts any quote unconditionally | Testing, demos |
| `BudgetGated` | Accepts if within budget; applies markup for outgoing quotes | Cost-conscious agents |
| `Negotiator` | Counter-offers with decreasing concession rate over rounds | Price optimization |
| `BestOfN` | Collects N quotes, selects best by composite score | Competitive procurement |
| `DynamicPricing` | Adjusts prices based on demand multipliers | High-throughput services |
| `CustomRules` | User-defined condition-action pairs | Domain-specific logic |
| `Composable` | Chains multiple strategies (first match wins) | Complex policies |

### Strategy Interface

Every strategy implements three methods:

```javascript
strategy.evaluateReceivedQuote(quote, context)
// → { action: 'accept' | 'counter' | 'decline', counterPrice?: number, reason?: string }

strategy.evaluateIncomingQuote(quote, context)
// → { price: number, terms: {...} }

strategy.evaluatePaymentRequest(request, context)
// → { approved: boolean, reason?: string }
```

### Example: Negotiator Strategy

```javascript
const strategy = createNegotiatorStrategy({
    basePrice: 100,
    markup: 0.20,           // Start 20% above base
    minAcceptable: 80,      // Walk away below $80
    concessionRate: 0.15    // Concede 15% per round
});

// Round 1: Seller quotes $120 → Agent counters with $96
// Round 2: Seller quotes $108 → Agent counters with $91.20
// Round 3: Seller quotes $100 → Agent accepts ($100 ≥ $80)
```

### Example: Composable Strategy

```javascript
const strategy = createComposableStrategy([
    // First: block low-reputation counterparties
    createCustomRulesStrategy([
        { condition: { field: 'counterparty_reputation', operator: 'lt', value: 3.0 },
          action: 'decline', reason: 'Reputation too low' }
    ]),
    // Then: negotiate on price
    createNegotiatorStrategy({ basePrice: 100, minAcceptable: 80 })
]);
```

## Workflow Orchestration (DAG)

For multi-step commerce flows involving multiple agents, the workflow engine executes steps as a directed acyclic graph (DAG) with dependency management.

### Defining a Workflow

```javascript
const workflow = await workflowService.createWorkflow({
    name: 'procurement-pipeline',
    steps: [
        {
            id: 'rfq',
            type: 'quote_request',
            config: { targets: ['supplier-a', 'supplier-b', 'supplier-c'] },
            dependsOn: []
        },
        {
            id: 'evaluate',
            type: 'condition_check',
            config: { scoringCriteria: 'best_value' },
            dependsOn: ['rfq']
        },
        {
            id: 'payment',
            type: 'payment',
            config: { useEscrow: true, condition: 'seller_fulfilled' },
            dependsOn: ['evaluate']
        },
        {
            id: 'notify',
            type: 'transform',
            config: { action: 'send_notification' },
            dependsOn: ['payment']
        }
    ]
});
```

### DAG Validation

The workflow engine validates the step graph using Kahn's algorithm for topological sort. Cycles are rejected at creation time.

### Execution

```javascript
const result = await workflowService.executeWorkflow(workflow.id);
// → {
//     workflowId: '...',
//     status: 'completed',
//     steps: [
//         { id: 'rfq', status: 'completed', result: { quotes: [...] } },
//         { id: 'evaluate', status: 'completed', result: { winner: 'supplier-b' } },
//         { id: 'payment', status: 'completed', result: { escrowId: '...' } },
//         { id: 'notify', status: 'completed', result: { delivered: true } }
//     ],
//     totalCost: 450.00,
//     durationMs: 1250
// }
```

Steps with no dependencies execute in parallel (fan-out). Steps resume from checkpoints on crash recovery.

## Agent Messaging

Agents communicate through a structured messaging system with typed messages, priority levels, and threaded conversations.

### Message Types

| Type | Description | Use Case |
|------|-------------|----------|
| `text` | Free-form text | Status updates, notifications |
| `task_delegation` | Task with deadline and reward | Outsourcing work |
| `status_query` | Request for agent status | Health monitoring |
| `status_response` | Reply to status query | Reporting |
| `data_request` | Structured data request | API-like queries |
| `data_response` | Structured data response | API-like responses |

### Sending Messages

```javascript
// Direct message
await messaging.sendMessage({
    from: 'orchestrator-agent',
    to: 'fulfillment-agent',
    type: 'text',
    payload: { body: 'Order ORD-123 is ready for fulfillment' }
});

// Task delegation with deadline and reward
await messaging.delegateTask({
    from: 'procurement-agent',
    to: 'logistics-agent',
    description: 'Ship 500 units of WIDGET-001 to Warehouse B',
    deadline: '2026-03-20T17:00:00Z',
    reward: 25.00,
    priority: 'high'
});

// Check inbox
const messages = await messaging.getInbox('fulfillment-agent', { unreadOnly: true });

// Thread a conversation
const thread = await messaging.getThread(messageId);
```

### Priority Levels

| Priority | Behavior |
|----------|----------|
| `low` | Standard processing |
| `medium` | Default priority |
| `high` | Expedited processing |
| `critical` | Immediate attention, may trigger alerts |

Messages expire after 24 hours by default (configurable TTL).

## Fan-Out / Join

Broadcast a task to multiple agents and aggregate responses with configurable join strategies.

### Join Strategies

| Strategy | Behavior |
|----------|----------|
| `all` | Wait for all responses (or timeout) |
| `first` | Return immediately on first response |
| `majority` | Return when > 50% have responded |
| `quorum(n)` | Return when exactly N have responded |
| `best` | Wait for all, return highest-scored |

### Example: Competitive Quote Collection

```javascript
// Scatter: send RFQ to 5 suppliers
const coordId = await fanOut.scatter({
    agentAddress: 'procurement-agent',
    targets: ['supplier-1', 'supplier-2', 'supplier-3', 'supplier-4', 'supplier-5'],
    taskType: 'quote_request',
    payload: { sku: 'WIDGET-001', quantity: 1000 },
    timeoutMs: 30_000,
    joinStrategy: 'best'     // Wait for all, pick cheapest
});

// Join: aggregate responses
const result = await fanOut.join(coordId);
// → {
//     strategy: 'best',
//     responses: [ { agent: 'supplier-3', price: 11.50 }, ... ],
//     bestResponse: { agent: 'supplier-3', price: 11.50 },
//     respondedCount: 4,
//     timedOutCount: 1
// }
```

## Simulation & Testing

Run full A2A scenarios with deterministic time control — no waiting, no real payments.

### Demo Scenarios

| Scenario | Description |
|----------|-------------|
| `basic-negotiation` | Buyer/seller quote exchange |
| `marketplace` | Multi-party RFQ broadcast |
| `escrow-deal` | 3-party escrow with conditions |
| `rfq-competition` | Competitive bidding with scoring |
| `workflow-pipeline` | DAG-based multi-agent pipeline |

```javascript
import { runDemoScenario } from '@stateset/cli/a2a/demo-scenarios';
await runDemoScenario('marketplace');
```

### Time Control

Simulate days or weeks of billing cycles in milliseconds:

```javascript
import { withSimulatedClock } from '@stateset/cli/a2a/simulator';

await withSimulatedClock(new Date('2026-01-01'), async (clock) => {
    // Create a monthly subscription
    await a2a.createSubscription({ interval: 'monthly', amount: 99 });

    // Fast-forward 35 days — triggers billing
    clock.advance(35 * 24 * 60 * 60 * 1000);
    await billingExecutor.executeBillingCycle();

    // Verify charge was created
    const charges = await store.listSubscriptionCharges(subId);
    assert.equal(charges.length, 1);
});
```

## Agent Introspection

Debug agent behavior by inspecting their decision history:

```javascript
const dashboard = introspection.getAgentDashboard('my-agent');
// → {
//     decisions: { total: 150, quotes_accepted: 45, quotes_declined: 30, ... },
//     performance: { avgTickDurationMs: 23, p95: 67 },
//     recentDecisions: [
//         { type: 'quote_eval', action: 'counter', reason: 'Price above threshold', ... }
//     ]
// }
```

### Decision Types

| Type | Description |
|------|-------------|
| `quote_eval` | Quote acceptance/rejection/counter |
| `payment` | Payment approval decision |
| `strategy_change` | Strategy or configuration change |
| `budget_check` | Budget threshold evaluation |

### Performance Report

```javascript
const report = introspection.getPerformanceReport('my-agent');
// → {
//     quoteAcceptRate: 0.60,
//     avgResponseTimeMs: 1200,
//     settlementSuccessRate: 0.98,
//     disputeRate: 0.02,
//     uptimePct: 99.7,
// }
```

## Checkpointing

Crash-safe agent state persistence. On restart, agents resume from their last checkpoint:

```javascript
const checkpoint = createCheckpointService('./checkpoints');

// Save state before processing
await checkpoint.save('my-agent', { lastProcessedQuoteId: 'q-123', balance: 450.00 });

// On restart, load state
const state = await checkpoint.load('my-agent');
// → { lastProcessedQuoteId: 'q-123', balance: 450.00 }

// Track processed IDs to prevent duplicate work
await checkpoint.saveProcessedIds('my-agent', new Set(['evt-1', 'evt-2', 'evt-3']));
```

File writes use atomic temp-file + rename for crash safety (no external dependencies).

### Checkpoint Management

```javascript
// List all checkpoints
const checkpoints = await checkpoint.listCheckpoints();
// → ['0xAgent1', '0xAgent2']

// Delete a checkpoint
await checkpoint.deleteCheckpoint('0xAgent1');

// Save/load arbitrary checkpoint data
await checkpoint.saveCheckpoint('0xAgent1', 'cursor', { lastSeq: 456 });
const cursor = await checkpoint.loadCheckpoint('0xAgent1', 'cursor');
```

## Tick Optimizer

The tick optimizer wraps the agent runtime loop with performance optimizations:

```javascript
const optimizer = createTickOptimizer({ baseIntervalMs: 5000 });
const tick = optimizer.wrapTick(async () => {
    const items = await pollQueue();
    for (const item of items) await process(item);
    return items.length;  // items processed
});

setInterval(() => tick(), optimizer.getAdaptiveInterval());
```

### Features

- **Overlap prevention**: Skips tick if the previous one is still running
- **Adaptive polling**: Snaps back to base interval on activity, exponential backoff on idle (max 30s)
- **Duration metrics**: p50, p95, p99, min, max across all ticks
- **Warnings**: Alerts when a tick consumes > 80% of the interval

### Processed ID Tracker (LRU)

Prevents duplicate processing with a size-bounded LRU cache:

```javascript
const tracker = createProcessedIdTracker({ maxSize: 100000 });
if (!tracker.has('evt-123')) {
    await process('evt-123');
    tracker.add('evt-123');
}
```

## Further Reading

For dedicated documentation on advanced A2A features:

- **[Handshake Protocol](handshake.md)** — Pre-transaction capability negotiation between agents
- **[Saga Orchestration](sagas.md)** — Multi-step commerce flows with automatic rollback
- **[Agent Memory & Learning](agent-memory.md)** — Counterparty profiling, risk scoring, recommendations
- **[Cost Analytics & Forecasting](cost-analytics.md)** — Spend tracking, anomaly detection, budget forecasting
- **[Rules Engine](rules-engine.md)** — Declarative guardrails for autonomous agent decisions
