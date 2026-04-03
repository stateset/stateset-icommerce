# AI Agent Quickstart

StateSet supports two onboarding modes for agents:

- **Embedded toolkit** for OpenAI, Vercel AI SDK, LangChain, and custom runtimes
- **MCP server** for Claude Desktop, Cursor, Windsurf, and other MCP-native clients

Both paths go through the same policy engine, permission system, and audit layer.

## Install

```bash
npm install @stateset/cli@0.9.5 @stateset/embedded@0.9.5
```

## Embedded Toolkit

The embedded toolkit gives your agent direct access to 520+ commerce tools:

```javascript
import { Commerce } from '@stateset/embedded';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const commerce = new Commerce('./store.db');
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    allowApply: false,   // Read-only by default
});

// Get tools in OpenAI JSON-schema format
const tools = toolkit.getTools({ format: 'openai' });

// Execute a tool
const result = await toolkit.executeTool('list_customers');
```

`getTools({ format: 'openai' })` returns JSON-schema function tool definitions.
`executeTool()` runs tools through the same policy, permission, and audit runtime as the MCP server.

## Vercel AI SDK

```javascript
import { generateText } from 'ai';
import { tool } from 'ai';
import Anthropic from '@anthropic-ai/sdk';

const tools = toolkit.createVercelAITools({
    tool,
    filter: ['list_customers', 'get_order', 'sales_summary'],
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

const tools = toolkit.createLangChainTools({
    DynamicStructuredTool,
    filter: ['list_customers', 'get_order', 'search_products'],
});

// Use with any LangChain agent or chain
```

These tools use the original Zod schemas and call back into the same embedded execution runtime.

## OpenAI Responses Loop

Complete example with tool call handling:

```javascript
import OpenAI from 'openai';

const client = new OpenAI();
const tools = toolkit.getTools({ format: 'openai' });

// 1. Initial request
const response = await client.responses.create({
    model: 'gpt-4.1',
    input: 'List the most recent customers.',
    tools,
});

// 2. Execute tool calls
const toolCall = response.output.find((item) => item.type === 'function_call');
const execution = await toolkit.executeOpenAIToolCall(toolCall);

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
// → { wouldAffect: { order: { from: 'processing', to: 'shipped' } } }
```

### Preview a Multi-Step Plan

```javascript
const plan = await toolkit.executePlan({
    dryRun: true,
    steps: [
        { tool: 'create_order', params: { customerId: 'cust-001', items: [...] } },
        { tool: 'capture_payment', params: { orderId: 'ord-123' } },
        { tool: 'ship_order', params: { orderId: 'ord-123' } }
    ]
});
// Each step shows its preview without executing
```

### Batch Tool Calls

```javascript
const results = await toolkit.executeToolCalls([
    { tool: 'get_customer', params: { id: 'cust-001' } },
    { tool: 'list_orders', params: { customerId: 'cust-001' } },
    { tool: 'get_loyalty_balance', params: { customerId: 'cust-001' } }
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

This registers the iCommerce MCP server with your client. All 520+ tools appear automatically in the tool palette.

### MCP Configuration

The setup creates a configuration entry in your MCP client's config file:

```json
{
    "mcpServers": {
        "stateset-commerce": {
            "command": "npx",
            "args": ["@stateset/cli@0.9.5", "stateset-mcp", "--db", "./store.db"]
        }
    }
}
```

## Which Approach Should I Use?

| Scenario | Recommended |
|----------|-------------|
| Claude Desktop / Cursor / Windsurf | MCP server (automatic tool discovery) |
| OpenAI GPT agents | Embedded toolkit (`format: 'openai'`) |
| Vercel AI SDK app | `createVercelAITools()` |
| LangChain / LangGraph agent | `createLangChainTools()` |
| Custom agent framework | Embedded toolkit (`getTools()` + `executeTool()`) |
| Server-side API | Embedded toolkit with `allowApply: true` |
| Testing / exploration | CLI with `stateset "..."` (read-only) |
