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
npm install @stateset/cli@1.39.0 @stateset/embedded@1.39.0
```

For Python runtimes:

```bash
pip install stateset-embedded==1.39.0
pip install "stateset-embedded[agents]==1.39.0"
```

From the repo checkout, the examples under `examples/agents/` also run against
workspace modules directly, so the embedded path is smoke-tested before
publish.

## Guardrails quickstarts

Four runnable examples show the property that distinguishes this engine: give
an agent real refund powers and it still cannot over-refund, because the
invariant is enforced inside the database transaction and comes back as a
sealed kernel receipt with the stable code `commerce.refund.exceeds_captured`.

```bash
node examples/agents/vercel-ai-guardrails.mjs      # Vercel AI SDK
node examples/agents/langchain-guardrails.mjs      # LangChain structured tools
node examples/agents/openai-guardrails.mjs         # OpenAI tool calls
node examples/agents/custom-runtime-guardrails.mjs # any agent loop / Claude Agent SDK
```

Each runs in CI. See `examples/agents/README.md` for the pattern and the
[kernel execution](kernel-execution.md) chapter for production policy and
principal configuration. One note for agent authors: the adapter result's
top-level `success` means the tool call executed; branch on
`result.receipt.error_code` for the commercial outcome.

## Embedded Toolkit

The embedded toolkit gives your agent direct access to the full registry-generated commerce tool surface:

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';
import { createToolDescriptors } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');
const tools = createOpenAITools(commerce, {
  filter: ['list_customers'],
});
const execution = await executeOpenAIToolCall(commerce, {
  call_id: 'demo_call_1',
  function: {
    name: 'list_customers',
    arguments: '{}',
  },
});
const descriptors = createToolDescriptors(commerce, {
  filter: ['list_customers', 'list_orders', 'get_sales_summary'],
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

## Meta Muse personal assistant

For the Meta Muse personal assistant, use the separate
[Muse HTTP/OpenAPI connector](guides/meta-muse-connector.md). It exposes a
curated store tool list with Bearer authentication and write previews.

## WebMCP browser connector

The dependency-free `@stateset/embedded/webmcp` entrypoint registers commerce
tools with the browser's `document.modelContext` API, with feature detection
for the older `navigator.modelContext` entrypoint. It follows the
[WebMCP draft](https://webmachinelearning.github.io/webmcp/). Browser support is
experimental; when unavailable, registration returns `supported: false`.

Keep the native `Commerce` instance and toolkit on your backend. Serialize a
small allowlist of descriptors for the current page and authenticated user:

```javascript
// Server: use your existing policy-configured toolkit.
const descriptors = toolkit.createToolDescriptors({ filter: ['list_products', 'get_product'] });
const browserCatalog = descriptors.map(({ name, description, inputSchema, permission }) => ({
  name,
  description,
  inputSchema,
  permission,
}));
// Return browserCatalog from your authenticated GET /api/commerce/tools route.
```

Native toolkit names use `products.list` and `products.get`; CLI toolkit names
use `list_products` and `get_product`. Use names from your chosen toolkit.

```javascript
// Browser: no native bindings or CLI dependencies are imported here.
import { registerWebMCPTools } from '@stateset/embedded/webmcp';

const response = await fetch('/api/commerce/tools');
if (!response.ok) throw new Error('Could not load commerce tools');
const descriptors = await response.json();
const registration = await registerWebMCPTools(descriptors, {
  // Writes remain preview-only unless trusted host code enables allowApply.
  executeTool: async (name, params, { signal }) => {
    const response = await fetch('/api/commerce/execute', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, params }),
      signal,
    });
    if (!response.ok) throw new Error('Commerce tool failed');
    return response.json();
  },
});

// Call when the view unmounts, tools change, or the user signs out.
registration.dispose();
```

The backend execute route must independently authenticate the user, authorize
the tool and record scope, validate arguments, and enforce the toolkit's
`allowApply`, capabilities, and kernel policy. Browser hints and browser
configuration do not grant backend permissions. Keep policy, principal,
credentials, and database paths on the server. The adapter passes decimal
strings and results through unchanged. Cancellation is forwarded to the
executor; it does not roll back a mutation already committed on the server.

`createWebMCPTools()` builds tool definitions without registering them. Both
helpers accept JSON descriptors plus an `executeTool` transport, or local
descriptors with `execute` callbacks (for an existing browser/WASM app).
Only descriptors with `permission: 'read'` are classified as reads; unknown
permissions remain preview-only. `filter: []` exposes no tools. Registration
uses an owned abort signal to clean up tools, including on partial failure,
without affecting unrelated tools. A host `signal` can also end registration.

Run the local read-only catalog demo from the repository:

```bash
node examples/agents/webmcp/server.mjs
# Open http://127.0.0.1:8091 in a WebMCP-capable browser.
```

The demo uses a fresh in-memory database and exposes only `products.list` and
`products.get`. Its catalog button works when WebMCP is unavailable. It is a
local example; production storefronts should reuse their authenticated API.

The [browser verification guide](../../examples/agents/webmcp/README.md)
includes Chrome's local feature flag and a repeatable native browser check.
Discovery, execution, previews, rollback, and cleanup have been verified with
Chromium 153.0.8010.47. Legacy implementations with `unregisterTool` receive
explicit cleanup, including when a host aborts during registration.

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
  params: { orderId: 'ord-123', trackingNumber: 'FEDEX-789' },
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
  { tool: 'list_orders', params: { limit: 10 } },
]);
```

