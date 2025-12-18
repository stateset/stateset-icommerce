# StateSet iCommerce

## The Commerce Runtime for the Agent Economy

---

## Executive Summary

Commerce is undergoing its third major platform shift. The first wave moved transactions online (eCommerce). The second wave decoupled frontends from backends (headless commerce). The third wave—now beginning—hands commerce operations to autonomous AI agents.

StateSet is building the infrastructure for this shift: an embedded, deterministic commerce engine designed for AI agents. Where eCommerce was built for humans clicking buttons, iCommerce is built for agents making decisions.

We are creating the SQLite of commerce—portable, embeddable, zero-configuration, and universally deployable. Our open protocol (ACP) positions StateSet as the standard execution layer for any AI agent transacting in the real world.

---

## The Shift: From eCommerce to iCommerce

### The Problem with Current Infrastructure

Today's commerce platforms were designed for humans operating dashboards:

| Assumption | Reality in Agent Era |
|------------|---------------------|
| Users authenticate via OAuth | Agents need embedded access |
| Operations happen at human speed | Agents operate in milliseconds |
| State lives in vendor clouds | Agents need portable state |
| UIs surface information | Agents need structured data |
| Rate limits assume human pace | Agents hit limits instantly |

Every major commerce platform—Shopify, BigCommerce, Salesforce Commerce Cloud—shares these assumptions. They are fundamentally human-centric.

### The Agent Commerce Gap

AI agents from OpenAI, Anthropic, Google, and the open-source ecosystem are rapidly becoming capable of complex reasoning and task execution. Commerce is among the most common real-world actions these agents need to perform:

- Processing returns and refunds
- Managing subscriptions
- Handling customer inquiries
- Coordinating fulfillment
- Negotiating B2B procurement
- Managing inventory and reordering

Yet there is no commerce infrastructure designed for agents. Developers resort to brittle API integrations, screen scraping, or building custom backends. The result: agents that are slow, unreliable, and disconnected from real commerce state.

---

## The StateSet Solution

### iCommerce: A New Category

We define **iCommerce** (intelligent commerce) as the category of commerce infrastructure built for autonomous agents rather than human operators.

iCommerce is characterized by:

- **Embedded execution** — Commerce logic runs in-process, not over the network
- **Portable state** — A single file contains complete operational state
- **Deterministic operations** — Predictable outcomes that agents can reason about
- **Policy enforcement** — Business rules that constrain agent behavior
- **Protocol-first design** — Standardized interfaces across any AI system

### StateSet iCommerce Engine

StateSet is the reference implementation of iCommerce—an embedded commerce library written in Rust with bindings for every major runtime:

```
┌─────────────────────────────────────────────────────────────┐
│                     AI Agent Layer                          │
│            (ChatGPT, Claude, Gemini, LangChain)             │
└─────────────────────────────────────────────────────────────┘
                              │
                   Agentic Commerce Protocol (ACP)
                              │
┌─────────────────────────────────────────────────────────────┐
│                   StateSet iCommerce                        │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────┐  │
│  │  Policy (NSR) │  │ State (SQLite)│  │    Execution    │  │
│  │   Guardrails  │  │   Portable    │  │  Deterministic  │  │
│  └───────────────┘  └───────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Core capabilities:**

| Domain | Modules |
|--------|---------|
| Commerce | Customers, Orders, Products, Returns, Payments |
| Supply Chain | Purchase Orders, Inventory, Shipments, Receiving |
| Operations | Work Orders, BOM, Warranties, Invoices |
| Agent-Native | Policies, Approvals, Events, Conversations |

**Developer experience:**

```javascript
import { Commerce } from '@stateset/icommerce';

const commerce = new Commerce('./store.db');

