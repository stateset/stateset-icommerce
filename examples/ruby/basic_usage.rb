#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - Ruby basic usage
#
# Customers, catalog, inventory and an order, persisted to a SQLite file and
# read back from a second Commerce instance.
#
# Run with: ruby basic_usage.rb [path/to/store.db]

require 'stateset_embedded'
require 'tmpdir'

db_path = ARGV[0] || File.join(Dir.mktmpdir('stateset-example'), 'store.db')
commerce = StateSet::Commerce.new(db_path)
puts "Opened #{db_path}"

customer = commerce.customers.create(
  email: "alice-#{Time.now.to_i}@example.com", first_name: 'Alice', last_name: 'Smith',
  phone: '+1-555-0123'
)
puts "Customer: #{customer.full_name} <#{customer.email}>"

widget = commerce.products.create(
  name: 'Premium Widget',
  description: 'A high-quality widget for all your needs',
  variants: [{ sku: "WIDGET-#{customer.id[0, 8]}", price: '29.99' }]
)
widget = commerce.products.activate(widget.id) # new products start as drafts
variant = commerce.products.get_variants(widget.id).first
puts "Product: #{widget.name} (#{variant.sku} @ #{variant.price.to_s('F')})"

commerce.inventory.create_item(sku: variant.sku, name: widget.name, initial_quantity: 100)
commerce.inventory.adjust(variant.sku, -3, 'cycle count correction')
puts "Stock on hand: #{commerce.inventory.get_stock(variant.sku).total_on_hand.to_s('F')}"

order = commerce.orders.create(
  customer_id: customer.id,
  currency: 'USD',
  items: [{ product_id: widget.id, sku: variant.sku, name: widget.name,
            quantity: 2, unit_price: variant.price }]
)
puts "Order #{order.order_number}: #{order.status}, total #{order.total_amount.to_s('F')} #{order.currency}"

commerce.orders.ship(order.id, tracking_number: '1Z999AA10123456784')
delivered = commerce.orders.deliver(order.id)
puts "Order is now #{delivered.status}"

# A second instance on the same file sees everything the first one wrote.
reread = StateSet::Commerce.new(db_path).orders.get(order.id)
puts "Reopened store: order #{reread.order_number} is #{reread.status}"
