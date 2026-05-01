# Rules Engine

The A2A rules engine provides declarative "if X then Y" guardrails for autonomous agent decisions. Agents register rules with conditions and actions; the engine evaluates context objects against all matching rules and returns an aggregate decision.

This is distinct from the [Policy Engine](../policy/engine.md), which governs commerce operations (orders, returns, refunds). The rules engine governs **A2A behavior** — which counterparties to transact with, when to require escrow, and how to handle spending limits.

## Defining Rules

```javascript
const engine = createRulesEngine();

engine.addRule({
    name: 'High-value guard',
    description: 'Require escrow for transactions over $1,000',
    agentAddress: '0xAgent1',
    condition: { field: 'amount', operator: 'gt', value: 1000 },
    action: { type: 'require_escrow', params: { reason: 'high value' } },
    priority: 90,
    enabled: true,
    tags: ['financial', 'safety'],
});
```

## Condition Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equal | `{ field: 'status', operator: 'eq', value: 'active' }` |
| `neq` | Not equal | `{ field: 'type', operator: 'neq', value: 'test' }` |
| `gt` | Greater than | `{ field: 'amount', operator: 'gt', value: 1000 }` |
| `gte` | Greater or equal | `{ field: 'score', operator: 'gte', value: 3.0 }` |
| `lt` | Less than | `{ field: 'reputation', operator: 'lt', value: 2.5 }` |
| `lte` | Less or equal | `{ field: 'dailySpend', operator: 'lte', value: 500 }` |
| `in` | In array | `{ field: 'network', operator: 'in', value: ['base', 'set_chain'] }` |
| `not_in` | Not in array | `{ field: 'asset', operator: 'not_in', value: ['DAI'] }` |
| `contains` | String contains | `{ field: 'name', operator: 'contains', value: 'test' }` |
| `matches` | Regex match | `{ field: 'agentId', operator: 'matches', value: '^prod-' }` |

## Compound Conditions

Combine multiple conditions with `all` (AND) or `any` (OR):

```javascript
engine.addRule({
    name: 'New + high-value guard',
    agentAddress: '0xAgent1',
    condition: {
        all: [
            { field: 'amount', operator: 'gt', value: 500 },
            { field: 'counterparty.interactions', operator: 'lt', value: 5 },
        ]
    },
    action: { type: 'require_escrow', params: { reason: 'new counterparty, high value' } },
    priority: 85,
    enabled: true,
});
```

## Action Types

| Action Type | Effect |
|-------------|--------|
| `allow` | Permit the transaction |
| `block` | Block the transaction (first block wins) |
| `require_escrow` | Require escrow for this transaction |
| `reduce_amount` | Suggest a lower amount |
| `notify` | Emit a notification/warning |
| `log` | Log for audit without affecting the decision |

## Evaluation

Pass a context object and the engine evaluates all matching rules in priority order:

```javascript
const result = engine.evaluate({
    amount: 5000,
    counterparty: {
        address: '0xNew',
        interactions: 2,
        reputation: 3.8,
    },
    network: 'base',
    asset: 'USDC',
});
// → {
//     allowed: true,              // No 'block' action fired
//     appliedRules: [
//         { name: 'High-value guard', action: 'require_escrow' },
//         { name: 'New + high-value guard', action: 'require_escrow' },
//     ],
//     explanation: 'require_escrow: high value; new counterparty, high value',
//     blocked: false,
// }
```

A `block` action immediately stops evaluation:

```javascript
// → {
//     allowed: false,
//     blocked: true,
//     blockedBy: 'Blacklisted counterparty',
//     explanation: 'Blocked: dispute rate exceeds 30%',
// }
```

## Priority System

Rules are evaluated in priority order (1–100, higher first). The first `block` action wins and halts evaluation. All non-blocking actions accumulate.

| Priority Range | Typical Use |
|---------------|-------------|
| 90–100 | Critical safety (blacklists, hard blocks) |
| 70–89 | Financial guardrails (amount limits, escrow requirements) |
| 50–69 | Business logic (network preferences, asset routing) |
| 1–49 | Advisory (logging, notifications) |

## Built-in Templates

The engine ships with 5 pre-built templates for common guardrails:

### HIGH_VALUE_GUARD

Require escrow for transactions above a threshold.

```javascript
engine.addFromTemplate('HIGH_VALUE_GUARD', {
    agentAddress: '0xAgent1',
    params: { threshold: 1000 },
});
```

### LOW_REPUTATION_FILTER

Block counterparties with reputation below a minimum.

```javascript
engine.addFromTemplate('LOW_REPUTATION_FILTER', {
    agentAddress: '0xAgent1',
    params: { minReputation: 3.0 },
});
```

### DAILY_SPEND_LIMIT

Block transactions that would exceed a daily spend cap.

```javascript
engine.addFromTemplate('DAILY_SPEND_LIMIT', {
    agentAddress: '0xAgent1',
    params: { dailyLimit: 5000 },
});
```

### FIRST_TIME_BUYER_ESCROW

Require escrow for first-time counterparties.

```javascript
engine.addFromTemplate('FIRST_TIME_BUYER_ESCROW', {
    agentAddress: '0xAgent1',
    params: { interactionThreshold: 3 },  // require escrow for < 3 interactions
});
```

### DISPUTE_RATE_BLACKLIST

Block counterparties with dispute rates above a threshold.

```javascript
engine.addFromTemplate('DISPUTE_RATE_BLACKLIST', {
    agentAddress: '0xAgent1',
    params: { maxDisputeRate: 0.30 },
});
```

## Testing Rules

Dry-run a rule against a context without side effects:

```javascript
const testResult = engine.testRule('rule-id-123', {
    amount: 5000,
    counterparty: { reputation: 2.0 },
});
// → { matched: true, action: 'block', reason: 'Reputation too low' }
```

## Rule Management

```javascript
// List all rules for an agent
const rules = engine.listRules({ agentAddress: '0xAgent1' });

// Enable/disable a rule
engine.enableRule('rule-id-123');
engine.disableRule('rule-id-123');

// Remove a rule
engine.removeRule('rule-id-123');

// Get a specific rule
const rule = engine.getRule('rule-id-123');
```

## Audit Log

Every evaluation is logged for compliance and debugging:

```javascript
const log = engine.getAuditLog({ agentAddress: '0xAgent1', limit: 50 });
// → [
//     {
//         timestamp: '2026-03-16T10:30:45Z',
//         context: { amount: 5000, ... },
//         result: { allowed: true, appliedRules: [...] },
//     },
//     ...
// ]
```

The audit log is bounded (default: 1,000 entries, FIFO eviction).

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_rules_add` | Add a new rule |
| `a2a_rules_add_template` | Add a rule from a built-in template |
| `a2a_rules_remove` | Remove a rule |
| `a2a_rules_list` | List rules for an agent |
| `a2a_rules_enable` | Enable a rule |
| `a2a_rules_disable` | Disable a rule |
| `a2a_rules_evaluate` | Evaluate context against all rules |
| `a2a_rules_test` | Dry-run a single rule |
| `a2a_rules_audit` | Get evaluation audit log |