// Your agent now has a complete commerce engine
const order = await commerce.orders.create({
  customerId: customer.id,
  items: [{ sku: 'WIDGET-001', quantity: 2, unitPrice: 29.99 }]
});
```

No accounts. No API keys. No network dependency. Just a file path.

### Universal Runtime Support

One Rust core, every platform:

| Platform | Package | Install |
|----------|---------|---------|
| Node.js | `@stateset/icommerce` | `npm install` |
| Python | `stateset-icommerce` | `pip install` |
| Browser/Edge | `@stateset/icommerce-wasm` | `import init` |
| Rust | `stateset-icommerce` | `cargo add` |
| CLI | `@stateset/cli` | `npm install -g` |

---

## Agentic Commerce Protocol (ACP)

### Protocol as Wedge

StateSet's go-to-market strategy centers on the **Agentic Commerce Protocol**—an open standard for AI agents performing commerce operations.

ACP defines:

- **Capabilities** — Standardized operations (inventory.check, orders.create, returns.initiate)
- **Schemas** — Common data models (Order, Customer, Product, Fulfillment)
- **Policies** — Declarative business rules that constrain agent behavior
- **Trust boundaries** — What agents can do autonomously vs. requiring approval

### Protocol Network Effects

```
┌──────────────────────┐
│  Claude (Anthropic)  │──────┐
└──────────────────────┘      │
┌──────────────────────┐      │        ┌─────────────────────┐
│  ChatGPT (OpenAI)    │──────┼──ACP───│  StateSet iCommerce │
└──────────────────────┘      │        │  (reference impl)   │
┌──────────────────────┐      │        └─────────────────────┘
│  Gemini (Google)     │──────┤                  OR
└──────────────────────┘      │        ┌─────────────────────┐
┌──────────────────────┐      │        │  ACP adapters for   │
│  LangChain / AutoGPT │──────┘        │  Shopify, Stripe,   │
└──────────────────────┘               │  WooCommerce, etc   │
                                       └─────────────────────┘
```

As ACP adoption grows, StateSet benefits regardless of which implementation is used—but the embedded engine remains the fastest, most complete, and most portable option.

### ACP + MCP Integration

ACP is designed as a domain-specific layer atop Anthropic's Model Context Protocol (MCP), positioning StateSet within the emerging agent tool ecosystem:

```
MCP (generic tool protocol)
 └── ACP (commerce-specific)
      └── StateSet (reference implementation)
```

---

## Neuro-Symbolic Reasoning Engine

### The Trust Problem

Large language models are non-deterministic. Letting GPT-4 directly control refund amounts, inventory allocations, or pricing decisions is operationally dangerous.

StateSet's **NSR (Neuro-Symbolic Reasoning) Engine** provides the trust boundary between agent intent and commerce execution:

```
Agent: "Give this customer a full refund"
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│                  StateSet Policy Engine                     │
│                                                             │
│   check_refund_eligibility(order)                           │
│   → order_age: 45 days                                      │
│   → policy: 30 day window                                   │
│   → result: DENIED                                          │
│                                                             │
│   suggest_alternatives()                                    │
│   → store_credit: eligible                                  │
│   → exchange: eligible                                      │
└─────────────────────────────────────────────────────────────┘
                    │
                    ▼
Agent: "I can't process a full refund past 30 days, but I can 
        offer store credit or an exchange. Which would you prefer?"
