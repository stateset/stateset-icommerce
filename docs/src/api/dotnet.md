# C# / .NET API Reference

The StateSet commerce engine (the Rust `stateset-embedded` crate) for .NET,
running in-process over P/Invoke and persisting to a local SQLite file.

> **Status.** Not yet published to NuGet — build it from this repository
> (below). Before October 2026 this binding was an in-memory fake that never
> called the engine and silently discarded data; it is now a real binding with
> a smaller, honest surface (see [Coverage](#coverage)).

## How it works

```
C# API (StateSetCommerce, CustomersApi, ...)
   │  LibraryImport, JSON with decimals as exact strings
   ▼
libstateset_dotnet  ──  re-exports crates/stateset-ffi (json_api.rs, crypto_api.rs)
   ▼
stateset-embedded (Rust)  ──  SQLite file (or :memory:)
```

Every method is one call to `stateset_json_call(handle, "orders.create",
"{...}")`, which dispatches to the engine in safe Rust and returns a JSON
envelope. The native side catches panics, reports typed error codes, and owns
every string it returns until `stateset_string_free`. Nothing is cached in
managed memory: close the instance, reopen the file, and the data is there.

## Build and test

Requires the .NET 8 SDK and a Rust toolchain.

```bash
cargo build -p stateset-dotnet --release          # -> target/release/libstateset_dotnet.{so,dylib} / stateset_dotnet.dll
cd bindings/dotnet/tests
dotnet test                                       # the csproj copies the native lib next to the tests
```

The library is found next to the application, under
`runtimes/<rid>/native` in a package, on the OS loader path, or at the exact
path in the `STATESET_NATIVE_LIB` environment variable. Point the build at a
different native directory with `-p:StateSetNativeDir=/path/to/dir/`.

## Quick start

```csharp
using StateSet.Embedded;

using var commerce = new StateSetCommerce("commerce.db");   // or ":memory:"

var customer = commerce.Customers.Create("alice@example.com", "Alice", "Smith");

var product = commerce.Products.Create("Premium Widget", sku: "WIDGET-001", price: 29.99m);
commerce.Inventory.CreateItem("WIDGET-001", "Premium Widget", initialQuantity: 100m);

var order = commerce.Orders.Create(customer.Id, new[]
{
    new CreateOrderItem { ProductId = product.Id, Sku = "WIDGET-001", Name = "Widget", Quantity = 2, UnitPrice = 29.99m },
});
// order.TotalAmount == 59.98m, exactly

order = commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);
var payment = commerce.Payments.Create(order.Id, order.TotalAmount);
commerce.Payments.Complete(payment.Id);
commerce.Payments.Refund(payment.Id, 10.00m, "goodwill");
```

Checkout from a cart:

```csharp
var cart = commerce.Carts.Create(customerId: customer.Id, customerEmail: customer.Email);
commerce.Carts.AddItem(cart.Id, "WIDGET-001", "Widget", quantity: 1, unitPrice: 29.99m);
commerce.Carts.SetShipping(cart.Id, new CartAddress
{
    FirstName = "Alice", LastName = "Smith", Line1 = "1 Main St",
    City = "Austin", State = "TX", PostalCode = "78701", Country = "US",
}, method: "standard", amount: 5.00m);
commerce.Carts.SetPayment(cart.Id, "credit_card", "tok_test");
CheckoutResult result = commerce.Carts.Complete(cart.Id);
```

## Coverage

| API | Methods |
|---|---|
| `Customers` | `Create`, `Get`, `GetByEmail`, `Update`, `List`, `Count`, `Delete` |
| `Products` | `Create` (single price or variants), `Get`, `GetBySlug`, `Update`, `List`, `Count`, `Search`, `Activate`, `Archive`, `Delete`, `AddVariant`, `GetVariants`, `GetVariantBySku` |
| `Inventory` | `CreateItem`, `GetItem`, `List`, `GetStock`, `Adjust`, `HasStock`, `Reserve`, `ReleaseReservation`, `ConfirmReservation`, `GetReservation` |
| `Carts` | `Create`, `Get`, `List`, `AddItem`, `UpdateItemQuantity`, `RemoveItem`, `GetItems`, `ClearItems`, `SetShippingAddress`, `SetBillingAddress`, `SetShipping`, `SetPayment`, `ApplyDiscount`, `RemoveDiscount`, `Recalculate`, `MarkReadyForPayment`, `BeginCheckout`, `Complete`, `Cancel`, `Abandon` |
| `Orders` | `Create`, `Get`, `GetByNumber`, `List`, `ListForCustomer`, `Count`, `UpdateStatus`, `Ship`, `Deliver`, `Cancel` |
| `Payments` | `Create`, `Get`, `List`, `ForOrder`, `MarkProcessing`, `Complete`, `Fail`, `Cancel`, `Refund`, `GetRefund`, `GetRefunds`, `CompleteRefund`, `FailRefund` |
| `Returns` | `Create`, `Get`, `List`, `ListForOrder`, `Approve`, `Reject`, `MarkReceived`, `Complete`, `Cancel`, `AddTracking` |
| `Shipments` | `Create`, `Get`, `GetByTracking`, `List`, `ForOrder`, `MarkProcessing`, `MarkReady`, `Ship`, `MarkInTransit`, `Deliver`, `Cancel` |
| `Crypto` | `JcsCanonicalize`, `PayloadPlainHash`, `MerkleRoot` (checked against `bindings/test-vectors/v1.json`) |

Domains the old fake pretended to support (analytics, warranties, suppliers,
purchase orders, invoices, BOM, work orders, currency, subscriptions,
promotions, tax, quality, lots, serials, warehouse, receiving, fulfillment,
AP/AR, cost accounting, credit, backorders, general ledger) were removed
rather than kept as fakes. They exist in the engine and can be added by
routing them in `crates/stateset-ffi/src/json_api.rs` plus a typed wrapper
here.

## Money and errors

- Every money and quantity field is `decimal`, and crosses the boundary as an
  exact decimal string (`"29.99"`). The native side refuses JSON floats.
- Lookups (`Get*`) return `null` when nothing matches. Everything else throws:
  `StateSetNotFoundException` (missing entity), `StateSetValidationException`
  (bad argument, refused state transition, insufficient stock, ...), or
  `StateSetException` with a `Code` (`StateSetErrorCode`) and `Kind`.
- `StateSetCommerce` is thread-safe; `Dispose` waits for in-flight calls.

Targets .NET 8 (`net8.0`); .NET 6 and 7 are out of support.

Source: [`bindings/dotnet`](https://github.com/stateset/stateset-icommerce/tree/master/bindings/dotnet); native surface: [`crates/stateset-ffi/src/json_api.rs`](https://github.com/stateset/stateset-icommerce/blob/master/crates/stateset-ffi/src/json_api.rs).
