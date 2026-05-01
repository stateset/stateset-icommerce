# Returns & RMA

Return Merchandise Authorization (RMA) processing handles return requests, approvals, inspections, and refund issuance.

## Return Lifecycle

```
Requested → Approved → Received → Completed
    └─────→ Rejected
```

## Operations

### Create a Return

```javascript
const rma = commerce.returns.create({
    orderId: order.id,
    reason: 'Defective product',
    items: [
        { sku: 'WIDGET-001', quantity: 1, reason: 'Manufacturing defect' }
    ]
});
```

### Approve or Reject

```javascript
commerce.returns.approve(rma.id);
// or
commerce.returns.reject(rma.id, { reason: 'Outside return window' });
```

### Complete a Return

```javascript
// Mark as received and issue refund
commerce.returns.complete(rma.id);
```

## Policy Integration

Returns are a prime use case for the [policy engine](../policy/engine.md). Define rules in YAML:

```yaml
name: Return Policy
domain: returns
rules:
  - name: auto-approve-under-50
    conditions:
      - field: amount
        operator: less_than
        value: 50
      - field: days_since_purchase
        operator: less_than
        value: 30
    actions:
      - type: allow
        reason: "Return under $50 within 30-day window"

  - name: block-final-sale
    conditions:
      - field: product_tags
        operator: contains
        value: "final-sale"
    actions:
      - type: deny
        reason: "Final sale items cannot be returned"
        remediation: "Contact support for warranty claims"
```

When an agent evaluates a return, the policy engine returns a structured decision:

```json
{
    "allowed": true,
    "rule": "auto-approve-under-50",
    "reason": "Return under $50 within 30-day window"
}
```

Or a denial with actionable remediation:

```json
{
    "allowed": false,
    "rule": "block-final-sale",
    "reason": "Final sale items cannot be returned",
    "remediation": "Contact support for warranty claims"
}
```

## Return Reasons

Common return reason codes that agents can use:

| Reason | Description |
|--------|-------------|
| `defective` | Manufacturing defect |
| `wrong_item` | Incorrect item shipped |
| `not_as_described` | Product doesn't match listing |
| `changed_mind` | Customer changed their mind |
| `arrived_late` | Delivered past promised date |
| `damaged_in_transit` | Shipping damage |

## Automated Return Flow

Combine the policy engine with the returns API for fully automated processing:

```javascript
// Agent receives return request
const rma = await toolkit.executeTool('create_return', {
    orderId: 'ord-123',
    reason: 'defective',
    items: [{ sku: 'WIDGET-001', quantity: 1 }]
});

// Policy engine evaluates
const decision = await toolkit.executeTool('evaluate_policy', {
    domain: 'returns',
    context: {
        amount: 29.99,
        days_since_purchase: 15,
        reason: 'defective',
        product_tags: ['electronics']
    }
});

if (decision.allowed) {
    await toolkit.executeTool('approve_return', { returnId: rma.id });
    await toolkit.executeTool('refund_payment', { paymentId: payment.id });
    // Inventory adjusted when item received back
} else {
    // Structured denial with remediation
    console.log(decision.reason);       // "Outside return window"
    console.log(decision.remediation);  // "Contact support for warranty claims"
}
```

## Events

| Event | Trigger |
|-------|---------|
| `return.requested` | New return created |
| `return.approved` | Return approved |
| `return.rejected` | Return rejected |
| `return.completed` | Item received and refund issued |

## Heartbeat Alert

The heartbeat monitor detects pending returns:

```json
{
    "id": "pending-returns",
    "checker": "pending-returns",
    "intervalMs": 43200000,
    "enabled": true,
    "config": { "maxAgeDays": 7 }
}
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_returns` | List all returns (filter by status) |
| `get_return` | Get return details with items |
| `create_return` | Create a return request |
| `approve_return` | Approve a return |
| `reject_return` | Reject with reason |
