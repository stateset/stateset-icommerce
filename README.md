# StateSet iCommerce Engine

**The SQLite of commerce.** A complete commerce runtime — orders, inventory,
checkout, payments, returns, subscriptions, warehouse operations, finance —
that runs inside your process against one database file. No hosted control
plane, no service account, no rate limit.

For autonomous execution it adds what an AI agent needs before it may spend
money: delegated identity, deny-by-default authority, exact budgets, idempotent
commands, and cryptographically verifiable receipts.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/stateset/stateset-icommerce/actions/workflows/ci.yml/badge.svg)](https://github.com/stateset/stateset-icommerce/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/stateset-sdk.svg?label=crates.io)](https://crates.io/crates/stateset-sdk)
[![npm](https://img.shields.io/npm/v/@stateset/embedded.svg?label=npm)](https://www.npmjs.com/package/@stateset/embedded)
[![PyPI](https://img.shields.io/pypi/v/stateset-embedded.svg?label=PyPI)](https://pypi.org/project/stateset-embedded/)

---

## Start here

Scaffold a working storefront:

```bash
npm create stateset-app@latest my-store
```

Then run it:

```bash
cd my-store
cp .env.example .env.local
npm run seed          # 10 products, stock, a demo catalog in ./store.db
npm run dev           # http://localhost:3000
```

That is a Next.js storefront with a real commerce engine behind it — the
generator and the seed step are covered end to end by the
[Storefront Golden Path](.github/workflows/storefront-golden-path.yml) CI job.

### Give an AI agent a commerce sandbox

For Claude Desktop, Cursor, Windsurf, or any MCP client:

```json
{
  "mcpServers": {
    "stateset-commerce": {
      "command": "npx",
      "args": ["-y", "-p", "@stateset/cli", "stateset-mcp", "--db", "./store.db", "--profile", "core"]
    }
  }
}
```

**Writes are preview-only by default.** The `core` profile keeps the
model-facing catalog small; `finance`, `operations`, `agents`, and `all` open
it up, and `--domains a2a,x402` adds individual domains. Give an agent apply
authority only through operator-owned files, never through tool arguments:

```json
{
  "args": [
    "-y", "-p", "@stateset/cli", "stateset-mcp", "--db", "./store.db", "--apply",
    "--kernel-policy", "./kernel-policy.json",
    "--kernel-principal", "./kernel-principal.json",
    "--kernel-store-id", "store:production"
  ]
}
```

For hosted deployments, `stateset-mcp-http` serves the same tools over MCP
Streamable HTTP (protocol revision 2026-07-28), stateless by construction — no
session ids, a fresh server per request, so it scales across replicas:

```bash
npx -y -p @stateset/cli stateset-mcp-http --db ./store.db --port 8090
```

Add `--host 0.0.0.0` to expose it, `--read-only` to disable writes at the
transport boundary, or `--strict-protocol` to refuse pre-2026-07-28 clients.

---

## Install

| Channel | Command |
| ------- | ------- |
| **crates.io** | `cargo add stateset-sdk --features full` |
| **npm** | `npm install @stateset/embedded@1.35.1` |
| **PyPI** | `pip install stateset-embedded==1.35.1` |
| **CLI + MCP servers** | `npm install -g @stateset/cli@1.35.1` |

The Ruby and WASM bindings have published packages that are not kept current:
RubyGems `stateset_embedded` is at 0.1.9 and npm `@stateset/embedded-wasm` at
0.7.22, both far behind 1.35.1. PHP, Java, Kotlin, Swift, .NET, and Go have no
published package at all — build those from source, and see
[`docs/src/api/`](docs/src/api/) for each binding's API and install snippet.

### Use it

```javascript
import { Commerce } from '@stateset/embedded';

const commerce = new Commerce('./store.db');
const customer = await commerce.customers.create({
  email: 'ada@example.com',
  firstName: 'Ada',
  lastName: 'Lovelace',
});
```

```rust
use stateset_sdk::prelude::*;

let commerce = Commerce::new("store.db")?;
let customer = commerce.customers().create(CreateCustomer {
    email: "ada@example.com".into(),
    first_name: "Ada".into(),
    last_name: "Lovelace".into(),
    ..Default::default()
})?;
```

Amounts are exact decimal values end to end. Every mutation is auditable.

**[Rust Quickstart →](QUICKSTART.md)** ·
**[CLI Quickstart →](docs/src/standalone-quickstart.md)** ·
**[Getting Started (all languages) →](docs/src/getting-started.md)**

### Serve a REST API

`stateset-http` is an embeddable layer, started from your Rust application. It
binds `127.0.0.1:3000` by default and serves the OpenAPI 3.1 spec at
`/api/v1/openapi.json`, with an interactive reference at `/api/v1/docs`. Auth
is on by default: skip `with_bearer_auth` and the server generates a token and
prints it at startup. See [step 4 of the Rust quickstart](QUICKSTART.md).

---

## Why iCommerce

Most commerce APIs answer "what endpoint can the model call?" StateSet answers
the harder question: "how can this actor commit resources safely, recover from
failure, and prove what happened?"

- **State, not chat history.** Orders, inventory, balances, approvals, and
  workflows survive process restarts and evolve through guarded state machines.
- **Authority, not model confidence.** The agent proposes an intent; an
  operator-owned, deny-by-default policy decides whether it may execute.
- **Transactions, not tool-call optimism.** Domain state, budgets, events, and
  idempotency records commit atomically or roll back together.
- **Receipts, not unverifiable narration.** Consequential actions return a
  sealed record of actor, principal, intent, policy decision, observed result,
  audit anchor, and settlement evidence.
- **A small public vocabulary over deep machinery.** Agents reason in `quote`,
  `buy`, `sell`, `pay`, `fulfill`, `return_order`, `refund`, and `subscribe`
  while the engine keeps its full domain API and generated MCP catalog.
- **Embedded by default.** SQLite is the zero-infrastructure path; PostgreSQL
  is there when multiple workers need a shared backend.

### Kernel-executed commerce

The kernel is the boundary between a model suggesting an action and software
being allowed to perform it:

```text
objective → typed intent → authenticated agent → delegated authority
          → policy + budget → atomic domain command → economic receipt
```

| Primitive | What it guarantees |
| --------- | ------------------ |
| `EconomicAgent` | The actor, delegating principal, role, tenant/store scope, credentials, capabilities, and public key are explicit. |
| `EconomicAuthority` | Exact autonomous, approval-required, and denied ranges compile into a deny-by-default kernel policy. |
| `EconomicIntent` | Eight stable commerce verbs carry exact commitments and idempotency, independent of any model framework. |
| Durable budgets | SQLite and PostgreSQL debit scoped money budgets atomically with the economic mutation. Preview and replay do not double-spend. |
| `EconomicReceipt` | Actor, intent, decision, result, audit anchor, and settlement evidence share one canonical digest that trusted parties can co-sign. |

Identity and policy are trusted runtime inputs. They are never accepted from a
model-authored tool call. Monetary limits use decimal strings; non-fiat assets
use separate asset identifiers; inventory commands bind the authorized quantity
to the executor-observed quantity.

Details: [Kernel Execution](docs/src/kernel-execution.md) ·
[Trust Foundation](docs/src/trust-foundation.md) (security model and its
explicit gap inventory) · [Kernel Roadmap](docs/src/kernel-roadmap.md)

---

## Agent frameworks

If your agent runtime lives inside your application process and wants
JSON-schema tools rather than stdio MCP, use the embedded toolkit:

| Entrypoint | For |
| ---------- | --- |
| `@stateset/embedded/openai` | OpenAI tool-calling / Agents SDK |
| `@stateset/embedded/vercel-ai` | Vercel AI SDK |
| `@stateset/embedded/langchain` | LangChain / LangGraph JS |
| `@stateset/embedded/generic` | any runtime (plain `{ name, description, schema, execute }` descriptors) |
| `stateset_embedded.openai` / `.langchain` / `.crewai` / `.autogen` | Python equivalents |

Install the Python framework extras in one step with
`pip install "stateset-embedded[agents]==1.35.1"`.

The toolkit also exposes payment-aware helpers (`getPayableToolCatalog()`,
`executePaidTool()`, `discoverRemotePaymentService()`) and contract/replay
helpers (`getRuntimeContract()`, `simulatePlan()`, `executePlan()`,
`replayMutation()`). Call `simulateMutation()` or
`executePlan({ dryRun: true })` before enabling writes, then set `allowApply`
only on agents that should mutate commerce state.

Working examples: [Embedded Agent Toolkit](docs/src/guides/agent-toolkit.md) ·
[AI Agents](docs/src/ai-agents.md)

### Agent-to-agent commerce

Direct agent-to-agent payments, quotes and negotiation, escrow with conditional
release, split payments, subscriptions, discovery and reputation, and signed
event streaming are first-class. See
[Intelligent Commerce Protocol](docs/src/icp.md),
[A2A Protocol Overview](docs/src/a2a/overview.md), and
[x402](docs/src/payments/x402.md).

The deterministic multi-agent marketplace demo puts a buyer, three merchants,
and a payment agent on one Ed25519-signed, totally ordered message board, where
the winning award crosses two independent kernel boundaries:

```bash
(cd bindings/node && npm run build)
node examples/sequencer-marketplace/demo.mjs --self-test --kernel
```

---

## Architecture

A layered Rust kernel with two outer product surfaces: language bindings, and
the Node-based operator runtime.

```text
stateset-primitives | stateset-crypto | stateset-pricing | stateset-observability
stateset-policy | stateset-authz | stateset-a2a | stateset-jobs
stateset-migrations | stateset-macros
        ->
stateset-core | stateset-sync
        ->
stateset-db
        ->
stateset-embedded
        ->
stateset-http | stateset-sdk | bindings/*
        ->
admin | cli
```

Read it in this order: `stateset-core`, `stateset-db`, `stateset-embedded`,
then `stateset-sync`/`stateset-policy`/`stateset-authz`/`stateset-pricing`,
then `stateset-http`, then `bindings/node` and `admin/` or `cli/`.

Full walkthrough: [Architecture](docs/src/architecture.md) ·
[Dependency Direction](docs/src/guides/dependency-direction.md) ·
[Workspace Inventory](docs/src/appendix/workspace-inventory.md) (generated)

---

## Development

```bash
nvm use                      # .nvmrc / .node-version (20.20.0)
rustup show active-toolchain # pinned by rust-toolchain.toml (1.90.0)

npm run check                # developer gate across the core workspace surfaces
npm run check:release        # release preflight; run before cutting a tag
```

Per-surface loops:

```bash
cargo test -p stateset-core -p stateset-db -p stateset-embedded
npm --prefix cli test
npm --prefix admin test
npm --prefix bindings/node run check
```

The authoritative remote gate is CI on `master`. CI also verifies the workspace
MSRV on Rust 1.85 and runs the binding and admin surfaces on the same pinned
Node 20.20.0.

Contributing: [CONTRIBUTING.md](CONTRIBUTING.md) ·
Releasing: [RELEASING.md](RELEASING.md) ·
Security: [SECURITY.md](SECURITY.md)

---

## Documentation

- **[The book](docs/src/SUMMARY.md)** — concepts, every commerce domain, A2A,
  payments, security, policy, adapters, operations, and the API reference.
  Build it locally with `mdbook serve docs`.
- **[Tool catalog](cli/docs/TOOLS.md)** — generated, authoritative list of the
  MCP/CLI tool surface (923 tools across 87 domains today).
- **[API reference per binding](docs/src/api/)** — Rust, Node, Python, Go,
  Java, Kotlin, Swift, .NET, Ruby, PHP, WASM.
- **[Examples](examples/README.md)** — runnable end-to-end programs.
- **[Rust API docs](https://docs.rs/stateset-sdk)** on docs.rs.
- **[CHANGELOG.md](CHANGELOG.md)** — release history.
- `llms.txt` — a machine-readable index for agents; [AGENTS.md](AGENTS.md) is
  the short orientation for an agent working against this repo.

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.
