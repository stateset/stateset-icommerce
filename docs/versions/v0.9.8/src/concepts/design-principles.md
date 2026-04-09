# Design Principles

These principles guide every architectural decision in StateSet iCommerce.

## 1. Local-First Execution

iCommerce runs entirely in-process using SQLite as its default storage backend. No network calls, no external services, no containers. An agent can `npm install @stateset/embedded` and have a full commerce engine running in the same process. This eliminates latency, reduces failure modes, and enables offline-first operation.

## 2. Deterministic Operations

Every operation in the commerce engine is a pure function of its inputs and the current database state. There are no hidden side effects, no background timers affecting computation, and no non-deterministic behavior. This property is critical for AI agents: it means operations can be safely replayed, simulated, and reasoned about.

## 3. Type Safety Through Newtypes

The Rust core uses strongly-typed newtypes for all 24 entity identifiers. An `OrderId` cannot be accidentally passed where a `CustomerId` is expected — the compiler rejects it at build time. This prevents an entire class of bugs that are common in stringly-typed commerce systems.

```rust
pub struct OrderId(Uuid);
pub struct CustomerId(Uuid);
pub struct ProductId(Uuid);
// ... 24 total — the compiler enforces correctness
```

## 4. Explicit State Machines

Every domain aggregate (Order, Payment, Return, Subscription, WorkOrder) has an explicit state machine with validated transitions. The `can_transition_to()` method returns whether a transition is valid, and `is_terminal()` indicates whether further transitions are possible. Invalid transitions produce typed errors rather than silently corrupting state.

```rust
// Order states: Pending → Processing → Shipped → Delivered
//                  └─────→ Cancelled
order.can_transition_to(OrderStatus::Shipped); // true if Processing
order.can_transition_to(OrderStatus::Pending); // false — no backward transitions
```

## 5. Preview Before Execute

All write operations are blocked by default. The `--apply` flag must be explicitly provided to enable mutations. Without it, every operation returns a preview of what would happen — how many records would be affected, what state changes would occur — without actually executing.

This is essential for autonomous agents. An LLM can:
1. Call a tool without `--apply` to see what would change
2. Reason about the preview in its context window
3. Decide whether to proceed
4. Call again with `--apply` to commit

## 6. Explainable Denials

Traditional APIs return opaque error codes (`400 Bad Request`) that cause LLMs to retry the same failing request in a loop. iCommerce's policy engine returns structured denials with per-condition breakdowns:

```json
{
  "allowed": false,
  "reason": "Return exceeds 30-day window",
  "conditions": [
    { "field": "days_since_purchase", "expected": "< 30", "actual": 45 }
  ],
  "remediation": "Submit for manager approval via returns.escalate"
}
```

This explanation flows directly into the LLM's context window, enabling the agent to autonomously correct its parameters or take an alternative action without human intervention.

## 7. Cryptographic Verifiability

Every state mutation is captured as a structured event, signed with Ed25519, and organized into Merkle trees. The VES v1.0 specification ensures tamper-proof audit trails that can be independently verified without trusting the system that produced them.

## 8. Layered Architecture

Dependencies flow strictly downward:

```
stateset-core        → Domain types, business rules (no I/O)
stateset-db          → Persistence backends (depends on core)
stateset-embedded    → High-level API surface (depends on db)
bindings/*           → Language wrappers (depends on embedded)
cli/                 → MCP tools, agents (depends on embedded)
```

Layers may only depend on layers below them. This makes the system modular, testable, and safe to evolve.

## 9. Convention Over Configuration

Sensible defaults everywhere:

- SQLite is the default backend (zero config)
- Read-only is the default mode (safe by default)
- All policies are opt-in (nothing is blocked until you write a rule)
- Tier detection is automatic (no manual tier selection)
- Adapters auto-discover based on config presence

## 10. Test Everything

The codebase maintains comprehensive test coverage across all layers:

- **3,477 Rust tests** — unit, integration, property-based, and snapshot tests
- **10,700+ CLI tests** — tool handlers, A2A protocol, adapters, security
- **261 admin tests** — UI components and API routes
- **0 clippy warnings** — all Rust code passes strict linting

## Trade-offs: When iCommerce Is NOT the Right Choice

These principles have costs. Be honest about when iCommerce is the wrong tool:

| Scenario | Why iCommerce May Not Fit | Better Alternative |
|----------|--------------------------|-------------------|
| High-concurrency retail site (10k+ req/s) | SQLite is single-writer; even PostgreSQL adds latency vs. Redis-backed caches | Shopify/Stripe + CDN + custom cache |
| Pure payment gateway (no commerce state) | iCommerce is a full commerce engine; overkill for payment-only | Stripe API directly |
| Real-time collaborative editing | Local-first means stale reads until sync; no real-time conflict resolution | Firestore / Supabase Realtime |
| Existing platform with 100k+ products | Migration effort is significant; adapters sync incrementally, not instantly | Keep existing platform, use iCommerce for agent layer only |

### When iCommerce Shines

- AI agents that need deterministic, auditable commerce operations
- Multi-agent systems where agents pay each other
- Offline-first or edge deployments
- Regulated industries needing cryptographic audit trails
- Greenfield agent-native commerce applications
