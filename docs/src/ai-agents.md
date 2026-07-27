# AI Agent Quickstart

StateSet supports two onboarding modes for agents:

- **Embedded toolkit** for OpenAI, Vercel AI SDK, LangChain, framework-neutral runtimes, and custom server-side agents
- **MCP server** for Claude Desktop, Cursor, Windsurf, and other MCP-native clients

Both paths go through the same policy engine, permission system, and audit layer.

If the goal is to make iCommerce the default commerce kernel for agents, prefer
the embedded toolkit path first and use MCP as the distribution path for
MCP-native clients.

## Install

```bash
npm install @stateset/cli@1.23.2 @stateset/embedded@1.23.2
```

For Python runtimes:

```bash
pip install stateset-embedded==1.23.2
pip install "stateset-embedded[agents]==1.23.2"
```

From the repo checkout, the examples under `examples/agents/` also run against
workspace modules directly, so the embedded path is smoke-tested before
publish.

## Embedded Toolkit

The embedded toolkit gives your agent direct access to the full registry-generated commerce tool surface:

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';
import { createToolDescriptors } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');
const tools = createOpenAITools(commerce, {
    filter: ['list_customers']
});
const execution = await executeOpenAIToolCall(commerce, {
    call_id: 'demo_call_1',
    function: {
        name: 'list_customers',
        arguments: '{}'
    }
});
const descriptors = createToolDescriptors(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary']
});

```

`createOpenAITools()` returns JSON-schema function tool definitions.
`executeOpenAIToolCall()` and `createToolDescriptors()` run through the same
policy, permission, and audit runtime as the MCP server.
If you want the embedded runtime to fan out to specialist agents, pass `autonomousEngine` and set `allowApply: true`; `delegate_to_agent` stays preview-only while `allowApply` is `false`.

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/embedded/agent-toolkit';

const delegatedToolkit = createEmbeddedAgentToolkit({
    commerce,
    allowApply: true,
    autonomousEngine,
});

await delegatedToolkit.executeTool('delegate_to_agent', {
    agent_name: 'orders',
    task_description: 'Review pending orders over $500',
    context: { limit: 10 },
});
```

## Vercel AI SDK

```javascript
import { generateText } from 'ai';
import { tool } from 'ai';
import { anthropic } from '@ai-sdk/anthropic';
import { createVercelAITools } from '@stateset/embedded/vercel-ai';

const tools = createVercelAITools(commerce, {
    tool,
    filter: ['list_customers', 'get_order', 'get_sales_summary'],
});

const result = await generateText({
    model: anthropic('claude-sonnet-4-6'),
    prompt: 'What are my top 5 customers by revenue?',
    tools,
});
```

This returns the object shape expected by `streamText()` / `generateText()`.

## LangChain / LangGraph

```javascript
import { DynamicStructuredTool } from '@langchain/core/tools';
import { createLangChainTools } from '@stateset/embedded/langchain';

const tools = createLangChainTools(commerce, {
    DynamicStructuredTool,
    filter: ['list_customers', 'get_order', 'search_products'],
});

// Use with any LangChain agent or chain
```

These tools use the original Zod schemas and call back into the same embedded execution runtime.

## Framework-Neutral Adapters

For any agent runtime that wants a plain `{ name, description, schema, execute }`
contract, use `createToolDescriptors()`:

```javascript
const tools = createToolDescriptors(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary'],
});

const result = await tools[0].execute({});
```

This is the lowest-common-denominator surface for custom orchestrators and thin
adapter layers.

## Python Runtime Toolkit

The Python binding now ships a native toolkit for core commerce operations:

```python
from stateset_embedded import Commerce, create_embedded_agent_toolkit

commerce = Commerce(":memory:")
toolkit = create_embedded_agent_toolkit(commerce, allow_apply=False)

tools = toolkit.get_tools(format="openai")
descriptors = toolkit.create_tool_descriptors(
    filter=["list_customers", "list_orders", "get_sales_summary"]
)
langchain_tools = toolkit.create_langchain_tools(filter=["list_customers"])
```

This is the right path for Python runtimes that want OpenAI-compatible tool
schemas or plain `{ name, description, schema, execute }`-style descriptors
over core embedded commerce operations.

When the framework is installed, the same toolkit also exposes optional helper
methods such as `create_langchain_tools()`, `create_crewai_tools()`, and
`create_autogen_tools()`. If you want to avoid hard dependencies, pass a custom
`tool_factory` callback and adapt the framework objects yourself.

For framework-first imports, the Python package also exposes:

