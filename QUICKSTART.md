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
stateset-init --demo
```

(`stateset-init --quickstart` does a zero-prompt standalone setup; `--db <path>` picks a custom database location, `--force` overwrites an existing one.)

### 3. Start selling

The `stateset` command is a natural-language agent over the embedded engine.
Reads run freely; writes are previewed unless you pass `--apply`:

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

Domain-specific CLIs are installed alongside it: `stateset-orders`,
`stateset-inventory`, `stateset-checkout`, `stateset-payments`,
`stateset-returns`, `stateset-analytics`, and more. For the MCP server
(Claude Desktop / Cursor / Windsurf), see the
[MCP Server section in the README](./README.md#mcp-server-claude-desktop--cursor--windsurf).

### 4. Serve the REST API

The REST API is an embeddable layer (`stateset-http`), started from your Rust
application:

```rust
use stateset_embedded::Commerce;
use stateset_http::ServerBuilder;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let commerce = Commerce::new("store.db")?;
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;

    ServerBuilder::new_from_env(commerce)?
        .bind(addr)
        .with_cors()
        .with_request_id()
        .with_bearer_auth("replace-me-with-a-secret")
        .serve()
        .await?;
    Ok(())
}
```

Now you have a full REST API at `http://localhost:3000/api/v1/` with the
OpenAPI spec at `/api/v1/openapi.json`. (If you skip `with_bearer_auth`, the
server generates a token and prints it at startup — auth is on by default.)

---

## Python

```bash
pip install stateset-embedded
```

```python
from stateset_embedded import Commerce, CreateOrderItemInput

commerce = Commerce(":memory:")

# Create a customer (APIs are properties: commerce.customers, not commerce.customers())
customer = commerce.customers.create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith",
)

# Create an order
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        CreateOrderItemInput(
            sku="WIDGET-001",
            name="Widget",
            quantity=3,
            unit_price=29.99,
        )
    ],
)
print(f"Order {order.order_number}: ${order.total_amount}")
```

---

## With Policy Checks

Requires the `policy` feature (included in `--features full`).

```rust
use stateset_sdk::policy::{
    Condition, ConditionGroup, ConditionNode, Logic, Operator,
    PolicyAction, PolicyEngine, PolicyRule, PolicySet,
};
use serde_json::json;

let mut engine = PolicyEngine::new();

// Deny orders over $10,000 (deny-overrides precedence, explainable denials)
let rule = PolicyRule::new("high-value-review", "Require review for high-value orders")
    .with_priority(10)
    .with_conditions(ConditionGroup::new(Logic::And, vec![
        ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(10000))),
    ]))
    .with_action(PolicyAction::deny(
        "Order exceeds $10,000 limit",
        "Request manager approval",
    ));

engine.register_policy_set(PolicySet::new("order-limits", "orders").with_rule(rule));

// Check before processing
let context = json!({ "order": { "total": 15000 } });
let result = engine.evaluate("orders", &context);

if result.should_deny {
    for explanation in &result.explanations {
        println!("Blocked: {}", explanation.reason);
    }
}
```

Policies can also be loaded from files: `policy::load_policy_set_from_yaml(...)`
parses a YAML `PolicySet`, and `engine.load_from_dir(...)` loads every
`.yaml`/`.yml`/`.json` policy in a directory.

---

## With Event Sourcing

Requires the `events` feature (on by default in `stateset-embedded`).

```rust
use stateset_sdk::prelude::*;

let commerce = Commerce::new("store.db")?;

// Subscribe to commerce events
let mut subscription = commerce.subscribe_events();

// In another task, process events as they stream in
tokio::spawn(async move {
    while let Some(event) = subscription.next().await {
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

Agent cards live on the x402 API surface (`commerce.x402()`):

```rust
use stateset_sdk::prelude::*;
use stateset_sdk::core::{
    A2ASkill, AgentCardFilter, CreateAgentCard, TrustLevel, X402Asset, X402Network,
};

let commerce = Commerce::new("agent-store.db")?;

// Register this agent for commerce
let card = commerce.x402().register_agent(CreateAgentCard {
    name: "Widget Supplier Bot".into(),
    wallet_address: "0x1234...5678".into(),
    public_key: "base64_ed25519_pubkey".into(),
    supported_networks: Some(vec![X402Network::SetChain]),
    supported_assets: Some(vec![X402Asset::Usdc]),
    a2a_skills: Some(vec![A2ASkill::Sell, A2ASkill::Quote]),
    endpoint_url: Some("https://api.example.com/a2a".into()),
    ..Default::default()
})?;
println!("Registered agent card {}", card.id);

// Discover partner agents that can buy, at standard trust or better
let buyers = commerce.x402().list_agents(AgentCardFilter {
    min_trust_level: Some(TrustLevel::Standard),
    ..Default::default()
})?;

// Quotes, purchases, and payment intents live on the same surface:
// commerce.x402().create_quote(...) / create_intent(...)
// (REST-based negotiation: see the /api/v1/negotiations endpoints)
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
