# stateset-embedded

The SQLite of Commerce — an embeddable, zero-dependency commerce engine for Rust applications and AI agents.

`stateset-embedded` is the high-level API that combines `stateset-core` (domain models), `stateset-db` (storage), and `stateset-observability` (metrics/tracing) into a single, batteries-included library.

## Usage

```rust
use stateset_embedded::Commerce;

let commerce = Commerce::new("./store.db");

// Create a customer
let customer = commerce.customers.create(CreateCustomerInput {
    email: "alice@example.com".into(),
    first_name: "Alice".into(),
    last_name: "Johnson".into(),
    ..Default::default()
});

// Create an order
let order = commerce.orders.create(CreateOrderInput {
    customer_id: customer.id,
    items: vec![OrderItemInput {
        sku: "WBH-001".into(),
        name: "Wireless Headphones".into(),
        quantity: 1,
        unit_price: 79.99,
    }],
    ..Default::default()
});
```

## Available via 11 Language Bindings

- Node.js (`@stateset/embedded`), Python (`stateset-embedded`), Ruby (`stateset_embedded`)
- Go, Java, Kotlin, Swift, C#/.NET, PHP, WASM

## License

MIT OR Apache-2.0
