# StateSet iCommerce Roadmap to 10/10

## Current Status: 6.5/10
**Goal: 10/10** - Enterprise-grade, production-ready commerce engine for AI agents

---

## Phase 1: Performance & Architecture Foundation (Weeks 1-3) ⚡
**Impact: +2.5 points** (6.5 → 9.0)

### 1.1 Eliminate Box<dyn> Trait Objects (Grid)
**Status**: 🔴 BLOCKING
**Impact**: 1.5/10
**Effort**: 2 weeks

**Problem**:
- Every repository access returns `Box<dyn Trait>` = heap allocation
- Performance killer in hot paths (orders, inventory, payments)
- Prevents compiler optimizations

**Solution**:
```rust
// BEFORE (Grid):
trait Database: Send + Sync {
    fn orders(&self) -> Box<dyn OrderRepository + '_>;
    // 30+ more methods returning Box<dyn>
}

// AFTER (Static dispatch):
pub struct Commerce<DB: Database = SqliteDatabase> {
    db: Arc<DB>,
}

// Commerce becomes generic over DB type
// Zero-cost abstraction: monomorphization generates specialized code
```

**Files to modify**:
- `crates/stateset-db/src/lib.rs` - Add static dispatch traits
- `crates/stateset-embedded/src/lib.rs` - Make Commerce generic
- All repository modules - Convert to static dispatch

**Acceptance criteria**:
- [ ] No `Box<dyn>` in hot path
- [ ] Benchmarks show 2-3x improvement
- [ ] All tests pass
- [ ] Backward compatibility via type alias

### 1.2 Macro-Generated Database Implementations
**Status**: 🔴 BLOCKING
**Impact**: 0.5/10
**Effort**: 3 days

**Problem**:
- `impl Database for SqliteDatabase` = 120 lines
- `impl Database for PostgresDatabase` = 120 lines (EXACT DUPLICATE)
- Two more backends (TBD) = 4x240 lines of copy-paste

**Solution**:
```rust
macro_rules! impl_database {
    ($type:ty) => {
        impl Database for $type {
            fn orders(&self) -> Box<dyn OrderRepository + '_> {
                Box::new(self.orders())
            }
            // ... 30 more methods
        }
    };
}

impl_database!(SqliteDatabase);
impl_database!(PostgresDatabase);
```

### 1.3 Consolidate Domain Modules (32 → 20)
**Status**: 🟡 PLANNED
**Impact**: 0.5/10
**Effort**: 1 week

**Current structure**:
```
accounts_payable.rs      (finance)
accounts_receivable.rs   (finance)
general_ledger.rs        (finance)
cost_accounting.rs       (finance)
credit.rs               (finance)
backorder.rs            (orders fulfillment)
```

**Target structure**:
```
finance.rs              (merged: ap + ar + gl + cost + credit)
fulfillment_core.rs     (merged: orders + backorder + shipment logic)
```

**Acceptance criteria**:
- [ ] Reduce from 32 to 20 domain modules
- [ ] Update all imports
- [ ] Update migration tests

---

## Phase 2: Enterprise Testing Infrastructure (Weeks 4-6) 🔬
**Impact: +0.5 points** (9.0 → 9.5)

### 2.1 State Machine Transition Tests (7 days)
**Status**: 🔴 MISSING
**Coverage**: 0% → 90%

**Missing tests**:
```rust
#[test]
fn test_order_status_transitions() {
    // Test all valid transitions
    assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Confirmed));
    assert!(OrderStatus::Confirmed.can_transition_to(OrderStatus::Processing));

    // Test invalid transitions
    assert!(!OrderStatus::Shipped.can_transition_to(OrderStatus::Pending));
}

#[test]
fn test_inventory_reservation_conflicts() {
    // Two orders reserving same stock
    // First succeeds, second fails or queues
}
```

**Acceptance**:
- [ ] 100% of state machines tested
- [ ] Invalid transitions assert
- [ ] Edge cases covered

### 2.2 Concurrency & Race Condition Tests (7 days)
**Status**: 🔴 MISSING

**Test scenarios**:
- Parallel order creation with inventory race
- Cart abandonment cleanup conflicts
- Payment double-booking prevention
- Reservation timeout complications
- Multiple agents updating same resource

