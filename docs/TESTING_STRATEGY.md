# StateSet iCommerce Testing Strategy

## Overview

This document outlines the testing strategy for StateSet iCommerce, including the quality gates enforced in CI and the broader test methodology used across crates and bindings.

## Test Pyramid

```
        /\
       /  \      E2E Tests (5%)
      /----\     - Complete commerce workflows
     /      \    - Multi-domain transactions
    /--------\   - Real-world scenarios
   /          \
  /------------\  Integration Tests (25%)
 /              \ - State machine validation
/                \ - Concurrency and conflicts
------------------ - Repository integration
                  - Database migrations
------------------  Unit Tests (70%)
                  - Domain model logic
                  - Validation rules
                  - Error handling
```

## Coverage Gates

| Signal | Gate | Enforced In | Notes |
|--------|------|-------------|-------|
| **Rust workspace line coverage** | **>= 80%** | CI and coverage workflow (`cargo llvm-cov`) | Excludes benches/tests from gate calculations where configured |
| **CLI line coverage** | **>= 75%** | Coverage workflow (`node --experimental-test-coverage`) | Parsed from Node coverage summary |
| **Per-crate quality** | Feature/MSRV/lint/test must pass | Main CI matrix | clippy, feature checks, Postgres parity, sanitizers, CodeQL, docs build |

## Test Categories

### 1. Unit Tests

**Purpose**: Test individual functions and methods in isolation.

**Location**: `crates/*/src/**/*.rs` (module-level `tests` modules)