### Enable Writes

```javascript
const toolkit = createEmbeddedAgentToolkit({
  commerce,
  allowApply: true, // Enables write operations
  capabilities: ['read:*', 'payments.create'],
});
```

`allowApply: true` is rejected unless `capabilities` is explicit. Exact tool
names, `read:*`, `permission:write`, domain wildcards, and governed kernel
capabilities such as `payments.create` are supported. High-risk tools still
require their configured approvals.

## Reading Tool Results

Every tool call — through the MCP server (stdio or HTTP) or the embedded
toolkit — is judged by one result contract
(`cli/src/mcp/tool-result-contract.js`). Branch on it, not on prose:

```javascript
{
  ok: boolean,        // false exactly when the call failed or was refused
  preview: boolean,   // true when nothing was mutated (no --apply, dry run)
  result?: unknown,   // the tool's own payload, untouched
  error?: {           // present exactly when ok === false
    code: string,     // stable, machine-readable
    message: string,
    retryable: boolean,
    hint?: string,
  },
  notice?: { code, message, hint? },  // why a result is a preview
}
```

- **A tool's own failure is a failure.** `{ success: false, error }`, a truthy
  `error`, and a kernel receipt whose `status` is `rejected` or `failed` are
  all `ok: false` — a rejected `ship_order` no longer looks like a success.
- **A preview is not an error.** A write without `--apply` is `ok: true,
preview: true` with `notice.code === 'APPLY_REQUIRED'` (`DRY_RUN` for a dry
  run, `KERNEL_PREVIEW` for a kernel preview). A governed preview whose
  receipt says the command _would_ be rejected is `ok: false, preview: true`.
- **Money is never rewritten.** `result` is the tool's payload by reference;
  the contract never converts strings to numbers. Prefer the `*Exact` decimal
  strings (`totalAmountExact`, `grandTotalExact`, `amountExact`, …) over their
  deprecated float twins.

`error.code` comes from the most structured source available:

| Source                                                                             | `error.code`                                                                                                                                                                    | `retryable`                                                                                                      |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Kernel receipt `status: rejected/failed`                                           | the receipt's `error_code` (e.g. `commerce.invalid_order_status_transition`), else `KERNEL_REJECTED` / `KERNEL_FAILED`                                                          | from the receipt's `retry`: `same_key`, `after_conflict`, `after_delay` are retryable; `never` is not            |
| Thrown engine/binding error                                                        | its `code`: `NOT_FOUND`, `CONFLICT`, `VALIDATION`, `INSUFFICIENT_STOCK`, `PRECONDITION_FAILED`, `POLICY_REJECTED`, `DATABASE`, `EXTERNAL_SERVICE`, `INTERNAL`, `INTERNAL_PANIC` | `CONFLICT` only for version/optimistic conflicts (not duplicates); `DATABASE` only when the store is busy/locked |
| `{ success: false }` with `errorCode` / `error_code` / `reasonCode` / `error.code` | that code                                                                                                                                                                       | no                                                                                                               |
| Input fails the tool schema                                                        | `INVALID_INPUT` (hint lists the failing fields)                                                                                                                                 | no                                                                                                               |
| Unknown tool                                                                       | `UNKNOWN_TOOL`                                                                                                                                                                  | no                                                                                                               |
| Permission gate / MCP policy / hook                                                | `PERMISSION_DENIED` / `POLICY_DENIED` / `HOOK_BLOCKED`                                                                                                                          | no                                                                                                               |
| Priced tool without payment / treasury refusal                                     | `PAYMENT_REQUIRED` / `TREASURY_BLOCKED`                                                                                                                                         | no                                                                                                               |
| Message says not found / timed out / rate limited                                  | `NOT_FOUND` / `TIMEOUT` / `RATE_LIMITED`                                                                                                                                        | timeouts and rate limits only                                                                                    |
| Anything else                                                                      | `TOOL_ERROR`                                                                                                                                                                    | no                                                                                                               |

**On the MCP wire** a `CallToolResult` carries the outcome in
`structuredContent` and sets `isError: true` exactly when `ok` is false. The
text content is still the tool's own JSON, so text-only clients see what they
always saw; a thrown error's text now also names its `code`. Previews are no
longer flagged `isError`.

