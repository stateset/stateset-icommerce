# stateset-primitives

[![crates.io](https://img.shields.io/crates/v/stateset-primitives.svg)](https://crates.io/crates/stateset-primitives)
[![docs.rs](https://docs.rs/stateset-primitives/badge.svg)](https://docs.rs/stateset-primitives)

Strongly-typed primitive types for commerce applications in Rust. Prevents entire classes of bugs at compile time by making it impossible to confuse an `OrderId` with a `CustomerId`, or to add `Money` in different currencies.

## Features

- **Newtype IDs**: `OrderId`, `CustomerId`, `ProductId`, `ShipmentId`, `PaymentId`, `InventoryItemId`, `SubscriptionId`, `CartId`, and 9 more — all `Copy + Eq + Hash + Serialize + Display`
- **Money**: Amount + currency pair that prevents arithmetic across currencies
- **CurrencyCode**: Three-letter uppercase currency codes with zero-allocation storage (USD, EUR, GBP, ...)
- **Sku**: Validated product SKU — trimmed, non-empty, at most 128 characters
- **Zero unsafe code**, `#[deny(unsafe_code)]`

## Usage

```rust
use stateset_primitives::{OrderId, CustomerId, Money, CurrencyCode, Sku};
use rust_decimal_macros::dec;
use uuid::Uuid;

// Type-safe IDs — can't accidentally swap them
let order_id: OrderId = Uuid::new_v4().into();
let customer_id: CustomerId = Uuid::new_v4().into();

// Money is currency-aware
let price = Money::new(dec!(29.99), CurrencyCode::USD);

// SKUs are validated and trimmed on construction
let sku = Sku::new("WBH-001").unwrap();
assert_eq!(sku.as_str(), "WBH-001");
assert!(Sku::new("   ").is_err());
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `std` | Standard library support (the crate is `no_std`-capable without it) | Yes |
| `arbitrary` | `proptest::Arbitrary` impls for property-based testing | No |
| `sqlx-postgres` | `sqlx` `Type`/`Encode`/`Decode` impls, for PostgreSQL backends | No |
| `rusqlite` | `rusqlite` conversions, for SQLite backends | No |

## Part of StateSet iCommerce

The foundation every other crate builds on —
[`stateset-core`](https://crates.io/crates/stateset-core) models,
[`stateset-db`](https://crates.io/crates/stateset-db) storage, and
[`stateset-pricing`](https://crates.io/crates/stateset-pricing) calculations all speak
these types. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
