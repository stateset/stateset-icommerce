# StateSet Embedded Commerce for Ruby

A native Ruby binding to the StateSet commerce engine (the Rust
`stateset-embedded` crate). Every call runs in the real engine inside your
process and persists to a local SQLite file -- no server, no API keys.

```ruby
gem 'stateset_embedded'
```

Requirements: Ruby 3.0+. Installing from source needs a Rust toolchain
(the extension is built with [rb-sys](https://github.com/oxidize-rb/rb-sys)).

## Quick start

```ruby
require 'stateset_embedded'

commerce = StateSet::Commerce.new('store.db')   # SQLite file, created if missing
# commerce = StateSet::Commerce.new(':memory:') # ephemeral store

customer = commerce.customers.create(
  email: 'ada@example.com', first_name: 'Ada', last_name: 'Lovelace'
)

product = commerce.products.create(
  name: 'Widget Pro',
  variants: [{ sku: 'WIDGET-001', price: '29.99' }]
)
commerce.inventory.create_item(sku: 'WIDGET-001', name: 'Widget Pro', initial_quantity: 100)

order = commerce.orders.create(
  customer_id: customer.id,
  items: [{ product_id: product.id, sku: 'WIDGET-001', name: 'Widget Pro',
            quantity: 2, unit_price: BigDecimal('29.99') }]
)
order.total_amount # => 0.5998e2 (BigDecimal, exactly 59.98)

payment = commerce.payments.create(order_id: order.id, amount: order.total_amount,
                                   payment_method: 'credit_card')
commerce.payments.mark_completed(payment.id)

commerce.orders.ship(order.id, tracking_number: '1Z999AA10123456784')
commerce.orders.deliver(order.id)
```

Reopen the same file later -- in this process or another -- and the data is
there: `StateSet::Commerce.new('store.db').orders.get(order.id)`.

## Money is exact

Amounts and quantities are `BigDecimal` on every record the engine returns.
On the way in, pass `BigDecimal`, `Integer`, or a decimal `String`.
**`Float` is refused** with `StateSet::ValidationError` (code
`binding.float_refused`): `0.1 + 0.2` is not `0.3` in binary floating point,
and a commerce engine must not silently take a rounded amount. Amounts must
also fit the currency's scale (`0.005` is not a USD amount).

## API

`StateSet::Commerce` exposes eight engine-backed domains. Create/update
methods take the engine input's fields as keywords; list/count take the
engine filter's fields as keywords (`orders.list(status: 'pending', limit: 20)`).
Unknown keywords raise `StateSet::ValidationError` rather than being dropped.

| Domain | Methods |
|---|---|
| `customers` | `create`, `get`, `get_by_email`, `update`, `list`, `count`, `delete` |
| `products` | `create`, `get`, `get_by_slug`, `update`, `list`, `count`, `search`, `delete`, `activate`, `archive`, `add_variant`, `get_variant`, `get_variant_by_sku`, `get_variants` |
| `inventory` | `create_item`, `get_item`, `get_item_by_sku`, `get_stock`, `list`, `adjust`, `reserve`, `get_reservation`, `confirm_reservation`, `release_reservation`, `get_transactions`, `has_stock?` |
| `carts` | `create`, `get`, `get_by_number`, `list`, `count`, `for_customer`, `delete`, `add_item`, `update_item`, `remove_item`, `get_items`, `clear_items`, `set_shipping_address`, `set_billing_address`, `set_shipping`, `get_shipping_rates`, `set_payment`, `set_tax`, `apply_discount`, `remove_discount`, `recalculate`, `reserve_inventory`, `release_inventory`, `mark_ready_for_payment`, `begin_checkout`, `complete`, `cancel`, `abandon` |
| `orders` | `create`, `get`, `get_by_number`, `list`, `count`, `list_for_customer`, `update_status`, `cancel`, `ship` (optionally partial via `lines:`), `deliver` |
| `payments` | `create`, `get`, `get_by_number`, `list`, `count`, `for_order`, `mark_processing`, `mark_completed` (alias `complete`), `mark_failed`, `cancel`, `create_refund`, `get_refund`, `get_refunds`, `complete_refund`, `fail_refund` |
| `returns` | `create`, `get`, `list`, `count`, `list_for_order`, `approve`, `reject`, `mark_received`, `set_item_disposition`, `complete`, `cancel`, `add_tracking` |
| `shipments` | `create`, `get`, `get_by_number`, `get_by_tracking`, `list`, `count`, `for_order`, `get_items`, `add_item`, `remove_item`, `mark_processing`, `mark_ready`, `ship`, `mark_in_transit`, `mark_out_for_delivery`, `mark_delivered`, `mark_failed`, `hold`, `cancel` |

Records (`StateSet::Customer`, `Order`, `Payment`, ...) are immutable value
objects: every engine field is a reader (`order.status`), also available as
`order[:status]` and `order.to_h`. Enum values are snake_case strings
(`'pending'`, `'credit_card'`); timestamps are `Time`. `get`-style methods
return `nil` when nothing matches.

`StateSet::Crypto` exposes the cross-binding primitives
(`jcs_canonicalize`, `payload_plain_hash`, `merkle_root`) verified against
`bindings/test-vectors/v1.json`.

## Errors

Every engine failure raises a subclass of `StateSet::Error`, which carries
`code` (the engine invariant code such as `commerce.refund.exceeds_captured`,
or the engine error variant) and `status` (HTTP-style):

| Class | When |
|---|---|
| `NotFoundError` | the record does not exist |
| `ValidationError` | malformed input, money-scale/invariant violations, unknown keywords, Float money |
| `ConflictError` | duplicate email/SKU/slug, concurrent modification |
| `InvalidOperationError` | refused in the current state (bad status transition, ...) |
| `InsufficientStockError` | (a kind of `InvalidOperationError`) not enough available stock |
| `NotPermittedError`, `DatabaseError`, `ExternalServiceError`, `InternalError` | as named; a panic inside the engine is caught and raised as `InternalError` |

## Building and testing

```bash
cd bindings/ruby
bundle install
bundle exec rake          # compile the extension, then run the specs
cargo test --lib          # the engine dispatcher alone, no Ruby needed
```

The specs run against the real engine; `spec/persistence_spec.rb` writes
through one `Commerce`, reopens the SQLite file (in-process and from a
separate Ruby process) and reads everything back.

## How it works

`lib/` holds the Ruby API. Each method sends one named operation plus JSON
arguments to the native extension (`src/runtime.rs`, magnus), which hands it
to `src/dispatch.rs`: that deserializes into the engine's own input types,
calls `stateset_embedded::Commerce`, and returns the engine's own output
types as JSON. Decimals cross as exact strings.

## License

MIT OR Apache-2.0
