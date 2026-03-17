# ADR-0003: Event-Driven Extensions and Auditability

- Status: Accepted
- Date: 2026-02-05

## Context

Commerce systems need reliable integration points for downstream systems (analytics, fulfillment, notifications) and an audit trail for regulatory and operational requirements. We considered three integration models:

1. **Synchronous hooks** — Call external systems during commerce operations. Simple but couples core workflows to external systems, increases latency, and creates cascading failures.
2. **Polling** — External systems periodically query for changes. Simple but wasteful and introduces latency between event and detection.
3. **Event-driven** — Emit structured events that external systems subscribe to asynchronously. Decoupled, low-latency, and naturally auditable.

## Decision

Provide an event system that emits structured commerce events and supports subscriptions and webhooks. The VES (Verifiable Event Sync) layer builds on these events to provide ordered replication and cryptographic proofs for auditability.

### Event Flow

```
Commerce Operation
    └── Emit Event (e.g., order.created)
         ├── Local Event Log (SQLite)
         ├── Outbox (for VES sync)
         ├── SSE Stream (real-time subscribers)
         ├── Webhook Delivery (HMAC-signed)
         └── Heartbeat Monitor (trigger checks)
```

### Event Categories

| Category | Events |
|----------|--------|
| Orders | `order.created`, `order.processing`, `order.shipped`, `order.delivered`, `order.cancelled` |
| Payments | `payment.created`, `payment.captured`, `payment.refunded`, `payment.failed` |
| Inventory | `inventory.adjusted`, `inventory.reserved`, `inventory.released`, `inventory.low` |
| Returns | `return.requested`, `return.approved`, `return.rejected`, `return.completed` |
| Subscriptions | `subscription.created`, `subscription.charged`, `subscription.cancelled` |
| A2A | `a2a.payment`, `a2a.quote`, `a2a.escrow`, `a2a.dispute` |

## Consequences

**Positive:**
- Integrations consume events asynchronously without blocking core writes
- The system supports verifiable, replayable histories for compliance and debugging
- VES provides cryptographic proof that events occurred and were not tampered with
- SSE streaming enables real-time multi-agent coordination
- Webhook delivery with HMAC-SHA256 signatures prevents spoofing

**Negative:**
- Event delivery requires operational considerations for backpressure management
- The outbox pattern adds a small amount of write overhead per operation
- Event schema versioning must be managed carefully to avoid breaking downstream consumers

## Event Schema Versioning

Events use a type string that implicitly includes version context. When an event schema changes:

1. New fields are added as optional — existing consumers are unaffected
2. If a breaking change is required, a new event type is created (e.g., `order.created.v2`)
3. The old event type continues to be emitted for one major version

## Replay Safety

Events are designed to be safely replayable:

- Each event has a unique ID and timestamp
- Events include a reference to the previous event hash (chain)
- Idempotency keys prevent duplicate processing on replay
