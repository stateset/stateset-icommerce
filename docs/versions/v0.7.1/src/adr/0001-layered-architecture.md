# ADR-0001: Layered Architecture

- Status: Accepted
- Date: 2026-02-05

## Context

StateSet iCommerce spans domain logic, persistence, public APIs, and multiple language bindings. Without explicit boundaries, changes in lower layers can leak upward and create tight coupling, slowing releases and increasing regression risk.

## Decision

Adopt a strict layered architecture with one-way dependencies:

- `stateset-core` defines domain types, invariants, and business rules.
- `stateset-db` implements persistence backends and migrations.
- `stateset-embedded` provides the high-level sync/async API surface.
- `bindings/*` expose the API to other languages without re-implementing domain logic.

Layers may only depend on layers below them. Cross-cutting concerns (events, tracing, validation) live in the lowest viable layer.

## Consequences

- Clear ownership and boundaries reduce coupling and make refactors safer.
- New backends or bindings can be added without touching domain logic.
- Some features require additional adapter code to avoid breaking layering rules.
