# Saga Orchestration

The saga framework executes multi-step commerce flows with automatic rollback on failure. Each saga is a sequence of steps with `execute` and `compensate` pairs — if any step fails, previously completed steps are compensated in reverse order.

## Why Sagas?

Real-world A2A transactions are rarely a single operation. A purchase involves:

1. Request a quote
2. Negotiate terms
3. Fund escrow
4. Execute payment
5. Confirm fulfillment
6. Release escrow
7. Rate counterparty

If step 5 fails (fulfillment not delivered), the system must reverse steps 4, 3, and 2 — refunding the escrow, cancelling the payment, and withdrawing the quote. Sagas automate this.

## Saga Lifecycle

```
pending → running → completed
                  ↘ compensating → compensated
                                 ↘ failed (compensation also failed)
```

| State | Description |
|-------|-------------|
| `pending` | Saga created, not yet started |
| `running` | Steps executing in order |
| `completed` | All steps succeeded |
| `compensating` | A step failed, compensating in reverse |
| `compensated` | All compensations succeeded |
| `failed` | Compensation also failed (requires manual intervention) |
| `cancelled` | Saga cancelled before completion |

## Step Lifecycle

Each step within a saga has its own state:

| State | Description |
|-------|-------------|
| `pending` | Not yet executed |
| `running` | Currently executing |
| `completed` | Succeeded |
| `failed` | Execution failed |
| `compensating` | Compensation in progress |
| `compensated` | Compensation succeeded |
| `compensation_failed` | Compensation also failed |
| `skipped` | Skipped (idempotent re-execution) |
| `timed_out` | Step exceeded its timeout |

## Creating a Saga

```javascript
import { createSagaOrchestrator } from './saga.js';

const orchestrator = createSagaOrchestrator(store, services);

const result = await orchestrator.execute({
    name: 'custom-purchase',
    steps: [
        {
            name: 'request_quote',
            execute: async (ctx) => {
                const quote = await a2a.requestQuote(ctx.seller, ctx.items);
                return { quoteId: quote.id, price: quote.price };
            },
            compensate: async (ctx, stepResult) => {
                await a2a.withdrawQuote(stepResult.quoteId);
            },
            timeoutMs: 30000,
            retries: 2,
        },
        {
            name: 'fund_escrow',
            execute: async (ctx, prevResults) => {
                const escrow = await a2a.createEscrow({
                    amount: prevResults.request_quote.price,
                    seller: ctx.seller,
                });
                return { escrowId: escrow.id };
            },
            compensate: async (ctx, stepResult) => {
                await a2a.refundEscrow(stepResult.escrowId);
            },
            timeoutMs: 15000,
        },
        {
            name: 'execute_payment',
            execute: async (ctx, prevResults) => {
                const payment = await a2a.pay({
                    to: ctx.seller,
                    amount: prevResults.request_quote.price,
                    escrowId: prevResults.fund_escrow.escrowId,
                });
                return { paymentId: payment.id };
            },
            compensate: async (ctx, stepResult) => {
                await a2a.refundPayment(stepResult.paymentId);
            },
        },
    ]
}, {
    seller: '0xSeller',
    items: [{ sku: 'WIDGET-001', quantity: 10 }],
});
```

## Pre-Built Saga Templates

Three common commerce flows are available as templates:

### PURCHASE_SAGA

End-to-end purchase: quote → negotiate → escrow → payment → fulfillment → rate.

```javascript
import { PURCHASE_SAGA } from './saga.js';

const result = await orchestrator.execute(PURCHASE_SAGA, {
    buyerAddress: '0xBuyer',
    sellerAddress: '0xSeller',
    amount: 100,
    items: [{ sku: 'WIDGET-001', quantity: 5 }],
});
```

### SUBSCRIPTION_SAGA

Subscription setup: create plan → initial payment → activate subscription → confirm.

```javascript
import { SUBSCRIPTION_SAGA } from './saga.js';

const result = await orchestrator.execute(SUBSCRIPTION_SAGA, {
    subscriberAddress: '0xSubscriber',
    providerAddress: '0xProvider',
    planId: 'pro-monthly',
    amount: 99,
    interval: 'monthly',
});
```

### RFQ_SAGA

Competitive procurement: broadcast RFQ → collect quotes → evaluate → award → pay winner.

```javascript
import { RFQ_SAGA } from './saga.js';

const result = await orchestrator.execute(RFQ_SAGA, {
    buyerAddress: '0xBuyer',
    suppliers: ['0xSupplier-A', '0xSupplier-B', '0xSupplier-C'],
    items: [{ sku: 'COMPONENT-X', quantity: 1000 }],
    scoringWeights: { price: 0.5, reputation: 0.3, speed: 0.2 },
});
```

## Step Timeouts and Retries

Each step can configure:

| Option | Default | Description |
|--------|---------|-------------|
| `timeoutMs` | 30,000 | Maximum execution time before timeout |
| `retries` | 0 | Number of retry attempts on failure |

Retries use exponential backoff. If all retries are exhausted, the step fails and compensation begins.

## Idempotency

Sagas are idempotent by `sagaId`. Re-executing the same saga ID skips completed steps:

```javascript
// First execution: runs all steps
await orchestrator.execute(saga, context, { sagaId: 'purchase-001' });

// Re-execution: skips completed steps, resumes from last failure
await orchestrator.execute(saga, context, { sagaId: 'purchase-001' });
```

This enables safe crash recovery — restart the saga and it picks up where it left off.

## Compensation (Rollback)

When a step fails, the orchestrator compensates all previously completed steps in **reverse order**:

```
Step 1: request_quote  ✅ completed
Step 2: fund_escrow    ✅ completed
Step 3: execute_payment ❌ failed
                         ↓
         compensate fund_escrow    ✅ compensated (escrow refunded)
         compensate request_quote  ✅ compensated (quote withdrawn)
```

If a compensation step also fails, the saga enters the `failed` state — manual intervention is required.

## Events

The orchestrator emits events at each lifecycle point:

| Event | Data |
|-------|------|
| `saga:started` | `{ sagaId, name }` |
| `saga:step:started` | `{ sagaId, stepName }` |
| `saga:step:completed` | `{ sagaId, stepName, result }` |
| `saga:step:failed` | `{ sagaId, stepName, error }` |
| `saga:step:compensated` | `{ sagaId, stepName }` |
| `saga:completed` | `{ sagaId, steps }` |
| `saga:compensated` | `{ sagaId }` |
| `saga:failed` | `{ sagaId, error }` |

## Querying Saga Status

```javascript
const status = orchestrator.getStatus(sagaId);
// → {
//     sagaId: 'purchase-001',
//     name: 'PURCHASE_SAGA',
//     status: 'completed',
//     steps: [
//         { name: 'request_quote', status: 'completed', durationMs: 1200 },
//         { name: 'fund_escrow', status: 'completed', durationMs: 800 },
//         { name: 'execute_payment', status: 'completed', durationMs: 450 },
//     ],
//     totalDurationMs: 2450,
//     startedAt: '2026-03-16T10:30:45Z',
//     completedAt: '2026-03-16T10:30:47Z',
// }

// List all sagas
const sagas = orchestrator.listSagas({ status: 'completed', limit: 20 });

// Cancel a running saga
orchestrator.cancelSaga(sagaId);
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_saga_execute` | Execute a saga (custom or template) |
| `a2a_saga_status` | Get saga execution status |
| `a2a_saga_list` | List sagas with optional status filter |
| `a2a_saga_cancel` | Cancel a running saga |
