# ADR-0001: Layered Architecture

- Status: Accepted
- Date: 2026-02-05

## Context

StateSet iCommerce spans domain logic, persistence, public APIs, and multiple language bindings. Without explicit boundaries, changes in lower layers can leak upward and create tight coupling, slowing releases and increasing regression risk.

We considered three architectural approaches:

1. **Monolith** — Single crate with everything. Simple to start but impossible to test domain logic without a database, and bindings would depend on all internal details.
2. **Microservices** — Separate services communicating over gRPC/HTTP. Too much operational overhead for an embedded library. Agents need in-process access, not network calls.
3. **Layered crates** — Separate Rust crates with one-way dependencies. Domain logic is pure (no I/O), persistence is swappable, and bindings depend only on the public API surface.

## Decision

Adopt a strict layered architecture with one-way dependencies:

```
stateset-primitives  ← Zero-dependency ID types and value objects
       ↑
stateset-core        ← Domain models, business rules, repository traits (no I/O)
       ↑
stateset-db          ← SQLite + PostgreSQL implementations
       ↑
stateset-embedded    ← High-level sync/async API surface
       ↑
bindings/*           ← Node.js, Python, Ruby, Go, etc.
       ↑
cli/                 ← MCP tools, agents, A2A protocol
```

Layers may only depend on layers below them. Cross-cutting concerns (events, tracing, validation) live in the lowest viable layer.

## Consequences

**Positive:**
- Domain logic (`stateset-core`) can be tested without any database — unit tests are fast and deterministic
- New backends can be added (e.g., DynamoDB) without touching domain logic
- New bindings can be added without touching persistence code
- Each layer has clear ownership and can evolve independently
- The `stateset-primitives` crate has zero dependencies and compiles in under 1 second

**Negative:**
- Some features require adapter code to avoid breaking layering rules (e.g., the `DatabaseBackend` enum in `stateset-embedded` wraps both SQLite and PostgreSQL behind a common interface)
- Cross-layer refactors require changes in multiple crates
- Build times increase slightly due to crate boundaries

## When to Deviate

If a feature genuinely spans all layers (e.g., a new domain entity needs primitives, core, db, embedded, and binding support), implement it bottom-up in a single PR to keep the layers consistent.