**From `executeTool`** (embedded toolkit, `server.executeTool`) the outcome
is flattened next to the existing fields: `ok`, `preview`, `failure` (the
contract's `error` object, `null` when ok), `notice`, and `result` (now the
tool's full result — earlier it was the compacted replay-log copy). `error`
stays the message string, and `success` keeps meaning "the tool executed and
succeeded": it is now also false when the tool's own result failed, and still
false for a preview. `toToolResultContract(executed)` rebuilds the canonical
object.

```javascript
const shipped = await toolkit.executeTool('ship_order', { orderId, trackingNumber });
if (!shipped.ok) {
  if (shipped.failure.retryable) {
    /* reload and retry */
  } else throw new Error(`${shipped.failure.code}: ${shipped.failure.message}`);
} else if (shipped.preview) {
  // nothing changed: re-run with --apply
}
```

## MCP-Native Clients

For Claude Desktop, Cursor, Windsurf, or other MCP-native clients:

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This registers the iCommerce MCP server with your client. The core tool tier appears automatically in the tool palette (see [Tool tiers and profiles](#tool-tiers-and-profiles) to widen it).

### MCP Configuration

The setup creates a configuration entry in your MCP client's config file:

```json
{
  "mcpServers": {
    "stateset-commerce": {
      "command": "npx",
      "args": ["-y", "-p", "@stateset/cli@1.39.0", "stateset-mcp", "--db", "./store.db"]
    }
  }
}
```

### Tool tiers and profiles

Every MCP tool has a stability tier, and with no `--profile` the servers expose
exactly the **core** tier: the everyday commerce loop (catalog, customers,
carts and checkout, orders, payments and refunds, returns, shipments,
inventory, promotions, tax, gift cards and store credit, analytics) plus the
agentic planning and replay tools. Every core tool is driven through the real
server on a fresh store in CI and must work or refuse cleanly — no crashes, no
known-defect backlog.

| Tier           | Exposed by                                         | Contents                                                                                                                    |
| -------------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `core`         | default (`--profile core`)                         | the commerce loop above                                                                                                     |
| `extended`     | `--profile all`, a curated profile, or `--domains` | finance suite, manufacturing, WMS, B2B, subscriptions, reviews, loyalty, integrations                                       |
| `experimental` | `--profile all`, a curated profile, or `--domains` | A2A, agent receipts, sync, vector search, on-chain treasury — tools that need a wallet, a chain, an API key or a demo stack |

`--profile finance`, `operations` and `agents` expose every tool in their
domains whatever its tier; `--profile all` exposes everything. The generated
catalog [`cli/docs/TOOLS.md`](https://github.com/stateset/stateset-icommerce/blob/master/cli/docs/TOOLS.md)
lists each tool's tier and each profile's size.

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

Use `--host ::1` for IPv6 loopback (`http://[::1]:8090/mcp`). Pass `--port 0`
to let the OS choose an available port; the startup message reports the actual
endpoint. Port arguments must contain only decimal digits in the range 0–65535.

Three request guards sit in front of the MCP handler, in the order
Host → Origin → Auth:

- **Host (DNS rebinding)** — `--allowed-host <hostname>` (repeatable). A
  loopback bind allows only localhost Host values by default. Any other
  `--host` **refuses to start** without `--allowed-host`; pass
  `--insecure-allow-any-host` only behind a proxy that already pins the Host
  header.
- **Origin (browser cross-origin)** — `--allowed-origin <origin>` (repeatable;
  a full origin like `https://agent.example.com:8443`, matched by scheme,
  hostname and port). Explicit bare hostnames retain HTTP(S), any-port matching
  for compatibility. Malformed, opaque, duplicate or unlisted origins get `403`. A loopback bind
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

| Binary                | Transport                              | Store                                 | Writes                                |
| --------------------- | -------------------------------------- | ------------------------------------- | ------------------------------------- |
| `stateset-mcp`        | stdio                                  | your database                         | preview-only unless `--apply`         |
| `stateset-mcp-http`   | Streamable HTTP, 2026-07-28, stateless | shared `--db`, seeded when `:memory:` | enabled on `:memory:`, else `--apply` |
| `stateset-mcp-events` | stdio + HTTP event sidecar             | your database                         | preview-only unless `--apply`         |
| `stateset-x402-mcp`   | stdio                                  | x402 payment tools only               | per its flags                         |

## Embedded-engine simulations

Agents can run what-if scenarios through the native engine without reading or
changing their live store. The `simulations` domain is included in the core, operations and
agents MCP profiles. It provides six tools:

| Tool                             | Purpose                                                                                      |
| -------------------------------- | -------------------------------------------------------------------------------------------- |
| `get_simulation_kernel_contract` | Discover action/plan schemas, limits, observations and iteration semantics                   |
| `simulate_inventory_experiment`  | Evaluate policies across shared demand seeds with empirical percentiles and paired deltas    |
| `simulate_inventory_plan`        | Project demand, stockouts, purchases, receipts, sales and cash                               |
| `compare_inventory_plans`        | Compare two or three replenishment policies or demand assumptions                            |
| `simulate_commerce_scenario`     | Execute typed purchasing, receiving, order, shipment, payment, refund and inspection actions |
| `compare_commerce_scenarios`     | Compare two or three commerce scenarios                                                      |

`simulate_inventory_experiment` adds shared-seed, multi-path policy evaluation.
Each run creates private SQLite files in a bounded child process. There is no
model-supplied database path, executable code, provider callback, or live-store
handle. The tools are reads relative to the operator store, so they work without
`--apply`; all mutations take place in disposable simulation stores. Purchases,
receipts, paid orders, and stock-consuming shipments use native engine APIs.
Purchase receipt quantities and inventory postings are separate transactions.
Shipping confirms holds; the runner then calls `inventory.fulfillOrderStock`
to post physical stock atomically for the fully shipped order. This explicit
operation is also available as the apply-gated `fulfill_order_stock` MCP tool
and is safe to retry.
Captures and refunds update the local ledger and transfer no money.

Use it as an execution kernel for a model's commerce hypotheses: discover the
contract, construct a baseline, run typed actions, inspect observations, compare
alternatives, and revise the inputs. `inspect_state` reads native stock, order
and payment status, plus purchase lines and received quantities at that point
in the episode. The final `state` is also returned. `purchase_inventory` takes
an `id` alias and items with `sku`, integer `quantity` and decimal-string
`unitCost`; `receive_purchase_order` references that purchase alias.
`ship_order` references an order alias and explicitly posts its physical stock.
Manual episodes record purchasing without a cash-budget rule; planning episodes
enforce the prepaid purchasing budget described below.

An action can specify `expect: "rejected"` to test a refusal. Unexpected
refusals set `expectationsMet: false` while later actions continue, allowing
state inspection after failures. Overall `success` means the worker completed;
check `expectationsMet` before accepting a manual episode. Runs are stateless:
revise the input and replay a fresh episode rather than retaining a session.
Native timestamps use wall time; virtual planning days order events without
changing the native clock.

Run the included examples from the repository root:

```bash
node cli/bin/stateset-simulate.js --inventory-plan cli/examples/inventory-plan.json --json
node cli/bin/stateset-simulate.js --commerce cli/examples/commerce-scenario.json --json
```

A JSON array compares inputs. The same seed, SKU and demand inputs
produce common demand paths for policy comparisons, even when products are
reordered or another SKU is added. Demand reports name the generator version
and show conditional expected units alongside the actual seeded daily path. The existing A2A
`--scenario` mode remains available separately; native modes reject live
settlement and operator database paths.

A planning request supplies daily demand ranges (`{"min":3,"max":8}`),
historical daily units (`{"history":[3,5,4,8,6,9,4]}`), or an explicit schedule
(`{"dailyUnits":[0,0,8,0,0,2]}` for a six-day horizon), opening stock, exact
sale/cost strings, available cash, reorder point, target stock and lead time.
Historical mode resamples observed daily units with a seeded generator. Initial
replenishments are placed on day zero; receipts arrive before daily demand.
When available stock plus outstanding purchases reaches the reorder point,
the policy orders up to target if cash covers the whole order. All daily sales
are captured before end-of-day purchasing; product input order defines purchase
priority when cash is limited. Purchases are
prepaid, sales capture immediately, and unmet demand becomes lost sales.

Explicit schedules replay the supplied trajectory in order: entry zero is
virtual day one, and there is no demand entry for day zero. They must contain
exactly `days` integer entries, each between 0 and 10000. Missing/extra days and
mixed demand modes are rejected before execution. The runner never truncates,
repeats or resamples a schedule, so demand seeds do not alter it. Reports label
both the projection and its assumptions `explicit-daily-units`; expected horizon
units equal the supplied sum. An agent can use this mode for a forecast curve,
promotion, launch, seasonal scenario or outage. The curve is an input assumption,
not an automatically fitted forecast. Different seeds still vary any sampled
supplier conditions.

The [promotion-spike experiment](../../cli/examples/inventory-demand-spike-experiment.json)
compares lean stock with a buffer under the same six-day schedule, including
real native purchases, arrivals, payments and shipment stock postings:

```bash
node cli/bin/stateset-simulate.js --inventory-experiment cli/examples/inventory-demand-spike-experiment.json --json
```

Opening inventory is already paid for; gross margin values sold units at the
supplied constant unit cost. Reports expose these assumptions, daily events,
stockouts, outstanding orders, exact cash totals and the input hash. Each run
reconciles native stock and record counts, cash movements, and inventory value
before reporting success. Comparisons include a metric table and exact money
deltas against the first plan, with explicit flags for matching horizons and
demand paths; they do not automatically select a purchasing policy.

These are conditional planning projections, not calibrated forecasts or
purchasing recommendations. Each input yields one demand path, without
confidence intervals, trend/seasonality fitting, permanent supplier losses, taxes,
fees or holding costs. No production data is imported implicitly. Single plans
allow 120 product-days; comparisons allow 180 across at most three plans.
Commerce scenarios allow 100 total actions. Runs time out after 30 seconds,
with at most two active workers per process. Temporary stores are removed after
worker exit during normal completion, failures and timeouts.

Admission is also bounded before schema validation, across the SDK and MCP
execution schemas. The entire request must fit 128 KiB, 16384 data nodes and
32 nesting levels (the root is level one). JSON string escapes and UTF-8 bytes
count toward the byte limit. Plain objects, arrays and finite JSON scalars are
accepted for typed validation; cycles, accessors, proxies, class instances,
symbol/custom array properties and executable values are refused. Ordinary
repeated references in SDK inputs are counted each time, without being mistaken
for cycles. Optional JavaScript `undefined` values remain supported and count
as `null`, conservatively including their fields in the budget. The full
comparison is checked before its cases, and the canonical request is checked
again after defaults. Invalid inputs do not allocate a simulation worker or
private database; MCP returns a structured non-retryable input refusal.

Inventory plans and every experiment run expose `cashFlow.daily`, with one row
for day zero and each simulated day: `openingCash`, `salesReceipts`,
`purchasingSpend`, `closingCash` and `minimumCash`. Opening cash participates in
the minimum, as does the balance after every native-backed sale and prepaid
purchase. `summary.minimumCash` is the overall low point;
`cashFlow.minimumCashAt` identifies its first occurrence, including SKU and
purchase alias when applicable. Equal minima retain the first occurrence.
Receipts, missed deliveries and blocked purchase attempts do not move cash.
Flows must reconcile independently against the recorded timeline and totals
before `reconciliation.cashFlowBalanced` is set.

Add `"minCashBalance": "5.00"` to experiment `goals` to check a minimum cash
reserve throughout each run. This can miss even when `minEndingCash` is met:
prepaying purchases can exhaust cash before sales replenish it. The threshold
is inclusive, uses exact decimal strings in the experiment currency, and is
evaluated jointly with the other per-run goals. It does not modify the policy
or prevent a purchase that would breach the requested reserve. Comparisons and
experiment distributions include `minimumCash` as an exact monetary metric.

To enforce a purchasing reserve, set `cashReserve` on each inventory plan or
experiment policy, for example `"cashReserve": "5.00"`. This optional exact
decimal amount uses the plan currency and defaults to zero when omitted.
All products share `max(cash - cashReserve, 0)` in the existing purchase
priority. The engine checks rounded whole-order quantities and inventory caps
first, then actual cash, then the reserve. It never shrinks an order to fit.
Blocked attempts create no native PO and spend nothing; later sales can make
the same policy affordable. Zero-cost orders remain possible when cash is
below the reserve. Starting below the reserve is allowed and creates no funds;
`minCashBalance` still evaluates the observed shortfall.

`purchase_reserve_blocked` timeline events include the required cost, cash,
reserve and spendable cash. `summary.reserveBlockedPurchases` counts attempts,
including repeated attempts on later days; experiment `reserveBlockedRuns`
counts runs with at least one such block. These are separate from insufficient
actual-cash and inventory-cap blocks. `sameCashReserve` compares normalized
reserve amounts across policies (omitted and explicit zero are equivalent).
Independent cash-flow reconciliation checks reserve compliance, block evidence
and counts before accepting results. A complete comparison is provided in
`cli/examples/inventory-cash-reserve-experiment.json`.

Supplier lead times accept a fixed integer or a bounded range such as
`"leadTimeDays": { "min": 1, "max": 4 }`. Add optional delivery assumptions:

```json
"delivery": {
  "firstReceiptBps": { "min": 2500, "max": 10000 },
  "remainderDelayDays": 2
}
```

Both `leadTimeDays` and `delivery.firstReceiptBps` also accept an explicit
`{ "dailyValues": [...] }` schedule. For a four-day plan:

```json
{
  "leadTimeDays": { "dailyValues": [3, 9, 9, 1, 9] },
  "delivery": {
    "firstReceiptBps": { "dailyValues": [5000, 0, 0, 10000, 0] },
    "remainderDelayDays": 1
  }
}
```

There must be exactly `days + 1` integer entries, indexed by **order-placement
day**, including initial purchases on day zero. Lead times stay within 1..60;
first-fill values stay within 0..10000. In this example a day-zero order arrives
on day three with half its units first; a day-three order arrives on day four
in full. Array index is neither arrival day nor purchase-order number. Existing
orders retain the conditions selected at placement. Values for days with no
orders are unused; schedules are never cycled, truncated or resampled.

Explicit fields ignore seeds while other sampled fields retain their original
independent streams. Supply projections expose `leadTimeMethod` and
`firstReceiptMethod` as `fixed`, `uniform-integer` or `explicit-daily-values`.
Path hashes reflect projected values, so equivalent scalar and scheduled paths
match. Repeated seeds with fully fixed or explicit supply do not create extra
distinct paths. Combine supplier schedules with `demand.dailyUnits` to test a
specific simultaneous demand and supply shock. See
`cli/examples/inventory-supply-calendar-experiment.json`.

Basis points divide by 10000: 2500 represents a 25% first delivery. The first
quantity is rounded down to whole units; the balance arrives after the remainder
delay. A zero first delivery logs `purchase_delivery_missed` without submitting
a native zero receipt. These are delayed balances, not permanent supplier loss
or supplier refunds. Unreceived units remain prepaid in-transit inventory at the
horizon. Reports distinguish `partialReceipts`, `missedDeliveries`, receipt-event
counts and fully received purchase orders. Outstanding orders expose ordered,
received and remaining quantities plus the next arrival day.

`mulberry32-supply-v1` uses independent streams per SKU and order-placement day,
including day zero. Changing reorder policy, quantities or product ordering
preserves common supplier conditions when assumptions and seed match, and does
not perturb the existing demand stream. Supply paths expose sampled daily lead
times and first-delivery basis points. Each report verifies native PO status,
line quantities, unit cost and total, as well as native stock and exact cash,
on-hand value, in-transit value and cost of goods sold.

Full plan reports retain normalized native `purchases` rows with status, exact
costs and ordered/received quantities. Experiment `disruptedSupplyRuns` counts
runs where at least one partial or missed first delivery occurred within the
horizon; supplier delays also affect the stockout and lost-sales metrics.

```bash
node cli/bin/stateset-simulate.js --inventory-experiment cli/examples/inventory-supply-experiment.json --json
```

Per-product `purchasing` supplies minimum-order and case-pack assumptions:

```json
{
  "purchasing": {
    "minimumOrderQuantity": 5,
    "orderMultiple": 4,
    "maximumInventoryPosition": 12
  }
}
```

When the reorder trigger fires, take the larger of the target shortfall and
minimum quantity, then round upward to an order multiple. A one-unit shortfall
with a five-unit minimum and four-unit case pack orders eight units. This may
exceed target stock. The optional inventory-position cap checks available stock
plus outstanding quantities plus the rounded order; only then does the policy
check cash for the whole order. The simulation bounds rounded orders to one
million units. Minimum quantities and pack sizes default to one.

These are model-supplied scenario assumptions. A blocked attempt creates no
native PO or purchasing spend. `purchase_constraint_blocked` events explain the
requested and rounded quantities, current position, required position and cap;
`purchase_budget_blocked` events use the rounded cost. Capacity takes precedence
if both limits block the attempt. `constraintBlockedPurchases` counts attempts,
including repeated eligible days. `roundedPurchaseOrders` and
`extraPurchasedUnits` count actual orders and cumulative units added by sizing,
rather than end-of-horizon excess inventory. Experiments expose
`constraintBlockedRuns` and paired quantity/constraint metric deltas. Every native
PO must satisfy the supplied minimum, multiple and position cap before the
report sets `purchasingConstraintsBalanced: true`.

```bash
node cli/bin/stateset-simulate.js --inventory-experiment cli/examples/inventory-purchasing-experiment.json --json
```

Purchasing policies may also set `purchasing.reviewIntervalDays`, an integer
from 1 to 60. Reviews are anchored at virtual day zero: `7` allows replenishment
on days 0, 7, 14, and so on. Omitted or `1` retains daily review. These intervals
do not represent weekdays, holidays or an external supplier calendar.

If inventory position is above the reorder point there is no action. Otherwise,
off-review days emit `purchase_review_deferred`, with `inventoryPosition`,
`reviewIntervalDays`, `nextReviewDay`, and unchanged `cash`. These attempts create
no native PO, receipt or payment. They do not reserve stock, reserve cash or
freeze a pending purchase quantity. On the next review, the planner recomputes
current available-plus-inbound stock and quantity before MOQ/pack sizing,
capacity, actual cash and reserve checks. Supplier terms are selected on that
actual placement day. Reviews beyond the planning horizon remain unexecuted.

`summary.deferredPurchaseReviews` counts eligible low-stock attempts deferred
off schedule, including repeated days. Experiments expose its empirical and
paired metrics, plus `reviewDeferredRuns`. Independent inventory replay checks
review dates, deferral evidence and counts; cash replay checks the unchanged
balance. Schedule checks precede sizing and financial blocks, so a low-stock
off-review day is a deferral even when capacity or cash would also block a PO.

The example below keeps projected demand and supplier paths identical and
changes only the review interval. Daily review fills all four units; a three-day
review fills three and misses day-three demand, with purchases on days 0 and 3:

```bash
node cli/bin/stateset-simulate.js --inventory-experiment cli/examples/inventory-review-interval-experiment.json --json
```

For multiple demand paths, call `simulate_inventory_experiment` with
`{ name, plans, seeds }`. Plans have the same fields as a single inventory plan,
except `seed` is omitted and rejected: supply two to eight distinct shared seeds
at experiment level. Each plan/seed pair runs in its own private database.
At most three plans, 12 total runs and 180 total product-days are allowed.

The `experiment` result contains per-policy metrics, observed p10/p50/p90
percentiles, `stockoutRuns` and `budgetBlockedRuns`, plus paired metric deltas
against the first policy for matching seed positions. Delta signs are raw
positive/zero/negative counts; their desirability depends on the metric. Check
`sameHorizon`, `sameDemandPaths` and `sameSupplyPaths` before treating differences as policy effects.
Money percentiles are exact decimal strings. Money means are exact rational
objects, for example `{ "numerator": "10.01", "denominator": 3 }`, without
rounding the amount to a float. Percentiles use nearest rank over the supplied
sample, so small samples are coarse and never constitute calibrated confidence
bounds or a policy recommendation.

Each inventory plan report and experiment run includes `inventoryFlow` alongside
`cashFlow`. `inventoryFlow.daily` covers virtual days zero through the horizon.
Each day has a `products` array in supplied purchase-priority order, with:

- `sku`, `openingStock`, and `openingOnOrder`;
- `receivedUnits`, `demanded`, `sold`, and `lostSales`;
- `orderedUnits`, `closingStock`, and remaining `onOrder`.

Stock means physical available units; inbound units remain separate until an
actual receipt. A missed first delivery receives zero units and leaves the
balance inbound. `firstStockout` returns the first unmet-demand event as
`{ day, sku, demanded, sold, lostSales }`, or `null` when all demand is met.
If several SKUs first miss demand on the same day, supplied product order breaks
the tie. Zero closing stock without lost demand is not a stockout.

The kernel independently replays these movements against projected demand,
supplier terms selected on each order-placement day, purchase aliases, split
receipt quantities and dates, native inventory positions and final balances.
It rejects sales before stock exists, missing or repeated demand observations,
receipts without a matching order, and offsetting daily errors even when final
totals balance. Receipts precede all sales; sales precede all purchasing.
`reconciliation.inventoryFlowBalanced` is `true` only after replay succeeds,
and experiment aggregation requires it. The
`cli/examples/inventory-supply-calendar-experiment.json` report shows the
delayed receipt, first lost sales and subsequent recovery day by day.

Each policy's `pathDiversity` reports unique full projected `demandPaths`,
`supplyPaths` and demand/supply `jointPaths`, plus `duplicateJointPathRuns`.
These counts use the normalized trajectory hashes, excluding seed labels;
fixed demand and fixed supply therefore yield one joint path even with several
different seeds. Counts describe full supplier conditions for all possible
order-placement days, not just observed receipts or unique financial results.
Identical paths retain each supplied seed's sample weight in metrics and goal
counts. Path counts do not measure independence or an effective sample size;
inspect them before treating repeated successful runs as additional evidence.

Experiments may supply shared business `goals`:

```json
{
  "minFillRateBps": 9500,
  "minEndingCash": "15.00",
  "minGrossMargin": "5.00",
  "maxPurchasingSpend": "20.00"
}
```

All fields are optional, but a supplied `goals` object must contain at least
one. Money thresholds use decimal strings in the experiment currency;
`minGrossMargin` also permits negative amounts. Comparisons are inclusive and
exact. `minEndingCash` tests the final balance, not the lowest balance during
the episode. `minFillRateBps` compares sold/demanded against basis points
(9500 means 95%) without rounding a percentage.

Use `goals.productFillRates` to protect specific SKUs when overall service can
hide a critical stockout:

```json
{
  "minFillRateBps": 9900,
  "productFillRates": [{ "sku": "CRITICAL", "minFillRateBps": 10000 }]
}
```

Supply one to eight unique targets, each naming a SKU present in every policy.
Each target compares that SKU's sold and demanded units on every seed, with the
same inclusive integer basis-point comparison as overall fill rate. All product
targets and other goals must pass on the same seed. A zero-demand SKU is
`not-evaluable`, even with a zero threshold; sales of other products cannot
compensate for it or for a known miss.

Run `evaluation.checks.productFillRates.products` contains each SKU, threshold,
exact `actual` fraction (or `null`) and status. Policy
`evaluation.byGoal.productFillRates.products` adds individual
`metRuns`, `missedRuns`, `notEvaluableRuns` and `serviceLevel` pooled across the
supplied seeds. The enclosing product goal counts runs where all targets pass
together. Pooled per-SKU service does not replace per-seed goal checks.

The example below contrasts on-time and delayed delivery of one critical unit
against 99 units of bulk demand. Both policies meet the 99% overall threshold;
only on-time supply meets the critical SKU's 100% target:

```bash
node cli/bin/stateset-simulate.js --inventory-experiment cli/examples/inventory-product-service-experiment.json --json
```

Each run's `evaluation` contains its status and `checks` keyed by goal, with
the threshold, actual value and `met`, `missed`, or `not-evaluable` status.
Policy `evaluation` includes `metRuns`, `missedRuns`, `notEvaluableRuns`,
`allRunsMet`, and `byGoal` counts. A run meets the goals only when all checks
pass on that same seed. A known miss makes the run `missed` even if another
check is not evaluable. These business outcomes leave execution `success`
unchanged. With no supplied goals, `evaluation` is `null`.

Run and policy `serviceLevel` reports contain demanded/sold/lost units and an
exact `fillRate: { numerator, denominator }`. Policy fill rate pools units
across seeds; it does not average per-run percentages. A zero-demand run has
`fillRate: null`; its fill-rate goal is `not-evaluable`, including a zero
threshold. `zeroDemandRuns` identifies these cases at policy level. Counts
describe the sampled seeds, not probabilities of meeting goals in production.

Before attributing outcome differences to replenishment alone, also inspect
`sameOpeningCash`, `sameOpeningInventory`, `sameUnitEconomics`, and
`sameProductPriority`. The first three compare exact amounts and SKU facts;
product order is checked separately because it controls cash allocation.
Changing these assumptions is allowed and explicitly disclosed. Goals do not
rank policies or authorize changes to the live store.

Both single-run comparisons and experiments now use these same condition checks.
Each single-run comparison row and experiment policy includes
`comparisonToBaseline`, with `sameHorizon`, `sameOpeningCash`, `sameCashReserve`,
`sameOpeningInventory`, `sameUnitEconomics`, `sameProductPriority`,
`sameDemandPaths`, and `sameSupplyPaths`. Its `changedConditions` array lists
mismatches as `horizon`, `openingCash`, `cashReserve`, `openingInventory`,
`unitEconomics`, `productPriority`, `demandPath`, or `supplyPath`. An empty array
means these checks match the first plan. Purchasing rules remain visible in the
supplied plans. Global flags require all alternatives to match; per-policy
diagnostics remain specific to each alternative when a third policy changes
conditions.

Money comparisons normalize decimal representations exactly, including
sub-cent differences beyond JavaScript's safe integer range. Opening stock and
prices/costs compare facts by SKU, while input product order is checked
separately as purchase priority. Experiment path checks pair every run by seed.
Matching paths describe realized projections for the supplied seeds; they do
not establish equivalent generating assumptions, causal attribution or
statistical significance. Single-run top-level path fields retain their names
`sameDemandPath` and `sameSupplyPath`; the nested diagnostics use the plural
names shared with experiments. See
`cli/examples/inventory-comparison-context-experiment.json` for a three-policy
example that separates a policy alternative from changed capital and prices.

Per-seed summaries include stock/cash/asset reconciliation and a normalized
demand and supply hashes. Replay a case with `runInventorySimulation({ ...plan, seed })` to
inspect its full timeline and demand path. The SDK exports
`runInventoryExperiment(experiment)` from `@stateset/cli/simulation`.

```bash
node cli/bin/stateset-simulate.js --inventory-experiment cli/examples/inventory-experiment.json --json
```

Applications that install `@stateset/cli` can use the same runner directly:

```javascript
import { getSimulationKernelContract, runInventorySimulation } from '@stateset/cli/simulation';
const kernel = getSimulationKernelContract(); // Pure discovery, including JSON schemas
const report = await runInventorySimulation({
  name: 'coffee',
  days: 14,
  seed: 42,
  startingCash: '1000.00',
  products: [
    {
      sku: 'COFFEE',
      initialStock: 30,
      unitPrice: '12.50',
      unitCost: '5.25',
      demand: { min: 3, max: 8 },
      reorderPoint: 15,
      targetStock: 50,
      leadTimeDays: 2,
    },
  ],
});
```

## Which Approach Should I Use?

| Scenario                              | Recommended                                                                       |
| ------------------------------------- | --------------------------------------------------------------------------------- |
| Claude Desktop / Cursor / Windsurf    | MCP server (automatic tool discovery)                                             |
| OpenAI GPT agents                     | `@stateset/embedded/openai`                                                       |
| Vercel AI SDK app                     | `@stateset/embedded/vercel-ai`                                                    |
| LangChain / LangGraph agent           | `@stateset/embedded/langchain`                                                    |
| Custom agent framework                | `@stateset/embedded/generic`                                                      |
| LangChain Python agent                | `create_langchain_tools()` or `create_callable_registry()`                        |
| CrewAI / AutoGen style Python runtime | `create_crewai_tools()` / `create_autogen_tools()` or `create_tool_descriptors()` |
| Server-side API                       | Embedded toolkit with `allowApply: true`                                          |
| Testing / exploration                 | CLI with `stateset "..."` (read-only)                                             |
