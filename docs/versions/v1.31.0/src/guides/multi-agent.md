# Multi-Agent System

iCommerce includes 18 specialized agent configurations, each focused on a specific commerce domain. Agents can operate independently or collaborate through shared events and the A2A protocol.

## Agent Configurations

| Agent | Domain | Key Capabilities |
|-------|--------|-----------------|
| `orders` | Order management | Create, track, fulfill, cancel orders |
| `inventory` | Stock management | Levels, reservations, adjustments, alerts |
| `returns` | Return processing | RMA creation, approval, refunds |
| `analytics` | Business intelligence | Revenue reports, forecasts, cohorts |
| `checkout` | Cart & checkout | Cart management, pricing, checkout |
| `payments` | Payment processing | Captures, refunds, reconciliation |
| `subscriptions` | Recurring billing | Plans, billing cycles, dunning |
| `customers` | Customer management | Profiles, segments, loyalty |
| `products` | Product catalog | Catalog, variants, search |
| `shipping` | Fulfillment | Shipments, tracking, carriers |
| `manufacturing` | Production | BOM, work orders, quality |
| `procurement` | Supply chain | POs, suppliers, receiving |
| `finance` | Accounting | A/P, A/R, GL, invoicing |
| `tax` | Tax compliance | Rates, exemptions, nexus |
| `a2a` | Agent-to-agent | Payments, quotes, escrow, reputation |
| `sync` | Data sync | VES, events, conflict resolution |
| `security` | Access control | Permissions, audit, compliance |
| `support` | Customer service | Ticket routing, escalation |

## Using Agents

### CLI

Each agent has a dedicated entry point:

```bash
stateset-orders "show pending orders ready to ship"
stateset-inventory "what SKUs are below reorder point?"
stateset-analytics "forecast revenue for next quarter"
stateset-a2a "list active escrows"
```

### Programmatic

```javascript
import { agentDefinitions } from '@stateset/cli/agent-definitions';

// Get agent config
const ordersAgent = agentDefinitions.find(a => a.name === 'orders');
console.log(ordersAgent.tools);     // Available MCP tools
console.log(ordersAgent.systemPrompt); // Agent instructions
```

## Agent Collaboration

Agents collaborate through:

### 1. Shared Event Log

When one agent takes an action, events are emitted to all subscribed agents:

```
Orders Agent creates order → order.created event
    → Inventory Agent reserves stock
    → Payments Agent initiates capture
    → Shipping Agent prepares fulfillment
```

### 2. A2A Protocol

Agents can transact directly with each other:

```
Procurement Agent detects low stock
    → Queries supplier agents for quotes
    → Accepts best quote
    → Creates escrow for payment
    → Receives delivery confirmation
    → Releases escrow
```

### 3. SSE Event Streaming

Agents subscribe to real-time event streams:

```javascript
const events = new EventSource('/a2a/events?filter=order.*');
events.onmessage = (event) => {
    // React to order events in real time
};
```

## Agent Memory

Agents can persist conversation memory and state snapshots:

```javascript
// Save agent state
await toolkit.executeTool('save_agent_memory', {
    agentId: 'orders-agent',
    key: 'last_processed_order',
    value: 'ORD-12345'
});

// Retrieve state
const memory = await toolkit.executeTool('get_agent_memory', {
    agentId: 'orders-agent',
    key: 'last_processed_order'
});
```

## Agent Keys

Each agent has its own Ed25519 signing key for VES event signatures:

```javascript
await toolkit.executeTool('generate_agent_key', {
    agentId: 'orders-agent'
});
```

## Which Agents Should I Enable?

Start with the minimum set for your use case and add more as needed:

| Use Case | Start With | Add Later |
|----------|-----------|-----------|
| **Customer service chatbot** | `customer-service` | `returns`, `analytics` |
| **Automated fulfillment** | `orders`, `inventory`, `shipments` | `procurement`, `manufacturing` |
| **Financial operations** | `payments`, `subscriptions`, `invoices` | `analytics`, `tax` |
| **Supply chain automation** | `inventory`, `suppliers`, `manufacturing` | `quality`, `warehouse` |
| **Agent-to-agent commerce** | `agents` (multi-agent orchestrator) | All commerce agents |

## Agent Dependency Graph

Some agents work best together:

```
orders ──► inventory (reserve stock before shipping)
   │
   └──► payments (capture before fulfillment)
          │
          └──► shipments (create after capture)

procurement ──► suppliers (find vendors)
    │
    └──► inventory (update on receipt)

returns ──► payments (refund on approval)
   │
   └──► inventory (restock on receipt)
```

## Customizing Agent Prompts

Each agent's system prompt can be customized by modifying `cli/src/agent-definitions.js`:

```javascript
// Add domain-specific instructions
AGENTS['orders'].systemPrompt += `
## Custom Rules
- Always check inventory before creating orders
- Flag orders over $10,000 for review
- Include tracking numbers in all shipping confirmations
`;
```

## Agent Scheduling

Run agents on a schedule using `stateset-autonomous`:

```bash
# Run the billing executor every minute
stateset-autonomous --billing-interval 60000

# Run the dispute resolver every 5 minutes
stateset-autonomous --dispute-interval 300000
```

See [Autonomous Engine](autonomous-engine.md) for all autonomous components.
