# Dependency Direction

This repository is easier to work in once you stop thinking of it as a single SDK and start thinking of it as a layered platform monorepo.

The current workspace manifests describe this dependency direction:

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

## How To Read The Graph

### Foundation layer

These crates are the lowest-level building blocks in the current workspace graph:

- `stateset-primitives`
- `stateset-crypto`
- `stateset-pricing`
- `stateset-observability`
- `stateset-policy`
- `stateset-authz`
- `stateset-a2a`
- `stateset-jobs`
- `stateset-migrations`
- `stateset-macros`

They are important because they stay narrow. Most of them are conceptually stable and do not depend on other internal crates.

### Domain kernel

`stateset-core` is the main fan-in point for the product/runtime graph. It is where domain types, repository traits, services, validation, events, and errors converge. Changes here travel far.

`stateset-sync` sits beside the core rather than above the CLI. That matters because sync is part of the Rust kernel story, not just operator tooling. Its wire-format types (event envelopes, sync batches) and Merkle construction live in `stateset-sync` and `stateset-crypto` (the latter implements the VES v1.0 domain-separated Merkle tree).

### Storage and embeddable API

`stateset-db` depends on `stateset-core`, and `stateset-embedded` depends on `stateset-db`, `stateset-core`, `stateset-observability`, and `stateset-pricing`.

This makes `stateset-embedded` the main productized Rust surface. It is where persistence, pricing, observability, and domain APIs meet before they fan out into HTTP and the bindings.

### Edge adapters

`stateset-http` is a thin edge adapter over the commerce engine and a few cross-cutting crates. It is important operationally, but it is not the architectural center of gravity.

`stateset-sdk` is the Rust-facing facade crate. It is useful for Rust consumers who want one dependency with feature flags, but it is not the universal substrate for every language binding.

`stateset-ffi` is an optional C-ABI oriented interop surface. It exists for explicit C-style integration, but the binding manifests in this repo mostly link directly to `stateset-embedded` and `stateset-core`.

## Binding Topology

The binding story is more direct than the top-level marketing language implies.

- `bindings/node` links directly to `stateset-embedded`, `stateset-core`, `stateset-db`, and `stateset-crypto`.
- `bindings/python` links directly to `stateset-embedded`, `stateset-core`, `stateset-primitives`, `stateset-db`, and `stateset-sdk`.
- Go, Swift, Kotlin, Java, and .NET also link directly to `stateset-embedded` and `stateset-core`.
- Ruby and PHP are present in the repo but excluded from default workspace membership because they require host runtimes or headers.

The practical takeaway is that API changes in `stateset-core` and `stateset-embedded` ripple outward quickly across the binding layer.

## Operator Surfaces

The Node-based runtime is a separate product surface on top of the Rust engine.

- `cli/` depends on `@stateset/embedded` directly.
- The MCP server is assembled centrally in `cli/src/mcp-server.js`.
- The tool surface is split across many `cli/src/tools/*.js` modules.
- The admin app depends on the local Node binding package and loads `@stateset/embedded` at runtime.

This means the Rust engine and the Node operator runtime are coupled at the binding boundary, not at a generic HTTP boundary.

## Where Changes Carry The Most Risk

If you want a fast mental model for blast radius:

- Highest conceptual stability: `stateset-primitives`, `stateset-pricing`, `stateset-observability`, `stateset-policy`, `stateset-authz`
- Highest downstream blast radius: `stateset-core`, `stateset-embedded`
- Highest storage and feature-coordination cost: `stateset-db`
- Highest integration and ecosystem churn: `bindings/node`, `cli/`, `admin/`

## Recommended Onboarding Order

Read the codebase in this order:

1. `stateset-core`
2. `stateset-db`
3. `stateset-embedded`
4. `stateset-sync`, `stateset-policy`, `stateset-authz`, `stateset-pricing`
5. `stateset-http`
6. `bindings/node`
7. `admin/`
8. `cli/`

That order follows the actual dependency direction and keeps the largest operator-facing surfaces for last.
