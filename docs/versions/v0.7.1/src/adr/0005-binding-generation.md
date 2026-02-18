# ADR-0005: Binding Generation From a Single Spec

- Status: Accepted
- Date: 2026-02-05

## Context

StateSet iCommerce ships 11 language bindings. Hand‑maintaining each binding would create drift, inconsistent behavior, and high maintenance cost as the API surface evolves.

## Decision

Define the public binding surface once in a declarative generator spec and derive bindings from it. The generator spec is the source of truth for exposed types and operations, while each binding remains free to provide language‑idiomatic wrappers.

## Consequences

- API parity across languages improves and regressions are easier to spot.
- Changes to the surface area require updates in a single place.
- Some language‑specific conveniences still require lightweight handwritten glue.
