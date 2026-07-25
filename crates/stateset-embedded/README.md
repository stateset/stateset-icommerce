# stateset-embedded

[![crates.io](https://img.shields.io/crates/v/stateset-embedded.svg)](https://crates.io/crates/stateset-embedded)
[![docs.rs](https://docs.rs/stateset-embedded/badge.svg)](https://docs.rs/stateset-embedded)

The SQLite of commerce operations — an embeddable commerce engine that runs anywhere,
with no server to operate and no network hop between your code and your orders.

`stateset-embedded` is the batteries-included API that combines
[`stateset-core`](https://crates.io/crates/stateset-core) (domain models),
[`stateset-db`](https://crates.io/crates/stateset-db) (storage),
[`stateset-pricing`](https://crates.io/crates/stateset-pricing) (totals), and
[`stateset-observability`](https://crates.io/crates/stateset-observability)
(metrics/tracing) into one library. Point it at a file and you have a commerce
backend.

## Usage

```rust,no_run
use stateset_embedded::{Commerce, CreateCustomer, CreateOrder, CreateOrderItem, CreateInventoryItem};
use rust_decimal_macros::dec;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// Opens the database, creating and migrating it if needed
let commerce = Commerce::new("./store.db")?;

let customer = commerce.customers().create(CreateCustomer {
    email: "alice@example.com".into(),
    first_name: "Alice".into(),
    last_name: "Smith".into(),
    ..Default::default()
})?;

commerce.inventory().create_item(CreateInventoryItem {
    sku: "SKU-001".into(),
    name: "Widget".into(),
    initial_quantity: Some(dec!(100)),
    ..Default::default()
})?;

let order = commerce.orders().create(CreateOrder {
    customer_id: customer.id,
    items: vec![CreateOrderItem {
        sku: "SKU-001".into(),
        name: "Widget".into(),
        quantity: 2,
        unit_price: dec!(29.99),
        ..Default::default()
    }],
    ..Default::default()
})?;

commerce.inventory().adjust("SKU-001", dec!(-2), "Order fulfillment")?;
# let _ = order;
# Ok(())
# }
```

Use `":memory:"` instead of a path for an ephemeral store — handy in tests.

## What You Get

- **A wide domain surface** reachable from one `Commerce` handle — orders, customers,
  products, inventory, returns, payments, shipments, plus the back office
  (purchasing, warehouse, manufacturing, quality, finance, traceability)
- **Money as decimals, never floats** — `rust_decimal` throughout
- **Migrations on open** — the schema is created and upgraded for you
- **Sync and async** — `Commerce` for blocking callers, `AsyncCommerce` for async
- **Backup and restore** — consistent `VACUUM INTO` snapshots with a checksummed
  manifest, plus portable JSON export/import
- **Events** — outbound webhooks with HMAC signing behind the `events` feature

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `sqlite` | SQLite storage via bundled rusqlite | Yes |
| `events` | Outbound webhooks with HMAC signing (implies `async`) | Yes |
| `postgres` | Async PostgreSQL storage via sqlx (implies `async`) | No |
| `sqlite-events` | SQLite-backed event queue | No |
| `vector` | Vector search over embeddings | No |
| `otel` | OpenTelemetry export via `stateset-observability` | No |
| `async` | Tokio + futures; usually pulled in by `events` or `postgres` | No |

## Also Available In

Node.js (`@stateset/embedded`), Python (`stateset-embedded`), Ruby
(`stateset_embedded`), Go, Java, Kotlin, Swift, C#/.NET, PHP, and WASM — all over the
same engine.

Prefer a single Rust dependency? Use
[`stateset-sdk`](https://crates.io/crates/stateset-sdk). Need HTTP? See
[`stateset-http`](https://crates.io/crates/stateset-http).

## License

MIT OR Apache-2.0
