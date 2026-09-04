# Ruby API Reference

The Ruby binding provides `StateSet::Commerce` for building commerce applications.

## Installation

```bash
gem install stateset_embedded
# or in Gemfile
gem 'stateset_embedded'
```

## Quick Start

```ruby
require 'stateset_embedded'

# Initialize with SQLite database
commerce = StateSet::Commerce.new("commerce.db")

# Or use in-memory database for testing
commerce = StateSet::Commerce.new(":memory:")

# Create a customer
customer = commerce.customers.create(
  email: "alice@example.com",
  first_name: "Alice",
  last_name: "Smith",
  phone: "+1-555-0123"
)

# Create a product
product = commerce.products.create(
  name: "Premium Widget",
  sku: "WIDGET-001",
  price: 29.99,
  description: "High-quality widget"
)

# Create inventory
item = commerce.inventory.create_item(
  sku: "WIDGET-001",
  name: "Premium Widget",
  initial_quantity: 100
)

# Create an order
order = commerce.orders.create(
  customer_id: customer.id,
  items: [
    { sku: "WIDGET-001", name: "Widget", quantity: 2, unit_price: 29.99 }
  ],
  currency: "USD"
)

# Ship the order
shipped = commerce.orders.ship(order.id)
puts "Order #{shipped.order_number} shipped!"
```

## Common Operations

### Customer Management

```ruby
# Create customer
customer = commerce.customers.create(
  email: "test@example.com",
  first_name: "Test",
  last_name: "User"
)

# Get customer by ID
found = commerce.customers.get(customer_id)

# List all customers
customers = commerce.customers.list

# Delete customer
deleted = commerce.customers.delete(customer_id)
```

### Inventory Management

```ruby
# Create inventory item
item = commerce.inventory.create_item(
  sku: "SKU-001",
  name: "Widget",
  initial_quantity: 100
)

# Adjust inventory
commerce.inventory.adjust("SKU-001", 50, "Received shipment")

# Reserve inventory
reservation = commerce.inventory.reserve("SKU-001", 10)

# Release reservation
commerce.inventory.release(reservation.id)

# Get stock level
level = commerce.inventory.get_level("SKU-001")
puts "Available: #{level.available}"
```

### Order Processing

```ruby
# Create order
order = commerce.orders.create(
  customer_id: customer.id,
  items: [
    { sku: "SKU-001", name: "Widget", quantity: 2, unit_price: 29.99 }
  ]
)

# Update status
commerce.orders.update_status(order.id, "processing")

# Ship order
shipped = commerce.orders.ship(order.id)

# Cancel order
cancelled = commerce.orders.cancel(order.id)

# List orders by status
pending = commerce.orders.list_by_status("pending")
```

### Subscriptions

```ruby
# Create a subscription plan
plan = commerce.subscriptions.create_plan(
  code: "PREMIUM",
  name: "Premium Plan",
  interval: "month",
  interval_count: 1,
  price: 19.99,
  currency: "USD"
)

# Subscribe a customer
subscription = commerce.subscriptions.subscribe(customer.id, plan.id)

# Pause/Resume/Cancel
paused = commerce.subscriptions.pause(subscription.id)
resumed = commerce.subscriptions.resume(subscription.id)
cancelled = commerce.subscriptions.cancel(subscription.id)
```

### Analytics

```ruby
# Get sales summary
summary = commerce.analytics.sales_summary
puts "Total revenue: #{summary.total_revenue}"

# Get top products
top_products = commerce.analytics.top_products(10)

# Get top customers
top_customers = commerce.analytics.top_customers(10)
```

## Error Handling

```ruby
begin
  order = commerce.orders.ship(order_id)
rescue StateSet::Error => e
  puts "StateSet error: #{e.message}"
rescue => e
  raise e
end
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers` | Customer management |
| `products` | Product catalog |
| `orders` | Order lifecycle |
| `inventory` | Stock management |
| `carts` | Shopping carts |
| `returns` | Return processing |
| `payments` | Payment operations |
| `shipments` | Shipping management |
| `warranties` | Warranty tracking |
| `suppliers` | Supplier management |
| `purchase_orders` | Purchase orders |
| `invoices` | B2B invoicing |
| `bom` | Bills of Materials |
| `work_orders` | Manufacturing |
| `currency` | Multi-currency |
| `subscriptions` | Recurring billing |
| `promotions` | Discounts & coupons |
| `tax` | Tax calculations |
| `quality` | Quality control |
| `lots` | Lot tracking |
| `serials` | Serial numbers |
| `warehouse` | Warehouse ops |
| `receiving` | Receiving |
| `fulfillment` | Picking & packing |
| `accounts_payable` | A/P management |
| `accounts_receivable` | A/R management |
| `cost_accounting` | Cost tracking |
| `credit` | Credit management |
| `backorders` | Backorder tracking |
| `general_ledger` | GL accounting |
| `analytics` | Reporting & forecasts |

## Source Files

- Entry point: `StateSet::Commerce`
- Ruby API wrapper: `bindings/ruby/lib/stateset_embedded.rb`

## Examples

- `examples/ruby/basic_usage.rb`
