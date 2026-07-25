# stateset-core

[![crates.io](https://img.shields.io/crates/v/stateset-core.svg)](https://crates.io/crates/stateset-core)
[![docs.rs](https://docs.rs/stateset-core/badge.svg)](https://docs.rs/stateset-core)

Pure domain models and business logic for commerce applications. No I/O, no runtime —
just the type system, validation, events, and repository traits you need to build a
commerce backend.

Because there's no I/O in here, the domain is testable without a database and
portable to any storage you like: implement the repository traits and the models,
state machines, and validation come along unchanged.

Part of the [StateSet iCommerce](https://github.com/stateset/stateset-icommerce)
engine.

## What's Inside

- **35+ domain modules**: orders, payments, inventory, customers, products, carts,
  returns, subscriptions, shipments, manufacturing, promotions, tax, analytics, and more
- **State machines**: `OrderStatus`, `PaymentTransactionStatus`, `SubscriptionStatus`
  with exhaustive match enforcement
- **Event sourcing**: `CommerceEvent` with 30+ variants for full audit trails
- **Validation framework**: composable `ValidationBuilder` with validators for email,
  SKU, phone, postal code, price, quantity
- **Repository traits**: generic `Repository`, auto-implemented for `&T`, `Box<T>`, `Arc<T>`
- **Error taxonomy**: `CommerceError` with `is_not_found()`, `is_retryable()`,
  `suggested_status_code()`, and domain-specific sub-errors
- **Agent-to-agent commerce**: A2A quotes, purchases, subscriptions, split payments, escrow
- **Crypto primitives**: x402 payment intents, ERC-8004 identity, Merkle proofs

## Usage

Errors are categorized, so callers branch on meaning rather than on strings:

```rust
use stateset_core::CommerceError;

fn handle_error(err: &CommerceError) {
    if err.is_not_found() {
        // 404
    } else if err.is_validation() {
        // 400
    } else if err.is_conflict() {
        // 409
    } else if err.is_retryable() {
        // back off and retry
    }
}
```

Validation composes and returns a single `Result` carrying every failure:

```rust
use stateset_core::{ValidationBuilder, Result};

fn validate_order(email: &str, quantity: i32) -> Result<()> {
    ValidationBuilder::new()
        .email("email", email)
        .positive_i32("quantity", quantity)
        .build()
}

assert!(validate_order("alice@example.com", 2).is_ok());
assert!(validate_order("not-an-email", 2).is_err());
assert!(validate_order("alice@example.com", -1).is_err());
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `embeddings` | Vector embedding support (adds `reqwest`) | No |
| `metrics` | Prometheus metrics (adds `prometheus`, `once_cell`) | No |
| `test-utils` | Expose test helpers for downstream integration tests | No |
| `sqlx-postgres` | `sqlx` type impls for primitives, for PostgreSQL backends | No |
| `rusqlite` | `rusqlite` type impls for primitives, for SQLite backends | No |

## Part of StateSet iCommerce

Built on [`stateset-primitives`](https://crates.io/crates/stateset-primitives) and
implemented against by [`stateset-db`](https://crates.io/crates/stateset-db). Most
applications consume it via
[`stateset-embedded`](https://crates.io/crates/stateset-embedded) or
[`stateset-sdk`](https://crates.io/crates/stateset-sdk).

## License

MIT OR Apache-2.0
