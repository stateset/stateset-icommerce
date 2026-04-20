# API Reference

The supported bindings expose the same core commerce surfaces with consistent behavior. The Rust crates are the source of truth, and repo-local bindings package that functionality for different runtimes.

## API Design Contract

| Property | Guarantee |
|----------|-----------|
| **Naming** | APIs use language-idiomatic conventions (snake_case in Rust/Python/Ruby, camelCase in JS/Kotlin, PascalCase in C#/Go) |
| **Parity** | Every binding targets the same core commerce surfaces |
| **Error shape** | All errors are typed: `NotFound`, `InvalidState`, `Validation`, `Conflict`, `Database` |
| **Idempotency** | Create operations with duplicate IDs return the existing entity |
| **Pagination** | List operations support `limit` and `offset` parameters |
| **Null safety** | Optional fields are explicitly typed (Rust `Option<T>`, TypeScript `T \| null`, Python `Optional[T]`) |

## Common APIs

All bindings expose these domain surfaces:

| Category | APIs |
|----------|------|
| **Core Commerce** | customers, products, orders, inventory, carts, payments, returns, shipments |
| **Billing** | subscriptions, invoices, promotions, tax, currency |
| **Supply Chain** | suppliers, purchase_orders, bom, work_orders, receiving, fulfillment, warehouse |
| **Financial** | accounts_payable, accounts_receivable, cost_accounting, credit, general_ledger |
| **Tracking** | warranties, quality, lots, serials, backorders |
| **Engagement** | gift_cards, store_credits, loyalty, reviews, wishlists, segments, shipping_zones |
| **Intelligence** | analytics, fraud, custom_objects |

## Choosing a Binding

| Use Case | Recommended Binding |
|----------|-------------------|
| AI agent / CLI tool | Node.js or Rust |
| Web server (high concurrency) | Rust (`AsyncCommerce` + PostgreSQL) |
| Data science / ML pipeline | Python |
| WordPress / Laravel integration | PHP |
| Mobile (iOS) | Swift |
| Mobile (Android) | Kotlin |
| Microservice (cloud-native) | Go |
| Enterprise (JVM) | Java or Kotlin |
| Desktop (.NET) | C# |
| Browser / Edge / Workers | WASM |

## Bindings

- [Rust](rust.md) — Primary, synchronous + async
- [Node.js](node.md) — NAPI-RS, ES modules
- [Python](python.md) — PyO3, type stubs
- [Ruby](ruby.md) — Magnus
- [PHP](php.md) — ext-php-rs
- [Java](java.md) — JNI
- [Kotlin](kotlin.md) — JNI + coroutines
- [Swift](swift.md) — C FFI
- [C# / .NET](dotnet.md) — P/Invoke
- [Go](go.md) — cgo
- [WASM](wasm.md) — wasm-bindgen

For the current binding set and direct dependency topology, see [Workspace Inventory](../appendix/workspace-inventory.md).