**Use `rayon` for parallel tests**:
```rust
#[test]
fn test_concurrent_order_creation() {
    use rayon::prelude::*;

    (0..100).into_par_iter().for_each(|i| {
        // Create 100 orders in parallel
        // Ensure no deadlocks, no data loss
    });
}
```

### 2.3 Integration Test Suite (7 days)
**Status**: ⚠️ THIN

**End-to-end workflows**:
- Customer registration → Browse products → Add to cart → Checkout → Payment → Ship → Deliver
- Customer return → RMA approval → Refund → Inventory adjustment
- Subscription lifecycle: Create → Bill → Pause → Resume → Cancel
- Purchase order: Create → Approve → Ship items → Receive → Pay supplier
- Manufacturing: Create BOM → Work order → Consume materials → Produce items → Update inventory

**Acceptance**:
- [ ] All major workflows automated as tests
- [ ] Snapshot testing for critical states
- [ ] Performance benchmarks included

### 2.4 Property-Based Testing
**Status**: ❌ ABSENT

**Use `proptest` for invariants**:
```rust
proptest! {
    #[test]
    fn prop_order_total_matches_items(
        items in vec(any::<OrderItem>(), 1..10)
    ) {
        let order = create_order_with_items(items);
        let calculated: Decimal = order.items.iter().map(|i| i.total).sum();
        prop_assert_eq!(order.total_amount, calculated);
    }
}
```

**Properties to test**:
- Order total = sum(item totals) + shipping - discounts + tax
- Inventory quantity never negative
- Reservations don't exceed available stock
- Payment refunds never exceed original payment
- Subscription billing cycles are sequential

---

## Phase 3: Observability & Operations (Weeks 7-9) 📊
**Impact: +0.3 points** (9.5 → 9.8)

### 3.1 Structured Tracing Everywhere (5 days)
**Status**: ⚠️ PARTIAL (only orders.rs)

**Add to every repository method**:
```rust
#[tracing::instrument(
    skip(self, input),
    fields(
        customer_id = %input.customer_id,
        items = input.items.len(),
        total_amount = %input.total_amount
    )
)]
pub fn create(&self, input: CreateOrder) -> Result<Order> {
    tracing::debug!("Creating order");
    let order = self.db.orders().create(input)?;
    tracing::info!("Order created: {}", order.order_number);
    Ok(order)
}
```

**Trace spans**:
- All CRUD operations
- State transitions
- External API calls (payment gateways, shipping carriers)
- Cache hits/misses
- Database query durations

### 3.2 Metrics Collection (3 days)
**Status**: ❌ ABSENT

**Integrate `metrics` crate**:
```rust
use metrics::{counter, gauge, histogram};

// Count operations
counter!("orders.created", 1);

// Track durations
histogram!("db.query.duration", start.elapsed());

// Gauge current state
gauge!("connections.active", pool.state().connections);
```

**Key metrics**:
- Order/create/update/delete rates by status
- Inventory reservation success/failure rates
- Payment processing times by method
- Cache hit rates
- Database connection pool health
- Error rates by type

### 3.3 Performance Benchmarks (5 days)
**Status**: ❌ ABSENT

**Add `criterion` benchmarks**:
```criterion benches/```

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_order_creation(c: &mut Criterion) {
    let commerce = Commerce::new(":memory:").unwrap();

    c.bench_function("create_order", |b| {
        b.iter(|| {
            commerce.orders().create(test_order_input());
        });
    });
}

criterion_group!(benches, bench_order_creation);
criterion_main!(benches);
```

**Benchmark**:
- All repository CRUD operations
- Complex queries (filtering, pagination)
- State machine transitions
- Batch operations (100+ items)

### 3.4 Health Check & Diagnostics CLI (3 days)
**Status**: ❌ ABSENT

**Create `stateset doctor`**:
```bash
$ stateset doctor
✓ Database: stateset.db (SQLite 3.42)
✓ Connection pool: 5/10 active (50%)
✓ Last migration: 026_idempotency_keys (applied)
✓ Migrations pending: 0
✓ Performance indexes: 24/24 present

⚠ WARNING: Database size: 2.3GB (consider VACUUM)
⚠ WARNING: Growth rate: 100MB/day (estimated disk full in 67 days)

