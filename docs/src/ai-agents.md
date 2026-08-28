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
npm install @stateset/cli@1.28.0 @stateset/embedded@1.28.0
```

For Python runtimes:

```bash
pip install stateset-embedded==1.28.0
pip install "stateset-embedded[agents]==1.28.0"
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
    capabilities: ['read:*', 'delegate_to_agent'],
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
    capabilities: ['read:*', 'payments.create'],
});
```

`allowApply: true` is rejected unless `capabilities` is explicit. Exact tool
names, `read:*`, `permission:write`, domain wildcards, and governed kernel
capabilities such as `payments.create` are supported. High-risk tools still
require their configured approvals.

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
            "args": ["-y", "-p", "@stateset/cli@1.28.0", "stateset-mcp", "--db", "./store.db"]
        }
    }
}
```

### Streamable HTTP (hosted sandboxes, remote agents)

`stateset-mcp-http` serves the same tool surface over MCP Streamable HTTP at
**protocol revision 2026-07-28**, and is **stateless by construction**: the SDK
serves that revision from a per-request server factory, so every exchange gets a
freshly built MCP server, no `Mcp-Session-Id` is issued, and nothing is retained
between requests. Any request can be served by any replica, so it scales
horizontally behind a load balancer. All requests share one store (`--db`,
default `:memory:`, seeded with demo data once at boot):

```bash
npx -y -p @stateset/cli stateset-mcp-http           # http://127.0.0.1:8090/mcp
```

```json
{ "mcpServers": { "stateset-sandbox": { "url": "http://localhost:8090/mcp" } } }
```

Because serving is per-request, only `POST /mcp` is supported — the 2025 session
verbs `GET` and `DELETE` return `405`. There is no `initialize` handshake on the
modern path: each request carries its own `_meta` envelope plus the `Mcp-Method`
and `Mcp-Name` headers.

Flags: `--host 0.0.0.0` to expose, `--db <path>` for a durable store (writes
then require `--apply`), `--read-only` to disable writes, `--no-seed` for an
empty store, `--strict-protocol` to refuse 2025-era clients.
`GET /health` reports status for deploy probes.

Three request guards sit in front of the MCP handler, in the order
Host → Origin → Auth:

- **Host (DNS rebinding)** — `--allowed-host <hostname>` (repeatable). A
  loopback bind allows only localhost Host values by default. Any other
  `--host` **refuses to start** without `--allowed-host`; pass
  `--insecure-allow-any-host` only behind a proxy that already pins the Host
  header.
- **Origin (browser cross-origin)** — `--allowed-origin <origin>` (repeatable;
  a full origin like `https://agent.example.com` or a bare hostname, matched by
  hostname). A request carrying any other `Origin` gets `403`. A loopback bind
  allows localhost origins by default; an exposed bind allows none. Requests
  with no `Origin` header — every non-browser MCP client — always pass.
- **Auth (API key)** — `--api-key <key>` (repeatable), `STATESET_MCP_API_KEYS`
  (comma-separated) or `--api-key-file <path>` (one per line). Once any key is
  configured every `/mcp` request must carry `Authorization: Bearer <key>` or
  `X-API-Key: <key>`; anything else gets `401` with a JSON-RPC error body and a
  `WWW-Authenticate: Bearer` challenge. Keys are at least 16 characters,
  compared in constant time, and never logged — the boot line shows only a
  6-char SHA-256 fingerprint per key. A non-loopback `--host` **refuses to
  start** without a key; pass `--insecure-no-auth` only behind a proxy that
  authenticates every request itself. On loopback, auth is off unless you give
  a key. `/health` stays open but shows only `status`/`version`/`protocol` to
  anonymous callers once auth is on.

```bash
STATESET_MCP_API_KEYS="$(openssl rand -hex 24)" \
stateset-mcp-http --host 0.0.0.0 --port 8090 \
  --allowed-host mcp.example.com \
  --allowed-origin https://agent.example.com
```

Clients pass the key as a header. Claude Desktop / Claude Code (`.mcp.json`):

```json
{
  "mcpServers": {
    "stateset": {
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": { "Authorization": "Bearer <key>" }
    }
  }
}
```

The TypeScript SDK's `StreamableHTTPClientTransport` takes the same header via
`requestInit: { headers: { Authorization: 'Bearer <key>' } }`; from a shell:

```bash
curl -sS https://mcp.example.com/mcp \
  -H "Authorization: Bearer $KEY" \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'Mcp-Method: tools/list' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"curl","version":"0"},"io.modelcontextprotocol/clientCapabilities":{}}}}'
```

2025-era clients (those predating the `_meta` envelope) are still served, on the
SDK's stateless legacy leg, from the **same** tool factory — the two eras cannot
drift apart. Pass `--strict-protocol` for a modern-only endpoint that rejects
them with an unsupported-protocol-version error.

### Which MCP entrypoint?

| Binary | Transport | Store | Writes |
|---|---|---|---|
| `stateset-mcp` | stdio | your database | preview-only unless `--apply` |
| `stateset-mcp-http` | Streamable HTTP, 2026-07-28, stateless | shared `--db`, seeded when `:memory:` | enabled on `:memory:`, else `--apply` |
| `stateset-mcp-events` | stdio + HTTP event sidecar | your database | preview-only unless `--apply` |
| `stateset-x402-mcp` | stdio | x402 payment tools only | per its flags |

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
