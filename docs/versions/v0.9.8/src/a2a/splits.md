# Split Payments

Split payments distribute a single payment across multiple recipients. Useful for marketplace commissions, multi-party services, and platform fees.

## Split Types

| Type | Description |
|------|-------------|
| `percentage` | Each recipient gets a percentage of the total |
| `fixed` | Each recipient gets a fixed amount |

## Create a Split Payment

### Percentage Split

```javascript
const split = await toolkit.executeTool('a2a_create_split_payment', {
    sourcePaymentId: payment.id,
    splitType: 'percentage',
    recipients: [
        { agentId: 'seller-agent', share: 85, label: 'Seller proceeds' },
        { agentId: 'platform-agent', share: 10, label: 'Platform fee' },
        { agentId: 'referral-agent', share: 5, label: 'Referral commission' }
    ]
});
```

### Fixed Split

```javascript
const split = await toolkit.executeTool('a2a_create_split_payment', {
    sourcePaymentId: payment.id,
    splitType: 'fixed',
    recipients: [
        { agentId: 'seller-agent', amount: 425.00, label: 'Seller proceeds' },
        { agentId: 'platform-agent', amount: 50.00, label: 'Platform fee' },
        { agentId: 'referral-agent', amount: 25.00, label: 'Referral commission' }
    ]
});
```

## Rounding Drift Prevention

For percentage splits, sub-cent rounding can cause the sum of individual amounts to differ from the total. The split engine handles this by:

1. Calculating each recipient's share with full precision
2. Rounding each share to 2 decimal places
3. Assigning any remaining drift (positive or negative) to the first recipient

This ensures the sum of disbursements always equals the source payment amount exactly.

## Platform Fees

A common pattern is to deduct a platform fee before splitting:

```javascript
const split = await toolkit.executeTool('a2a_create_split_payment', {
    sourcePaymentId: payment.id,
    splitType: 'percentage',
    platformFee: { agentId: 'platform', percentage: 2.5 },
    recipients: [
        { agentId: 'seller', share: 100, label: 'Net proceeds' }
    ]
});
// Platform gets 2.5%, seller gets 97.5%
```

## Rounding Proof

For a $100 payment split 3 ways at 33.33% each:

```
Recipient A: $100 × 0.3333 = $33.33
Recipient B: $100 × 0.3333 = $33.33
Recipient C: $100 × 0.3333 = $33.33
                     Sum = $99.99  (1 cent drift)

→ Drift of $0.01 assigned to Recipient A
→ Final: A=$33.34, B=$33.33, C=$33.33 = $100.00 exactly
```

This guarantees the sum of disbursements always equals the source amount.

## Error Recovery

If a split disbursement partially fails (e.g., 2 of 3 recipients paid, third fails):

- Completed disbursements are recorded
- Failed disbursements are retried with backoff
- If retry exhaustion occurs, the failed portion is returned to the source
- The split status shows which recipients were paid and which failed

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_create_split_payment` | Create a split payment |
| `a2a_get_split_payment` | Get split details with per-recipient status |
| `a2a_list_split_payments` | List splits (filter by status, source) |
| `a2a_execute_split` | Disburse to all recipients |
