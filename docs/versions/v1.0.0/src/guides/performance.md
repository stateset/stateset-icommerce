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

Run the Criterion suites locally:

```bash
cargo bench -p stateset-benches
# Enforce perf budgets in CI/local verification
STATESET_PERF_GATE=1 cargo bench -p stateset-benches
```

StateSet ships explicit perf budgets in [`crates/stateset-benches/perf-gates.json`](../../../crates/stateset-benches/perf-gates.json). Those thresholds are what CI treats as the regression guardrail.

### Published Perf Gates

| Benchmark | Baseline budget | Notes |
|-----------|-----------------|-------|
| `money_add` / `money_sub` | 800 ns/op | Hot-path arithmetic |
| `currency_code_parse` | 1,500 ns/op | Parsing + validation |
| `jcs_small` / `jcs_medium` / `jcs_large` | 80 us / 400 us / 4 ms | Canonicalization by payload size |
| `merkle_10` / `merkle_100` / `merkle_1000` / `merkle_10000` | 12 us / 80 us / 800 us / 10 ms | Tree construction scaling |
| `publish_1000_no_sub` | 20 ms total | Event bus publish throughput |
| `publish_subscribe_1000` | 40 ms total | Event bus with one subscriber |
| `publish_multi_sub_1000` | 120 ms total | Event bus fan-out |
| `batch_orders_100` / `batch_orders_1000` | 250 ms / 2.2 s | SQLite batch insert throughput |
| `batch_customers_100` / `batch_customers_1000` | 200 ms / 1.8 s | SQLite batch insert throughput |

Perf gates allow a `25%` tolerance over the checked-in baseline. If you need to tune locally, `STATESET_PERF_GATE_TOLERANCE` and `STATESET_PERF_GATE_ITERATIONS` override the defaults without editing the repo.

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

### SLA Guidance

Treat the checked-in perf gates as the minimum release bar for hot paths:

- Core scalar operations stay sub-microsecond.
- Canonicalization stays sub-`4 ms` even for the large benchmark payload.
- Event fan-out of 1,000 publishes stays under `120 ms` total in the benchmark harness.
- SQLite bulk inserts of 1,000 records stay under `2.2 s` for orders and `1.8 s` for customers.

If a deployment needs tighter production SLOs than these repo-wide gates, pin your own environment-specific thresholds on top of the Criterion baselines.

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
