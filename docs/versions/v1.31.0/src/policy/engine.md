# Policy Engine

The policy engine provides declarative safety guardrails for commerce operations. Rules are defined in YAML, evaluated at runtime, and produce explainable decisions that AI agents can reason about.

## Quick Start

Create a `policies/returns.yaml` file:

```yaml
name: Return Policy
domain: returns
rules:
  - name: auto-approve-small
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
        remediation: "Auto-approved per return policy"

  - name: require-review-large
    conditions:
      - field: amount
        operator: greater_than_or_equal
        value: 50
    actions:
      - type: require-approval
        reason: "Returns over $50 require manual review"
        remediation: "Submit for manager approval"

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

## Rule Evaluation

### Deny-Override Semantics

When multiple rules match, deny always wins:

1. If any matching rule has a `deny` action, the operation is denied
2. If any matching rule has a `require-approval` action, approval is required
3. Otherwise, the operation is allowed

### Explainable Decisions

Every policy decision includes:

- **Which rule matched** — the rule name and conditions
- **Why it matched** — per-condition breakdown with expected vs. actual values
- **What to do about it** — remediation guidance for the agent

```json
{
    "allowed": false,
    "rule": "block-final-sale",
    "reason": "Final sale items cannot be returned",
    "conditions": [
        {
            "field": "product_tags",
            "operator": "contains",
            "expected": "final-sale",
            "actual": ["final-sale", "clearance"],
            "matched": true
        }
    ],
    "remediation": "Contact support for warranty claims"
}
```

This structured response flows directly into the LLM's context window, enabling the agent to understand the denial and take appropriate action.

## Condition Operators

| Operator | Description |
|----------|-------------|
| `equals` | Exact match |
| `not_equals` | Not equal |
| `less_than` | Numeric less than |
| `less_than_or_equal` | Numeric less than or equal |
| `greater_than` | Numeric greater than |
| `greater_than_or_equal` | Numeric greater than or equal |
| `contains` | Array/string contains |
| `not_contains` | Array/string does not contain |
| `in` | Value is in a list |
| `not_in` | Value is not in a list |
| `matches` | Regex match |

## Action Types

| Type | Description |
|------|-------------|
| `allow` | Permit the operation |
| `deny` | Block the operation |
| `require-approval` | Require explicit approval |
| `transform` | Modify the operation parameters |

### Transform Actions

Transform actions modify the operation before execution:

```yaml
rules:
  - name: cap-discount
    conditions:
      - field: discount_percentage
        operator: greater_than
        value: 50
    actions:
      - type: transform
        field: discount_percentage
        value: 50
        reason: "Discount capped at 50% per policy"
```

## Dry Run

Evaluate a policy without executing the operation:

```javascript
const result = await toolkit.executeTool('evaluate_policy', {
    domain: 'returns',
    context: {
        amount: 75.00,
        days_since_purchase: 15,
        product_tags: ['electronics']
    },
    dryRun: true
});
```

## Hot Reload

The policy watcher monitors the policies directory for changes and reloads rules automatically:

```javascript
import { PolicyWatcher } from '@stateset/cli/policies/watcher';

const watcher = new PolicyWatcher('./policies');
watcher.start(); // Watches for file changes
```

## Configuration

In `.stateset/config.json`:

```json
{
    "policies": {
        "dir": "./policies",
        "autoLoad": true,
        "unknownDomainMode": "allow"
    }
}
```

| Setting | Description | Default |
|---------|-------------|---------|
| `dir` | Path to policy YAML files | `./policies` |
| `autoLoad` | Load policies on startup | `true` |
| `unknownDomainMode` | Behavior for domains without rules | `allow` |

## Policy Domains

Policies are scoped by domain. Each commerce operation is evaluated against rules in its domain:

| Domain | Governs | Example Rules |
|--------|---------|---------------|
| `orders` | Order creation, updates, cancellation | Max order value, customer verification |
| `returns` | Return authorization, refund amounts | Return window, final-sale exclusion |
| `payments` | Payment processing, refund issuance | Fraud gates, currency restrictions |
| `inventory` | Stock adjustments, reservations | Minimum stock thresholds, warehouse access |
| `shipping` | Shipping method selection, rates | Free shipping thresholds, zone restrictions |
| `subscriptions` | Plan creation, billing, cancellation | Trial limits, upgrade/downgrade rules |
| `promotions` | Discount application, coupon validation | Max discount percentage, stacking rules |

When no policy file exists for a domain, the `unknownDomainMode` setting applies (default: `allow`).

## Complete Policy Example

A comprehensive return policy with multiple rules:

```yaml
name: Return Policy
domain: returns
rules:
  - name: block-final-sale
    conditions:
      - field: product_tags
        operator: contains
        value: "final-sale"
    actions:
      - type: deny
        reason: "Final sale items cannot be returned"
        remediation: "Contact support for warranty claims"

  - name: auto-approve-small-recent
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

  - name: require-review-large
    conditions:
      - field: amount
        operator: greater_than_or_equal
        value: 50
      - field: days_since_purchase
        operator: less_than_or_equal
        value: 90
    actions:
      - type: require-approval
        reason: "Returns over $50 require manager approval"
        remediation: "Submit for manager approval via internal ticketing"

  - name: block-expired-window
    conditions:
      - field: days_since_purchase
        operator: greater_than
        value: 90
    actions:
      - type: deny
        reason: "Return window has expired (90 days)"
        remediation: "File a warranty claim if the product is defective"
```

## Writing Effective Policies

**Rule ordering matters.** Rules are evaluated top-to-bottom, but deny-override semantics mean a `deny` in any rule wins regardless of position.

**Be specific with remediation.** The remediation field tells the AI agent what to do next — suggest an alternative, escalate, or explain the denial to the customer.

**Use transforms sparingly.** Transform actions modify operation parameters before execution (e.g., capping a discount at 50%). They're powerful but can be surprising.

**Test before deploying.** Use `evaluate_policy` with `dryRun: true` to see what a rule would do before it goes live.

## Multi-Rule Interactions

When an operation matches multiple rules:

```
Rule A: allow   ─┐
Rule B: allow   ─┤──► Result: deny (deny-override)
Rule C: deny    ─┘

Rule A: allow   ─┐
Rule B: allow   ─┤──► Result: require-approval
Rule C: require ─┘

Rule A: allow   ─┐
Rule B: allow   ─┤──► Result: allow
Rule C: allow   ─┘
```

## Policy vs Rules Engine

iCommerce has two rule systems:

| Feature | [Policy Engine](engine.md) | [Rules Engine](../a2a/rules-engine.md) |
|---------|---------------|---------------|
| **Scope** | Commerce operations | A2A agent behavior |
| **Format** | YAML files | Programmatic API |
| **Evaluation** | Per-operation | Per-transaction |
| **Examples** | Return windows, fraud gates | Counterparty blacklists, escrow guards |
| **Hot reload** | File watcher | In-memory |

Use the Policy Engine for commerce business rules. Use the Rules Engine for A2A autonomous behavior.

## MCP Tools

| Tool | Description |
|------|-------------|
| `evaluate_policy` | Evaluate a policy for a domain/context |
| `list_policies` | List loaded policies |
| `reload_policies` | Force reload from disk |
| `validate_policy` | Check a YAML file for errors |
| `policy_dry_run` | Preview policy decision |
