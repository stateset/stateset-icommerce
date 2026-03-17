# Operations

StateSet iCommerce standardizes core operational workflows across all language bindings. Every operation follows the same pattern: validate inputs, check policy, execute (or preview), emit event.

## Order-to-Cash

The primary revenue flow:

```
Create Order → Reserve Inventory → Capture Payment → Process → Ship → Deliver
```

1. `create_order` — validates customer, items, and pricing
2. `reserve_inventory` — holds stock for each line item (prevents overselling)
3. `create_payment` — initiates payment capture
4. `update_order_status("processing")` — moves to processing
5. `create_shipment` — creates shipment with carrier and tracking
6. `ship_order` — finalizes shipment, releases inventory holds

Each step emits a commerce event (`order.created`, `inventory.reserved`, `payment.captured`, `order.shipped`). Events flow to webhooks, SSE streams, and the VES sync layer.

## Procure-to-Pay

The supply chain flow:

```
Detect Low Stock → Find Suppliers → Create PO → Approve → Receive → Adjust Inventory
```

1. Heartbeat detects low stock via `low-stock` checker
2. Query supplier registry
3. `create_purchase_order` — with items, quantities, costs
4. `approve_purchase_order` — policy evaluation for spending limits
5. `receive_purchase_order` — record received quantity
6. `adjust_inventory` — add received stock with PO reference

## Return-to-Refund

The reverse logistics flow:

```
Customer Returns → Policy Check → Approve RMA → Receive → Inspect → Refund
```

1. `create_return` — captures reason, items, and amount
2. `evaluate_policy` — checks return window, product eligibility
3. `approve_return` or `reject_return` — based on policy decision
4. Receive returned goods, inspect quality
5. `refund_payment` — full or partial refund
6. `adjust_inventory` — restock if item passes inspection

## Subscription Lifecycle

```
Create Plan → Subscribe → Bill → Renew → (Pause/Cancel)
```

1. `create_subscription_plan` — define pricing and interval
2. `subscribe_customer` — create subscription with optional trial
3. Billing executor auto-charges at each interval
4. On payment failure: move to `past_due`, retry with backoff
5. `pause_subscription` / `resume_subscription` / `cancel_subscription`

## Multi-Currency

All monetary operations support 150+ currencies:

```javascript
// Convert between currencies
const converted = commerce.currency.convert(100.00, 'USD', 'EUR');

// Get current exchange rates
const rates = commerce.currency.getRates('USD');
```

## Audit Trail

Every operation produces a signed event (when Tier 2+ is configured):

1. Event payload is JSON-canonicalized (RFC 8785)
2. Hash with domain separation (`VES_EVENTSIG_V1`)
3. Sign with agent's Ed25519 key
4. Store in event log
5. Include in next Merkle tree batch

See [VES v1.0](../security/ves.md) for cryptographic details.

## Workflow Composition

Agents can compose workflows from individual operations:

```javascript
const plan = await toolkit.executePlan({
    dryRun: true,  // Preview the entire workflow
    steps: [
        { tool: 'create_order', params: { customerId, items } },
        { tool: 'reserve_inventory', params: { sku: 'WIDGET-001', quantity: 2 } },
        { tool: 'create_payment', params: { amount: 59.98 } },
        { tool: 'ship_order', params: { carrier: 'FedEx' } }
    ]
});

// Each step shows what would change without executing
console.log(plan.steps[0].preview); // Order preview
console.log(plan.steps[1].preview); // Inventory impact
```

## Idempotency

All write operations support idempotency keys to prevent duplicate execution:

```javascript
await toolkit.executeTool('create_order', {
    customerId: 'cust-123',
    items: [...],
    idempotencyKey: 'order-req-abc-123',
});

// Retrying with the same key returns the original result
await toolkit.executeTool('create_order', {
    customerId: 'cust-123',
    items: [...],
    idempotencyKey: 'order-req-abc-123',
});
// → returns same orderId, no duplicate created
```

## Error Recovery

When an operation fails mid-workflow, use these patterns:

### Retry with Backoff

```javascript
// The autonomous engine retries transient failures automatically:
// Attempt 1: immediate
// Attempt 2: 1 second delay
// Attempt 3: 2 second delay
// Attempt 4: 4 second delay (exponential backoff)
```

### Saga Compensation

For multi-step workflows, use [saga orchestration](../a2a/sagas.md) with automatic rollback:

```
Step 1: reserve_inventory  ✅
Step 2: create_payment     ❌ failed
         ↓ compensation
Step 1: release_inventory  ✅ rolled back
```

### Dead Letter Queue

Events that fail delivery after all retries are stored in a dead letter queue for manual inspection:

```javascript
const failed = await toolkit.executeTool('a2a_notification_dlq', {});
// → [{ eventId: '...', error: 'Connection refused', attempts: 5, lastAttempt: '...' }]
```

## Concurrency Control

### Inventory Reservations

Inventory uses optimistic locking to prevent overselling:

```
Agent A: reserve 5 units of WIDGET-001 (stock: 10) → ✅ reserved (stock: 5)
Agent B: reserve 8 units of WIDGET-001 (stock: 5)  → ❌ insufficient stock
```

### Order State Transitions

State machine transitions are atomic. Invalid transitions are rejected:

```
order.status = 'pending'  → update to 'processing'  ✅
order.status = 'pending'  → update to 'delivered'    ❌ invalid transition
order.status = 'shipped'  → update to 'pending'      ❌ invalid transition
```

## Event Sourcing

Every operation produces a structured commerce event:

```javascript
{
    type: 'order.created',
    entityType: 'order',
    entityId: 'ord-123',
    payload: { customerId: 'cust-123', total: 59.98, items: [...] },
    timestamp: '2026-03-17T10:30:45Z',
}
```

Events are:
- **Signed** with Ed25519 (when Tier 2+ is configured)
- **Sequenced** with gap-free ordering by the Sequencer
- **Anchored** on-chain via SET Chain (Tier 3)
- **Replayable** to reconstruct point-in-time state

## Batch Operations

For high-volume scenarios, use batch tools:

```javascript
// Batch payment processing
const result = await toolkit.executeTool('a2a_batch_pay', {
    payments: [
        { to: '0xAgent1', amount: 10, asset: 'USDC' },
        { to: '0xAgent2', amount: 20, asset: 'USDC' },
        { to: '0xAgent3', amount: 15, asset: 'USDC' },
    ],
    concurrency: 5,
});
// → { succeeded: 3, failed: 0 }
```

## Further Reading

- [Core Commerce](../commerce/domain-model.md) — Detailed domain operations
- [B2B Operations](../commerce/b2b-operations.md) — Supplier and invoice workflows
- [Saga Orchestration](../a2a/sagas.md) — Multi-step flows with rollback
- [Autonomous Engine](autonomous-engine.md) — Self-governing operations
