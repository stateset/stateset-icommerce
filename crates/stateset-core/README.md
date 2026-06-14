# stateset-core

Pure domain models and business logic for commerce applications. No I/O, no runtime dependencies — just the type system, validation, events, and repository traits you need to build commerce backends.

Part of the [StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## What's Inside

- **35+ domain modules**: orders, payments, inventory, customers, products, carts, returns, subscriptions, shipments, manufacturing, promotions, tax, analytics, and more
- **State machines**: `OrderStatus`, `PaymentTransactionStatus`, `SubscriptionStatus` with exhaustive match enforcement
- **Event sourcing**: `CommerceEvent` enum with 30+ variants for full audit trails
- **Validation framework**: Composable `ValidationBuilder` with pre-built validators for email, SKU, phone, postal code, price, quantity
- **Repository traits**: Generic `Repository` with `auto_impl` for `&T`, `Box<T>`, `Arc<T>`
- **Error taxonomy**: `CommerceError` with `is_not_found()`, `is_retryable()`, `suggested_status_code()` and domain-specific sub-errors
- **Agent-to-agent commerce**: A2A quotes, purchases, subscriptions, split payments, escrow
- **Crypto primitives**: x402 payment intents, ERC-8004 identity, Merkle proofs

## Usage

```rust
use stateset_core::models::orders::{Order, OrderStatus};
use stateset_core::models::customers::Customer;
use stateset_core::events::CommerceEvent;
use stateset_core::validation::Validate;

// Domain models with type-safe state transitions
let order = Order::default();
assert_eq!(order.status, OrderStatus::Pending);

// All models implement Validate
let errors = order.validate();

// Events for audit trails
let event = CommerceEvent::OrderCreated {
    order_id: order.id,
    customer_id: order.customer_id,
};
```

## Feature Flags

- `embeddings` — Enable vector embedding support (adds `reqwest`)
- `metrics` — Prometheus metrics (adds `prometheus`, `once_cell`)
- `test-utils` — Expose test helpers for downstream integration tests

## License

MIT OR Apache-2.0