**Examples**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_total_calculation() {
        let mut order = Order::default();
        order.items.push(item_with_price(29.99));
        order.items.push(item_with_price(49.99));
        assert_eq!(order.total(), dec!(79.98));
    }
    
    #[test]
    fn test_currency_validation() {
        assert!(Currency::from_str("USD").is_ok());
        assert!(Currency::from_str("XXX").is_err());
    }
}
```

**Tools**: `cargo test`, `assert!`, `assert_eq!`, `assert_matches!`

### 2. Integration Tests

**Purpose**: Test interactions between components.

**Location**: `crates/stateset-embedded/tests/`, `crates/stateset-db/tests/`

**Examples**:
- `state_machine_tests.rs` - Order, payment, inventory state transitions
- `concurrency_tests.rs` - Concurrent reservations, conflicts
- `e2e_commerce_workflow_test.rs` - Complete order-to-ship lifecycle

**Tools**: `cargo test`, `tempfile`, `rand`

### 3. Property-Based Tests

**Purpose**: Ensure code follows invariants across all inputs.

**Location**: `crates/stateset-core/tests/proptest_models.rs`

**Examples**:
```rust
proptest! {
    #[test]
    fn prop_order_total_roundtrips(items: Vec<OrderItem>) {
        let total = items.iter().map(|i| i.unit_price * i.quantity as i64).sum();
        let order = Order { items, ..Default::default() };
        prop_assert_eq!(order.total(), total);
    }
}
```

**Tools**: `proptest`, `quickcheck`

### 4. Load and Stress Tests

**Purpose**: Verify performance under load.

**Location**: `crates/stateset-embedded/tests/stress_test.rs`

**Examples**:
- 100+ concurrent order creations
- 1000+ inventory reservations
- Memory leaks over 24h

**Tools**: `criterion`, `tokio`, `rayon`

### 5. Snapshot Tests

**Purpose**: Ensure migrations don't break backward compatibility.

**Location**: `crates/*/tests/snapshots/`

**Examples**:
```rust
#[test]
fn test_migration_v001_to_v002() {
    let db = setup_test_database(":memory:");
    run_migration(&db, "001_initial.sql");
    insert_test_data(&db);
    run_migration(&db, "002_add_audit_fields.sql");
    
    // Snapshot to verify data preservation
    assert_snapshot!(dump_database(&db));
}
```

**Tools**: `insta`, `sqlx`

## State Machine Testing

### Order State Machine

**States**: Pending → Confirmed → Processing → Shipped → Delivered → Refunded
**Terminal States**: Cancelled, Refunded

**Test Coverage**:
- ✅ All valid state transitions
- ✅ Invalid state transitions fail
- ✅ Cancel before shipment succeeds
- ✅ Cancel after shipment fails
- ✅ Refund after delivery succeeds

**File**: `crates/stateset-embedded/tests/state_machine_tests.rs`

### Payment State Machine

**States**: Pending → Processing → Completed → Refunded
**Terminal States**: Failed, Cancelled

**Test Coverage**:
- ✅ All valid state transitions
- ✅ Payment amount validation
- ✅ Refund up to original amount
- ✅ Partial refunds

### Inventory Reservation Lifecycle

**States**: Pending → Allocated → Confirmed → Released
**Terminal States**: Expired

**Test Coverage**:
- ✅ Reserve available inventory
- ✅ Conflict when insufficient stock
- ✅ Confirm reservation
- ✅ Release reservation
- ✅ Concurrent reservations

### Subscription Billing Lifecycle

**States**: Trial → Active → Paused → Cancelled
**Terminal States**: Expired, Cancelled

**Test Coverage**:
- ✅ Create subscription from plan
- ✅ Pause and resume subscription
- ✅ Cancel subscription
- ✅ Billing cycle advancement
- ✅ Proration on plan changes

## Concurrency Testing

### Scenarios

1. **Concurrent Order Creation** - 100 orders created simultaneously
2. **Inventory Race Conditions** - 50 concurrent reservations for same SKU
3. **Payment Idempotency** - Duplicate payment processing
4. **Subscription Billing** - Concurrent billing cycles

### Test Pattern

```rust
#[tokio::test]
async fn test_concurrent_inventory_reservations() {
    let commerce = Arc::new(Commerce::new(":memory:").unwrap());
    
    commerce.inventory().create_item(CreateInventoryItem {
        sku: "SKU-001".into(),
        name: "Widget".into(),
        initial_quantity: Some(dec!(100)),
        ..Default::default()
    }).unwrap();
    
    let handles: Vec<_> = (0..50)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            tokio::spawn(async move {
                commerce.inventory().reserve("SKU-001", dec!(2), "test", &Uuid::new_v4().to_string(), None)
            })
        })
        .collect();
    
    let results: Vec<_> = handles.into_iter().filter_map(|h| h.await.ok()).collect();
    
    // Should succeed for 50 reservations of 2 each = 100 units
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 50);
}
```

## Performance Testing

### Baselines

| Operation | Target p95 | Current p95 | Status |
|-----------|------------|-------------|--------|
| CRUD (Order) | <50ms | ~150ms | 🟡 Needs optimization |
| Inventory Reserve | <30ms | ~80ms | 🟡 Needs optimization |
| Payment Record | <40ms | ~60ms | 🟢 Good |
| Query (Analytics) | <100ms | ~200ms | 🟡 Needs optimization |

### Benchmark Suite

**Location**: `crates/*/benches/*.rs`

**Examples**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_order_creation(c: &mut Criterion) {
    let commerce = setup_test_commerce();
    
    c.bench_function("create_order", |b| {
        b.iter(|| {
            black_box(
                commerce.orders().create(test_order_input())
            )
        })
    });
}

criterion_group!(benches, bench_order_creation);
criterion_main!(benches);
```

**Run**: `cargo bench`

### Performance Regression Testing

**CI Job**: `performance-benchmarks`

**Process**:
1. Run benchmarks on PR
2. Compare to baseline (main branch)
3. Fail if >5% regression
4. Report in PR comments

## Mutation Testing

### Purpose

Ensure tests catch real bugs by introducing "mutations" (faults) and verifying that tests fail when they should.

### Tools

**cargo-mutants**: Automatically generate mutants from source code

### Configuration

```toml
# .cargo/config.toml
[mutants]
exclude = [
    "crates/bindings/*",
    "crates/cli/*"
]
timeout-factor = 2.0
```

### CI Job

```yaml
mutation-testing:
  name: Mutation Testing
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Install cargo-mutants
      run: cargo install cargo-mutants
    - name: Run mutation testing
      run: cargo mutants -p stateset-core -p stateset-db -p stateset-embedded
```

### Coverage

**Target**: 80%+ mutants killed

**Current**: Unknown (needs baseline)

### Example Mutations

| Original | Mutated | Expected |
|----------|---------|----------|
| `if x > 0` | `if x >= 0` | Test should fail |
| `result?` | `result.unwrap()` | Test should fail |
| `OrderStatus::Confirmed` | `OrderStatus::Pending` | Test should fail |

## CI/CD Integration

### Continuous Integration

**Jobs**:
1. **fmt** - Code formatting
2. **clippy** - Linting
3. **audit** - Security audit
4. **deny** - Dependency policy
5. **rust** - Unit and integration tests
6. **coverage** - Test coverage reporting
7. **benchmarks** - Performance regression tests
8. **mutation** - Mutation testing (weekly)
9. **postgres** - PostgreSQL integration
10. **bindings** - Language binding tests

### Coverage Reporting

**Tool**: `cargo llvm-cov`

**Configuration**:
```yaml
coverage:
  name: Code Coverage
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Install cargo-llvm-cov
      run: cargo install cargo-llvm-cov
    - name: Generate coverage
      run: cargo llvm-cov --workspace --cobertura --output-path coverage/cobertura.xml --fail-under-lines 80
    - name: Upload to Codecov
      uses: codecov/codecov-action@v4
```

**Thresholds**:
- Pull requests: Must not decrease coverage by >2%
- Main branch: Maintain 80%+ overall coverage

### Quality Gates

**Before Merge**:
- ✅ All tests pass
- ✅ Clippy warnings = 0
- ✅ Formatting correct
- ✅ Security audit passes
- ✅ Dependency policy passes
- ✅ Coverage not decreased by >2%
- ✅ Performance not regressed by >5%

## Test Data Management

### Fixtures

**Location**: `crates/stateset-embedded/tests/fixtures/`

**Examples**:
```rust
pub fn sample_customer() -> CreateCustomer {
    CreateCustomer {
        email: "test@example.com".into(),
        first_name: "Test".into(),
        last_name: "User".into(),
        ..Default::default()
    }
}

pub fn sample_order(customer_id: Uuid) -> CreateOrder {
    CreateOrder {
        customer_id,
        items: vec![sample_order_item()],
        ..Default::default()
    }
}
```

### Test Factories

```rust
pub struct TestFactory {
    commerce: Commerce,
}

impl TestFactory {
    pub fn new() -> Self {
        Self {
            commerce: Commerce::new(":memory:").unwrap(),
        }
    }
    
    pub fn create_customer(&self) -> Customer {
        self.commerce.customers().create(sample_customer()).unwrap()
    }
    
    pub fn create_order(&self, customer_id: Uuid) -> Order {
        self.commerce.orders().create(sample_order(customer_id)).unwrap()
    }
}
```

## Best Practices

### 1. Test Naming

✅ **Good**:
```rust
fn test_order_cancellation_before_shipment_succeeds()
fn test_inventory_reservation_conflict_when_insufficient_stock()
```

❌ **Bad**:
```rust
fn test_order()  // Too vague
fn test1()       // Meaningless
```

### 2. AAA Pattern

```rust
#[test]
fn test_order_total_calculation() {
    // Arrange
    let order = Order {
        items: vec![
            OrderItem { unit_price: dec!(29.99), quantity: 2, ..Default::default() }
        ],
        ..Default::default()
    };
    
    // Act
    let total = order.total();
    
    // Assert
    assert_eq!(total, dec!(59.98));
}
```

### 3. One Assertion Per Test

```rust
// ❌ Bad
fn test_order_validation() {
    let order = Order { /* ... */ };
    assert!(order.is_valid());
    assert_eq!(order.total(), dec!(100));
    assert_eq!(order.status, OrderStatus::Pending);
}

