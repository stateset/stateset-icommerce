# StateSet iCommerce — for AI agents

An **embedded commerce engine** ("the SQLite of commerce"): orders, inventory,
customers, products, carts/checkout, payments, returns, subscriptions,
promotions, analytics, a full finance suite (general ledger, month-end close,
AP/AR, fixed assets, revenue recognition), warehouse management, and
traceability — running **inside your process** against a single database file
with the default SQLite backend (`stateset-db` with the `sqlite` feature).
No external services, no API keys, no rate limits in that configuration
(the optional `postgres` backend connects to an external PostgreSQL).
Money is exact decimal end-to-end; mutations that touch the store write a
best-effort `audit_log` entry (see `crates/stateset-db/src/audit.rs`).

## If you are an MCP-native agent (Claude Desktop, Cursor, Windsurf, ...)

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

Every tool carries a **stability tier** (`cli/src/tools/tool-tiers.js`):

| Tier | Tools | What it means |
|---|---:|---|
| `core` | 204 | The default surface. Catalog, customers, carts/checkout, orders, payments and refunds, returns, shipments, inventory, promotions, tax, gift cards and store credit, analytics, plus the agentic planning/replay tools. Smoke-gated: every core tool works or refuses cleanly on a fresh store, with no known-defect backlog. |
| `extended` | 491 | Real but specialised domains: finance suite, manufacturing, WMS, B2B, subscriptions/reviews/loyalty, integrations. |
| `experimental` | 264 | Demo, external-stack-dependent (wallet, chain, API key, demo stack) or known-incomplete: A2A, agent receipts, sync, vector search, on-chain treasury. |

With no `--profile`, the MCP servers expose exactly the `core` tier. Use
`--profile all` for everything (959 tools: 944 across 88 domains plus 15
agentic runtime tools), or `finance`, `operations`, and `agents` for curated
workloads (these expose every tool in their domains, whatever its tier). Add
individual domains with `--domains a,b`. **Writes are preview-only by
default** — tools describe what they would do; add `--apply` to the args to
enable mutations. The generated tool catalog, with each tool's tier and each
profile's size, is [`cli/docs/TOOLS.md`](cli/docs/TOOLS.md) (authoritative).

For an autonomous production endpoint, also provide operator-owned kernel
policy and principal files. This enables strict mode: only typed governed write
commands are exposed, while read tools remain available. Identity and policy
never come from model arguments. The governed catalog covers a complete
checkout — customers, carts (items, shipping address, payment method, coupon,
tax), checkout, payment create/capture, order transitions, shipments, returns
(create, tracking, transitions) and refunds — so grant those capabilities
(see [`kernel/examples/strict-policy.json`](kernel/examples/strict-policy.json))
and a strict agent can sell end to end. Promotions and coupons are operator
configuration: strict agents redeem them (`apply_cart_discount`) but cannot
create them; provision them outside the agent endpoint.

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

**Over HTTP instead** (hosted deployments, remote agents): `stateset-mcp-http`
serves the same tools via MCP Streamable HTTP at protocol revision 2026-07-28,
stateless by construction — no session ids, no per-client state, a fresh server
per request, so it scales across replicas. 2025-era clients are still served.
Writes go to one shared store (`--db`, default an ephemeral demo-seeded
`:memory:`):

```json
{ "mcpServers": { "stateset-sandbox": { "url": "http://localhost:8090/mcp" } } }
```

Run it with `npx -y -p @stateset/cli stateset-mcp-http` (add
`--host 0.0.0.0` to expose, `--read-only` to disable writes, or
`--strict-protocol` to refuse pre-2026-07-28 clients).

## If you are building an application (coding agent)

Pick the ecosystem; every install below is published, current, and verified:

```bash
cargo add stateset-sdk --features full     # Rust (the native engine)
npm install @stateset/embedded             # Node.js (prebuilt per-platform binary)
pip install stateset-embedded              # Python (wheels for 5 platforms)
```

Minimal working program (same shape in all three languages):

```javascript
import { Commerce } from '@stateset/embedded';
const commerce = new Commerce('./store.db');   // or ':memory:'
const customer = await commerce.customers.create({
  email: 'a@example.com', firstName: 'Ada', lastName: 'L',
});
```

Scaffold a full storefront: `npm create stateset-app`.

## If you are wiring agent frameworks

`@stateset/embedded` ships typed adapter entrypoints — see
[docs/src/ai-agents.md](docs/src/ai-agents.md) for working examples:

| Entrypoint | For |
|---|---|
| `@stateset/embedded/openai` | OpenAI tool-calling / Agents SDK |
| `@stateset/embedded/vercel-ai` | Vercel AI SDK |
| `@stateset/embedded/langchain` | LangChain JS |
| `@stateset/embedded/generic` | any framework (plain descriptors) |
| `stateset_embedded.openai` / `[langchain]` / `[crewai]` / `[autogen]` | Python equivalents |

## Conventions that matter

- **`--apply` gates every write**, in the CLI, the MCP server, and the
  toolkits. Preview first is the default posture; respect it.
- **Money is never floats.** Amount fields are decimal strings/`Decimal`.
- Agent-to-agent rails: x402 payment intents, agent cards/discovery, escrow,
  and split payments are first-class tools (see the `a2a` and `x402` domains).
- The repo's own agent guidance for working on this codebase lives in
  `cli/.claude/` (skills, agents, CLAUDE.md).

## Pointers

- Docs: https://docs.stateset.com (also `/llms.txt`)
- Tool catalog (generated, authoritative): `cli/docs/TOOLS.md`
- API references: `docs/src/api/` (rust, node, python, and 8 more)
- Trust & verifiability model: `docs/src/trust-foundation.md`