```python
from stateset_embedded.generic import create_tool_descriptors, create_callable_registry
from stateset_embedded.openai import create_openai_tools, execute_openai_tool_call
from stateset_embedded.langchain import create_langchain_tools
from stateset_embedded.crewai import create_crewai_tools
from stateset_embedded.autogen import create_autogen_tools
```

The repo now ships dedicated Python examples for each of those paths:
`examples/python/openai_tools.py`, `examples/python/generic_tools.py`,
`examples/python/langchain_tools.py`, `examples/python/crewai_tools.py`, and
`examples/python/autogen_tools.py`.

For full parity with the JS toolkit's registry-generated surface, priced tools,
or policy/runtime helpers, keep using the JS embedded toolkit or the MCP server.

## OpenAI Responses Loop

Complete example with tool call handling:

```javascript
import OpenAI from 'openai';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';

const client = new OpenAI();
const tools = createOpenAITools(commerce);

// 1. Initial request
const response = await client.responses.create({
    model: 'gpt-4.1',
    input: 'List the most recent customers.',
    tools,
});

// 2. Execute tool calls
const toolCall = response.output.find((item) => item.type === 'function_call');
const execution = await executeOpenAIToolCall(commerce, toolCall);

// 3. Return results to model
const finalResponse = await client.responses.create({
    model: 'gpt-4.1',
    previous_response_id: response.id,
    input: [execution.outputMessage],
    tools,
});

console.log(finalResponse.output_text);
```

## Claude API (Anthropic SDK)

```javascript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic();
const tools = toolkit.getTools({ format: 'anthropic' });

const response = await client.messages.create({
    model: 'claude-sonnet-4-6',
    max_tokens: 4096,
    messages: [{ role: 'user', content: 'Show me pending orders' }],
    tools,
});

// Handle tool use
for (const block of response.content) {
    if (block.type === 'tool_use') {
        const result = await toolkit.executeTool(block.name, block.input);
        // Send result back to Claude...
    }
}
```

## Safe Writes

Start with `allowApply: false` and progressively enable writes:

### Preview a Mutation

```javascript
const preview = await toolkit.simulateMutation({
    tool: 'ship_order',
    params: { orderId: 'ord-123', trackingNumber: 'FEDEX-789' }
});
// preview.outcome.preview === true
// preview.outcome.wouldDo.tool === 'ship_order'
// preview.outcome.permission.reason explains why the mutation stayed in preview
```

### Preview a Multi-Step Plan

```javascript
const plan = await toolkit.executePlan({
    dryRun: true,
    steps: [
        { tool: 'create_order', params: { customerId: 'cust-001', items: [...] } },
        { tool: 'create_payment', params: { orderId: 'ord-123', amount: 59.98 } },
        { tool: 'ship_order', params: { orderId: 'ord-123' } }
    ]
});
// plan.steps[0].preview === true
// plan.steps[0].status is 'dry_run_success' or 'dry_run_blocked'
```

### Batch Tool Calls

```javascript
const results = await toolkit.executeToolCalls([
    { tool: 'list_customers', params: {} },
    { tool: 'get_customer', params: { identifier: 'cust-001' } },
    { tool: 'list_orders', params: { limit: 10 } }
]);
```

### Enable Writes

```javascript
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    allowApply: true,   // Enables write operations
});
```

Even with `allowApply: true`, 20 high-risk tools (delete, refund, A2A payments) require explicit approval.

## MCP-Native Clients

For Claude Desktop, Cursor, Windsurf, or other MCP-native clients:

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This registers the iCommerce MCP server with your client. The full registry-generated tool inventory appears automatically in the tool palette.

### MCP Configuration

The setup creates a configuration entry in your MCP client's config file:

```json
{
    "mcpServers": {
        "stateset-commerce": {
            "command": "npx",
            "args": ["@stateset/cli@1.23.2", "stateset-mcp", "--db", "./store.db"]
        }
    }
}
```

## Which Approach Should I Use?

| Scenario | Recommended |
|----------|-------------|
| Claude Desktop / Cursor / Windsurf | MCP server (automatic tool discovery) |
| OpenAI GPT agents | `@stateset/embedded/openai` |
| Vercel AI SDK app | `@stateset/embedded/vercel-ai` |
| LangChain / LangGraph agent | `@stateset/embedded/langchain` |
| Custom agent framework | `@stateset/embedded/generic` |
| LangChain Python agent | `create_langchain_tools()` or `create_callable_registry()` |
| CrewAI / AutoGen style Python runtime | `create_crewai_tools()` / `create_autogen_tools()` or `create_tool_descriptors()` |
| Server-side API | Embedded toolkit with `allowApply: true` |
| Testing / exploration | CLI with `stateset "..."` (read-only) |
