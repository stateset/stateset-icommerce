# Embedded Agent Toolkit

The embedded agent toolkit enables programmatic integration of iCommerce with
any AI framework — OpenAI, Vercel AI SDK, LangChain, or custom agent runtimes.
Instead of going through the CLI, your application embeds the full
registry-generated tool surface directly.

If the strategy is "engine first", this toolkit is the primary distribution
surface for agent-framework adoption.

The repo ships runnable examples for this surface under `examples/agents/`, and
the release gate now smoke-tests those examples from source so the framework
adapters do not silently drift behind the published packages.

For most JS agent hosts, start with the engine-owned helper subpaths:

- `@stateset/embedded/openai`
- `@stateset/embedded/generic`
- `@stateset/embedded/langchain`
- `@stateset/embedded/vercel-ai`

Use `@stateset/embedded/agent-toolkit` when you need the full advanced runtime
surface: delegation, priced tools, remote MPP routes, plan simulation, replay,
or raw multi-format tool inspection.

## Quick Start

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';
import { createToolDescriptors } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');

const tools = createOpenAITools(commerce, {
    filter: ['list_orders']
});

const result = await executeOpenAIToolCall(commerce, {
    call_id: 'orders_1',
    function: {
        name: 'list_orders',
        arguments: JSON.stringify({ limit: 10 })
    }
});

const descriptors = createToolDescriptors(commerce, {
    filter: ['list_orders']
});
```

## Supported Formats

The advanced toolkit object can still produce multiple formats for different AI frameworks:

| Format | Framework | Output Shape |
|--------|-----------|-------------|
| `openai` | OpenAI API, GPT-4 | `{ type: "function", function: { name, description, parameters } }` |
| `anthropic` | Anthropic Messages API | `{ name, description, input_schema }` |
| `mcp` | MCP-compatible runtimes | `{ name, toolName, description, inputSchema }` |
| `generic` | Custom runtimes | `{ name, description, inputSchema }` |

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/embedded/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });

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
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';

const openai = new OpenAI();
const commerce = new Commerce('./store.db');
const tools = createOpenAITools(commerce);

// Send tools to OpenAI
const response = await openai.chat.completions.create({
    model: 'gpt-4o',
    messages: [{ role: 'user', content: 'Show me pending orders' }],
    tools,
});

// Execute tool calls from the response
for (const toolCall of response.choices[0].message.tool_calls || []) {
    const result = await executeOpenAIToolCall(commerce, toolCall);
    console.log(result);
}
```

## Vercel AI SDK Integration

```javascript
import { generateText, tool } from 'ai';
import { openai } from '@ai-sdk/openai';
import { Commerce } from '@stateset/embedded';
import { createVercelAITools } from '@stateset/embedded/vercel-ai';

const commerce = new Commerce('./store.db');
const tools = createVercelAITools(commerce, { tool });

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
import { Commerce } from '@stateset/embedded';
import { createLangChainTools } from '@stateset/embedded/langchain';

const commerce = new Commerce('./store.db');
const tools = createLangChainTools(commerce, { DynamicStructuredTool });

const model = new ChatOpenAI({ model: 'gpt-4o' });
const modelWithTools = model.bindTools(tools);
const response = await modelWithTools.invoke('List all customers');
```

## Framework-Neutral Integration

For runtimes that do not want a framework-specific adapter, use
`createToolDescriptors()` and wire the returned descriptors into your own
registry:

```javascript
import { Commerce } from '@stateset/embedded';
import { createToolDescriptors, createCallableRegistry } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');
const tools = createToolDescriptors(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary'],
});
const registry = createCallableRegistry(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary'],
});

const result = await registry.list_customers({ limit: 10 });
```

Each descriptor includes:

- `name`
- `description`
- `schema`
- `inputSchema`
- `permission`
- `policyDomain`
- `runtime`
- `execute()`
- `preparePayment()` and `executeWithPayment()` for priced tools

## Python Runtime Toolkit

The Python binding now includes a native toolkit for core embedded commerce
operations:

```python
from stateset_embedded import Commerce, create_embedded_agent_toolkit

commerce = Commerce(":memory:")
toolkit = create_embedded_agent_toolkit(commerce, allow_apply=False)

openai_tools = toolkit.get_tools(format="openai")
descriptors = toolkit.create_tool_descriptors(
    filter=["list_customers", "list_orders", "get_sales_summary"]
)
callable_registry = toolkit.create_callable_registry(filter=["list_customers"])
```

The Python toolkit is intentionally narrower than the JS toolkit. It is the
direct path for Python agent frameworks that need core commerce reads and writes
inside the host process, with write gating through `allow_apply`.

When the host framework is installed, the toolkit also offers optional adapter
helpers:

```python
langchain_tools = toolkit.create_langchain_tools(filter=["list_customers"])
crewai_tools = toolkit.create_crewai_tools(filter=["count_customers"])
autogen_tools = toolkit.create_autogen_tools(filter=["get_sales_summary"])
```

Those methods also accept a `tool_factory` callback so you can keep framework
dependencies out of your base environment and still generate framework-specific
objects from the underlying descriptors.

If you want framework-branded imports instead of calling methods on the toolkit,
the Python package also exposes wrapper modules:

```python
from stateset_embedded.langchain import create_langchain_tools
from stateset_embedded.crewai import create_crewai_tools
from stateset_embedded.autogen import create_autogen_tools

langchain_tools = create_langchain_tools(commerce, filter=["list_customers"])
crewai_tools = create_crewai_tools(commerce, filter=["count_customers"])
autogen_tools = create_autogen_tools(commerce, filter=["get_sales_summary"])
```

When you need the full registry-generated tool surface, priced-tool helpers,
policy/runtime contracts, or specialist-agent orchestration, use the JS toolkit
or MCP server instead.

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
import { Commerce } from '@stateset/embedded';
import { executeTool } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');

// List orders
const orders = await executeTool(commerce, 'list_orders', { limit: 10 }, {
    allowApply: true,
});

// Create a customer (requires allowApply)
const customer = await executeTool(commerce, 'create_customer', {
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Agent',
}, {
    allowApply: true,
});

// Run analytics
const revenue = await executeTool(commerce, 'get_sales_summary', { period: 'month' });
```

For advanced preview controls, use the full toolkit object:

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

For AI frameworks like LangChain Python, use the native Python toolkit for core
embedded tools or the JS toolkit/MCP server when you need the full generated
surface. See the [Python binding docs](../api/python.md).
