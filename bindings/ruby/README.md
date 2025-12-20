# StateSet Embedded Commerce for Ruby

[![Gem Version](https://badge.fury.io/rb/stateset_embedded.svg)](https://badge.fury.io/rb/stateset_embedded)

Local-first commerce engine for Ruby. Provides a complete commerce API with embedded SQLite storage.

## Installation

Add to your Gemfile:

```ruby
gem 'stateset_embedded'
```

Or install directly:

```bash
gem install stateset_embedded
```

### Requirements

- Ruby 3.0+
- Rust toolchain (for building from source)

## Quick Start

```ruby
require 'stateset_embedded'

# Create commerce instance (uses SQLite)
commerce = StateSet::Commerce.new('commerce.db')
# Or use in-memory database
commerce = StateSet::Commerce.new(':memory:')

# Create a customer
customer = commerce.customers.create(
  email: 'john@example.com',
  first_name: 'John',
  last_name: 'Doe'
)

# Create a product
product = commerce.products.create(
  name: 'Widget Pro',
  description: 'The best widget ever'
)

# Create an order
order = commerce.orders.create(
  customer_id: customer.id,
  items: [
    { sku: 'WIDGET-001', name: 'Widget Pro', quantity: 2, unit_price: 29.99 }
  ]
)

# Process the order
commerce.orders.confirm(order.id)
commerce.orders.ship(order.id, tracking_number: '1Z999AA10123456784')
commerce.orders.deliver(order.id)
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers` | Customer management |
| `orders` | Order processing |
| `products` | Product catalog |
| `inventory` | Stock management |
| `returns` | Return requests |
| `payments` | Payment recording |
| `carts` | Shopping carts |
| `analytics` | Sales reports |
| `shipments` | Shipment tracking |
| `warranties` | Warranty management |
| `purchase_orders` | Supplier orders |
| `invoices` | Invoice management |
| `bom` | Bills of materials |
| `work_orders` | Manufacturing |
| `currency` | Currency conversion |
| `subscriptions` | Recurring billing |
| `promotions` | Discount codes |
| `tax` | Tax calculations |

## Building from Source

```bash
cd bindings/ruby
bundle install
bundle exec rake compile
```

## Running Tests

```bash
bundle exec rake spec
```

## Publishing

```bash
# Build native gems for all platforms
bundle exec rake native gem

# Or use rb-sys-dock for cross-compilation
rb-sys-dock --platform x86_64-linux -- bundle exec rake native gem

# Publish to RubyGems
gem push pkg/stateset_embedded-*.gem
```

## License

MIT
