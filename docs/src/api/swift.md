# Swift API Reference

The StateSet commerce engine (the Rust `stateset-embedded` crate) for Swift,
running in-process through a C ABI and persisting to a local SQLite file.

> **Status.** Not yet published as a release; build it from this repository
> (below). Before October 2026 this binding was an in-memory fake (its C shim
> was empty and no engine call was ever made, so data was silently lost); it
> is now a real binding with a smaller, honest surface (see
> [Coverage](#coverage)).

## How it works

```
Swift API (StateSetCommerce, CustomersAPI, ...)
   │  StateSetC module map + stateset.h, JSON with decimals as exact strings
   ▼
libstateset_swift  ──  re-exports crates/stateset-ffi (json_api.rs, crypto_api.rs)
   ▼
stateset-embedded (Rust)  ──  SQLite file (or :memory:)
```

Every method is one call to `stateset_json_call(handle, "orders.create",
"{...}")`, which dispatches to the engine in safe Rust and returns a JSON
envelope. The native side catches panics, reports typed error codes, and owns
every string it returns until `stateset_string_free`.

## Build and test

Requires Swift 5.7+ and a Rust toolchain.

```bash
cargo build -p stateset-swift --release
cd bindings/swift
LIB=$(pwd)/../../target/release
# Linux
LD_LIBRARY_PATH=$LIB swift test -Xlinker -L$LIB
# macOS
DYLD_LIBRARY_PATH=$LIB swift test -Xlinker -L$LIB
```

For an app, link the static `libstateset_swift.a` (or ship the dynamic
library) and add its directory to the linker search path.

## Quick start

```swift
import StateSet

let commerce = try StateSetCommerce(dbPath: "commerce.db")   // or ":memory:"
defer { commerce.close() }

let customer = try commerce.customers.create(email: "alice@example.com", firstName: "Alice", lastName: "Smith")
let product = try commerce.products.create(name: "Premium Widget", sku: "WIDGET-001",
                                           price: Decimal(string: "29.99")!)
_ = try commerce.inventory.createItem(sku: "WIDGET-001", name: "Premium Widget", initialQuantity: 100)

var order = try commerce.orders.create(customerId: customer.id, items: [
    CreateOrderItem(productId: product.id, sku: "WIDGET-001", name: "Widget", quantity: 2,
                    unitPrice: Decimal(string: "29.99")!),
])
// order.totalAmount == 59.98, exactly

order = try commerce.orders.updateStatus(id: order.id, status: .confirmed)
let payment = try commerce.payments.create(orderId: order.id, amount: order.totalAmount)
_ = try commerce.payments.complete(id: payment.id)
_ = try commerce.payments.refund(paymentId: payment.id, amount: 10, reason: "goodwill")
```

## Coverage

| API | Methods |
|---|---|
| `customers` | `create`, `get(id:)`, `get(email:)`, `update`, `list`, `count`, `delete` |
| `products` | `create` (single price or variants), `get(id:)`, `get(slug:)`, `list`, `count`, `search`, `activate`, `archive`, `delete`, `addVariant`, `variants`, `variant(sku:)` |
| `inventory` | `createItem`, `item(sku:)`, `list`, `getStock`, `adjust`, `hasStock`, `reserve`, `releaseReservation`, `confirmReservation` |
| `carts` | `create`, `get`, `list`, `addItem`, `updateItemQuantity`, `removeItem`, `items`, `clearItems`, `setShippingAddress`, `setBillingAddress`, `setShipping`, `setPayment`, `applyDiscount`, `recalculate`, `complete`, `cancel`, `abandon` |
| `orders` | `create`, `get(id:)`, `get(orderNumber:)`, `list`, `list(customerId:)`, `count`, `updateStatus`, `ship`, `deliver`, `cancel` |
| `payments` | `create`, `get`, `list`, `list(orderId:)`, `markProcessing`, `complete`, `fail`, `cancel`, `refund(paymentId:...)`, `refund(id:)`, `refunds`, `completeRefund`, `failRefund` |
| `returns` | `create`, `get`, `list`, `list(orderId:)`, `approve`, `reject`, `markReceived`, `complete`, `cancel`, `addTracking` |
| `shipments` | `create`, `get(id:)`, `get(trackingNumber:)`, `list`, `list(orderId:)`, `markProcessing`, `markReady`, `ship`, `markInTransit`, `deliver`, `cancel` |
| `Crypto` | `jcsCanonicalize`, `payloadPlainHash`, `merkleRoot` (checked against `bindings/test-vectors/v1.json`) |

Domains the old fake pretended to support (analytics, warranties, suppliers,
purchase orders, invoices, BOM, work orders, currency, subscriptions,
promotions, tax, quality, lots, serials, warehouse, receiving, fulfillment,
AP/AR, cost accounting, credit, backorders, general ledger) were removed
rather than kept as fakes. They exist in the engine and can be added by
routing them in `crates/stateset-ffi/src/json_api.rs` plus a typed wrapper
here.

## Money and errors

- Every money and quantity field is `Decimal`, decoded from and encoded as an
  exact decimal string (`"29.99"`) — never a `Double`. The native side refuses
  JSON floats.
- Lookups return `nil` when nothing matches. Everything else `throws` a
  `StateSetError` with a `code` (`.notFound`, `.invalidArgument`,
  `.databaseError`, ...), a `kind`, and a `message`.
- `StateSetCommerce` may be shared across threads; `close()` waits for
  in-flight calls and is idempotent.

Source: [`bindings/swift`](https://github.com/stateset/stateset-icommerce/tree/master/bindings/swift); native surface: [`crates/stateset-ffi/src/json_api.rs`](https://github.com/stateset/stateset-icommerce/blob/master/crates/stateset-ffi/src/json_api.rs).
