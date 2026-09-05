# Budget Governance

Budget governance prevents AI agents from overspending. Every agent has configurable daily spending caps that are enforced at the protocol level.

## Setting Budgets

```javascript
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'research-agent',
    dailyLimit: 5.00,
    monthlyLimit: 100.00,
    perTransactionLimit: 1.00
});
```

## Checking Budget Status

```javascript
const budget = await toolkit.executeTool('x402_budget_status', {});
// → {
//     budget: {
//         spentToday: 2.34,
//         dailyBudget: 5.00,
//         perCallLimit: 1.00
//     }
// }
```

## Budget Enforcement

When an agent attempts to spend beyond its budget:

1. The x402 client checks the remaining daily budget
2. If the payment would exceed the limit, a `BudgetExceededError` is thrown
3. The error includes the remaining budget and the attempted amount
4. The agent can reason about this error and decide to wait or request a budget increase

```json
{
    "error": "BudgetExceededError",
    "dailyLimit": 5.00,
    "spent": 4.80,
    "attempted": 0.50,
    "remaining": 0.20,
    "message": "Payment of $0.50 exceeds remaining daily budget of $0.20"
}
```

## Budget Reset

Budgets reset daily at midnight UTC. The reset time is included in the budget status response.

## Multi-Agent Budgets

In a multi-agent system, each agent has its own independent budget:

```javascript
// Set different limits for different agents
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'research-agent', dailyLimit: 5.00, monthlyLimit: 100.00, perTransactionLimit: 1.00
});
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'fulfillment-agent', dailyLimit: 100.00, monthlyLimit: 1000.00, perTransactionLimit: 25.00
});
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'analytics-agent', dailyLimit: 2.00, monthlyLimit: 50.00, perTransactionLimit: 0.50
});
```

## Spending Limits (Circuit Breaker)

Beyond daily budget caps, agents have per-transaction and monthly limits:

```javascript
await toolkit.executeTool('agent_set_spending_limits', {
    agentId: 'research-agent',
    dailyLimit: 5.00,
    monthlyLimit: 100.00,
    perTransactionLimit: 1.00
});
```

```javascript
const summary = await toolkit.executeTool('agent_get_spending_summary', {
    agentId: 'research-agent'
});
// → {
//     dailySpend: 2.34,
//     monthlySpend: 45.00,
//     limits: { daily: 5.00, monthly: 100.00, perTransaction: 1.00 },
//     remaining: { daily: 2.66, monthly: 55.00 }
// }
```

If any limit is exceeded, the agent's circuit breaker trips and all payments halt until the reset window or manual override.

## Policy Integration

Budget limits can also be enforced via the policy engine for more complex rules:

```yaml
name: Budget Policy
domain: x402
rules:
  - name: weekend-reduced-budget
    conditions:
      - field: day_of_week
        operator: in
        value: [saturday, sunday]
    actions:
      - type: transform
        field: dailyLimit
        value: 1.00
        reason: "Reduced weekend budget"

  - name: block-high-value-without-escrow
    conditions:
      - field: amount
        operator: greater_than
        value: 100
      - field: useEscrow
        operator: equals
        value: false
    actions:
      - type: deny
        reason: "Transactions over $100 require escrow"
        remediation: "Add useEscrow: true to the payment request"
```

See also: [Circuit Breaker / Kill Switch](../advanced/compliance.md#circuit-breaker--kill-switch) for emergency safety controls.
