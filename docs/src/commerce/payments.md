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
| `evaluate_fraud_risk` | Score a transaction for fraud risk |
| `list_chargebacks` | List chargeback disputes |
| `create_chargeback` | Record a chargeback |
| `resolve_chargeback` | Mark a chargeback as resolved |
| `get_fraud_rules` | List active fraud detection rules |
| `create_fraud_rule` | Create a velocity or anomaly rule |

## Reconciliation

```javascript
// Get reconciliation report
const report = await toolkit.executeTool('reconcile_payments', {
    startDate: '2026-03-01',
    endDate: '2026-03-16'
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
| `capture_payment` | Capture authorized payment |
| `refund_payment` | Issue full/partial refund |
| `list_payments_by_order` | Payments for an order |
| `reconcile_payments` | Generate reconciliation report |
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
const report = await toolkit.executeTool('reconcile_payments', {
    startDate: '2026-03-01',
    endDate: '2026-03-16'
});
// → {
//     matched: 487,      // iCommerce records match external records
//     mismatched: 3,     // Amount or status differs
//     missingLocal: 1,   // In Stripe but not in iCommerce
//     missingExternal: 0, // In iCommerce but not in Stripe
//     details: [
//         { paymentId: 'pay-456', issue: 'amount_mismatch', local: 59.98, external: 59.97 }
//     ]
// }
```

For agent-to-agent payments, see [x402 Payment Protocol](../payments/x402.md).
