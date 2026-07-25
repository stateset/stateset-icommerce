# stateset-authz

[![crates.io](https://img.shields.io/crates/v/stateset-authz.svg)](https://crates.io/crates/stateset-authz)
[![docs.rs](https://docs.rs/stateset-authz/badge.svg)](https://docs.rs/stateset-authz)

An IO-free, framework-agnostic authorization engine: roles and permissions, rate
limiting, audit logging, and sensitive-field redaction.

Zero IO is the point. No filesystem, no network, no database — you bring the
persistence, so the same engine runs in a CLI, an axum server, a WASM module, or a
Node NAPI binding and reaches the same decision in all four.

## Features

- **Permission levels** — `None` < `Read` < `Preview` < `Write` < `Delete` < `Admin`
- **Role-based access control** — built-in `admin`, `operator`, `viewer`, `none` roles,
  plus a `RoleBuilder` for custom roles
- **Window-based rate limiting** — per-actor, per-resource request tracking
- **Audit logging** — in-memory ring buffer, filterable by actor, resource, and time
- **Sensitive field redaction** — recursive JSON traversal with partial string masking
- **Access decisions** — `Allowed`, `Denied`, `RequiresApproval`, each with reasons
- **Zero IO** — bring your own persistence

## Usage

```rust
use stateset_authz::{AuthzEngineBuilder, Role, Action, Resource, RateLimitRule};
use std::time::Duration;

let mut engine = AuthzEngineBuilder::new()
    .add_role(Role::admin())
    .add_role(Role::viewer())
    .assign_role("alice", "admin")
    .assign_role("bob", "viewer")
    .rate_limit_rule(RateLimitRule::new("orders", 100, Duration::from_secs(60)))
    .build();

// Alice (admin) can create orders
let decision = engine.authorize("alice", &Resource::new("orders"), &Action::Create);
assert!(decision.is_allowed());

// Bob (viewer) cannot
let decision = engine.authorize("bob", &Resource::new("orders"), &Action::Create);
assert!(decision.is_denied());

// Every decision is recorded in the audit log
assert_eq!(engine.audit_log().len(), 2);
```

Decisions carry a third state that matters for agent workflows — `RequiresApproval`
lets you gate an action on a human without modeling it as a denial.

## Why Redaction Lives Here

An authorization engine already knows which fields an actor may not see, so redaction
belongs next to the decision rather than in each caller. `redaction` walks JSON
recursively and masks partially (`4111********1111`), so audit records stay useful
without leaking card numbers or secrets into logs.

## Part of StateSet iCommerce

A Rust port of the permission model in the StateSet CLI, so a decision made in the
CLI and one made in the Rust engine agree. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine, consumed
by [`stateset-http`](https://crates.io/crates/stateset-http).

## License

MIT OR Apache-2.0
