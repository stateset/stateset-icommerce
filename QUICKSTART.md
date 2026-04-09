# 10-Minute Quickstart

Get from zero to a running commerce operation with orders, payments, inventory, policy checks, and event sourcing.

## Rust

### 1. Add the dependency

```bash
cargo add stateset-sdk --features full
```

### 2. Create a commerce engine and start selling

```rust
use stateset_sdk::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an embedded commerce engine (SQLite, zero config)
    let commerce = Commerce::new("store.db")?;

    // Create a customer
    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    })?;
    println!("Customer: {} {}", customer.first_name, customer.email);

    // Create a product
    let product = commerce.products().create(CreateProduct {
        name: "Rust Programming Book".into(),
        description: Some("The definitive guide".into()),
        ..Default::default()
    })?;
    println!("Product: {} ({})", product.name, product.id);

    // Add inventory
    let _item = commerce.inventory().create_item(CreateInventoryItem {
        sku: "RUST-BOOK-001".into(),
        name: "Rust Programming Book".into(),
        ..Default::default()
    })?;
    commerce.inventory().adjust(
        "RUST-BOOK-001",
        rust_decimal_macros::dec!(100),
        "Initial stock",
    )?;
    println!("Stock: 100 units of RUST-BOOK-001");

    // Place an order
    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            product_id: product.id,
            sku: "RUST-BOOK-001".into(),
            name: "Rust Programming Book".into(),
            quantity: 2,
            unit_price: rust_decimal_macros::dec!(49.99),
            ..Default::default()
        }],
        ..Default::default()
    })?;
    println!("Order: {} — ${}", order.order_number, order.total_amount);

    // Process payment
    let payment = commerce.payments().create(CreatePayment {
        order_id: Some(order.id),
        customer_id: Some(customer.id),
        amount: order.total_amount,
        ..Default::default()
    })?;
    commerce.payments().mark_completed(payment.id)?;
    println!("Payment: {} — completed", payment.id);

    // Ship the order
    let shipment = commerce.shipments().create(CreateShipment {
        order_id: order.id,
        tracking_number: Some("1Z999AA10123456784".into()),
        recipient_name: format!("{} {}", customer.first_name, customer.last_name),
        ..Default::default()
    })?;
    println!("Shipment: {} — {}", shipment.shipment_number, shipment.tracking_number.unwrap_or_default());

    // Check metrics
    let metrics = commerce.metrics_snapshot();
    println!("\n--- Metrics ---");
    println!("Orders created: {}", metrics.orders_created);
    println!("Payments completed: {}", metrics.payments_completed);

    Ok(())
}
```

### 3. Run it

```bash
cargo run
```

Output:
```
Customer: Alice alice@example.com
Product: Rust Programming Book (550e8400-...)
Stock: 100 units of RUST-BOOK-001
Order: ORD-1711539600-000001-00000001 — $99.98
Payment: 7a3b4c5d-... — completed
Shipment: SHP-... — 1Z999AA10123456784

--- Metrics ---
Orders created: 1
Payments completed: 1
```

**That's it.** No database setup, no migrations to run, no config files. The engine creates the SQLite database, runs 9 migrations, and is ready for commerce.

---

## Node.js / CLI

### 1. Install

```bash
npm install -g @stateset/cli
```

### 2. Initialize with demo data

```bash
stateset init --demo
```

### 3. Start selling

```bash
# List customers
stateset "show me all customers"

# Create an order
stateset --apply "create an order for alice@example.com with 2 widgets at $29.99"

# Check inventory
stateset "what products are low on stock?"

# Process a return
stateset --apply "create a return for order ORD-123 reason: defective"

# Analytics
stateset "what is my revenue this month?"
stateset "who are my top customers?"
```

### 4. Start the HTTP API

```bash
stateset serve --port 3000
```

Now you have a full REST API at `http://localhost:3000/api/v1/` with OpenAPI docs at `/api/v1/docs`.

---

## Python

```bash
pip install stateset-embedded
```

```python
from stateset_embedded import Commerce

commerce = Commerce(":memory:")

# Create a customer
customer = commerce.customers().create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith"
)

# Create an order
order = commerce.orders().create(
    customer_id=customer.id,
    items=[{
        "sku": "WIDGET-001",
        "name": "Widget",
        "quantity": 3,
        "unit_price": "29.99"
    }]
)
print(f"Order {order.order_number}: ${order.total_amount}")
```

---

## With Policy Checks

