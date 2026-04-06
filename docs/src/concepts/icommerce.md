# What is iCommerce?

## From eCommerce to iCommerce

The commerce software stack has remained structurally unchanged for two decades: a centralized server exposes REST APIs, human operators manage state through dashboards, and integration is achieved through webhook pipelines and manual orchestration. This architecture assumes a human in the loop at every decision point.

The emergence of autonomous AI agents — systems capable of reasoning, planning, and executing multi-step operations — demands a fundamentally different commerce runtime.

**iCommerce** (Intelligent Commerce) is the paradigm shift from human-operated commerce platforms to agent-native commerce engines. StateSet iCommerce is the reference implementation of this paradigm.

## The Platform Shift

```
eCommerce (2000s)     → Monolithic platforms, human operators, REST APIs
Headless Commerce     → Decoupled frontend, API-first, still human-operated
  (2015s)
Composable Commerce   → Microservices, best-of-breed, still assumes dashboards
  (2020s)
iCommerce (2025+)     → Embedded engines, agent-native, deterministic,
                         cryptographically verifiable
```

## What Agents Need (That Platforms Don't Provide)

| Traditional Platform | What Agents Actually Need |
|---------------------|--------------------------|
| REST APIs with opaque error codes | Structured errors with per-field explanations that fit in a context window |
| Webhooks for integration | Cryptographically verifiable event streams with replay |
| Dashboard for monitoring | Deterministic health checks that return machine-readable status |
| Manual approval workflows | Policy engine with programmatic deny/allow and auto-remediation |
| Server-side deployment | Embeddable library that runs in-process with zero dependencies |
| Separate payment gateway | Native payment primitives with agent-to-agent settlement |

## The Three Properties of iCommerce

### 1. Embeddable

iCommerce runs in-process, like SQLite. There is no server to deploy, no container to manage, no network call to make. An agent can `npm install @stateset/embedded` and have a full commerce engine — orders, inventory, payments, subscriptions, manufacturing, accounting — running in the same process.

This is not a simplified toy. The engine exposes 41 domain APIs with the same surface area as enterprise ERPs, backed by a type-safe Rust core with 21 crates and 3,477 passing tests.

### 2. Deterministic

Every operation is a pure function of its inputs and the current database state. There are no hidden side effects, no background timers affecting computation, and no non-deterministic behavior. This property is critical:

- Operations can be **simulated** before execution (preview mode)
- Operations can be **replayed** for debugging or audit
- Operations can be **reasoned about** by LLMs without surprises

### 3. Verifiable

Every state mutation is captured as a structured event, cryptographically signed with Ed25519, and organized into Merkle trees for efficient proof generation. The VES (Verifiable Encrypted Signatures) v1.0 specification ensures that:

- No event can be tampered with after signing
- Any subset of events can be independently verified
- The complete event history can be replayed to reconstruct any point-in-time state

## The Agent Economy

iCommerce isn't just about individual agents operating commerce stores. It's about agents transacting with each other:

- **Agent A** discovers **Agent B**'s capabilities via its ERC-8004 Agent Card
- Agent A requests a quote for a service, negotiating price and terms
- Funds are held in escrow with cryptographically enforced release conditions
- Reputation scores track agent reliability across dimensions (quality, speed, communication)
- Budget governance prevents any agent from overspending its daily cap
- Settlement happens on-chain for permanent, verifiable record-keeping

This is the **A2A (Agent-to-Agent) Commerce Protocol** — the first open protocol for autonomous economic transactions between AI agents.

## Why Not Just Use Stripe + Shopify + a Database?

You could. But consider what happens when an AI agent tries to:

1. **Check inventory before promising delivery** — requires synchronous, in-process access to inventory state, not an API call that might fail
2. **Preview a refund before executing it** — requires a dry-run capability that returns the exact financial impact without side effects
3. **Understand why a policy denied a return** — requires structured denial reasons, not a 403 status code
4. **Pay another agent $0.02 for an API call** — requires sub-cent payment primitives, not a $0.30 minimum Stripe charge
5. **Prove that an order was placed at a specific time** — requires cryptographic signatures, not database timestamps

iCommerce provides all of these as first-class primitives, not bolted-on integrations.

## How It Works

```
┌─────────────────────────────────────────────┐
│              AI Agent (LLM)                  │
│     Claude, GPT, Gemini, Llama, Ollama      │
└──────────────────┬──────────────────────────┘
                   │ MCP / Embedded Toolkit
                   ▼
┌─────────────────────────────────────────────┐
│           StateSet iCommerce                 │
│                                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐   │
│  │  Policy   │ │ MCP Tool │ │  A2A +   │   │
│  │  Engine   │ │ Surface  │ │  x402    │   │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘   │
│       └─────────────┼─────────────┘         │
│              ┌──────┴──────┐                │
│              │ Rust Core   │                │
│              │ (21 crates) │                │
│              └──────┬──────┘                │
│              ┌──────┴──────┐                │
│              │   SQLite    │                │
│              └─────────────┘                │
└─────────────────────────────────────────────┘
```

The agent talks to iCommerce through either:
- **MCP Tools** — for Claude Desktop, Cursor, Windsurf, and other MCP-native clients
- **Embedded Toolkit** — for OpenAI, Vercel AI SDK, LangChain, and custom agent runtimes

Both paths go through the same policy engine, permission system, and audit layer. The only difference is the transport.
