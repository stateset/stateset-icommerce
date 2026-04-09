# Embedded Agent Toolkit

The embedded agent toolkit enables programmatic integration of iCommerce with any AI framework — OpenAI, Vercel AI SDK, LangChain, or custom agent runtimes. Instead of going through the CLI, your application embeds the full registry-generated tool surface directly.

## Quick Start

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({
    dbPath: './store.db',
    allowApply: true,
});

// Get all tools in OpenAI format
const tools = toolkit.getTools({ format: 'openai' });

// Execute a tool directly
const result = await toolkit.executeTool('list_orders', { limit: 10 });
```

## Supported Formats

The toolkit produces tool definitions in multiple formats for different AI frameworks:

| Format | Framework | Output Shape |
|--------|-----------|-------------|
| `openai` | OpenAI API, GPT-4 | `{ type: "function", function: { name, description, parameters } }` |
| `anthropic` | Anthropic Messages API | `{ name, description, input_schema }` |
| `mcp` | MCP-compatible runtimes | `{ name, toolName, description, inputSchema }` |
| `generic` | Custom runtimes | `{ name, description, inputSchema }` |

```javascript
// OpenAI format
const openaiTools = toolkit.getTools({ format: 'openai' });

// Anthropic Messages API format
const anthropicTools = toolkit.getTools({ format: 'anthropic' });

// MCP descriptor format
const mcpTools = toolkit.getTools({ format: 'mcp' });
```

## OpenAI Integration

```javascript
import OpenAI from 'openai';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const openai = new OpenAI();
const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const tools = toolkit.getTools({ format: 'openai' });

// Send tools to OpenAI
const response = await openai.chat.completions.create({
    model: 'gpt-4o',
    messages: [{ role: 'user', content: 'Show me pending orders' }],
    tools,
});

// Execute tool calls from the response
for (const toolCall of response.choices[0].message.tool_calls || []) {
    const result = await toolkit.executeOpenAIToolCall(toolCall);
    console.log(result);
}
```

## Vercel AI SDK Integration

```javascript
import { generateText, tool } from 'ai';
import { openai } from '@ai-sdk/openai';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const tools = toolkit.createVercelAITools({ tool });

const result = await generateText({
    model: openai('gpt-4o'),
    tools,
    prompt: 'What products are low on stock?',
});
```

## LangChain Integration

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { DynamicStructuredTool } from '@langchain/core/tools';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const tools = toolkit.createLangChainTools({ DynamicStructuredTool });

const model = new ChatOpenAI({ model: 'gpt-4o' });
const modelWithTools = model.bindTools(tools);
const response = await modelWithTools.invoke('List all customers');
```

## MCP Server Integration

For Claude Desktop, Cursor, or any MCP-compatible client, use the CLI setup flow from the
[AI Agent Quickstart](../ai-agents.md). For custom MCP transports inside your own process,
use `createStatesetMcpServer()` directly:

```javascript
import { createStatesetMcpServer } from '@stateset/cli/mcp-server';

const server = createStatesetMcpServer({
    dbPath: './store.db',
    allowApply: false,  // read-only by default
});

// Wire the registry-generated MCP surface into your transport runtime
await server.connect(transport);
```

If the MCP runtime should support specialist-agent delegation, pass `autonomousEngine`; `delegate_to_agent` remains preview-only until `allowApply` is enabled.

See the [AI Agent Quickstart](../ai-agents.md) for full MCP configuration.

## Direct Tool Execution

Execute tools without an AI framework:

```javascript
const toolkit = createEmbeddedAgentToolkit({
    dbPath: './store.db',
    allowApply: true,
});

// List orders
const orders = await toolkit.executeTool('list_orders', { limit: 10 });

// Create a customer (requires allowApply)
const customer = await toolkit.executeTool('create_customer', {
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Agent',
});

// Run analytics
const revenue = await toolkit.executeTool('get_sales_summary', { period: 'month' });
```

## Autonomous Delegation

When your runtime includes the autonomous engine, the toolkit can delegate work to the shared specialist-agent registry:

```javascript
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    allowApply: true,
    autonomousEngine,
});

const delegation = await toolkit.executeTool('delegate_to_agent', {
    agent_name: 'orders',
    task_description: 'Review pending orders over $500',
    context: { limit: 10 },
});
```

`delegate_to_agent` is a write-gated runtime tool. With `allowApply: false`, the toolkit returns a preview payload instead of executing the delegation.

## Configuration Options

```javascript
const toolkit = createEmbeddedAgentToolkit({
    dbPath: './store.db',       // SQLite database path
    allowApply: false,           // Enable write operations (default: false)
    commerce: existingInstance,   // Re-use an existing Commerce instance
    autonomousEngine,             // Enable delegate_to_agent and runtime orchestration
    policyStorePath: './.stateset', // Root containing policies/ and related runtime state
});
```

| Option | Default | Description |
|--------|---------|-------------|
| `dbPath` | `./store.db` | Path to SQLite database |
| `allowApply` | `false` | Enable write operations |
| `commerce` | — | Existing Commerce instance (skip DB init) |
| `autonomousEngine` | — | Enables runtime tools such as `delegate_to_agent` |
| `policyStorePath` | Derived from `dbPath` as an adjacent `.stateset/` directory | Policy store root used by the embedded MCP runtime |
| `treasury` | — | Enables priced-tool discovery, payment challenges, and receipt-aware execution |
| `mpp` | — | Default payer and HTTP payment options for remote or paid tool helpers |