```rust
use stateset_sdk::prelude::*;
use stateset_sdk::policy;

// Load a policy from YAML
let engine = policy::PolicyEngine::from_yaml(r#"
  rules:
    - name: "max_order_value"
      condition: "order.total > 10000"
      effect: deny
      message: "Orders over $10,000 require manager approval"
    - name: "blocked_countries"
      condition: "customer.country in ['XX', 'YY']"
      effect: deny
      message: "Shipping to this country is not available"
"#)?;

// Check before processing
let context = policy::Context::new()
    .set("order.total", 15000)
    .set("customer.country", "US");

match engine.evaluate(&context) {
    policy::Decision::Allow => println!("Order approved"),
    policy::Decision::Deny(reasons) => {
        for reason in reasons {
            println!("Blocked: {}", reason.message);
        }
    }
}
```

---

## With Event Sourcing

```rust
use stateset_sdk::prelude::*;

let commerce = Commerce::new("store.db")?;

// Subscribe to commerce events
let mut subscription = commerce.subscribe();

// In another thread/task, process events
tokio::spawn(async move {
    while let Some(event) = subscription.recv().await {
        match event {
            CommerceEvent::OrderCreated { order_id, total_amount, .. } => {
                println!("New order: {} for ${}", order_id, total_amount);
            }
            CommerceEvent::PaymentCompleted { payment_id, amount, .. } => {
                println!("Payment received: {} for ${}", payment_id, amount);
            }
            CommerceEvent::InventoryAdjusted { sku, quantity_change, .. } => {
                println!("Stock changed: {} by {}", sku, quantity_change);
            }
            _ => {}
        }
    }
});
```

---

## With Agent-to-Agent Commerce

```rust
use stateset_sdk::prelude::*;

let commerce = Commerce::new("agent-store.db")?;

// Register this agent for commerce
commerce.agent_cards().register(RegisterAgent {
    name: "Widget Supplier Bot".into(),
    wallet: "0x1234...5678".into(),
    skills: vec!["sell", "quote", "fulfill"],
    networks: vec!["set_chain"],
    assets: vec!["USDC"],
    ..Default::default()
})?;

// Discover buyer agents
let buyers = commerce.agent_cards().discover(DiscoverAgents {
    skill: Some("buy"),
    min_trust_tier: Some("standard"),
    ..Default::default()
})?;

// Negotiate a price autonomously
// (See /api/v1/negotiations endpoints for REST-based negotiation)
```

---

## Architecture

```
Your App / AI Agent / CLI
        │
        ▼
┌─────────────────────────┐
│   stateset-sdk           │  ← cargo add stateset-sdk
│   (unified facade)       │
├─────────────────────────┤
│   Commerce API           │  Orders, Customers, Products,
│   (stateset-embedded)    │  Inventory, Payments, Returns,
│                          │  Reviews, Wishlists, Gift Cards,
│                          │  Loyalty, Fraud, Segments...
├─────────────────────────┤
│   Pricing Engine         │  Tax, promotions, rounding
│   Policy Engine          │  YAML rules, deny-overrides
│   A2A Commerce           │  Negotiation, messaging, credit
│   VES Crypto             │  Ed25519, Merkle, AES-GCM
├─────────────────────────┤
│   SQLite (embedded)      │  Zero config, 9 migrations
│   PostgreSQL (optional)  │  Async, high-concurrency
└─────────────────────────┘
```

---

## What's Included

- **Full REST API** with OpenAPI 3.1 docs
- **Rust plus native bindings** for Node.js, Python, Go, Java, Kotlin, Swift, .NET, Ruby, PHP, and WASM
- **9 database migrations** with rollback support
- **A2A commerce modules** for autonomous agent workflows
- **Repo-wide Rust and JS verification** in the root `npm run check` pipeline
- **~3x performance** vs naive implementation (fat LTO, native CPU, lock-free atomics)
- **Generated inventories** for current workspace topology and MCP/tool surfaces:
  [Workspace Inventory](./docs/src/appendix/workspace-inventory.md) and
  [MCP Tool Inventory](./docs/src/appendix/mcp-tool-inventory.md)

## Next Steps

- [API Reference](https://docs.rs/stateset-sdk) — Full Rust API documentation
- [OpenAPI Spec](http://localhost:3000/api/v1/openapi.json) — REST endpoint reference
- [CHANGELOG](./CHANGELOG.md) — Release history
- [GitHub](https://github.com/stateset/stateset-icommerce) — Source code and issues
