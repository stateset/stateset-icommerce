# Case Studies

Three real-world scenarios that demonstrate how autonomous agents use iCommerce in production.

## 1. Autonomous Supply Chain Procurement

An **Inventory Agent** monitors stock levels via the heartbeat system and detects that Widget-A has fallen below its reorder threshold. Without human intervention, it:

1. Queries the supplier registry and identifies three qualified vendor agents
2. Issues an `a2a_request_quote` to each vendor with the required SKU, quantity, and delivery window
3. Receives competing quotes (the RFQ protocol caps negotiation at 5 rounds to prevent infinite loops)
4. Evaluates quotes against procurement policy rules:
   - Price must be below the configured ceiling
   - Lead time must fit the delivery window
   - Supplier reputation must be ≥ 3.5 across all dimensions
5. Accepts the best quote via `a2a_accept_quote`, which creates a purchase order and an x402 payment intent
6. Funds are held in escrow with a `seller_fulfilled` condition — released only when the vendor agent confirms shipment
7. Upon delivery confirmation, inventory is automatically adjusted and the VES sync system propagates the event to all subscribed agents

```javascript
// Policy that governs this flow:
// policies/procurement.yaml
// rules:
//   - name: price-ceiling
//     conditions:
//       - field: unit_price
//         operator: less_than
//         value: 15.00
//     actions:
//       - type: allow
//
//   - name: reputation-gate
//     conditions:
//       - field: supplier_reputation
//         operator: less_than
//         value: 3.5
//     actions:
//       - type: deny
//         reason: "Supplier reputation below minimum threshold"
```

**Total human involvement: zero.** The entire flow executes within the policy guardrails set by the operations team and is fully auditable through cryptographically signed event logs.

## 2. Micro-Payment API Economy

A **Research Agent** needs real-time pricing data from a **Market Data Agent** that charges $0.02 per API call. The interaction is:

1. The Research Agent discovers the Market Data Agent via its ERC-8004 Agent Card, which declares capabilities and pricing
2. It calls `x402Fetch()`, which automatically attaches a signed payment header to each HTTP request
3. The Market Data Agent verifies the payment signature, serves the data, and returns a receipt
4. Budget governance caps the Research Agent at $5/day — if the budget is exhausted, a `BudgetExceededError` halts further requests rather than silently overspending
5. At end-of-day, the x402 sequencer batch-settles all accumulated micro-intents on-chain in a single transaction

```javascript
import { x402Fetch } from '@stateset/cli/x402';

// Each call costs $0.02 — budget governance enforces the $5/day cap
const data = await x402Fetch('https://marketdata.agent/api/v1/prices', {
    agent: researchAgent,
    maxAmount: 0.02,
    currency: 'USD'
});

// If budget exceeded:
// → BudgetExceededError {
//     dailyLimit: 5.00, spent: 4.98, attempted: 0.02, remaining: 0.02
//   }
```

This pattern enables an ecosystem where agents pay agents for services at machine speed, with sub-cent granularity and cryptographic accountability.

## 3. End-to-End Order Fulfillment

A **Customer Service Agent** receives a natural language order request via the messaging gateway. It:

1. Creates a cart, applies a promotional discount (validated by the policy engine), and calculates tax
2. Processes payment via x402, receiving a signed receipt
3. The **Fulfillment Agent** picks up the `order.created` event via SSE streaming, reserves inventory, and creates a shipment
4. Tracking events propagate in real-time to the customer via webhook notification
5. If the customer initiates a return, the **Returns Agent** evaluates the return policy (window, condition, reason), creates an RMA, and issues a refund — all within policy-defined guardrails

```
Customer Service Agent         Fulfillment Agent           Returns Agent
        │                             │                          │
        │── create_cart ──►           │                          │
        │── add_cart_item ──►         │                          │
        │── apply_cart_discount ──►   │                          │
        │── complete_checkout ──►     │                          │
        │                             │                          │
        │         order.created ─────►│                          │
        │                             │── reserve_inventory ──►  │
        │                             │── create_shipment ──►    │
        │                             │── ship_order ──►         │
        │                             │                          │
        │◄── order.shipped ──────────│                          │
        │                             │                          │
        │         (customer returns)  │                          │
        │── create_return ──────────────────────────────────────►│
        │                             │         evaluate_policy ─┤
        │                             │         approve_return ──┤
        │◄── return.approved ────────────────────────────────────│
```

Each agent operates with its own tool permissions, budget limits, and policy constraints, yet they collaborate seamlessly through the shared event log and A2A protocol.

## Key Takeaways

| Property | How It Manifests |
|----------|-----------------|
| **Deterministic** | Same quote, same policy, same result — every time |
| **Preview-first** | Agents see what would change before committing |
| **Policy-governed** | Spending caps, price ceilings, reputation gates |
| **Cryptographically verifiable** | Every event signed with Ed25519, organized in Merkle trees |
| **Multi-agent** | Agents collaborate through events, not direct calls |
| **Self-healing** | Budget errors, circuit breakers, and retry logic handle failures automatically |
