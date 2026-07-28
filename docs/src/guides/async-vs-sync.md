# Async vs Sync API Guide

StateSet Embedded provides two API patterns for Rust applications: synchronous (`Commerce`) and asynchronous (`AsyncCommerce`). This guide helps you choose the right approach.

## Quick Decision Tree

```
Do you need PostgreSQL?
├── Yes → Use AsyncCommerce (full async support)
└── No (SQLite) → Use Commerce (sync, simpler)

Is your application async (tokio/async-std)?
├── Yes + PostgreSQL → Use AsyncCommerce
├── Yes + SQLite → Use Commerce (SQLite is blocking)
└── No → Use Commerce
```

## The Two APIs

### Commerce (Synchronous)

The default API that works with both SQLite and PostgreSQL.

```rust
use stateset_embedded::Commerce;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SQLite (default, recommended for most use cases)
    let commerce = Commerce::new("commerce.db")?;

    // Or PostgreSQL (wrapped in blocking calls)
    let commerce = Commerce::with_postgres("postgres://localhost/stateset")?;

    // Synchronous operations
    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    })?;

    let orders = commerce.orders().list()?;

    Ok(())
}
```

**Use Commerce when:**
- Using SQLite (embedded database)
- Building CLI tools or agents
- Application doesn't use async runtime
- Simplicity is preferred over maximum concurrency

### AsyncCommerce (Asynchronous)

Available only with the `postgres` feature. Provides true async operations.

```rust
use stateset_embedded::AsyncCommerce;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // PostgreSQL with async support
    let commerce = AsyncCommerce::connect("postgres://localhost/stateset").await?;

    // Async operations
    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    }).await?;

    let orders = commerce.orders().list().await?;

    Ok(())
}
```

**Use AsyncCommerce when:**
- Using PostgreSQL for production scale
- Building web servers (actix, axum, etc.)
- Need concurrent request handling
- Application already uses tokio runtime

## Feature Flags

```toml
# Cargo.toml

[dependencies]
# SQLite only (default)
stateset-embedded = "1.23.4"

# PostgreSQL support (enables AsyncCommerce)
stateset-embedded = { version = "1.23.4", features = ["postgres"] }

# Both SQLite and PostgreSQL
stateset-embedded = { version = "1.23.4", features = ["sqlite", "postgres"] }
```

## Database Backend Comparison

| Feature | SQLite | PostgreSQL |
|---------|--------|------------|
| Setup | Zero config | Requires server |
| Deployment | Single file | External service |
| Sync API | Native (fast) | Wrapped (blocking) |
| Async API | Not available | Native (fast) |
| Concurrency | Single writer | Multiple writers |
| Portability | Excellent | Server-dependent |
| Best for | Agents, CLI, embedded | Web servers, scaling |

## Important: SQLite is Always Blocking

Even if your application uses an async runtime, SQLite operations are blocking:

```rust
// This still blocks the current thread!
let commerce = Commerce::new("commerce.db")?;

// For async apps with SQLite, use spawn_blocking:
let customer = tokio::task::spawn_blocking(move || {
    commerce.customers().get(&customer_id)
}).await??;
```

**Why?** SQLite uses file-level locking and doesn't have native async support. The `rusqlite` driver is synchronous by design.

## PostgreSQL with Commerce (Sync Wrapper)

When you use `Commerce::with_postgres()`, each operation:
1. Creates a tokio runtime (if needed)
2. Calls `block_on()` to run the async operation
3. Blocks until completion

```rust
// Under the hood:
pub fn create(&self, data: CreateCustomer) -> Result<Customer> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        self.async_impl.create(data).await
    })
}
```

This adds overhead. For high-throughput PostgreSQL usage, prefer `AsyncCommerce`.

## Migration Path

### SQLite to PostgreSQL

```rust
// Before: SQLite
let commerce = Commerce::new("commerce.db")?;

// After: PostgreSQL (sync wrapper)
let commerce = Commerce::with_postgres("postgres://localhost/stateset")?;

// No code changes needed for business logic!
let customer = commerce.customers().create(...)?;
```

### Sync to Async

```rust
// Before: Sync
fn process_order(commerce: &Commerce, id: &str) -> Result<Order> {
    let order = commerce.orders().get(id)?;
    commerce.orders().ship(id)
}

// After: Async
async fn process_order(commerce: &AsyncCommerce, id: &str) -> Result<Order> {
    let order = commerce.orders().get(id).await?;
    commerce.orders().ship(id).await
}
```

## Performance Characteristics

### SQLite + Commerce
- **Latency**: <1ms for simple operations
- **Throughput**: ~10,000 ops/sec (single writer)
- **Concurrency**: Readers can run in parallel, writes serialize

### PostgreSQL + Commerce (sync wrapper)
- **Latency**: Runtime creation overhead per call
- **Throughput**: Lower than async due to blocking
- **Concurrency**: Limited by runtime creation

### PostgreSQL + AsyncCommerce
- **Latency**: Network RTT + query time
- **Throughput**: High with connection pooling
- **Concurrency**: Excellent with async/await

## Connection Pooling

### SQLite
Connection pooling is handled automatically via `r2d2`:

```rust
// Internal configuration (defaults)
let pool = r2d2::Pool::builder()
    .max_size(10)
    .build(manager)?;
```

### PostgreSQL (AsyncCommerce)
Uses `sqlx` connection pool:

```rust
// Pool is configured via connection string
let commerce = AsyncCommerce::connect(
    "postgres://user:pass@localhost/db?max_connections=20"
).await?;
```

## Recommendations

| Scenario | Recommendation |
|----------|----------------|
| AI Agent / CLI tool | `Commerce` + SQLite |
| Desktop application | `Commerce` + SQLite |
| IoT / Edge device | `Commerce` + SQLite |
| Web API server | `AsyncCommerce` + PostgreSQL |
| High-concurrency service | `AsyncCommerce` + PostgreSQL |
| Microservice | `AsyncCommerce` + PostgreSQL |
| Testing | `Commerce` + SQLite (`:memory:`) |
| Development | `Commerce` + SQLite |

## Example: Web Server with Async

```rust
use axum::{Router, Json, extract::State};
use stateset_embedded::AsyncCommerce;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let commerce = AsyncCommerce::connect("postgres://localhost/stateset")
        .await
        .expect("Failed to connect");

    let app = Router::new()
        .route("/orders", get(list_orders))
        .with_state(Arc::new(commerce));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn list_orders(
    State(commerce): State<Arc<AsyncCommerce>>,
) -> Json<Vec<Order>> {
    let orders = commerce.orders().list().await.unwrap();
    Json(orders)
}
```

## Example: CLI Tool with Sync

```rust
use stateset_embedded::Commerce;
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = "commerce.db")]
    database: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let commerce = Commerce::new(&args.database)?;

    // Simple, blocking operations
    let customers = commerce.customers().list()?;

    for customer in customers {
        println!("{}: {}", customer.id, customer.email);
    }

    Ok(())
}
```

## Summary

- **SQLite users**: Use `Commerce` (sync). It's fast and simple.
- **PostgreSQL users**: Use `AsyncCommerce` in async apps, `Commerce` for scripts.
- **Migrating**: The API surface is identical, just add `.await` for async.
- **Testing**: SQLite `:memory:` is fast and isolated.
