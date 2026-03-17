# Escrow & Conditional Payments

Escrow holds funds in a neutral account with programmatic release conditions. Funds are released only when specified conditions are met.

## Condition Types

| Condition | Description |
|-----------|-------------|
| `seller_fulfilled` | Seller confirms delivery of goods/services |
| `buyer_confirmed` | Buyer confirms satisfaction |
| `time_lock` | Automatic release after a specified time |
| `milestone` | Release upon completion of specific milestones |

## Create an Escrow

```javascript
const escrow = await toolkit.executeTool('a2a_create_escrow', {
    payerId: 'agent-buyer-001',
    payeeId: 'agent-seller-002',
    amount: 500.00,
    currency: 'USD',
    conditions: [
        { type: 'seller_fulfilled', description: 'Data feed delivered and accessible' },
        { type: 'buyer_confirmed', description: 'Buyer verifies data quality' }
    ],
    expiresAt: '2026-04-16T00:00:00Z'
});
```

## Fund an Escrow

```javascript
await toolkit.executeTool('a2a_fund_escrow', {
    escrowId: escrow.id,
    paymentIntentId: intent.id
});
```

## Fulfill Conditions

```javascript
// Seller marks their condition as fulfilled
await toolkit.executeTool('a2a_fulfill_escrow_condition', {
    escrowId: escrow.id,
    conditionType: 'seller_fulfilled',
    evidence: { deliveryProof: 'https://...' }
});

// Buyer confirms
await toolkit.executeTool('a2a_fulfill_escrow_condition', {
    escrowId: escrow.id,
    conditionType: 'buyer_confirmed'
});
```

## Release Funds

When all conditions are met, funds are released to the payee:

```javascript
await toolkit.executeTool('a2a_release_escrow', {
    escrowId: escrow.id
});
```

## Escrow States

```
Created → Funded → Conditions Met → Released
                 → Expired → Refunded
                 → Disputed → (see Disputes)
```

## Milestone Escrow

For larger projects, split the escrow into milestones with partial releases:

```javascript
const escrow = await toolkit.executeTool('a2a_create_escrow', {
    payerId: 'agent-buyer-001',
    payeeId: 'agent-seller-002',
    amount: 10000.00,
    currency: 'USD',
    conditions: [
        { type: 'milestone', description: 'Phase 1 complete', releaseAmount: 3000.00 },
        { type: 'milestone', description: 'Phase 2 complete', releaseAmount: 3000.00 },
        { type: 'milestone', description: 'Final delivery', releaseAmount: 4000.00 }
    ]
});
```

## Dispute Flow

If a party disagrees about condition fulfillment, they can open a dispute:

```
Created → Funded → Active → Disputed → Under Review → Resolved
                          → Conditions Met → Released
                          → Expired → Refunded
```

```javascript
// Buyer disputes: "Service was not delivered"
await toolkit.executeTool('a2a_open_dispute', {
    transactionId: escrow.id,
    filedBy: 'buyer-agent',
    reason: 'non_delivery',
    evidence: [{ type: 'log', description: 'API returned 503 for 7 days', hash: '...' }]
});
```

The [dispute resolver](../guides/autonomous-engine.md) evaluates evidence and applies arbitration rules. See [Disputes & Resolution](disputes.md) for the full dispute lifecycle.

## Timeout / Expiration

Escrows have an `expiresAt` timestamp. If no conditions are fulfilled by expiration:

1. Escrow status moves to `expired`
2. Funds are automatically refunded to the payer
3. Both parties are notified

Default expiration: 90 days. Configurable per escrow.

## Error Handling

| Error | Cause | Resolution |
|-------|-------|------------|
| `EscrowNotFoundError` | Invalid escrow ID | Check `a2a_list_escrows` |
| `InsufficientFundsError` | Payment intent doesn't cover amount | Fund with sufficient amount |
| `ConditionAlreadyFulfilledError` | Condition already met | No action needed |
| `EscrowExpiredError` | Past expiration date | Funds auto-refunded |
| `EscrowDisputedError` | Cannot release during dispute | Wait for dispute resolution |

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_create_escrow` | Create an escrow with conditions |
| `a2a_fund_escrow` | Deposit funds into escrow |
| `a2a_fulfill_escrow_condition` | Mark a condition as met |
| `a2a_release_escrow` | Release funds to payee |
| `a2a_refund_escrow` | Refund to payer |
| `a2a_get_escrow` | Get escrow details with condition status |
| `a2a_list_escrows` | List escrows (filter by status, agent, date) |
