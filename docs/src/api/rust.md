# Rust API Reference

The Rust API is defined in the `stateset-embedded` crate and exposes the unified `Commerce` entry point.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
stateset-embedded = "1.23.1"

# For PostgreSQL support
stateset-embedded = { version = "1.23.1", features = ["postgres"] }
```

## Quick Start

```rust
use stateset_embedded::{Commerce, CreateCustomer, CreateOrder, OrderItem};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize with SQLite (default)
    let commerce = Commerce::new("commerce.db")?;

    // Create a customer
    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    })?;

    // Create an order
    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id.clone(),
        items: vec![OrderItem {
            sku: "WIDGET-001".into(),
            name: "Premium Widget".into(),
            quantity: 2,
            unit_price: rust_decimal::Decimal::new(2999, 2),
            ..Default::default()
        }],
        ..Default::default()
    })?;

    // Ship the order
    let shipped = commerce.orders().ship(&order.id)?;
    println!("Order {} shipped!", shipped.order_number);

    Ok(())
}
```

## Common Operations

### Customer Management

```rust
// Create customer
let customer = commerce.customers().create(CreateCustomer {
    email: "test@example.com".into(),
    first_name: "Test".into(),
    last_name: "User".into(),
    phone: Some("+1-555-0123".into()),
    ..Default::default()
})?;

// Get customer by ID
let customer = commerce.customers().get(&customer_id)?;

// List all customers
let customers = commerce.customers().list()?;

// Delete customer
commerce.customers().delete(&customer_id)?;
```

### Inventory Management

```rust
// Create inventory item
let item = commerce.inventory().create_item(CreateInventoryItem {
    sku: "SKU-001".into(),
    name: "Widget".into(),
    initial_quantity: Some(100),
    ..Default::default()
})?;

// Adjust inventory
commerce.inventory().adjust("SKU-001", 50, "Received shipment")?;

// Reserve inventory
let reservation = commerce.inventory().reserve("SKU-001", 10, None)?;

// Release reservation
commerce.inventory().release(&reservation.id)?;

// Get stock level
let level = commerce.inventory().get_level("SKU-001")?;
```

### Order Processing

```rust
// Create order
let order = commerce.orders().create(CreateOrder {
    customer_id: customer.id,
    items: vec![OrderItem { /* ... */ }],
    currency: Some("USD".into()),
    ..Default::default()
})?;

// Update status
commerce.orders().update_status(&order.id, "processing")?;

// Ship order
let shipped = commerce.orders().ship(&order.id)?;

// Cancel order
let cancelled = commerce.orders().cancel(&order.id)?;

// List orders by status
let pending = commerce.orders().list_by_status("pending")?;
```

### Analytics

```rust
// Get sales summary
let summary = commerce.analytics().sales_summary()?;
println!("Total revenue: {}", summary.total_revenue);

// Get top products
let top_products = commerce.analytics().top_products(10)?;

// Get top customers
let top_customers = commerce.analytics().top_customers(10)?;
```

## Error Handling

```rust
use stateset_embedded::{Commerce, CommerceError};

fn process_order(commerce: &Commerce, order_id: &str) -> Result<(), CommerceError> {
    match commerce.orders().ship(order_id) {
        Ok(order) => {
            println!("Order shipped: {}", order.order_number);
            Ok(())
        }
        Err(CommerceError::NotFound(msg)) => {
            eprintln!("Order not found: {}", msg);
            Err(CommerceError::NotFound(msg))
        }
        Err(CommerceError::InvalidState(msg)) => {
            eprintln!("Cannot ship order: {}", msg);
            Err(CommerceError::InvalidState(msg))
        }
        Err(e) => Err(e),
    }
}
```

## Async API (PostgreSQL)

```rust
use stateset_embedded::AsyncCommerce;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let commerce = AsyncCommerce::connect("postgres://localhost/stateset").await?;

    let customer = commerce.customers().create(CreateCustomer {
        email: "async@example.com".into(),
        first_name: "Async".into(),
        last_name: "User".into(),
        ..Default::default()
    }).await?;

    Ok(())
}
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers()` | Customer management |
| `products()` | Product catalog |
| `orders()` | Order lifecycle |
| `inventory()` | Stock management |
| `carts()` | Shopping carts |
| `returns()` | Return processing |
| `payments()` | Payment operations |
| `shipments()` | Shipping management |
| `warranties()` | Warranty tracking |
| `suppliers()` | Supplier management |
| `purchase_orders()` | Purchase orders |
| `invoices()` | B2B invoicing |
| `bom()` | Bills of Materials |
| `work_orders()` | Manufacturing |
| `currency()` | Multi-currency |
| `subscriptions()` | Recurring billing |
| `promotions()` | Discounts & coupons |
| `tax()` | Tax calculations |
| `quality()` | Quality control |
| `lots()` | Lot tracking |
| `serials()` | Serial numbers |
| `warehouse()` | Warehouse ops |
| `receiving()` | Receiving |
| `fulfillment()` | Picking & packing |
| `accounts_payable()` | A/P management |
| `accounts_receivable()` | A/R management |
| `cost_accounting()` | Cost tracking |
| `credit()` | Credit management |
| `backorders()` | Backorder tracking |
| `general_ledger()` | GL accounting |
| `analytics()` | Reporting & forecasts |

## Source Files

- Entry point: `Commerce`, `AsyncCommerce`
- API crate: `crates/stateset-embedded/`
- Shared domain models: `crates/stateset-core/`
- Key files:
  - `crates/stateset-embedded/src/lib.rs`
  - `crates/stateset-core/src/`

## Examples

- `examples/basic_usage.rs`
- `examples/error_handling.rs`
- `examples/events_webhooks.rs`