Database health: 92/100
```

**Checks**:
- Database file integrity
- Migration status
- Index presence
- Connection pool health
- Cache effectiveness
- Disk usage projection

---

## Phase 4: Transaction & Saga Support (Weeks 10-11) 🔄
**Impact: +0.2 points** (9.8 → 10.0)

### 4.1 Domain-Level Transactions (5 days)
**Status**: ⚠️ PRIMITIVE

**Current**:
```rust
db.with_transaction(|conn| {
    conn.execute("INSERT INTO orders ...", [])?;
    conn.execute("UPDATE inventory ...", [])?;
    Ok(())
})
```

**Target**:
```rust
commerce.orders().create_with_reservation(input)?;
// Internally runs atomic transaction:
// - BEGIN
// - INSERT INTO orders
// - UPDATE inventory_balances (reserve)
// - INSERT INTO inventory_reservations
// - COMMIT
```

### 4.2 Saga Pattern for Long-Running Workflows (7 days)
**Status**: ❌ ABSENT

**Use saga-orchestration**:
```rust
let saga = Saga::new("order_fulfillment")
    .step("create_order", create_order_step)
    .step("reserve_inventory", reserve_inventory_step)
    .step("process_payment", process_payment_step)
    .step("ship_order", ship_order_step);

saga.execute(input)?;
// If any step fails, automatically compensates previous steps
```

**Workflows**:
- Order fulfillment (create → reserve → pay → ship)
- Subscription billing → payment → renewal
- Manufacturing BOM → work order → consume materials → produce
- Purchase order → ship → receive → pay supplier

### 4.3 Event Sourcing + CQRS (7 days)
**Status**: ⚠️ PARTIAL

**Implement**:
```rust
// Events table
CREATE TABLE events (
    id UUID PRIMARY KEY,
    aggregate_type TEXT NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    event_data JSONB,
    version INTEGER NOT NULL,
    occurred_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(aggregate_type, aggregate_id, version)
);

// Projections (CQRS)
CREATE TABLE order_summary_projection (
    order_id UUID PRIMARY KEY,
    customer_id UUID,
    status TEXT,
    total_amount DECIMAL,
    last_updated TIMESTAMP
);
```

---

## Phase 5: Developer Experience (Weeks 12-13) 🚀
**Impact: Keeps 10/10 maintainable**

### 5.1 Type-Asorganized Prelude Modules (3 days)
**Status**: ⚠️ OVERWHELMING

**Before**: 821-line lib.rs with 700 re-exports

**After**:
```rust
pub mod prelude {
    pub use crate::orders::*;
    pub use crate::customers::*;
    pub use crate::inventory::*;
    pub use crate::products::*;
    // ... 15-20 domain preludes
}

pub mod finance_prelude {
    pub use crate::finance::*;
    pub use crate::payments::*;
    pub use crate::invoices::*;
    pub use crate::accounts_payable::*;
}
```

### 5.2 Code-Generated Language Bindings (7 days)
**Status**: 🔴 HIGH EFFORT MANUAL MAINTENANCE

**Create binding spec**:
```yaml
# bindings/api-spec.yaml
exports:
  orders:
    - method: create
      inputs: [CreateOrder]
      output: Order
      description: "Create a new order"

    - method: get
      inputs: [Uuid]
      output: Option<Order>
      description: "Get order by ID"

    - method: update_status
      inputs: [Uuid, OrderStatus]
      output: Order
      description: "Update order status"

  customers:
    - method: create
      inputs: [CreateCustomer]
      output: Customer

# Run: cargo bindgen --all
# Generates: bindings/{python,node,ruby,java,swift,go}
```

### 5.3 Feature Gate Cleanup (3 days)
**Status**: 🟡 SPRAWLED

**Reorganize**:
```toml
[features]
default = ["sqlite", "sync"]
sqlite = ["stateset-db/sqlite"]
postgres = ["stateset-db/postgres", "tokio"]
events = ["dep:tokio", "dep:tracing"]
ves = ["dep:ed25519", "dep:x25519", "dep:sha2"]
metrics = ["dep:metrics", "dep:tracing-subscriber"]
full = ["sqlite", "postgres", "events", "ves", "metrics", "sync"]
```

### 5.4 Migration Rollback Support (2 days)
**Status**: ❌ ABSENT

**Implement**:
```rust
// Migration approach: always up/down SQL
crate::migrations::Migration {
    version: 027,
    up: r#"
        CREATE TABLE accounts_receivable (...);
    "#,
    down: r#"
        DROP TABLE accounts_receivable;
    "#
};

