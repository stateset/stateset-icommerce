#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - Ruby Example
#
# Demonstrates the full commerce workflow:
# - Customer creation
# - Product catalog management
# - Inventory tracking
# - Order processing
# - Analytics
#
# Run with: ruby basic_usage.rb

require 'stateset_embedded'

puts "=== StateSet iCommerce - Ruby Example ==="
puts

# Initialize commerce with in-memory database
commerce = StateSet::Commerce.new(':memory:')
puts "✓ Commerce initialized"
puts

# 1. Create a customer
puts "1. Creating customer..."
customer = commerce.customers.create(
  email: 'alice@example.com',
  first_name: 'Alice',
  last_name: 'Smith',
  phone: '+1-555-0123'
)
puts "   Created customer: #{customer.first_name} #{customer.last_name} (#{customer.email})"

# 2. Create products
puts
puts "2. Creating products..."
widget = commerce.products.create(
  name: 'Premium Widget',
  sku: 'WIDGET-001',
  price: 29.99,
  description: 'A high-quality widget for all your needs'
)
puts "   Created product: #{widget.name} (#{widget.slug})"

gadget = commerce.products.create(
  name: 'Super Gadget',
  sku: 'GADGET-001',
  price: 49.99,
  description: 'An amazing gadget'
)
puts "   Created product: #{gadget.name} (#{gadget.slug})"

# 3. Create inventory
puts
puts "3. Setting up inventory..."
commerce.inventory.create_item(
  sku: 'WIDGET-001',
  name: 'Premium Widget',
  initial_quantity: 100
)
puts "   Created inventory for WIDGET-001 (100 units)"

commerce.inventory.create_item(
  sku: 'GADGET-001',
  name: 'Super Gadget',
  initial_quantity: 50
)
puts "   Created inventory for GADGET-001 (50 units)"

# Check stock
widget_stock = commerce.inventory.get_stock('WIDGET-001')
puts "   Stock check WIDGET-001: #{widget_stock.total_available} available"

# 4. Create an order
puts
puts "4. Creating order..."
order = commerce.orders.create(
  customer_id: customer.id,
  items: [
    {
      product_id: widget.id,
      sku: 'WIDGET-001',
      name: 'Premium Widget',
      quantity: 2,
      unit_price: '29.99'
    },
    {
      product_id: gadget.id,
      sku: 'GADGET-001',
      name: 'Super Gadget',
      quantity: 1,
      unit_price: '49.99'
    }
  ],
  currency: 'USD'
)
puts "   Created order #{order.order_number} (total: $#{order.total_amount})"

# 5. Process the order
puts
puts "5. Processing order..."

# Update order status
order = commerce.orders.update_status(order.id, :confirmed)
puts "   Order status: #{order.status}"

# Adjust inventory (fulfill)
commerce.inventory.adjust('WIDGET-001', -2, 'Order fulfillment')
commerce.inventory.adjust('GADGET-001', -1, 'Order fulfillment')
puts "   Inventory adjusted"

# Ship the order
order = commerce.orders.ship(order.id, 'TRACK123456')
puts "   Order shipped with tracking: #{order.tracking_number}"

# 6. Check final inventory
puts
puts "6. Final inventory check..."
final_widget_stock = commerce.inventory.get_stock('WIDGET-001')
puts "   WIDGET-001: #{final_widget_stock.total_available} available (was 100)"

final_gadget_stock = commerce.inventory.get_stock('GADGET-001')
puts "   GADGET-001: #{final_gadget_stock.total_available} available (was 50)"

# 7. Analytics
puts
puts "7. Analytics..."
sales_summary = commerce.analytics.sales_summary(period: :today)
puts "   Revenue: $#{sales_summary.total_revenue}"
puts "   Orders: #{sales_summary.order_count}"
puts "   AOV: $#{sales_summary.average_order_value}"

# Get top products
top_products = commerce.analytics.top_products(limit: 5)
puts "   Top products: #{top_products.size}"

# 8. Subscriptions demo
puts
puts "8. Subscriptions..."
plan = commerce.subscriptions.create_plan(
  name: 'Pro Monthly',
  price: 29.99,
  interval: :month,
  description: 'Professional features with monthly billing'
)
puts "   Created plan: #{plan.name} ($#{plan.price}/#{plan.interval})"

subscription = commerce.subscriptions.subscribe(
  plan_id: plan.id,
  customer_id: customer.id
)
puts "   Customer subscribed to #{plan.name}"

# 9. Summary
puts
puts "=== Summary ==="
customers = commerce.customers.list
products = commerce.products.list
orders = commerce.orders.list
puts "Customers: #{customers.size}"
puts "Products: #{products.size}"
puts "Orders: #{orders.size}"

puts
puts "✓ Example completed successfully!"
