# Payments & Refunds

Payment processing covers captures, refunds, chargebacks, reconciliation, and fraud detection.

## Payment Lifecycle

```
Pending → Authorized → Captured → Settled
   └────→ Failed        └────→ Refunded (partial or full)
```

## Operations

### Create a Payment

```javascript
const payment = commerce.payments.create({
    orderId: order.id,
    amount: 59.98,
    currency: 'USD',
    method: 'card'
});
```

### Capture and Refund

```javascript
// Capture an authorized payment
commerce.payments.capture(payment.id);

// Full refund
commerce.payments.refund(payment.id);

// Partial refund
commerce.payments.refund(payment.id, { amount: 29.99 });
```

### List Payments

```javascript
// By order
const payments = commerce.payments.listByOrder(orderId);

// By status
const pending = commerce.payments.listByStatus('pending');
```

## Fraud Detection

MCP tools for fraud scoring and chargeback management:

| Tool | Description |
|------|-------------|
| `assess_order_fraud` | Score an order for fraud risk before fulfillment |
| `get_fraud_assessment` | Fetch the latest fraud assessment by ID |
| `list_fraud_signals` | Inspect captured fraud signals and rules |
| `create_fraud_rule` | Create a velocity or anomaly rule |
| `update_fraud_rule` | Tune an existing fraud rule |
| `review_flagged_order` | Resolve a flagged order after manual review |

## Reconciliation

```javascript
// Get reconciliation report
const report = await toolkit.executeTool('reconcile_payment_provider', {
    providerId: 'deterministic-mock',
    includeBalanced: false,
    limit: 100
});
```

## Gift Cards and Store Credits

```javascript
// Issue a gift card
const card = await toolkit.executeTool('create_gift_card', {
    amount: 50.00,
    currency: 'USD',
    recipientEmail: 'bob@example.com'
});

// Issue store credit
const credit = await toolkit.executeTool('create_store_credit', {
    customerId: customer.id,
    amount: 25.00,
    reason: 'Return refund'
});
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_payments` | List all payments |
| `get_payment` | Get payment details |
| `create_payment` | Create a payment |
| `complete_payment` | Mark a payment as completed |
| `create_refund` | Issue a refund against a payment |
| `create_payment_intent` | Start a provider-backed payment intent |
| `capture_payment_intent` | Capture all or part of a payment intent |
| `refund_payment_intent` | Refund a provider-backed payment intent |
| `reconcile_payment_provider` | Compare intents with settlement records |
| `create_gift_card` | Issue a gift card |
| `create_store_credit` | Issue store credit |

## Payment Failure Modes

| Scenario | Behavior |
|----------|----------|
| Card declined | Payment status → `failed`, structured error with decline reason |
| Network timeout during capture | Payment stays `authorized`, safe to retry capture |
| Partial refund on settled payment | New refund record created, original payment stays `settled` |
| Chargeback / dispute | Payment status → `disputed`, chargeback record created |
| Double-capture attempt | Idempotency prevents duplicate — returns existing capture |

### Retry Policy

Failed payments are safe to retry. The idempotency guard ensures that retrying with the same idempotency key returns the cached result without re-executing:

```javascript
// First attempt fails (network error)
try {
    await commerce.payments.create({ orderId, amount, idempotencyKey: 'pay-ord-123' });
} catch (e) {
    // Retry with same key — safe
    const payment = await commerce.payments.create({ orderId, amount, idempotencyKey: 'pay-ord-123' });
}
```

### Reconciliation Deep-Dive

```javascript
const report = await toolkit.executeTool('reconcile_payment_provider', {
    providerId: 'deterministic-mock',
    includeBalanced: false,
    limit: 100
});
// → {
//     providerId: 'deterministic-mock',
//     count: 3,
//     summary: {
//         pendingCount: 2,
//         overSettledCount: 0,
//         outstandingAmount: 59.98
//     },
//     reconciliation: [
//         { intentId: 'pi_123', reconciliationStatus: 'pending_settlement', outstandingAmount: 59.98 }
//     ]
// }
```

For agent-to-agent payments, see [x402 Payment Protocol](../payments/x402.md).
