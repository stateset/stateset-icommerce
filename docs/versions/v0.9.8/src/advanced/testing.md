# Testing Strategy

iCommerce maintains comprehensive test coverage across all layers: 3,477 Rust tests, 10,700+ CLI tests, and 261 admin tests.

## Test Counts

| Layer | Tests | Runner |
|-------|-------|--------|
| Rust (full workspace) | 3,477 | `cargo test` |
| CLI (tool handlers, A2A, adapters) | ~10,700 | `node --test` |
| Admin (UI components, API routes) | 261 | Vitest + jsdom |
| **Total** | **~14,400** | |

## Rust Tests

### Unit Tests

Each crate contains unit tests alongside the source code:

```bash
cargo test -p stateset-core
cargo test -p stateset-crypto
cargo test -p stateset-db
```

### Integration Tests

Cross-crate integration tests validate end-to-end flows:

```bash
cargo test -p stateset-integration-tests
# 226+ integration tests
```

### Property-Based Tests

Proptest generates random inputs to find edge cases:

```bash
cargo test -p stateset-core -- proptest
# 21+ property-based tests
```

### Snapshot Tests

Insta snapshot tests verify serialization stability:

```bash
cargo test -p stateset-core -- snapshot
# 8 serialization snapshots
```

### Benchmarks

Criterion benchmarks track performance regressions:

```bash
cargo bench -p stateset-core
cargo bench -p stateset-db
cargo bench -p stateset-embedded
```

### Quality Gates

- **0 clippy warnings** (`cargo clippy --workspace`)
- **0 doc errors** (`cargo doc --workspace --no-deps`)
- **deny(unwrap_used)** on all 6 core crates
- **`#[must_use]`** on all fallible functions

## CLI Tests

CLI tests use Node's built-in test runner (`node --test`), **not** Vitest.

### Running Tests

```bash
cd cli
node --test test/tools/orders.test.js
node --test test/a2a/quotes.test.js
node --test test/adapters/stripe.test.js
```

### Test Organization

```
cli/test/
├── tools/           # MCP tool handler tests (16 files, ~487 tests)
│   ├── orders.test.js
│   ├── payments.test.js
│   ├── inventory.test.js
│   └── ...
├── a2a/             # A2A protocol tests (~638 tests)
│   ├── quotes.test.js
│   ├── escrow.test.js
│   ├── splits.test.js
│   ├── subscriptions.test.js
│   └── ...
├── adapters/        # Platform adapter tests
│   ├── stripe.test.js
│   ├── woocommerce.test.js
│   └── shopify.test.js
└── x402/            # Payment protocol tests
    └── budget.test.js
```

### Test Patterns

Tests use in-memory SQLite databases for isolation:

```javascript
import { test, describe, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

describe('orders', () => {
    let store;
    beforeEach(() => {
        store = createTestStore(':memory:');
    });

    test('create order', () => {
        const order = store.createOrder({ ... });
        assert.equal(order.status, 'pending');
    });
});
```

## Admin Tests

The admin dashboard uses Vitest with jsdom:

```bash
cd admin
npx vitest
```

## Test Pyramid

```
         ╱╲
        ╱  ╲        E2E (5%)
       ╱    ╲       Multi-agent workflows, full CLI flows
      ╱──────╲
     ╱        ╲     Integration (25%)
    ╱          ╲    Cross-crate tests, adapter sync, A2A flows
   ╱────────────╲
  ╱              ╲  Unit (70%)
 ╱                ╲ Domain models, tool handlers, validators
╱──────────────────╲
```

## Coverage Gates

| Layer | Target | Tool |
|-------|--------|------|
| Rust workspace | ≥ 80% line coverage | `cargo tarpaulin` |
| CLI | ≥ 75% coverage | `node --experimental-test-coverage` |
| Admin | ≥ 70% coverage | Vitest coverage |

## State Machine Testing

Every domain aggregate has explicit state machine tests:

```javascript
// Order transitions
test('order: pending → processing → shipped → delivered', () => { ... });
test('order: pending → cancelled (valid)', () => { ... });
test('order: shipped → pending (invalid, throws)', () => { ... });

// Payment transitions
test('payment: pending → authorized → captured → settled', () => { ... });
test('payment: captured → refunded (partial)', () => { ... });

// A2A Quote transitions
test('quote: requested → quoted ⇄ counter_offered → accepted', () => { ... });
test('quote: quoted → expired (after timeout)', () => { ... });
```

## Concurrency Testing

```rust
#[test]
fn concurrent_inventory_reservations() {
    // 100 threads competing for the same SKU
    let handles: Vec<_> = (0..100).map(|_| {
        let commerce = commerce.clone();
        thread::spawn(move || commerce.inventory().reserve("SKU-001", 1, None))
    }).collect();

    let successes: usize = handles.into_iter()
        .filter(|h| h.join().unwrap().is_ok())
        .count();

    // Exactly 100 should succeed (100 units available)
    assert_eq!(successes, 100);
}
```

## CI Pipeline

Tests run on every commit across 10 job types:

| Job | What It Checks |
|-----|---------------|
| `fmt` | `cargo fmt --check` — consistent formatting |
| `clippy` | `cargo clippy --workspace` — 0 warnings |
| `audit` | `cargo audit` — no known vulnerabilities |
| `deny` | `cargo deny check` — license compliance |
| `rust` | `cargo test --workspace` — all Rust tests |
| `coverage` | `cargo tarpaulin` — coverage gates |
| `benchmarks` | `cargo bench` — no performance regressions |
| `cli` | `node --test cli/test/` — all CLI tests |
| `admin` | `npx vitest` — admin tests |
| `docs` | `cargo doc --workspace --no-deps` — doc build |

## Test Fixtures

The `stateset-test-utils` crate provides shared fixtures:

```rust
use stateset_test_utils::fixtures::{OrderFixture, CustomerFixture};

let customer = CustomerFixture::new().build();
let order = OrderFixture::new()
    .with_customer(customer.id)
    .with_items(3)
    .build();
```

Assertion macros for common patterns:

```rust
use stateset_test_utils::assertions::assert_state_transition;

assert_state_transition!(order, "pending" => "processing"); // passes
assert_state_transition!(order, "pending" => "delivered");   // fails with clear message
```