When you provide `policyStorePath`, the runtime looks for policy files under
`<policyStorePath>/policies/`. That matches the policy engine layout documented
in the [policy guide](../policy/engine.md).

## Tool Filtering

Get a specific tool or subset:

```javascript
// Get a single tool definition
const tool = toolkit.getTool('create_order', { format: 'openai' });

// Get raw tool definitions (before format conversion)
const rawTools = toolkit.getRawTools();
```

## Payment-Aware Tools

The embedded toolkit can discover priced tools, prepare MPP payment challenges,
and execute paid calls with automatic retry once a compatible payment method is
configured.

```javascript
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    treasury: {
        enabled: true,
        agentId: 'buyer-agent',
        dbPath: './treasury.db',
        pricingPath: './pricing.json',
    },
});

const payableCatalog = await toolkit.getPayableToolCatalog({
    tool: 'list_customers',
});

const prepared = await toolkit.prepareToolPayment({
    tool: 'list_customers',
    params: {},
    requestId: 'toolkit-req-1',
});

const result = await toolkit.executePaidTool('list_customers', {}, {
    payment: {
        acceptedMethods: ['bitcoin'],
        maxAmountSmallest: '10000',
    },
});
```

`getPaymentDiscovery()` and `discoverPayableTools()` expose the same priced-tool
surface in machine-readable form, while `executeToolWithPayment()` and
`executePaidOpenAIToolCall()` attach payment receipts to successful results.

## Remote Payable HTTP Routes

You can also discover MPP-enabled HTTP services and wrap their payable routes as
tool-like descriptors:

```javascript
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    mpp: {
        payer: 'buyer-agent',
    },
});

const discovery = await toolkit.discoverRemotePaymentService(
    'https://merchant.example',
    { fetch },
);

const descriptors = await toolkit.createRemoteHttpToolDescriptors(
    'https://merchant.example',
    {
        fetch,
        executionOptions: {
            payment: {
                acceptedMethods: ['bitcoin'],
                maxAmountSmallest: '10000',
            },
        },
    },
);

const payableResult = await descriptors[0].execute({
    method: 'POST',
    body: JSON.stringify({ sku: 'SKU-123' }),
    headers: { 'content-type': 'application/json' },
});
```

Use `discoverRemotePayableRoutes()` when you only need the discovered route
metadata, or `executeRemoteHttpRoute()` when you want to run a specific route
without first building descriptors.

## Planning, Contracts, and Replay

The toolkit also exposes the same planning and replay helpers used by the
agentic runtime:

```javascript
const contract = await toolkit.getRuntimeContract({
    tool: 'create_customer',
});

const plan = await toolkit.simulatePlan({
    steps: [
        {
            tool: 'create_customer',
            params: {
                email: 'plan@example.com',
                firstName: 'Plan',
                lastName: 'User',
            },
        },
    ],
});

const execution = await toolkit.executePlan({
    dryRun: true,
    steps: [
        {
            tool: 'create_customer',
            params: {
                email: 'plan@example.com',
                firstName: 'Plan',
                lastName: 'User',
            },
        },
    ],
});
```

`getRuntimeContract()` returns the current runtime contract hash and tool
metadata for the filtered tool set. `simulatePlan()` returns per-step outcomes,
including routing metadata such as `plan.outcomes[0].routing.primary.agent`.
`executePlan()` uses the same step model for dry-run or apply execution and
reports an overall `finalStatus`.

For write operations, you can replay prior mutations and inspect the replay log:

```javascript
const direct = await toolkit.executeTool('create_customer', {
    email: 'replay@example.com',
    firstName: 'Replay',
    lastName: 'User',
});

const replay = await toolkit.replayMutation({
    requestId: direct.requestId,
    dryRun: true,
});

const log = await toolkit.getReplayLog({
    requestId: direct.requestId,
});
```

`replayMutation()` reruns the latest matching write event in dry-run mode by
default and reports the original source tool plus deterministic hash checks.
`getReplayLog()` returns the persisted replay events that match the supplied
filters.

## Error Handling

Tool execution returns structured errors:

```javascript
const result = await toolkit.executeTool('create_order', { customerId: 'invalid' });
// → {
//     success: false,
//     tool: 'create_order',
//     status: 'invalid',
//     error: "Invalid parameters for tool 'create_order'",
//     notes: { validation: [...] }
// }
```

When `allowApply` is `false`, write operations return a preview:

```javascript
const result = await toolkit.executeTool('create_customer', {
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Agent',
});
// → {
//     success: false,
//     status: 'preview',
//     error: "Preview mode: would execute 'create_customer' if --apply flag is set",
//     preview: true,
//     wouldDo: {
//         tool: 'create_customer',
//         params: {
//             email: 'alice@example.com',
//             firstName: 'Alice',
//             lastName: 'Agent',
//             acceptsMarketing: false
//         }
//     }
// }
```

## Python Integration

Using the Rust bindings via PyO3:

```python
from stateset import Commerce

commerce = Commerce("./store.db")
orders = commerce.orders.list(status="pending")
```

For AI frameworks like LangChain Python, use the Node.js toolkit as an MCP server and connect via the MCP protocol. See the [Python binding docs](../api/python.md).
