# Ruby API Reference

The Ruby gem `stateset_embedded` is a native binding (magnus / rb-sys) to the
Rust engine `stateset-embedded`. Every call runs in the real engine inside
your process and persists to the SQLite file you open.

> **Changed in the next release:** earlier versions of this gem were an
> in-memory stand-in that ignored `db_path`, so nothing was ever saved. The
> gem now links the engine, persists to SQLite, returns `BigDecimal` money,
> and exposes only the engine-backed domains listed below.

## Installation

```bash
gem install stateset_embedded
# or in Gemfile
gem 'stateset_embedded'
```

Ruby 3.0+. Building from source needs a Rust toolchain.

## Quick Start

```ruby
require 'stateset_embedded'

commerce = StateSet::Commerce.new("commerce.db")  # SQLite file
# commerce = StateSet::Commerce.new(":memory:")   # ephemeral store

customer = commerce.customers.create(
  email: "alice@example.com", first_name: "Alice", last_name: "Smith"
)

product = commerce.products.create(
  name: "Premium Widget",
  description: "High-quality widget",
  variants: [{ sku: "WIDGET-001", price: "29.99" }]
)

commerce.inventory.create_item(sku: "WIDGET-001", name: "Premium Widget", initial_quantity: 100)

order = commerce.orders.create(
  customer_id: customer.id,
  currency: "USD",
  items: [{ product_id: product.id, sku: "WIDGET-001", name: "Premium Widget",
            quantity: 2, unit_price: BigDecimal("29.99") }]
)
order.total_amount # => BigDecimal("59.98")

shipped = commerce.orders.ship(order.id, tracking_number: "1Z999")
puts "Order #{shipped.order_number} is #{shipped.status}"
```

## Conventions

- **Keywords mirror the engine.** `create`/`update` take the engine input's
  fields as keywords; `list`/`count` take the engine filter's fields
  (`orders.list(status: "pending", limit: 20)`). Unknown keywords raise
  `StateSet::ValidationError` instead of being silently dropped.
- **Money is exact.** Amounts and quantities come back as `BigDecimal`. Pass
  `BigDecimal`, `Integer` or a decimal `String`; a `Float` raises
  `StateSet::ValidationError` (code `binding.float_refused`).
- **Records** (`StateSet::Order`, `Payment`, ...) are frozen value objects:
  every engine field is a reader, plus `record[:field]` and `record.to_h`.
  Enums are snake_case strings; timestamps are `Time`.
- `get`-style methods return `nil` when nothing matches; a state change on a
  missing record raises `StateSet::NotFoundError`.

## Common Operations

### Customers

```ruby
customer = commerce.customers.create(email: "test@example.com", first_name: "Test", last_name: "User")
commerce.customers.get(customer.id)
commerce.customers.get_by_email("test@example.com")
commerce.customers.update(customer.id, phone: "+1-555-0100")
commerce.customers.list(limit: 50)
commerce.customers.delete(customer.id)  # => true
```

### Inventory

```ruby
commerce.inventory.create_item(sku: "SKU-001", name: "Widget", initial_quantity: 100)
commerce.inventory.adjust("SKU-001", 50, "Received shipment")
reservation = commerce.inventory.reserve("SKU-001", 10, reference_type: "order", reference_id: "ORD-1")
commerce.inventory.release_reservation(reservation.id)
commerce.inventory.get_stock("SKU-001").total_available  # => BigDecimal
```

### Carts and checkout

```ruby
cart = commerce.carts.create(customer_id: customer.id, currency: "USD")
commerce.carts.add_item(cart.id, sku: "SKU-001", name: "Widget", quantity: 2, unit_price: "29.99")
commerce.carts.set_shipping(cart.id, address: {
  first_name: "Alice", last_name: "Smith", line1: "1 Main St",
  city: "Springfield", postal_code: "12345", country: "US"
})
commerce.carts.set_payment(cart.id, payment_method: "credit_card")
commerce.carts.mark_ready_for_payment(cart.id)
commerce.carts.begin_checkout(cart.id)
result = commerce.carts.complete(cart.id)  # order is Confirmed, payment Pending
commerce.payments.create(order_id: result.order_id, amount: result.total_charged, payment_method: "credit_card")
```

### Orders, payments, refunds

```ruby
commerce.orders.update_status(order.id, "processing")
commerce.orders.ship(order.id, lines: [{ order_item_id: order.items.first.id, quantity: 1 }])  # partial
commerce.orders.deliver(order.id)

payment = commerce.payments.create(order_id: order.id, amount: order.total_amount, payment_method: "credit_card")
commerce.payments.mark_completed(payment.id)
refund = commerce.payments.create_refund(payment_id: payment.id, amount: "10.00", reason: "damaged")
commerce.payments.complete_refund(refund.id)
```

### Returns and shipments

```ruby
ret = commerce.returns.create(order_id: order.id, reason: "defective",
                              items: [{ order_item_id: order.items.first.id, quantity: 1 }])
commerce.returns.approve(ret.id)
commerce.returns.mark_received(ret.id)
commerce.returns.set_item_disposition(ret.id, ret.items.first.id, disposition: "restock")
commerce.returns.complete(ret.id)

shipment = commerce.shipments.create(order_id: order.id, recipient_name: "Alice Smith",
                                     shipping_address: "1 Main St, Springfield", carrier: "ups")
commerce.shipments.mark_processing(shipment.id)
commerce.shipments.mark_ready(shipment.id)
commerce.shipments.ship(shipment.id, tracking_number: "1Z999")
commerce.shipments.mark_in_transit(shipment.id)
commerce.shipments.mark_out_for_delivery(shipment.id)
commerce.shipments.mark_delivered(shipment.id)
```

## Error Handling

```ruby
begin
  commerce.payments.create_refund(payment_id: payment.id, amount: "9999.00")
rescue StateSet::ValidationError => e
  e.code    # => "commerce.refund.exceeds_captured"
  e.status  # => 400
rescue StateSet::Error => e
  # NotFoundError, ConflictError, InvalidOperationError (incl. InsufficientStockError),
  # NotPermittedError, DatabaseError, ExternalServiceError, InternalError
  raise
end
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers` | Customer accounts |
| `products` | Product catalog and variants |
| `inventory` | Stock items, balances, reservations |
| `carts` | Shopping carts and checkout |
| `orders` | Order lifecycle, (partial) shipping |
| `payments` | Payments and refunds |
| `returns` | Return requests |
| `shipments` | Outbound shipments |

`StateSet::Crypto` provides the cross-binding primitives (`jcs_canonicalize`,
`payload_plain_hash`, `merkle_root`). Other engine domains (finance,
subscriptions, promotions, manufacturing, ...) are not exposed in Ruby yet;
use the Python or Node binding, the CLI, or the HTTP server for those.

## Source Files

- Entry point: `StateSet::Commerce` (`bindings/ruby/lib/stateset_embedded/commerce.rb`)
- Ruby API: `bindings/ruby/lib/stateset_embedded/apis.rb`
- Engine dispatcher: `bindings/ruby/src/dispatch.rs`

## Examples

- `examples/ruby/basic_usage.rb`
- `examples/ruby/carts_example.rb`
- `examples/ruby/payments_example.rb`
- `examples/ruby/returns_example.rb`
