# Testing Best Practices

This document outlines the testing philosophy, patterns, and conventions used across the StateSet iCommerce project.

## Overview

StateSet follows a comprehensive testing strategy with multiple test types:

- **Unit Tests**: Test individual functions and methods in isolation
- **Integration Tests**: Test interactions between multiple components
- **State Machine Tests**: Verify valid/invalid state transitions
- **Property-Based Tests**: Use proptest to find edge cases
- **Snapshot Tests**: Ensure migrations and output remain stable
- **Performance Tests**: Benchmark and detect regressions
- **Concurrent Tests**: Verify thread safety and data consistency

## Test Organization

```
crates/
├── stateset-core/
│   └── tests/
│       └── proptest_models.rs          # Property-based model tests
├── stateset-db/
│   └── tests/
│       ├── sqlite_migrations.rs       # Migration tests
│       ├── postgres_migrations.rs     # PostgreSQL-specific tests
│       ├── postgres_crud.rs           # CRUD endpoint tests
│       ├── sqlite_atomic_writes.rs    # Atomic write tests
│       ├── sqlite_order_transitions.rs # Order state machine
│       ├── sqlite_order_versioning.rs # Order version conflicts
│       ├── sqlite_validations.rs      # Input validation tests
│       ├── batch_atomic_validation.rs # Batch operation tests
│       └── product_slug_validation.rs # Slug validation tests
└── stateset-embedded/
    └── tests/
        ├── state_machine_tests.rs            # State machine coverage
        ├── concurrency_test.rs               # Concurrent operations
        ├── property_based_tests.rs           # Proptest-based tests
        ├── comprehensive_integration_test.rs # E2E workflows
        ├── migration_snapshot_tests.rs        # Snapshot tests
        ├── carts_test.rs                     # Cart-specific tests
        ├── orders_test.rs                    # Order-specific tests
        ├── inventory_advanced_test.rs        # Inventory edge cases
        ├── payments_test.rs                  # Payment lifecycle
        ├── returns_lifecycle_test.rs         # Return processing
        ├── subscriptions_test.rs             # Subscription billing
        ├── manufacturing_test.rs             # Manufacturing workflows
        ├── stress_test.rs                    # Load testing
        ├── idempotency_test.rs               # Idempotency guarantees
        ├── error_paths_test.rs               # Error handling
        ├── fulfillment_test.rs               # Fulfillment workflows
        └── tax_test.rs                       # Tax calculations
```

## Writing Effective Tests

### 1. Test Helper Functions

Create reusable helper functions to reduce duplication:

```rust
fn create_test_customer(commerce: &Commerce) -> Uuid {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
}
```

### 2. Use In-Memory Databases

For fast, isolated tests:

```rust
#[test]
fn test_customer_creation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    // Test implementation
}
```

### 3. State Machine Tests

Test all valid and invalid transitions:

```rust
#[test]
fn test_order_state_machine_valid_transitions() {
    let order = create_test_order();
    
    // Test valid transition: Pending → Confirmed
    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");
    assert_eq!(order.status, OrderStatus::Confirmed);
}

#[test]
fn test_order_state_machine_invalid_transitions() {
    let order = create_test_order();
    
    // Test invalid transition: Pending → Shipped
    let result = commerce
        .orders()
        .update_status(order.id, OrderStatus::Shipped);
    assert!(result.is_err());
}
```

### 4. Property-Based Tests

Use proptest to find edge cases:

```rust
#[test]
fn test_reserve_inventory_always_maintains_invariants(
    initial_qty in 10i64..1000,
    reserve_qty in 1i64..100,
) {
    prop_assume!(reserve_qty <= initial_qty);
    
    // Test implementation
}
```

### 5. Error Path Testing

Test error conditions explicitly:

```rust
#[test]
fn test_insufficient_inventory_returns_error() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    
    commerce.inventory().create_item(CreateInventoryItem {
        sku: "SKU-001".into(),
        name: "Widget".into(),
        initial_quantity: Some(dec!(10)),
        ..Default::default()
    }).expect("Failed to create inventory");
    
    let result = commerce.inventory().reserve("SKU-001", dec!(100), "order", &Uuid::new_v4().to_string(), None);
    assert!(result.is_err());
}
```

### 6. Snapshot Tests

Use insta for migration and output testing:

```rust
#[test]
fn test_migration_schema_snapshot() {
    let db_path = "test.db";
    let commerce = Commerce::new(db_path).expect("Failed to create commerce");
    
    let schema = get_schema(db_path);
    insta::assert_snapshot!(schema);
}
```

## Coverage Goals

- **Overall Target**: 80%+ code coverage
- **Core Models**: 90%+ coverage (orders, customers, inventory, payments)
- **Business Logic**: 85%+ coverage (state machines, validation, calculations)
- **Error Paths**: 100% coverage for error conditions

## Running Tests

### Run All Tests
```bash
cargo test --workspace
```

### Run Specific Test
```bash
cargo test -p stateset-embedded test_order_state_machine
```

### Run Tests with Output
```bash
cargo test --workspace -- --nocapture
```

### Run Property-Based Tests
```bash
cargo test -p stateset-embedded property_based -- --test-threads=1
```

### Run Benchmarks
```bash
cargo bench -p stateset-embedded
```

### Run Migration Tests
```bash
cargo test -p stateset-db --test sqlite_migrations
cargo test -p stateset-db --test postgres_migrations
```

## CI/CD Integration

The CI pipeline runs:

1. **Format Check**: `cargo fmt --all -- --check`
2. **Linting**: `cargo clippy --workspace --all-targets -- -D warnings`
3. **Security Audit**: `cargo audit`
4. **Dependency Policy**: `cargo deny check`
5. **Unit Tests**: `cargo test -p stateset-core -p stateset-db -p stateset-embedded`
6. **Coverage**: `cargo-tarpaulin --workspace --out Xml` (generates coverage report)
7. **Performance Benchmarks**: `cargo bench` (detects regressions > 5%)
8. **Mutation Testing**: `cargo mutants` (detects weak tests)

## Continuous Monitoring

- **Coverage Regression**: CI fails if coverage drops below 70%
- **Performance Regression**: CI fails if benchmarks degrade > 5%
- **Mutation Score**: CI fails if mutation score < 70%

## Test Naming Conventions

- **Unit Tests**: `fn test_<functionality>()`
- **State Machine Tests**: `fn test_<entity>_state_machine_<transitions>()`
- **Integration Tests**: `fn test_<workflow>_workflow()`
- **Property Tests**: `fn test_<property>(<param1> in <range>, ...)`
- **Error Tests**: `fn test_<error_condition>_returns_error()`

## Common Pitfalls

1. **Not Testing Error Paths**: Always test both success and failure cases
2. **Shared State**: Use fresh database instances per test
3. **Flaky Tests**: Avoid time-dependent logic; use deterministic data
4. **Slow Tests**: Use in-memory databases for speed
5. **Hardcoded Values**: Use helper functions and generated data

## Adding New Tests

When adding a new feature:

1. Add unit tests in the same module or `tests/` directory
2. Add integration tests in `crates/stateset-embedded/tests/`
3. Add state machine tests for all status transitions
4. Add property-based tests for input validation
5. Update snapshot tests if migrations change
6. Add benchmarks if performance-sensitive
7. Update coverage goals in `TESTING_STRATEGY.md`

## Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [proptest Documentation](https://altsysrq.github.io/proptest-book/proptest-tutorial/README.html)
- [insta Documentation](https://insta.rs/)
- [Criterion Documentation](https://bheisler.github.io/criterion.rs/book/)