// ✅ Good
fn test_order_is_valid_with_valid_data() {
    let order = valid_order();
    assert!(order.is_valid());
}

fn test_order_total_calculation() {
    let order = order_with_items(vec![item_with_price(100)]);
    assert_eq!(order.total(), dec!(100));
}
```

### 4. Test Isolation

```rust
#[test]
fn test_order_creation() {
    // ✅ Good: Each test uses its own database
    let commerce = Commerce::new(":memory:").unwrap();
    
    // ❌ Bad: Tests share state
    static mut COMMERCE: Option<Commerce> = None;
    unsafe {
        COMMERCE = Some(Commerce::new(":memory:").unwrap());
    }
}
```

## Coverage Gaps

### High Priority

1. **Error Paths** - Many error conditions not tested
2. **Edge Cases** - Boundary conditions (0, -1, MAX_INT)
3. **Concurrent Modifications** - Race conditions, conflicts
4. **Migration Rollback** - Rollback scenarios
5. **Database Backward Compatibility** - Old data in new schema

### Medium Priority

1. **Analytics Queries** - Complex aggregations
2. **Vector Search** - Embedding queries
3. **Sync Protocol** - VES event ordering
4. **Bindings Edge Cases** - Language-specific issues

### Low Priority

1. **CLI Commands** - Most have good coverage
2. **Documentation Examples** - Verified by docs/build
3. **Performance Hot Paths** - Covered by benchmarks

## Metrics

### Success Metrics

| Metric | Target | Current | Trend |
|--------|--------|---------|-------|
| Test Coverage | 80%+ | ~65% | 📈 Increasing |
| Test Flakiness | <1% | <1% | ✅ Stable |
| CI Duration | <15 min | ~12 min | ✅ Good |
| Mutation Killer Rate | 80%+ | Unknown | 📊 TBD |
| Performance p95 | <50ms | ~150ms | 📉 Needs work |

### Dashboard

- **Coverage**: [Codecov](https://codecov.io/gh/stateset/stateset-icommerce)
- **Performance**: CI benchmark results
- **Mutations**: Weekly mutation testing report
- **Flaky Tests**: CI test flakiness tracking

## Roadmap to A+

### Week 1-2: Test Coverage Boost
- [ ] Add coverage reporting to CI
- [ ] Fill top 20 coverage gaps
- [ ] Add snapshot testing for migrations
- [ ] Target: 75%+ coverage

### Week 3: Performance Testing
- [ ] Establish performance baseline
- [ ] Add performance regression tests
- [ ] Optimize slow operations
- [ ] Target: <50ms p95 for CRUD

### Week 4: Mutation Testing
- [ ] Run mutation testing baseline
- [ ] Fix weak tests (mutations surviving)
- [ ] Add mutants to CI (weekly)
- [ ] Target: 80%+ mutants killed

### Week 5: Advanced Testing
- [ ] Add chaos engineering tests
- [ ] Add property-based tests for invariants
- [ ] Add load testing scripts
- [ ] Target: 80%+ overall coverage

## Conclusion

This testing strategy ensures StateSet iCommerce achieves and maintains A+ product quality through comprehensive test coverage, performance monitoring, and continuous quality gates.
