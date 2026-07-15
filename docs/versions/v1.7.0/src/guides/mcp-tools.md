# MCP Tools

StateSet iCommerce exposes a registry-generated tool surface via the Model Context Protocol (MCP). The exact live count, policy-domain breakdown, and tool list are generated from code in the [MCP Tool Inventory](../appendix/mcp-tool-inventory.md) rather than maintained by hand in prose.

## How MCP Works

The MCP server registry spans multiple entrypoints. The main commerce server (`mcp-server.js`) acts as a thin orchestrator:

```
LLM → MCP Client → MCP Server → adaptTool() → Permission Check → Telemetry → Handler → Response
```

Each tool is defined with:
- A unique name (e.g., `list_orders`)
- A JSON Schema for input validation (via Zod)
- A handler function that executes the operation
- A permission level (read, write, admin)

## Tool Coverage

The registry spans commerce, A2A, compliance, connectors, sync, checkout, search, and platform operations. Use the generated inventory when you need an exact answer to any of these questions:

- How many tools are currently shipped
- Which modules are loaded into the CLI registry
- Which tools are `read`, `write`, `delete`, or `admin`
- Whether a specific tool name is actually present in the live registry

## Using Tools

### Via MCP Client (Claude Desktop, Cursor)

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This registers the MCP server with your client. Tools appear automatically in the tool palette.

### Via Embedded Toolkit

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools } from '@stateset/embedded/openai';
import { executeTool } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');

// List available tools
const tools = createOpenAITools(commerce, {
    filter: ['list_orders']
});

// Execute a tool
const result = await executeTool(commerce, 'list_orders', { limit: 10 });
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
| **Read** | `list_orders`, `get_customer`, `get_sales_summary` |
| **Write** | `create_order`, `ship_order`, `adjust_inventory` |
| **Admin** | `delete_customer`, `configure_stripe`, `reload_policies` |

20 high-risk tools require explicit approval (configured in `permissions.js`):
- `delete_customer`, `cancel_order`, `refund_payment`
- All A2A payment tools
- Policy modification tools

## Tool Response Format

Embedded-toolkit execution returns a structured envelope around the tool payload:

```json
{
  "success": true,
  "tool": "list_orders",
  "status": "success",
  "result": {
    "success": true,
    "orders": []
  },
  "policy": { "domain": "orders" },
  "permission": { "allowed": true }
}
```

Validation failures and preview-mode write calls use the same top-level envelope:

```json
{
  "success": false,
  "tool": "get_customer",
  "status": "invalid",
  "error": "Invalid parameters for tool 'get_customer'",
  "notes": {
    "validation": [
      { "path": "identifier", "message": "Required" }
    ]
  }
}
```

## Batch Execution

Execute multiple tools in a single agent loop:

```javascript
const results = await toolkit.executeToolCalls([
    { tool: 'list_customers', params: {} },
    { tool: 'get_customer', params: { identifier: 'cust-001' } },
    { tool: 'list_orders', params: { limit: 10 } }
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
// preview.outcome.preview === true
// preview.outcome.wouldDo.tool === 'ship_order'
// preview.outcome.permission.reason explains why execution stayed in preview

// Simulate a multi-step plan
const plan = await toolkit.executePlan({
    dryRun: true,
    steps: [
        { tool: 'create_order', params: { ... } },
        { tool: 'create_payment', params: { ... } },
        { tool: 'ship_order', params: { ... } }
    ]
});
// plan.steps[0].preview === true
// plan.steps[0].status is 'dry_run_success' or 'dry_run_blocked'
```

## Tool Categories

### Read Tools (no `--apply` required)

`list_orders`, `get_customer`, `get_sales_summary`, `search_products`, `get_inventory_level`, `list_subscriptions`, `a2a_get_reputation`, `x402_budget_status`

### Write Tools (require `--apply`)

`create_order`, `ship_order`, `adjust_inventory`, `create_payment`, `refund_payment`, `create_return`, `create_subscription`, `a2a_create_escrow`, `x402_call`

### Admin Tools (require explicit approval)

`delete_customer`, `cancel_order`, `configure_stripe`, `reload_policies`, `a2a_resolve_dispute`

The 20 highest-risk tools are configured in `permissions.js` with `requireApprovalFor` — even with `--apply`, these tools prompt for confirmation.