```

### Symbolic + Neural

| Layer | Responsibility | Characteristics |
|-------|---------------|-----------------|
| LLM | Intent, conversation, judgment | Flexible, non-deterministic |
| NSR | Business rules, constraints | Rigid, auditable, deterministic |
| Core | State changes, transactions | ACID, reliable, portable |

This architecture gives enterprises confidence that agent-driven commerce will respect their policies while preserving the flexibility that makes AI valuable.

---

## Market Opportunity

### Addressable Markets

| Market | Size | StateSet Position |
|--------|------|-------------------|
| Commerce Platforms | $15B | Embedded alternative to SaaS |
| Order Management Systems | $3B | Agent-native OMS |
| Customer Service Automation | $12B | NSR-powered, deterministic |
| Inventory & Warehouse | $5B | Lightweight WMS |
| Procurement Software | $10B | Embedded procurement |
| AI Agent Infrastructure | $2B → $50B+ | Ground floor |

**Total addressable market: $40B+ in existing categories, with AI agent infrastructure representing uncapped greenfield opportunity.**

### The Wedge

Our initial focus: **AI agent infrastructure for commerce operations**.

Every company deploying customer service agents, procurement bots, or operational automation needs a commerce execution layer. StateSet is the only embedded, agent-native option.

### Expansion Path

```
Year 1: Agent commerce infrastructure (returns, support, ops)
Year 2: Embedded OMS for D2C and B2B
Year 3: Full operational ERP for SMB
Year 4+: Standard infrastructure for agent economy
```

---

## Business Model

### Open Core + Cloud

| Tier | Offering | Price |
|------|----------|-------|
| **Open Source** | Embedded engine, local-only | Free |
| **Pro** | Managed sync, cloud backup, hosted endpoints | $99-499/mo |
| **Enterprise** | Multi-tenant, SSO, audit logs, SLAs, NSR studio | Custom |

### Revenue Streams

1. **StateSet Cloud** — Hosted sync, backup, and multi-device coordination
2. **Enterprise Licenses** — On-premise deployment with support
3. **NSR Studio** — Visual policy builder for enterprise customers
4. **Marketplace** — Pre-built agent templates and integrations

### Unit Economics

The embedded model inverts traditional SaaS economics:

| Traditional SaaS | StateSet |
|------------------|----------|
| Host everything | Customer hosts compute |
| Scale infrastructure cost | Near-zero marginal cost |
| Per-seat pricing | Per-sync or GMV pricing |
| High CAC, high churn | Developer adoption, sticky infrastructure |

---

## Competitive Landscape

### Current Options for Agent Commerce

| Option | Limitation |
|--------|------------|
| Shopify/BigCommerce APIs | Human-centric, rate-limited, network-dependent |
| Custom backends | Expensive, slow, non-standard |
| Stripe | Payments only, not full commerce |
| Headless (Medusa, Saleor) | Still SaaS architecture, not embeddable |

### StateSet Differentiation

| Capability | StateSet | Alternatives |
|------------|----------|--------------|
| Embedded (no network) | ✓ | ✗ |
| Single-file portable state | ✓ | ✗ |
| Agent-native protocol | ✓ | ✗ |
| Policy/guardrail engine | ✓ | ✗ |
| Multi-runtime (Node, Python, WASM) | ✓ | ✗ |
| Offline-first | ✓ | ✗ |

**No one else is building agent-native commerce infrastructure.**

---

## Go-to-Market Strategy

### Phase 1: Protocol Establishment

- Publish ACP specification (open, MIT licensed)
- Ship MCP integration for Claude
- Ship function definitions for ChatGPT
- LangChain/LlamaIndex toolkit
- Developer documentation and tutorials

### Phase 2: Developer Adoption

- Presence in AI/agent communities
- Reference architectures for common use cases
- Integration with agent frameworks
- Open source community building

### Phase 3: Enterprise Expansion

- Customer service automation deployments
- B2B procurement agent implementations
- Manufacturing operations pilots
- NSR Studio for policy management

### Key Metrics

| Metric | Year 1 Target | Year 3 Target |
|--------|---------------|---------------|
| npm/PyPI downloads | 50K | 2M+ |
| Active deployments | 1,000 | 50,000+ |
| Cloud subscribers | 100 | 5,000+ |
| ARR | $500K | $15M+ |
| GMV through StateSet | $100M | $10B+ |

---

## Technical Architecture

### Crate Structure

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/        # Domain models, business logic
│   ├── stateset-db/          # SQLite persistence
│   ├── stateset-nsr/         # Neuro-symbolic reasoning
│   ├── stateset-sync/        # cr-sqlite CRDT sync
│   ├── stateset-acp/         # Protocol definitions
│   └── stateset-embedded/    # Unified interface
├── bindings/
│   ├── node/                 # N-API bindings
│   ├── python/               # PyO3 bindings
│   └── wasm/                 # WebAssembly
└── cli/                      # AI-powered CLI
```

### Design Principles

1. **Embedded-first** — Network is optional, not required
2. **Portable state** — Single file contains everything
3. **Deterministic execution** — Same input, same output, always
4. **Policy-enforced** — Business rules are code, not suggestions
5. **Protocol-native** — ACP is the interface, not an afterthought

---

## Why Now

### Convergence of Trends

1. **Agent capabilities** — GPT-4, Claude, Gemini can now handle complex reasoning
2. **Tool use protocols** — MCP, function calling standardizing agent-tool interaction
3. **Edge compute** — WASM, edge workers enable embedded execution anywhere
4. **Local-first movement** — Developers want control, not SaaS dependency
5. **Enterprise AI adoption** — Companies deploying agents need guardrails

### Window of Opportunity

The agent commerce infrastructure category is forming now. Standards are being set. The company that defines how agents do commerce will own the category for decades.

SQLite shipped in 2000 and remains the most deployed database 25 years later. Infrastructure categories, once won, are permanent.

---

## The Vision

### 2025
Agents use StateSet for customer service, returns, and basic operations.

### 2027
iCommerce becomes the standard protocol. Every major agent framework supports it. StateSet processes $100B+ GMV.

### 2030
iCommerce surpasses traditional eCommerce in transaction volume. Agents handle the majority of routine commerce operations. StateSet is infrastructure—invisible, ubiquitous, essential.

---

## Summary

**The opportunity:** Commerce infrastructure for the agent economy

**The product:** StateSet iCommerce—embedded, deterministic, portable

**The strategy:** Open protocol (ACP) + reference implementation + cloud services

**The outcome:** The SQLite of commerce—universal infrastructure for a new era

---

*LLMs reason. StateSet executes. Every agent needs a commerce engine.*

---

## Contact

StateSet Inc.
https://stateset.com