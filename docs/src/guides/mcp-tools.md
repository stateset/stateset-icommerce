# MCP Tools

StateSet iCommerce exposes 520+ tools via the Model Context Protocol (MCP), making it the largest known domain-specific MCP server. Tools are organized into 48 modules covering all commerce, A2A, and platform operations.

## How MCP Works

The MCP server (`mcp-server.js`) acts as a thin orchestrator:

```
LLM → MCP Client → MCP Server → adaptTool() → Permission Check → Telemetry → Handler → Response
```

Each tool is defined with:
- A unique name (e.g., `list_orders`)
- A JSON Schema for input validation (via Zod)
- A handler function that executes the operation
- A permission level (read, write, admin)

## Tool Modules

| Module | Tools | Domain |
|--------|-------|--------|
| `orders.js` | 6 | Order CRUD and fulfillment |
| `products.js` | 4 | Product catalog |
| `customers.js` | 3 | Customer management |
| `inventory.js` | 6 | Stock tracking and reservations |
| `carts.js` | 14 | Cart operations and checkout |
| `payments.js` | 17 | Payment processing and reconciliation |
| `returns.js` | 5 | RMA processing |
| `subscriptions.js` | 15 | Recurring billing |
| `promotions.js` | 10 | Discounts and coupons |
| `tax.js` | 19 | Multi-jurisdiction tax |
| `shipments.js` | 11 | Shipping and tracking |
| `manufacturing.js` | 11 | BOM, work orders, quality |
| `analytics.js` | 10 | Revenue, forecasting, cohorts |
| `a2a.js` | 58 | A2A payments, quotes, escrow |
| `a2a-automation.js` | 30 | Autonomous execution |
| `a2a-intelligence.js` | 17 | Agent discovery and trust |
| `a2a-platform.js` | 16 | Platform write-back |
| `a2a-observability.js` | 14 | A2A metrics and health |
| `x402.js` | 13 | Payment intents and budget |
| `vector.js` | 16 | Semantic search and RAG |
| `sync.js` | 18 | Event sync and VES |
| `import.js` | 10 | Data import |
| `custom-objects.js` | 12 | Schema extensions |
| `policies.js` | 5 | Policy evaluation |
| `agent-runtime.js` | 29 | Agent lifecycle |

## Using Tools

### Via MCP Client (Claude Desktop, Cursor)

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This registers the MCP server with your client. Tools appear automatically in the tool palette.

### Via Embedded Toolkit

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ commerce, allowApply: false });

// List available tools
const tools = toolkit.getTools({ format: 'openai' });

// Execute a tool
const result = await toolkit.executeTool('list_orders', { status: 'pending' });
```

### Via CLI

```bash
stateset "list pending orders"
# The CLI routes this to the list_orders tool automatically
```

## Permission Levels

Tools are tagged with permission levels:

| Level | Examples |
|-------|---------|
| **Read** | `list_orders`, `get_customer`, `sales_summary` |
| **Write** | `create_order`, `ship_order`, `adjust_inventory` |
| **Admin** | `delete_customer`, `configure_stripe`, `reload_policies` |

20 high-risk tools require explicit approval (configured in `permissions.js`):
- `delete_customer`, `cancel_order`, `refund_payment`
- All A2A payment tools
- Policy modification tools

## Tool Response Format

All tools return structured JSON:

```json
{
    "success": true,
    "data": { ... },
    "metadata": {
        "tool": "list_orders",
        "executionTimeMs": 12,
        "recordCount": 5
    }
}
```

Error responses include actionable context:

```json
{
    "success": false,
    "error": {
        "code": "NOT_FOUND",
        "message": "Order ORD-999 not found",
        "suggestion": "Use list_orders to find valid order IDs"
    }
}
```

## Batch Execution

Execute multiple tools in a single agent loop:

```javascript
const results = await toolkit.executeToolCalls([
    { tool: 'get_customer', params: { id: 'cust-001' } },
    { tool: 'list_orders', params: { customerId: 'cust-001' } },
    { tool: 'get_loyalty_balance', params: { customerId: 'cust-001' } }
]);
```

## Zod Validation

Every tool parameter is validated with Zod schemas before execution. This prevents malformed inputs from reaching the commerce engine:

```javascript
// Example: create_order tool schema
const schema = z.object({
    customerId: z.string().uuid('Must be a valid UUID'),
    items: z.array(z.object({
        sku: z.string().min(1).max(64),
        name: z.string().min(1).max(256),
        quantity: z.number().int().positive().max(10000),
        unitPrice: z.number().nonnegative().max(999999.99)
    })).min(1).max(100),
    currency: z.enum(['USD', 'EUR', 'GBP', 'JPY', 'CAD', 'AUD']).optional()
});
```

If validation fails, the tool returns a structured error:

```json
{
    "success": false,
    "error": {
        "code": "VALIDATION_ERROR",
        "message": "Invalid input",
        "details": [
            { "path": "items[0].quantity", "message": "Expected positive number, received -1" }
        ]
    }
}
```

The 120+ Zod constraints across all tools cover:
- UUID format for entity IDs
- Email format validation
- Numeric ranges for quantities and amounts
- Enum validation for status fields and currencies
- String length limits to prevent abuse
- Array size bounds

## Simulation & Dry Run

Before executing write operations, agents can simulate:

```javascript
// Simulate a single mutation
const preview = await toolkit.simulateMutation({
    tool: 'ship_order',
    params: { orderId: 'ord-123', carrier: 'FedEx', trackingNumber: 'FEDEX-789' }
});
// → { wouldAffect: { order: { from: 'processing', to: 'shipped' }, shipment: { created: true } } }

// Simulate a multi-step plan
const plan = await toolkit.executePlan({
    dryRun: true,
    steps: [
        { tool: 'create_order', params: { ... } },
        { tool: 'capture_payment', params: { ... } },
        { tool: 'ship_order', params: { ... } }
    ]
});
// Each step shows its preview without executing
```

## Tool Categories

### Read Tools (no `--apply` required)

`list_orders`, `get_customer`, `sales_summary`, `search_products`, `get_inventory_level`, `list_subscriptions`, `a2a_get_reputation`, `x402_get_budget`

### Write Tools (require `--apply`)

`create_order`, `ship_order`, `adjust_inventory`, `create_payment`, `refund_payment`, `create_return`, `create_subscription`, `a2a_create_escrow`, `x402_create_intent`

### Admin Tools (require explicit approval)

`delete_customer`, `cancel_order`, `configure_stripe`, `reload_policies`, `a2a_resolve_dispute`

The 20 highest-risk tools are configured in `permissions.js` with `requireApprovalFor` — even with `--apply`, these tools prompt for confirmation.
