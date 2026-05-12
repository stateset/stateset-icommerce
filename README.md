# StateSet iCommerce Engine

**The SQLite of Commerce** - An embedded, zero-dependency commerce engine for autonomous AI agents.

AI agents that reason, decide, and execute—replacing tickets, scripts, and manual operations across your entire commerce stack.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/stateset/stateset-icommerce/actions/workflows/ci.yml/badge.svg)](https://github.com/stateset/stateset-icommerce/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/stateset/stateset-icommerce/graph/badge.svg)](https://codecov.io/gh/stateset/stateset-icommerce)

> **Looking to accept ChatGPT checkout?** iCommerce 1.0 pairs with
> [`stateset-acp-handler`](https://github.com/stateset/stateset-acp-handler)
> as the reference implementation of the Agentic Commerce Protocol.
> One container, 60 seconds: `npx create-acp-commerce@latest my-store`.

> 🆕 **Intelligent Commerce Protocol (ICP).** This repo also hosts the
> open spec and reference implementations of ICP-1.0 — the operational
> lifecycle layer (quote, escrow, fulfillment, dispute, settlement)
> that sits between checkout protocols (ACP/AP2) and payment rails
> (x402/USDC). **40+ end-to-end tests pass on every CI run** across
> HTTP handler, MCP server (Claude Desktop / Cursor / Windsurf), on-chain
> custody contract, off-chain Settler daemon, and a cross-language
> conformance suite. Start at **[ICP.md](./ICP.md)** or jump to the
> **[partnership packet](./icp-spec/PACKET.md)**.

---

**Install:**
```bash
cargo add stateset-sdk --features full   # Rust (recommended)
pip install stateset-embedded==1.3.0     # Python
npm install @stateset/embedded@1.3.0     # Node.js
npm install -g @stateset/cli@1.3.0       # CLI
gem install stateset_embedded -v 1.3.0   # Ruby
```

**Zero to commerce in 5 lines:**
```rust
use stateset_sdk::prelude::*;

let commerce = Commerce::new("store.db")?;
let customer = commerce.customers().create(CreateCustomer {
    email: "alice@example.com".into(),
    first_name: "Alice".into(),
    last_name: "Smith".into(),
    ..Default::default()
})?;
let order = commerce.orders().create(CreateOrder {
    customer_id: customer.id,
    items: vec![CreateOrderItem {
        sku: "WIDGET-001".into(),
        name: "Widget".into(),
        quantity: 2,
        unit_price: rust_decimal_macros::dec!(29.99),
        ..Default::default()
    }],
    ..Default::default()
})?;
println!("Order {} — ${}", order.order_number, order.total_amount);
```

No database setup. No config files. No migrations to run. It just works.

**[10-Minute Quickstart →](./QUICKSTART.md)** | **[API Reference](https://docs.rs/stateset-sdk)** | **[OpenAPI Spec](http://localhost:8080/api/v1/openapi.json)** | **[Security](./docs/src/security/overview.md)** | **[Trust Foundation](./TRUST_FOUNDATION.md)**

<details>
<summary><b>Table of contents</b></summary>

- [Why iCommerce](#why-icommerce) — what's different about a commerce engine built for AI agents
- [Engine-First Adoption](#engine-first-adoption) — embed it, don't service-mesh it
- [Embedded Agent Toolkit](#embedded-agent-toolkit-openai--langgraph--server-side-agents) — OpenAI / LangGraph / server-side
- [MCP Server](#mcp-server-claude-desktop--cursor--windsurf) — Claude Desktop / Cursor / Windsurf
- [What's New in v1.3.0](#whats-new-in-v110)
- [Architecture](#architecture) — Rust kernel, language bindings, operator runtime
- [Quick Start](#quick-start) — working snippets in every language
- [Production Notes](#production-notes) — running on Postgres, scaling, observability
- [Domain Models, MCP Tools, AI Agents](#domain-models)
- [Key Features](#key-features) — full feature matrix grouped by domain
- [Installation](#installation) — package commands + per-language gotchas
- [Language Bindings](#language-bindings) — Rust, Node, Python, Go, Java, Kotlin, Swift, .NET, Ruby, PHP, WASM
- [Configuration](#configuration) — SQLite vs Postgres, env vars
- [Examples & Development](#examples)
- [Project Structure](#project-structure) — how the monorepo is laid out
- [Core Concepts](#core-concepts) — ACP, safety architecture, event-driven design

</details>

---

## Why iCommerce

Most commerce platforms expose APIs that AI agents can call. iCommerce is built
*for* AI agents — with the protocol, runtime, and economic primitives that
autonomous agents actually need to transact safely:

- **🤝 [Agent-to-Agent (A2A) Commerce](./AGENTIC_COMMERCE.md)** — agents
  negotiate, quote, escrow, and settle directly. Reputation and verification
  built in. Splits, subscriptions, conditional payments, webhook events.
- **💳 [x402 Payment Protocol](./docs/src/payments/x402.md)** — cryptographically
  verifiable AI-agent payment intents (Ed25519, replay-protected nonces, optional
  PQC). On-chain settlement on Solana, SET Chain, Base, Ethereum, Arbitrum.
- **🧠 [Autonomous Engine](./cli/src/autonomous/)** — scheduled jobs, state-machine
  workflows, policy-as-code rules, webhook event handlers, and human-in-the-loop
  approvals. One stack to run a business unattended.
- **📜 [Policy DSL](./crates/stateset-policy/)** — declarative, deny-overrides
  rules with explainable denials. Block agents from writes when limits are
  exceeded; record the reason for every decision.
- **🛠️ [700+ MCP Tools](./docs/whitepaper.md#mcp-tools) across 63 domain modules** —
  the largest known domain-specific MCP surface. Discoverable, payable per call
  via Machine Payments Protocol, replayable, audit-logged.
- **🔐 [Verifiable Encrypted Signatures (VES v1.0)](./docs/PQC_INITIAL_SPEC.md)**
  — Merkle-anchored event log with Ed25519 + ML-DSA-65 hybrid signatures and
  X25519 + ML-KEM-768 hybrid encryption. Every state change is signed and
  reconstructable.
- **⚙️ Embedded engine, not a service** — single-process SQLite by default,
  PostgreSQL when you scale up. No separate database to provision. No network
  hops between agent and runtime.

Most of the differentiators above are *not* "coming soon" — they're shipping in
v1.0.x. See [`AGENTIC_COMMERCE.md`](./AGENTIC_COMMERCE.md) for the full A2A
case study and [`TRUST_FOUNDATION.md`](./TRUST_FOUNDATION.md) for the security
and verification architecture (including the explicit gap inventory: PQC hard
finality and SOC 2 are honest "in progress" items, not aspirational claims).

---

## Engine-First Adoption

If the bet is the engine, the primary product surface is the embedded runtime:

- embed `@stateset/embedded` or `stateset-sdk` inside the agent host process
- expose tools through `@stateset/embedded/openai`, `@stateset/embedded/generic`, `@stateset/embedded/langchain`, or `@stateset/embedded/vercel-ai`
- integrate with agent frameworks through native adapters or OpenAI-compatible tool schemas
- treat the MCP server as a distribution path for MCP-native clients, not as the center of the product

Today the embedded toolkit supports:

- OpenAI Responses / Chat Completions style tool loops via `@stateset/embedded/openai`
- Vercel AI SDK via `@stateset/embedded/vercel-ai`
- LangChain / LangGraph JS via `@stateset/embedded/langchain`
- framework-neutral runtimes via `@stateset/embedded/generic`
- Python agent runtimes via `create_embedded_agent_toolkit()` in `stateset_embedded`
- Python framework helpers via `create_langchain_tools()`, `create_crewai_tools()`, and `create_autogen_tools()`
- Python OpenAI helper surface via `create_openai_tools()` and `execute_openai_tool_call()`
- Python framework-neutral helper surface via `create_tool_descriptors()`, `create_callable_registry()`, and `execute_tool()`
- Python framework modules via `stateset_embedded.langchain`, `stateset_embedded.crewai`, and `stateset_embedded.autogen`
- MCP-native clients like Claude Desktop / Cursor / Windsurf via the CLI server

For Python ecosystems such as CrewAI or AutoGen, the native Python toolkit now
covers core embedded commerce operations with OpenAI-compatible tool schemas,
framework-neutral descriptors, and framework adapter helpers. Install
`stateset-embedded[agents]` when you want the optional Python framework
dependencies in one step. Use the JS toolkit or MCP server when you need the
full registry-generated CLI surface, policy runtime, or priced-tool helpers.
The repo also includes dedicated Python example entry points for
`stateset_embedded.generic`, `stateset_embedded.openai`,
`stateset_embedded.langchain`, `stateset_embedded.crewai`, and
`stateset_embedded.autogen` under `examples/python/`, and the shared
`check:engine-examples` gate now exercises both the JS and Python engine
examples before release.

---

## Embedded Agent Toolkit (OpenAI / LangGraph / server-side agents)

Use the embedded toolkit when your agent runtime lives inside your application process and wants JSON-schema tools instead of stdio MCP.

```bash
npm install @stateset/embedded@1.3.0 @stateset/cli@1.3.0
```

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';

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
// execution.result.status === 'success'
```

If your runtime should fan out to specialist agents, pass an `autonomousEngine` and enable writes for delegation:

```javascript
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

`delegate_to_agent` follows the same safety model as other write tools, so it stays in preview mode until `allowApply` is enabled.

For OpenAI Responses API loops, `executeOpenAIToolCall()` returns a ready-to-send `function_call_output` payload:

```javascript
const execution = await toolkit.executeOpenAIToolCall(toolCall);

await client.responses.create({
  model: 'gpt-4.1',
  previous_response_id: response.id,
  input: [execution.outputMessage],
  tools,
});
```

For framework-native and framework-neutral adapters:

```javascript
import { createVercelAITools } from '@stateset/embedded/vercel-ai';
import { createLangChainTools } from '@stateset/embedded/langchain';
import { createToolDescriptors } from '@stateset/embedded/generic';

const vercelTools = createVercelAITools(commerce, { tool });
const langChainTools = createLangChainTools(commerce, { DynamicStructuredTool });
const genericTools = createToolDescriptors(commerce);
```

`createToolDescriptors()` is the lowest-common-denominator adapter surface for runtimes that want `{ name, description, schema, execute }` objects instead of a framework-specific wrapper.

For priced tools and remote MPP-enabled HTTP services, the same toolkit also exposes
payment-aware helpers such as `getPayableToolCatalog()`, `prepareToolPayment()`,
`executePaidTool()`, `discoverRemotePaymentService()`, and
`createRemoteHttpToolDescriptors()`.

For contract-aware or replay-aware agents, the toolkit also exposes
`getRuntimeContract()`, `simulatePlan()`, `executePlan()`, `replayMutation()`,
and `getReplayLog()`.

Use `simulateMutation()` or `executePlan({ dryRun: true, ... })` before enabling writes, then turn on `allowApply` only for agents that should mutate commerce state.

---

## MCP Server (Claude Desktop / Cursor / Windsurf)

StateSet exposes **hundreds of commerce tools** via the [Model Context Protocol](https://modelcontextprotocol.io). Add it to your AI editor in one step.
The exact current count and policy-domain breakdown are generated from code in the [MCP Tool Inventory](./docs/src/appendix/mcp-tool-inventory.md).

**One-command onboarding (including OpenClaw):**
```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This writes an MCP config at `.openclaw/mcp.json` and wires `stateset-mcp-events` automatically.
It also installs starter guardrail policies + an agent prompt pack under `./.stateset/`.
`--quickstart` enables: `--demo --agent openclaw --starter-pack ops --agent-only --verify`.
It also generates `./.stateset/agent-starters/start-mcp.sh` and `check-mcp.sh` for launch + health checks.
For a fully explicit run, use:
```bash
npx -y @stateset/cli@latest stateset-setup --yes --demo --agent openclaw --starter-pack ops --print-handoff --verify --db ./store.db
```
Use `--agent claude`, `--agent cursor`, or `--agent windsurf` for those clients, or pass
`--mcp-config <path>` for any MCP-compatible agent runtime.
Use `--starter-pack support` or `--starter-pack checkout` for different operating modes.
Use `--agent-only` when you only need MCP onboarding and don't want to require a local Anthropic key.
Use `--verify-strict` in CI to fail setup if any readiness warnings are present.

**Claude Desktop** — add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "stateset-commerce": {
      "command": "npx",
      "args": ["-y", "@stateset/cli@latest", "stateset-mcp-events"],
      "env": { "DB_PATH": "./store.db" }
    }
  }
}
```

**Cursor** — add to `.cursor/mcp.json` in your project:
```json
{
  "mcpServers": {
    "stateset-commerce": {
      "command": "npx",
      "args": ["-y", "@stateset/cli@latest", "stateset-mcp-events"],
      "env": { "DB_PATH": "./store.db" }
    }
  }
}
```

This gives your AI assistant access to the full commerce stack: orders, inventory, customers, products, payments, returns, subscriptions, analytics, promotions, manufacturing, A2A commerce, stablecoin payments, and more.

---

**Development toolchain (repo root):**
```bash
nvm use                      # uses .nvmrc / .node-version (20.20.0)
rustup show active-toolchain # dev toolchain pinned by rust-toolchain.toml (1.90.0)
npm run check                # developer gate for the core workspace surfaces
npm run check:release        # release preflight before cutting a tag
```

The authoritative remote release gate is the GitHub Actions `CI Success` job in
`.github/workflows/ci.yml`. A release tag should only be cut from a commit that
passes both `npm run check:release` locally and `CI Success` remotely.

CI also verifies the workspace MSRV on Rust 1.85 and runs the wider binding and
admin surfaces under the same pinned Node 20.20.0 runtime.

---

## What's New in v1.3.0

**v1.3.0 adds the `inventory.query` verb** — the highest-call-volume verb
in B2B agentic commerce. ICP-1.0 now covers **four intent verbs spanning
~99% of commerce dollar volume**:

- `inventory.query`     — discovery (read-only, highest call volume)
- `purchase.create`     — one-shot retail
- `subscription.create` — recurring revenue (SaaS, streaming, B2B services)
- `purchase.return`     — returns / refunds with `max_refund` ceiling

The new `inventory.query` returns a merchant-signed **InventorySnapshot**
with a `valid_until` validity window. It doesn't trigger an escrow; it
serves as the discovery primitive that precedes every value-transferring
Intent. Snapshot-quote consistency rule: when a subsequent
`purchase.create` Quote diverges from a still-valid snapshot's price for
the same SKU, the merchant SHOULD include `snapshot_id` in the Quote
metadata; conformant buyers MAY refuse divergent Quotes as price-walk
attempts.

Both the HTTP handler and the MCP server now advertise and accept all
four verbs. **Handler 12/12 PASS, MCP 6/6 PASS** end-to-end.

**v1.1.0** introduced the Intelligent Commerce Protocol (ICP) reference
implementation set: normative spec, four cross-language conformance
IUTs (JavaScript / Rust / Go / Python — all byte-identical), HTTP and
MCP transports, on-chain custody contract (15/15 Foundry tests),
off-chain Settler daemon, Docker Compose deployment package, Foundation
charter, LOI template, and a partnership packet. See [`CHANGELOG.md`](./CHANGELOG.md)
for the full release history.

Start at **[ICP.md](./ICP.md)** for the discoverable entry point.

The 250k-LOC commerce engine continues to ship unchanged — ICP is
additive infrastructure, not a refactor.

---

## Documentation

- mdBook docs live in `docs/` (see `docs/README.md`).
- API reference pointers per binding: `docs/src/api/`.
- End-to-end examples and workflows: `examples/`.
- Release history: `CHANGELOG.md`.
- Security policy: `SECURITY.md`.

---

## Support Matrix

| Tier | What’s Covered | CI Coverage |
|------|----------------|------------|
| Tier 1 | Default-workspace Rust crates, Node binding, admin app, CLI | Verified by `npm run check` locally and required in `CI Success` |
| Tier 2 | Python, Go, .NET, Java/Kotlin, Swift, and WASM bindings | Dedicated CI jobs are required in `CI Success` before tagging |
| Tier 3 | Ruby native gem and PHP extension distribution flows | Representative CI jobs plus release-workflow validation on publish commits |

Ruby and PHP are kept in-repo but intentionally excluded from default workspace
membership because they require host runtimes or headers that are often
unavailable in local dev. They are still exercised in dedicated CI lanes and
must not be released from a commit without a green `CI Success` aggregate.

## Release Gate

- Local preflight: `npm run check:release`
- Remote tag gate: green `CI Success` in `.github/workflows/ci.yml`
- Local git hooks are convenience checks only; the release authority is the
  checked command above plus the protected CI aggregate

Production note: use `config/stateset.production.properties` as the baseline for secure defaults.

---

## Architecture

This repository is best understood as a platform monorepo with a layered Rust kernel and two large outer product surfaces: language bindings and the Node-based operator runtime.

The current workspace manifests form this dependency direction:

```text
stateset-primitives | stateset-crypto | stateset-pricing | stateset-observability
stateset-policy | stateset-authz | stateset-a2a | stateset-jobs
stateset-migrations | stateset-macros
        ->
stateset-core | stateset-protocol | stateset-sync
        ->
stateset-db
        ->
stateset-embedded
        ->
stateset-http | stateset-sdk | bindings/*
        ->
admin | cli
```

### What Each Layer Does

| Layer | Primary crates/surfaces | Role |
|-------|--------------------------|------|
| Foundation | `stateset-primitives`, `stateset-crypto`, `stateset-pricing`, `stateset-observability`, `stateset-policy`, `stateset-authz`, `stateset-a2a`, `stateset-jobs`, `stateset-migrations`, `stateset-macros` | Narrow building blocks and cross-cutting capabilities |
| Domain kernel | `stateset-core`, `stateset-protocol`, `stateset-sync` | Pure commerce logic, wire formats, sync/runtime contracts |
| Storage + product API | `stateset-db`, `stateset-embedded` | Persistence and the main embeddable commerce surface |
| Edge adapters | `stateset-http`, `stateset-sdk`, `stateset-ffi`, `bindings/*` | Transport, Rust facade, C-style interop, and language-specific packaging |
| Operator surfaces | `cli/`, `admin/` | MCP, agents, automation, and admin UX |

### Binding Topology

- `bindings/node` is the shared JS-facing native layer and links directly to `stateset-embedded`, `stateset-core`, `stateset-db`, and `stateset-crypto`.
- The admin app and CLI both consume `@stateset/embedded` directly rather than going through `stateset-sdk`.
- Python and the compiled language bindings also link directly to `stateset-embedded` and `stateset-core`; they are not all routed through `stateset-ffi`.
- `stateset-sdk` is the Rust-facing facade for consumers that want one dependency with feature-gated re-exports.
- `stateset-ffi` is an optional C-ABI oriented interop surface, not the mandatory substrate for every binding in this repository.

### Operational Surfaces

- `cli/` is a large Node 20.20+ runtime with the MCP server, tool registry, sync/x402 logic, agent routing, messaging channels, and scaffolding flows.
- `admin/` is a Next.js surface that depends on the local Node binding package and loads `@stateset/embedded` at runtime.
- The root `npm run check` pipeline validates release hygiene, Rust fmt/tests/lints/feature-matrix checks, shell scripts, the Node binding, the admin app, and the CLI.
- `npm run check:release` is the authoritative local release preflight; it extends `npm run check` with doc-tool validation and generated inventory checks before a tag is cut.

### Recommended Onboarding Order

1. `stateset-core`
2. `stateset-db`
3. `stateset-embedded`
4. `stateset-sync`, `stateset-policy`, `stateset-authz`, `stateset-pricing`
5. `stateset-http`
6. `bindings/node` and then `admin/` or `cli/`

For a manifest-grounded dependency walkthrough, see [Dependency Direction](./docs/src/guides/dependency-direction.md).

---

## Quick Start

### Rust

```rust
use stateset_sdk::prelude::*;
use rust_decimal_macros::dec;

// Initialize with a local database file
let commerce = Commerce::new("./store.db")?;

// Create a customer
let customer = commerce.customers().create(CreateCustomer {
    email: "alice@example.com".into(),
    first_name: "Alice".into(),
    last_name: "Smith".into(),
    ..Default::default()
})?;

// Create inventory
commerce.inventory().create_item(CreateInventoryItem {
    sku: "SKU-001".into(),
    name: "Widget".into(),
    initial_quantity: Some(dec!(100)),
    ..Default::default()
})?;

// Create an order
let order = commerce.orders().create(CreateOrder {
    customer_id: customer.id,
    items: vec![CreateOrderItem {
        sku: "SKU-001".into(),
        name: "Widget".into(),
        quantity: 2,
        unit_price: dec!(29.99),
        ..Default::default()
    }],
    ..Default::default()
})?;

// Ship the order
commerce.orders().ship(order.id, Some("FEDEX123456".into()))?;
```

### Node.js

```javascript
const { Commerce } = require('@stateset/embedded');

async function main() {
  const commerce = new Commerce('./store.db');

  // Create a cart and checkout
  const cart = await commerce.carts.create({
    customerEmail: 'alice@example.com',
    customerName: 'Alice Smith'
  });

  await commerce.carts.addItem(cart.id, {
    sku: 'SKU-001',
    name: 'Widget',
    quantity: 2,
    unitPrice: 29.99
  });

  const result = await commerce.carts.complete(cart.id);
  console.log(`Order created: ${result.orderNumber}`);

  // Convert currency
  const conversion = await commerce.currency.convert({
    from: 'USD',
    to: 'EUR',
    amount: 100
  });
  console.log(`$100 USD = €${conversion.convertedAmount} EUR`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

### Python

```python
from stateset_embedded import Commerce

commerce = Commerce('./store.db')

# Get analytics
summary = commerce.analytics.sales_summary(period='last30days')
print(f"Revenue: ${summary.total_revenue}")
print(f"Orders: {summary.order_count}")

# Forecast demand
forecasts = commerce.analytics.demand_forecast(days_ahead=30)
for f in forecasts:
    if f.days_until_stockout and f.days_until_stockout < 14:
        print(f"WARNING: {f.sku} will stock out in {f.days_until_stockout} days")
```

### Other bindings

Same `Commerce` API surface, idiomatic per language. Expand any block
to see the snippet — all ten bindings ship from the same Rust core
and pass the same [cross-binding parity tests](./bindings/test-vectors/v1.json).

<details>
<summary><b>Ruby</b></summary>

```ruby
require 'stateset_embedded'

commerce = StateSet::Commerce.new('./store.db')

customer = commerce.customers.create(
  email: 'alice@example.com', first_name: 'Alice', last_name: 'Smith'
)
order = commerce.orders.create(
  customer_id: customer.id,
  items: [{ sku: 'SKU-001', name: 'Widget', quantity: 2, unit_price: 29.99 }]
)
plan = commerce.subscriptions.create_plan(name: 'Pro', price: 29.99, interval: 'month')
commerce.subscriptions.subscribe(plan_id: plan.id, customer_id: customer.id)
```
</details>

<details>
<summary><b>PHP</b></summary>

```php
<?php
use StateSet\Commerce;

$commerce = new Commerce('./store.db');
$customer = $commerce->customers()->create(
    email: 'alice@example.com', firstName: 'Alice', lastName: 'Smith'
);
$order = $commerce->orders()->create(
    customerId: $customer->getId(),
    items: [['sku' => 'SKU-001', 'name' => 'Widget', 'quantity' => 2, 'unit_price' => 29.99]]
);
$tax = $commerce->tax()->calculate(amount: 100.00, jurisdictionId: $jur->getId());
```
</details>

<details>
<summary><b>Java</b></summary>

```java
import com.stateset.embedded.*;

try (Commerce commerce = new Commerce("./store.db")) {
    Customer customer = commerce.customers().create("alice@example.com", "Alice", "Smith");
    Order order = commerce.orders().create(customer.getId(), "USD");
    commerce.payments().recordPayment(order.getId(), order.getTotalAmount(), "card", "txn_123");
    SalesSummary s = commerce.analytics().salesSummary(30);
    System.out.println("Revenue: $" + s.getTotalRevenue());
}
```
</details>

<details>
<summary><b>Kotlin</b></summary>

```kotlin
import com.stateset.embedded.*

val commerce = StateSetCommerce("./store.db")
val customer = commerce.customers.create(
    email = "alice@example.com", firstName = "Alice", lastName = "Smith"
)
val product = commerce.products.create(name = "Widget", sku = "SKU-001", price = 29.99)
commerce.orders.create(
    customerId = customer.id,
    items = listOf(OrderItem(product.id, "SKU-001", "Widget", 2, "29.99")),
    currency = "USD",
)
commerce.close()
```
</details>

<details>
<summary><b>Swift</b></summary>

```swift
import StateSet

let commerce = try StateSetCommerce(path: "./store.db")
defer { commerce.close() }

let customer = try commerce.customers.create(
    email: "alice@example.com", firstName: "Alice", lastName: "Smith"
)
let product = try commerce.products.create(name: "Widget", sku: "SKU-001", price: 29.99)
let order = try commerce.orders.create(
    customerId: customer.id,
    items: [OrderItem(productId: product.id, sku: "SKU-001", name: "Widget", quantity: 2, unitPrice: "29.99")],
    currency: "USD",
)
try commerce.orders.updateStatus(id: order.id, status: .shipped)
```
</details>

<details>
<summary><b>C# / .NET</b></summary>

```csharp
using StateSet;

using var commerce = new StateSetCommerce("./store.db");
var customer = commerce.Customers.Create(
    email: "alice@example.com", firstName: "Alice", lastName: "Smith"
);
var product = commerce.Products.Create(name: "Widget", sku: "SKU-001", price: 29.99m);
commerce.Orders.Create(
    customerId: customer.Id,
    items: new[] {
        new OrderItem { ProductId = product.Id, Sku = "SKU-001", Name = "Widget", Quantity = 2, UnitPrice = "29.99" }
    },
    currency: "USD",
);
```
</details>

<details>
<summary><b>Go</b></summary>

```go
package main

import (
    "fmt"
    "github.com/stateset/stateset-icommerce/bindings/go/stateset"
)

func main() {
    commerce, _ := stateset.New("./store.db")
    defer commerce.Close()

    customer, _ := commerce.Customers().Create("alice@example.com", "Alice", "Smith", "")
    product, _ := commerce.Products().Create("Widget", "SKU-001", 29.99, "")
    items := []stateset.OrderItem{{ProductID: product.ID, SKU: "SKU-001", Name: "Widget", Quantity: 2, UnitPrice: "29.99"}}
    order, _ := commerce.Orders().Create(customer.ID, items, "USD")
    fmt.Printf("Order: %s\n", order.OrderNumber)
}
```
</details>

### CLI (AI-Powered)

Tip: `ss` is a shorthand alias for `stateset`.

```bash
# Natural language interface
stateset "show me pending orders"
stateset "what's my revenue this month?"
stateset "convert $100 USD to EUR"

# Execute operations (requires --apply)
stateset --apply "create a cart for alice@example.com"
stateset --apply "ship order #12345 with tracking FEDEX123"
stateset --apply "approve return RET-001"

# Tax calculation
stateset "calculate tax for an order shipping to California"
stateset "what's the VAT rate for Germany?"

# Promotions
stateset --apply "create a 20% off promotion called Summer Sale"
stateset "is coupon SAVE20 valid?"
stateset --apply "apply promotions to cart CART-123"

# Subscriptions
stateset "show me all subscription plans"
stateset --apply "create a monthly plan called Pro at $29.99"
stateset --apply "subscribe customer alice@example.com to the Pro plan"

# Payments & Refunds
stateset "list payments for order #12345"
stateset --apply "create payment for order #12345 amount $99.99 via card"
stateset --apply "refund payment PAY-001 amount $25.00"

# Shipments
stateset --apply "create shipment for order #12345 via FedEx tracking FEDEX123"
stateset --apply "mark shipment SHIP-001 as delivered"

# Supply Chain
stateset "list purchase orders"
stateset --apply "create purchase order for supplier SUP-001"
stateset --apply "approve purchase order PO-001"

# Invoices (B2B)
stateset "show overdue invoices"
stateset --apply "create invoice for customer CUST-001"
stateset --apply "record payment on invoice INV-001"

# Warranties
stateset "list warranties for customer CUST-001"
stateset --apply "create warranty for product SKU-001"
stateset --apply "file warranty claim for warranty WRN-001"

# Manufacturing
stateset "list bills of materials"
stateset --apply "create BOM for product WIDGET-ASSEMBLED"
stateset --apply "add component SKU-PART-A quantity 2 to BOM-001"
stateset --apply "create work order from BOM-001 quantity 50"
stateset --apply "complete work order WO-001 with 48 units produced"
```

### Agent-to-Agent (A2A) Commerce

AI agents can pay, quote, subscribe, and negotiate with each other autonomously.

```bash
# Direct agent-to-agent payment
stateset --apply "pay 10 USDC to 0x1234...5678 on Set Chain"

# Request payment from another agent
stateset --apply "request 50 USDC from 0xBuyer for API credits"

# Quotes — request, provide, accept
stateset --apply "request quote from data provider for 100 API credits"
stateset --apply "provide quote of $150 for premium data access"
stateset --apply "accept quote QT-001 and pay"

# Recurring subscriptions between agents
stateset --apply "create monthly subscription for 0xSubscriber at $49.99 with 14-day trial"
stateset "list active a2a subscriptions"
stateset --apply "pause a2a subscription SUB-001"
stateset --apply "cancel a2a subscription SUB-001"

# Split payments (multi-party)
stateset --apply "create split payment of $100: 70% to 0xSeller, 20% to 0xAffiliate, 10% platform fee"
stateset --apply "execute split payment SPLIT-001"

# Escrow with conditions
stateset --apply "create escrow payment of 100 USDC with condition seller_fulfilled"
stateset --apply "fulfill escrow condition on ESCROW-001"
stateset --apply "release escrow ESCROW-001"

# Agent discovery and trust
stateset "discover agents that can process payments on Set Chain"
stateset "find verified sellers with USDC support"
stateset "get reputation for agent 0xSeller"

# Webhook notifications
stateset --apply "configure webhook for agent 0xMyAgent to https://hooks.example.com/a2a"
stateset "list a2a notifications for agent 0xMyAgent"

# Event streaming
stateset --apply "subscribe to a2a_payment.* events"
stateset "get a2a event history for last 24 hours"
```

### Sync CLI (VES v1.0)

Verifiable Event Sync (VES) enables local SQLite databases to synchronize with the `stateset-sequencer` service, providing deterministic event ordering, conflict resolution, and cryptographic audit trails.

```bash
# Initialize sync for a store
stateset-sync init --sequencer-url grpc://sequencer.stateset.network:443 \
  --tenant-id <uuid> --store-id <uuid> --api-key <key>

# Sync operations
stateset-sync push              # Push pending events to sequencer
stateset-sync pull              # Pull remote events locally
stateset-sync status            # Show sync status
stateset-sync verify <event-id> # Verify event inclusion proof
stateset-sync rebase            # Rebase after conflict
stateset-sync conflicts         # List unresolved conflicts
stateset-sync history           # Show sync history

# Key Management (Ed25519/X25519)
stateset-sync keys:generate     # Generate signing and encryption keys
stateset-sync keys:list         # List agent keys
stateset-sync keys:register     # Register signing key with sequencer
stateset-sync keys:rotate --all --register  # Rotate keys and re-register
stateset-sync keys:export       # Export public keys for sharing

# Key Rotation Policies
stateset-sync keys:policy --key-type signing --max-age 720 --grace-period 72
stateset-sync keys:expiry       # Check key expiration warnings
stateset-sync keys:batch-rotate --key-type all --register

# Encryption Groups (multi-agent)
stateset-sync groups:create --name "warehouse-agents"
stateset-sync groups:add-member --group-id <id> --agent-id <id>
stateset-sync groups:remove-member --group-id <id> --agent-id <id>
stateset-sync groups:list       # List all groups
stateset-sync groups:show <id>  # Show group details
stateset-sync groups:my-groups  # List your group memberships
```

### Messaging Channels CLI

Deploy AI commerce agents to 10 messaging platforms. Each channel runs as a thin adapter over the shared base module.

```bash
# Single-channel gateways
stateset-telegram --db ./store.db --agent customer-service
stateset-discord --db ./store.db --agent customer-service
stateset-slack --db ./store.db --agent customer-service
stateset-whatsapp --db ./store.db --agent customer-service
stateset-signal --db ./store.db --agent customer-service
stateset-google-chat --db ./store.db --agent customer-service

# Experimental channels
stateset-webchat --db ./store.db --port 3000
stateset-imessage --db ./store.db --agent customer-service
stateset-teams --db ./store.db --agent customer-service
stateset-matrix --db ./store.db --agent customer-service --homeserver matrix.example.com

# Multi-channel orchestrator (all channels in one process)
stateset-channels --config channels.yaml

# With autonomous engine notifications
stateset-autonomous --db ./store.db --agent customer-service \
  --notify-config notify-config.json
```

**In-chat bot commands** (available on all channels):

```
/help                   — List available commands
/orders [n]             — Show recent orders (default 5)
/order <id>             — Order detail with rich card
/inventory <sku>        — Stock levels with rich card
/cart [id]              — Cart summary
/track <id>             — Shipment tracking
/customers              — Customer count
/analytics              — Sales summary with rich card
/stats                  — Channel metrics (messages, response time, errors)
/whoami                 — Show linked customer identity
/link <email>           — Link chat to a customer by email
/unlink                 — Remove customer link
/escalate [reason]      — Hand off to a human agent
/release                — Return conversation to AI bot
/reset                  — Reset session state
```

**Example `channels.yaml`:**

```yaml
shared:
  db: ./store.db
  agent: customer-service
  model: claude-sonnet-4-20250514
  middleware:
    rateLimiter: { maxPerMinute: 20 }
    contentFilter: { action: warn }
    logger: true

channels:
  telegram:
    token: ${TELEGRAM_BOT_TOKEN}
    allowList: [-1001234567890]
  discord:
    token: ${DISCORD_BOT_TOKEN}
    channelIds: ["1234567890123456"]
  slack:
    token: ${SLACK_BOT_TOKEN}
    appToken: ${SLACK_APP_TOKEN}
    channels: ["C01ABCDEF"]

notifications:
  routes:
    "order.shipped": [{ channel: slack, target: "#orders" }]
    "inventory.low": [{ channel: slack, target: "#ops" }]
    "*": [{ channel: slack, target: "#all-alerts" }]
```

### Skills CLI

Browse, install, and manage 38 commerce domain skills that enhance agent capabilities.

```bash
# List all loaded skills
stateset-skills list

# Search for skills
stateset-skills search "inventory"

# Install from marketplace
stateset-skills install commerce-warehouse

# Temporary migration mode for legacy catalogs without checksums
stateset-skills install commerce-warehouse --allow-insecure-downloads

# Uninstall a skill
stateset-skills uninstall commerce-warehouse

# Show skill details
stateset-skills info commerce-fulfillment

# List skill categories
stateset-skills categories

# Browse the marketplace
stateset-skills marketplace

# Health check all skills
stateset-skills doctor
```

Remote installs are verified by checksum by default. For controlled migrations, you can opt out with `--allow-insecure-downloads` or `STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS=1`.

**Available skills:** accounts-payable, accounts-receivable, analytics, autonomous-engine, autonomous-runbook, backorders, checkout, cost-accounting, credit, currency, customer-service, customers, embedded-sdk, engine-setup, events, fulfillment, general-ledger, inventory, invoices, lots-and-serials, manufacturing, mcp-tools, orders, payments, products, promotions, quality, receiving, returns, shipments, storefront, subscriptions, suppliers, sync, tax, vector-search, warehouse, warranties.

### Daemon CLI

Manage the StateSet gateway as a background service with systemd integration.

```bash
# Start the gateway daemon
stateset-daemon start --config gateway.config.json

# Stop the daemon
stateset-daemon stop

# View logs
stateset-daemon logs --follow

# Health check
stateset-daemon health

# Tailscale VPN management
stateset-daemon tailscale up
stateset-daemon tailscale status

# SSH tunnel management
stateset-daemon tunnel add --host remote.example.com --port 8080
stateset-daemon tunnel list
stateset-daemon tunnel remove <name>
```

### Voice Mode

Hands-free chat with pluggable STT/TTS (ElevenLabs, OpenAI Whisper).
Per-session settings + 30-min inactivity TTL — see the
[Key Features](#key-features) row for the full capability list.

```bash
stateset-chat --voice
stateset-chat --voice --tts-provider elevenlabs --stt-provider whisper
```

### Multi-Provider AI

Claude is the default with full MCP tool integration. OpenAI, Gemini,
and local Ollama work in chat-only mode. For embedded server-side
agents, use `@stateset/embedded/*` entrypoints (or
`@stateset/embedded/agent-toolkit` for advanced runtime controls).

```bash
stateset "ship order #12345"                    # Claude (default, full MCP)
stateset --provider openai "list orders"        # chat-only
stateset --provider ollama --model llama3 "…"   # local-only
```

### Validation & Errors

Inputs are validated at the repository layer (SKU format, required fields, currency codes, etc.)
and errors include field-level context when possible.

```rust
use stateset_core::CommerceError;

match commerce.orders().create(order_input) {
    Ok(order) => println!("created order {}", order.order_number),
    Err(err) if err.is_validation() => {
        eprintln!("validation error: {}", err);
    }
    Err(err) => {
        eprintln!("unexpected error: {}", err);
    }
}
```

Order status updates enforce the core state machine (cancel before shipment, refund after delivery).

---

## Production Notes

- Payments are recorded as ledger events; integrate a PCI-compliant PSP for capture and store tokens/last4 only.
- Sync is event-ordered and can surface conflicts; for order/payment state use a single writer or a sequenced event log.
- Treat external processor IDs and webhook IDs as idempotency keys and de-dupe on ingest to avoid double charges/refunds.

---

## Domain Models

20 first-class domains: Orders, Customers, Products, Inventory, Carts,
Payments, Returns, Shipments, Manufacturing, Purchase Orders, Warranties,
Invoices, Currency, Analytics, Tax, Promotions, Subscriptions, Stablecoin,
x402, A2A Commerce.

Each domain ships with strongly-typed Rust models, generated language
bindings, MCP tool descriptors, and OpenAPI schemas. The authoritative
inventory lives in the [OpenAPI spec](http://localhost:8080/api/v1/openapi.json)
and the source crates under `crates/stateset-core/src/models/`.

---

## Database Schema

60+ tables grouped into Core, Inventory, Financial, Fulfillment,
Manufacturing, Tax, Promotions, Subscriptions, A2A Commerce, and
Cross-cutting (warranties, carts, currency, event log). Migrations are
the authoritative source — see
[`crates/stateset-db/migrations/`](./crates/stateset-db/migrations/)
for the full schema with indexes and foreign keys, and
[`docs/src/guides/dependency-direction.md`](./docs/src/guides/dependency-direction.md)
for how the schema layers map onto the Rust kernel.

---

## MCP Tools

The MCP surface is generated from the live CLI registry instead of maintained as a static table in this README.
For the exact current tool count, policy-domain breakdown, permissions summary, and full tool list, see the generated [MCP Tool Inventory](./docs/src/appendix/mcp-tool-inventory.md).

---

## AI Agents

Specialized agents cover different commerce domains, including customer
service, checkout, orders, inventory, analytics, sync, stablecoin payments,
and multi-agent orchestration. The exact current agent count, descriptions, and
tool access are generated from code in the
[Agent Inventory](./docs/src/appendix/agent-inventory.md).

---

## Key Features

A capability matrix — what ships in v1.0.x, with depth links. The agentic
primitives that distinguish iCommerce from a generic commerce engine
([A2A](./AGENTIC_COMMERCE.md), [x402](./docs/src/payments/x402.md),
[VES v1.0](./docs/PQC_INITIAL_SPEC.md), [Policy DSL](./crates/stateset-policy/),
[700+ MCP tools](./docs/whitepaper.md#mcp-tools)) are summarized in
[Why iCommerce](#why-icommerce) and not duplicated here.

| Domain | Capabilities |
| --- | --- |
| **Commerce** | Order lifecycle (create → confirm → ship → deliver) · multi-location inventory with reservations · customer profiles · product catalog with variants/attributes |
| **Financial** | 35+ currencies (BTC, ETH, USDC included) · multi-method payments + refunds · invoice generation · supplier purchase orders |
| **Tax** | US/EU/Canada multi-jurisdiction calc · state/province auto-lookup · exemptions & certificates · EU VAT · CA GST/HST/PST |
| **Promotions** | %/fixed discounts · coupon codes with usage limits · BXGY · free shipping · time-limited campaigns |
| **Subscriptions** | Daily/weekly/monthly/annual plans · trial periods · pause/resume/cancel/skip · billing cycle management |
| **Supply chain** | Manufacturing BOMs · work orders with task tracking · carrier shipment tracking · RMA processing |
| **Analytics** | Sales summaries · demand forecast per SKU · revenue projections (with confidence intervals) · inventory health · customer LTV |
| **A2A commerce** | Direct agent-to-agent payments (USDC/USDT/ssUSD/DAI on SET Chain, Base, Ethereum, Arbitrum) · quote/negotiate · recurring subscriptions · split payments · escrow with conditional release · HMAC-signed webhooks · SSE event streaming · agent discovery + reputation. See [`AGENTIC_COMMERCE.md`](./AGENTIC_COMMERCE.md). |
| **VES v1.0** | Local-first with sequencer sync · Ed25519 + ML-DSA-65 hybrid signatures · X25519 + ML-KEM-768 hybrid encryption · key rotation · encryption groups · Merkle proofs · conflict resolution (remote-wins/local-wins/merge). See [`PQC_INITIAL_SPEC.md`](./docs/PQC_INITIAL_SPEC.md). |
| **Messaging** | 10 platforms (WhatsApp, Telegram, Discord, Slack, Signal, Google Chat, WebChat, iMessage, Teams, Matrix) · SQLite-backed sessions · Koa-style middleware · rich messages (Embeds, Block Kit, HTML) · 15+ bot commands · proactive notifications · AI↔human handoff |
| **Skills system** | 38 domain skills (AP/AR, fulfillment, quality, warehouse, cost accounting, credit, lots/serials, …) · skills marketplace for browse/install/manage |
| **Voice mode** | STT input + TTS output · pluggable providers (ElevenLabs, OpenAI Whisper) · 30-min inactivity TTL on per-session settings |
| **Multi-provider AI** | Claude (full MCP), OpenAI, Gemini, local Ollama · auto-detection · fallback chains. Non-Claude providers run chat-only. |
| **Conversation memory** | SQLite-backed across sessions · summaries + facts + token counts · keyword search · age-based auto-cleanup |
| **Browser automation** | CDP integration (no Puppeteer) · headless Chrome lifecycle · navigation, DOM, JS eval, screenshots — for scraping, storefront tests, catalog sync |
| **Heartbeat monitor** | 6 proactive checkers: low-stock, abandoned-carts, revenue-milestone, pending-returns, overdue-invoices, subscription-churn · EventBridge → channels · HTTP API for status / manual runs / enable+disable |
| **Permission sandboxing** | Bearer + query-param API keys · 6 levels: none < read < preview < write < delete < admin · sandbox mode blocks dangerous routes |
| **AI-ready architecture** | Deterministic operations · registry-generated MCP tool inventory · `--apply` write gate · event-driven full auditability · portable single-file state |

---

## Installation

Per-language install commands and working code samples live under
[Quick Start](#quick-start) above. The [Language Bindings](#language-bindings)
table immediately below summarizes every package and links to its docs.
Platform-specific notes that don't fit either:

- **Java (Maven)** — alternative to the Gradle line in the table:

  ```xml
  <dependency>
    <groupId>com.stateset</groupId>
    <artifactId>embedded</artifactId>
    <version>1.3.0</version>
  </dependency>
  ```

- **PHP** — after `composer require stateset/embedded`, enable the native
  extension by running `composer install-extension` and adding
  `extension=stateset_embedded` to your `php.ini`. Without the extension
  the autoloaded stubs throw at runtime.

- **Swift** — Swift Package Manager is the supported path. CocoaPods is
  community-maintained at `pod 'StateSet', '~> 1.3.0'`.

- **CLI** — clone the repo, then `cd cli && npm install && npm link`. After
  that, `stateset --help` works anywhere.

---

## Language Bindings

StateSet provides a Rust SDK plus native runtime bindings built from the same Rust core:

| Language | Package | Install | Docs |
|----------|---------|---------|------|
| **Rust** | `stateset-sdk` / `stateset-embedded` | `cargo add stateset-sdk --features full` or `stateset-embedded = "1.3.0"` | [docs.rs](https://docs.rs/stateset-sdk) |
| **Node.js** | `@stateset/embedded` | `npm install @stateset/embedded@1.3.0` | [npm](https://www.npmjs.com/package/@stateset/embedded) |
| **Python** | `stateset-embedded` | `pip install stateset-embedded` | [PyPI](https://pypi.org/project/stateset-embedded/) |
| **Ruby** | `stateset_embedded` | `gem install stateset_embedded` | [RubyGems](https://rubygems.org/gems/stateset_embedded) |
| **PHP** | `stateset/embedded` | `composer require stateset/embedded` | [Packagist](https://packagist.org/packages/stateset/embedded) |
| **Java** | `com.stateset:embedded` | `implementation 'com.stateset:embedded:1.3.0'` | [Maven Central](https://central.sonatype.com/artifact/com.stateset/embedded) |
| **Kotlin** | `com.stateset:embedded-kotlin` | `implementation("com.stateset:embedded-kotlin:1.3.0")` | [Maven Central](https://central.sonatype.com/artifact/com.stateset/embedded-kotlin) |
| **Swift** | `StateSet` | `.package(url: "https://github.com/stateset/stateset-swift.git", from: "1.3.0")` | [GitHub](https://github.com/stateset/stateset-swift) |
| **C# / .NET** | `StateSet.Embedded` | `dotnet add package StateSet.Embedded --version 1.3.0` / `<PackageReference Include="StateSet.Embedded" Version="1.3.0" />` | [NuGet](https://www.nuget.org/packages/StateSet.Embedded) |
| **Go** | `stateset` | `go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v1.3.0` | [pkg.go.dev](https://pkg.go.dev/github.com/stateset/stateset-icommerce/bindings/go/stateset) |
| **WASM** | `@stateset/embedded-wasm` | `npm install @stateset/embedded-wasm` | [npm](https://www.npmjs.com/package/@stateset/embedded-wasm) |

For Rust specifically, `stateset-sdk` is the recommended facade crate. Use
`stateset-embedded` directly when you want the lower-level core commerce API
without the feature-gated SDK layer.

### Platform Support

| Platform | Node.js | Python | Ruby | PHP | Java | Kotlin | Swift | C# | Go |
|----------|---------|--------|------|-----|------|--------|-------|-----|-----|
| Linux x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Linux arm64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS arm64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Windows x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | - | ✅ | ✅ |
| iOS | - | - | - | - | - | - | ✅ | - | - |
| Android | - | - | - | - | ✅ | ✅ | - | - | - |
| Browser (WASM) | ✅ | - | - | - | - | - | - | ✅* | - |

*Via Blazor WebAssembly

### Framework Integration

**Laravel (PHP)**
```php
// AppServiceProvider.php
$this->app->singleton(Commerce::class, fn() =>
    new Commerce(storage_path('stateset/commerce.db'))
);
```

**Rails (Ruby)**
```ruby
# config/initializers/stateset.rb
Rails.application.config.stateset = StateSet::Commerce.new(
  Rails.root.join('db', 'commerce.db').to_s
)
```

**Spring Boot (Java)**
```java
@Configuration
public class CommerceConfig {
    @Bean(destroyMethod = "close")
    public Commerce commerce() {
        return new Commerce("commerce.db");
    }
}
```

**Android (Kotlin)**
```kotlin
// Application class
class CommerceApp : Application() {
    val commerce by lazy {
        StateSetCommerce(getDatabasePath("commerce.db").absolutePath)
    }
}
```

**iOS/macOS (Swift)**
```swift
// AppDelegate or @main struct
let commerce = try! StateSetCommerce(
    path: FileManager.default
        .urls(for: .documentDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("commerce.db").path
)
```

**ASP.NET Core (C#)**
```csharp
// Program.cs or Startup.cs
builder.Services.AddSingleton<StateSetCommerce>(sp =>
    new StateSetCommerce("commerce.db"));
```

**Go (net/http or Gin)**
```go
// main.go
var commerce *stateset.Commerce

func init() {
    var err error
    commerce, err = stateset.New("commerce.db")
    if err != nil {
        log.Fatal(err)
    }
}
```

---

## Configuration

### Database Backends

**SQLite (Default):**
```rust
let commerce = Commerce::new("./store.db")?;
// Or in-memory for testing
let commerce = Commerce::new(":memory:")?;
```

**PostgreSQL:**
```rust
let commerce = Commerce::with_postgres("postgres://user:pass@localhost/db")?;
// Or with options
let commerce = Commerce::builder()
    .postgres("postgres://localhost/stateset")
    .max_connections(20)
    .build()?;
```

### CLI Environment

```bash
export ANTHROPIC_API_KEY=sk-ant-...  # Required for AI mode
export STATESET_POLICY_DIR=./.stateset  # Optional policy set directory (defaults to <db-path>/.stateset)
stateset --db ./store.db "list customers"
```

---

## Examples

```bash
# Run Rust examples
cargo run --example basic_usage
cargo run --example manufacturing

# CLI examples
stateset "how many orders do we have?"
stateset "what's the exchange rate from USD to EUR?"
stateset --apply "add 50 units to SKU-001"
```

---

## Development

Rust:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test                       # Run the workspace Rust test suite
cargo bench -p stateset-benches  # Criterion benchmarks
```

Bindings:

```bash
cd bindings/node && npm ci && npm test
cd bindings/python && python -m pip install maturin pytest && maturin develop --release && pytest -q
```

CLI:

```bash
cd cli
npm test                    # All tests
npm run test:unit           # Unit tests only
npm run test:integration    # Integration tests
npm run test:e2e            # End-to-end tests
npm run lint                # ESLint
npm run typecheck           # JSDoc type checking
npm run test:coverage       # Coverage report
```

Admin:

```bash
cd admin
npm test                    # Vitest test suite
```

---

## Project Structure

```
stateset-icommerce/
├── Cargo.toml                      # Workspace manifest
├── crates/
│   ├── stateset-primitives/        # Strongly-typed newtypes (OrderId, Sku, Money, CurrencyCode)
│   ├── stateset-core/              # Domain models, services, repository traits
│   ├── stateset-crypto/            # VES v1.0 (JCS, Ed25519, AES-256-GCM, Merkle)
│   ├── stateset-db/                # Database layer (SQLite + PostgreSQL)
│   ├── stateset-embedded/          # High-level embeddable commerce API
│   ├── stateset-observability/     # Metrics + tracing helpers
│   ├── stateset-macros/            # Proc macros
│   ├── stateset-policy/            # Policy DSL engine (YAML, conditions, transforms)
│   ├── stateset-protocol/          # Wire types (EventEnvelope, SyncBatch, Merkle)
│   ├── stateset-http/              # Axum REST + SSE transport layer
│   ├── stateset-a2a/               # Agent-to-Agent commerce
│   ├── stateset-sync/              # Sync engine (outbox, conflict, transport)
│   ├── stateset-authz/             # Authorization (RBAC, rate limiting, audit)
│   ├── stateset-pricing/           # Deterministic pricing (line items, tax, FX)
│   ├── stateset-migrations/        # SQL migrations (checksums, rollback)
│   ├── stateset-jobs/              # Background jobs (cron, retries, backoff)
│   ├── stateset-ffi/               # Stable C ABI (#[repr(C)], extern "C")
│   ├── stateset-sdk/               # Facade re-exports + prelude
│   ├── stateset-test-utils/        # Shared test fixtures & assertion macros
│   ├── stateset-integration-tests/ # Cross-crate integration tests
│   └── stateset-benches/           # Criterion benchmarks
├── bindings/
│   ├── node/                  # NAPI bindings (@stateset/embedded)
│   ├── python/                # PyO3 bindings (stateset-embedded)
│   ├── ruby/                  # Magnus bindings (stateset_embedded gem)
│   ├── php/                   # ext-php-rs bindings (stateset/embedded)
│   ├── java/                  # JNI bindings (com.stateset:embedded)
│   ├── kotlin/                # JNI bindings (com.stateset:embedded-kotlin)
│   ├── swift/                 # C FFI bindings (StateSet Swift package)
│   ├── dotnet/                # P/Invoke bindings (StateSet.Embedded NuGet)
│   ├── go/                    # cgo bindings (stateset Go module)
│   └── wasm/                  # WebAssembly bindings (@stateset/embedded-wasm)
├── cli/
│   ├── bin/                   # CLI entry points
│   ├── src/mcp-server.js      # MCP orchestrator
│   ├── src/tools/             # Modular tool modules
│   ├── src/a2a/               # Agent-to-Agent commerce
│   │   ├── index.js           # Direct payments, quotes, escrow, conditions
│   │   ├── store.js           # SQLite persistence (7 tables)
│   │   ├── notifications.js   # HMAC-SHA256 signed webhooks
│   │   ├── subscriptions.js   # Recurring agent subscriptions
│   │   ├── splits.js          # Multi-party split payments
│   │   └── event-stream.js    # SSE push, event log, subscriptions
│   ├── src/channels/          # 10-channel messaging gateway
│   ├── src/providers/         # Multi-provider AI (Claude, OpenAI, Gemini, Ollama)
│   ├── src/voice/             # Voice mode (STT + TTS)
│   ├── src/memory/            # Persistent conversation memory
│   ├── src/browser/           # Chrome DevTools Protocol automation
│   ├── src/skills/            # Skills loader, registry, marketplace
│   ├── src/chains/            # Blockchain integration (Solana, Base, SET Chain, etc.)
│   ├── src/x402/              # x402 AI agent payment protocol
│   ├── src/imessage/          # iMessage gateway (BlueBubbles)
│   ├── src/matrix/            # Matrix protocol gateway
│   ├── src/teams/             # Microsoft Teams gateway (Bot Framework)
│   ├── src/sync/              # VES sync engine
│   │   ├── engine.js          # Sync orchestration
│   │   ├── outbox.js          # Event outbox management
│   │   ├── client.js          # gRPC sequencer client
│   │   ├── keys.js            # Ed25519/X25519 key management
│   │   ├── groups.js          # Encryption group management
│   │   ├── rotation-policy.js # Key rotation policies
│   │   ├── crypto.js          # VES cryptographic operations
│   │   └── conflict.js        # Conflict resolution
│   ├── skills/                # Commerce domain skills
│   ├── deploy/                # Systemd services, Tailscale, SSH tunnels
│   └── .claude/               # AI agent definitions
└── examples/
```

Current manifest-backed counts and topology live in [Workspace Inventory](./docs/src/appendix/workspace-inventory.md).

---

## Core Concepts

### Agentic Commerce Protocol (ACP)

StateSet implements the Agentic Commerce Protocol, an open standard defining how AI agents perform commerce operations:

```javascript
// ACP operations are deterministic and auditable
commerce.orders.create(...)     // Create order
commerce.inventory.reserve(...) // Reserve stock
commerce.returns.approve(...)   // Approve return
commerce.currency.convert(...)  // Convert currency
```

### Safety Architecture

All write operations require explicit opt-in:

```bash
# Preview only (safe)
stateset "create a cart for alice@example.com"

# Actually execute (requires --apply)
stateset --apply "create a cart for alice@example.com"
```

### Event-Driven Architecture

All operations emit events for auditability:

```rust
pub enum CommerceEvent {
    OrderCreated(Order),
    OrderStatusChanged { id, from, to },
    InventoryAdjusted { sku, delta, reason },
    PaymentProcessed(Payment),
    // ...
}
```

---

## License

MIT OR Apache-2.0

---

## Contributing

Contributions welcome! Please read the contributing guidelines before submitting PRs.

---

Built with Rust for reliability, designed for AI agents.
