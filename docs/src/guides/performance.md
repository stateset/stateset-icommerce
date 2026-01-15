# Performance Tuning Guide

This guide covers optimization strategies for StateSet Embedded across different deployment scenarios.

## Connection Pool Tuning

### SQLite (r2d2)

SQLite connection pooling is configured internally. For write-heavy workloads:

```rust
// SQLite supports one writer at a time
// Multiple readers can run concurrently
// Default pool size: 10 connections
let commerce = Commerce::new("commerce.db")?;
```

**Recommendations:**
- Keep pool size moderate (5-15) for SQLite
- SQLite's WAL mode enables concurrent reads
- Writes are serialized regardless of pool size

### PostgreSQL (sqlx)

Configure via connection string:

```rust
let commerce = AsyncCommerce::connect(
    "postgres://user:pass@host/db?\
     max_connections=25&\
     min_connections=5&\
     connect_timeout=30&\
     idle_timeout=600"
).await?;
```

**Recommendations:**
- `max_connections`: 2-4x CPU cores for compute-bound, higher for I/O-bound
- `min_connections`: Keep warm connections for latency-sensitive apps
- Monitor connection wait times to tune pool size

## Batch Operations

Batch operations are more efficient than individual calls:

```rust
// GOOD: Batch adjustment
let results = commerce.inventory().batch_adjust(vec![
    BatchAdjustment { sku: "SKU-001".into(), delta: 10, reason: "restock".into() },
    BatchAdjustment { sku: "SKU-002".into(), delta: -5, reason: "sale".into() },
    BatchAdjustment { sku: "SKU-003".into(), delta: 20, reason: "restock".into() },
])?;

// AVOID: Individual adjustments
for adjustment in adjustments {
    commerce.inventory().adjust(&adjustment.sku, adjustment.delta, &adjustment.reason)?;
}
```

**Performance gains:**
- Single transaction vs multiple
- Reduced round-trips for PostgreSQL
- Atomic success/failure semantics

## Query Optimization

### Pagination

Always paginate large result sets:

```rust
// GOOD: Paginated query
let orders = commerce.orders().list_with_options(ListOptions {
    limit: Some(100),
    offset: Some(0),
    ..Default::default()
})?;

// AVOID: Loading all records
let all_orders = commerce.orders().list()?; // May load millions
```

### Selective Loading

Fetch only what you need:

```rust
// Get specific order instead of filtering in memory
let order = commerce.orders().get(&order_id)?;

// Use purpose-built queries
let pending = commerce.orders().list_by_status("pending")?;
let customer_orders = commerce.orders().list_by_customer(&customer_id)?;
```

## Database Indexes

StateSet creates performance indexes automatically (migration 025). Key indexes include:

```sql
-- Orders
CREATE INDEX idx_orders_customer ON orders(customer_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_created ON orders(created_at);

-- Inventory
CREATE INDEX idx_inventory_sku ON inventory_items(sku);
CREATE INDEX idx_reservations_sku ON inventory_reservations(sku);
CREATE INDEX idx_reservations_expires ON inventory_reservations(expires_at);

-- Payments
CREATE INDEX idx_payments_order ON payments(order_id);
CREATE INDEX idx_payments_status ON payments(status);
```

For custom queries, ensure indexes exist on filtered/joined columns.

## Memory Optimization

### Event System Buffer

Configure event channel capacity based on throughput:

```rust
let config = EventConfig {
    channel_capacity: 4096,  // Default: 1024
    persist_events: false,   // Disable for memory savings
    enable_webhooks: false,  // Disable if not used
    ..Default::default()
};

let commerce = Commerce::with_config("commerce.db", config)?;
```

### SQLite Memory Mode

For temporary/testing workloads:

```rust
// In-memory database (fastest, not persisted)
let commerce = Commerce::new(":memory:")?;

// Shared cache for multi-connection scenarios
let commerce = Commerce::new("file::memory:?cache=shared")?;
```

## Benchmarks

### Typical Performance (SQLite, single-threaded)

| Operation | Time (p50) | Time (p99) |
|-----------|------------|------------|
| Customer create | 0.2ms | 0.8ms |
| Order create (5 items) | 0.5ms | 1.5ms |
| Inventory adjust | 0.1ms | 0.4ms |
| Order list (100) | 0.8ms | 2.5ms |
| Analytics summary | 2ms | 8ms |

### Typical Performance (PostgreSQL, network)

| Operation | Time (p50) | Time (p99) |
|-----------|------------|------------|
| Customer create | 2ms | 8ms |
| Order create (5 items) | 4ms | 15ms |
| Inventory adjust | 1.5ms | 6ms |
| Order list (100) | 5ms | 20ms |
| Analytics summary | 10ms | 50ms |

*Network latency dominates PostgreSQL performance.*

## Concurrency Patterns

### SQLite: Reader-Writer Pattern

```rust
use std::sync::Arc;
use parking_lot::RwLock;

// Share Commerce for reads, serialize writes
let commerce = Arc::new(RwLock::new(Commerce::new("commerce.db")?));

// Concurrent reads
let orders = commerce.read().orders().list()?;

// Serialized writes
commerce.write().orders().ship(&order_id)?;
```

### PostgreSQL: Connection-per-Request

```rust
// AsyncCommerce is thread-safe, share freely
let commerce = Arc::new(AsyncCommerce::connect(url).await?);

// Each request gets its own connection from pool
async fn handler(commerce: Arc<AsyncCommerce>) {
    let order = commerce.orders().get(&id).await?;
}
```

## Monitoring

### SQLite Statistics

```rust
// Enable SQLite statistics
let commerce = Commerce::new("commerce.db")?;

// Query internal stats
let stats = commerce.database_stats()?;
println!("Cache hits: {}", stats.cache_hits);
println!("Cache misses: {}", stats.cache_misses);
```

### PostgreSQL Metrics

Monitor via PostgreSQL's built-in views:

```sql
SELECT * FROM pg_stat_activity WHERE datname = 'stateset';
SELECT * FROM pg_stat_user_tables;
SELECT * FROM pg_stat_user_indexes;
```

## Production Checklist

- [ ] Connection pool sized appropriately
- [ ] Batch operations used where possible
- [ ] Large queries paginated
- [ ] Indexes verified for custom queries
- [ ] Event buffer sized for throughput
- [ ] WAL mode enabled (SQLite)
- [ ] Connection timeouts configured (PostgreSQL)
- [ ] Monitoring in place