// CLI support
stateset migrate down 027  # Rollback to 026
stateset migrate down all # Rollback all
stateset migrate status   # Show applied/pending
```

---

## Acceptance Criteria for 10/10

✅ **Architecture** (10/10)
- [x] Static dispatch (zero-cost abstractions)
- [x] No code duplication (macros)
- [x] 15-20 domain modules (well-organized)
- [x] Generic Commerce<DB> (monomorphization)

✅ **Performance** (10/10)
- [x] No heap allocations in hot paths
- [x] All operations < 10ms (p95)
- [x] Benchmarks track performance over time
- [x] Connection pooling properly tuned

✅ **Testing** (10/10)
- [x] 70%+ coverage (with dangerous parts > 90%)
- [x] All state machines tested
- [x] Concurrency/race condition tests
- [x] Property-based tests for invariants
- [x] Integration test suite

✅ **Observability** (10/10)
- [x] Structured tracing everywhere
- [x] Metrics collection (Prometheus format)
- [x] Performance benchmarks (criterion)
- [x] `stateset doctor` for diagnostics

✅ **Reliability** (10/10)
- [x] Domain-level transactions
- [x] Saga orchestration for multi-step workflows
- [x] Event sourcing + CQRS for auditability
- [x] Migration support (up + down)
- [x] Rollback procedures documented

✅ **Developer Experience** (10/10)
- [x] Type-organized preludes
- [x] Code-generated bindings (no manual maintenance)
- [x] Clean feature gates
- [x] Comprehensive docs
- [x] Examples for all major operations

---

## Timeline Summary

| Phase | Duration | Start | End | Impact |
|-------|----------|-------|-----|--------|
| 1. Performance & Architecture | 3 weeks | Week 1 | Week 3 | +2.5 |
| 2. Enterprise Testing | 3 weeks | Week 4 | Week 6 | +0.5 |
| 3. Observability & Operations | 3 weeks | Week 7 | Week 9 | +0.3 |
| 4. Transaction & Saga | 2 weeks | Week 10 | Week 11 | +0.2 |
| 5. Developer Experience | 2 weeks | Week 12 | Week 13 | +0.0 |
| **Total** | **13 weeks** | | | **+3.5** |

**Week 13 Score**: 6.5 → 10.0

---

## Blocking Issues

🔴 **CRITICAL**: Phase 1.1 (static dispatch) blocks almost everything else优先 solving performance fundamental problems before adding features.

⚠️ **HIGH**: Test coverage must reach 70% before production deployment

🟡 **MEDIUM**: Observability needed for production debugging

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Static dispatch breaks API | Medium | High | Maintain type aliases for backward compat |
| Test writing takes longer | High | Medium | Start with high-risk state machines first |
| Performance regressions | Low | High | Benchmark instrumentation before and after |
| Bindings generation fails | Medium | Medium | Fallback: Keep manual bindings as fallback |

---

## Success Metrics

**Technical**:
- 70%+ code coverage
- All hot path operations < 10ms (p95)
- Zero `Box<dyn>` in hot paths
- All state machines fully tested
- Migration suite with rollback

**Business**:
- Production deployment ready
- Enterprise-level SLA (99.9% uptime)
- Developer onboarding < 1 hour
- Bug rate < 0.1/blocking/critical per release

---

## Next Steps

1. ✅ **Review this roadmap with team** - Get alignment
2. ✅ **Create detailed tickets** - Break into Jira/GitHub issues
3. ✅ **Set up benchmarking** - Establish baseline metrics
4. ✅ **Start Phase 1.1** - Critical path: static dispatch

---

## Questions for Team

1. Can we break API compatibility for performance? (Type aliases can provide migration path)
2. Timeline flexibility? Can we dedicate 2 engineers full-time for 13 weeks?
3. Prioritize testing vs observability? Should we swap Phase 2/3?
4. Binding generation scope? Start with Node.js/Python or all 11 at once?

---

**Created**: 2025-01-25
**Last Updated**: 2025-01-25
**Owner**: Engineering Leadership
**Status**: 🔴 READY FOR APPROVAL