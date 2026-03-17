# stateset-primitives

Strongly-typed primitive types for commerce applications in Rust. Prevents entire classes of bugs at compile time by making it impossible to confuse an `OrderId` with a `CustomerId`, or to add `Money` in different currencies.

## Features

- **Newtype IDs**: `OrderId`, `CustomerId`, `ProductId`, `ShipmentId`, `PaymentId`, `InventoryItemId`, `SubscriptionId`, `CartId`, and 9 more — all `Copy + Eq + Hash + Serialize + Display`
- **Money**: Amount + currency pair that prevents arithmetic across currencies
- **CurrencyCode**: Three-letter uppercase currency codes with zero-allocation storage (USD, EUR, GBP, ...)
- **Sku**: Validated product SKU (alphanumeric + hyphen/underscore)
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

// SKUs are validated
let sku: Sku = "WBH-001".parse().unwrap();
```

## Feature Flags

- `std` (default) — Standard library support
- `arbitrary` — Enable `proptest::Arbitrary` for property-based testing

## License

MIT OR Apache-2.0
