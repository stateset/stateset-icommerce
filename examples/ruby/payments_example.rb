#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - Payments Example
#
# Demonstrates payment recording workflow:
# - Creating orders
# - Recording payments with different methods
# - Tracking order payment status
# - Partial payments
#
# Run with: ruby payments_example.rb

require 'stateset_embedded'

puts "=== StateSet iCommerce - Payments Example ==="
puts

# Initialize commerce with in-memory database
commerce = StateSet::Commerce.new(':memory:')
puts "✓ Commerce initialized"
puts

# 1. Create customers
puts "1. Creating customers..."
customer1 = commerce.customers.create(
  email: 'alice@example.com',
  first_name: 'Alice',
  last_name: 'Johnson',
  phone: '+1-555-0100'
)
puts "   Created: #{customer1.full_name}"

customer2 = commerce.customers.create(
  email: 'bob@example.com',
  first_name: 'Bob',
  last_name: 'Smith',
  phone: '+1-555-0200'
)
puts "   Created: #{customer2.full_name}"

# 2. Create products
puts
puts "2. Creating products..."
monitor = commerce.products.create(
  name: '4K Monitor 27"',
  sku: 'MONITOR-4K-27',
  price: 449.99,
  description: '27-inch 4K IPS monitor'
)
puts "   Created: #{monitor.name} - $#{monitor.price}"

webcam = commerce.products.create(
  name: 'HD Webcam',
  sku: 'WEBCAM-HD',
  price: 79.99,
  description: '1080p HD webcam with microphone'
)
puts "   Created: #{webcam.name} - $#{webcam.price}"

dock = commerce.products.create(
  name: 'USB-C Dock',
  sku: 'DOCK-USBC',
  price: 199.99,
  description: 'USB-C docking station with multiple ports'
)
puts "   Created: #{dock.name} - $#{dock.price}"

# 3. Create orders
puts
puts "3. Creating orders..."

# Order 1: Single item, will be paid with credit card
order1 = commerce.orders.create(
  customer_id: customer1.id,
  items: [
    {
      product_id: monitor.id,
      sku: 'MONITOR-4K-27',
      name: '4K Monitor 27"',
      quantity: 1,
      unit_price: '449.99'
    }
  ],
  currency: 'USD'
)
puts "   Order 1: #{order1.order_number} - $#{order1.total_amount} (#{customer1.first_name})"

# Order 2: Multiple items, will be paid with PayPal
order2 = commerce.orders.create(
  customer_id: customer1.id,
  items: [
    {
      product_id: webcam.id,
      sku: 'WEBCAM-HD',
      name: 'HD Webcam',
      quantity: 2,
      unit_price: '79.99'
    },
    {
      product_id: dock.id,
      sku: 'DOCK-USBC',
      name: 'USB-C Dock',
      quantity: 1,
      unit_price: '199.99'
    }
  ],
  currency: 'USD'
)
puts "   Order 2: #{order2.order_number} - $#{order2.total_amount} (#{customer1.first_name})"

# Order 3: Large order, will demonstrate partial payment
order3 = commerce.orders.create(
  customer_id: customer2.id,
  items: [
    {
      product_id: monitor.id,
      sku: 'MONITOR-4K-27',
      name: '4K Monitor 27"',
      quantity: 3,
      unit_price: '449.99'
    }
  ],
  currency: 'USD'
)
puts "   Order 3: #{order3.order_number} - $#{order3.total_amount} (#{customer2.first_name})"

# 4. Record payment for Order 1 (Credit Card)
puts
puts "4. Recording credit card payment for Order 1..."
result = commerce.payments.record(order1.id, 449.99, 'credit_card')
if result
  puts "   ✓ Payment recorded successfully"
  puts "   Amount: $449.99"
  puts "   Method: credit_card"
end

# Check order status after payment
order1_updated = commerce.orders.get(order1.id)
puts "   Order status: #{order1_updated.status}"
puts "   Payment status: #{order1_updated.payment_status}"

# 5. Record payment for Order 2 (PayPal)
puts
puts "5. Recording PayPal payment for Order 2..."
order2_total = 359.97  # 2x79.99 + 199.99
result = commerce.payments.record(order2.id, order2_total, 'paypal')
if result
  puts "   ✓ Payment recorded successfully"
  puts "   Amount: $#{order2_total}"
  puts "   Method: paypal"
end

order2_updated = commerce.orders.get(order2.id)
puts "   Order status: #{order2_updated.status}"
puts "   Payment status: #{order2_updated.payment_status}"

# 6. Demonstrate partial payment for Order 3
puts
puts "6. Recording partial payment for Order 3 (installment plan)..."
order3_total = 1349.97  # 3x449.99

# First installment (50%)
first_payment = order3_total * 0.5
result = commerce.payments.record(order3.id, first_payment, 'bank_transfer')
if result
  puts "   ✓ First installment recorded"
  puts "   Amount: $#{first_payment.round(2)} (50%)"
  puts "   Method: bank_transfer"
end

order3_partial = commerce.orders.get(order3.id)
puts "   Payment status: #{order3_partial.payment_status}"

# Second installment (remaining 50%)
puts
puts "   Recording second installment..."
second_payment = order3_total - first_payment
result = commerce.payments.record(order3.id, second_payment, 'bank_transfer')
if result
  puts "   ✓ Second installment recorded"
  puts "   Amount: $#{second_payment.round(2)} (50%)"
end

order3_updated = commerce.orders.get(order3.id)
puts "   Final payment status: #{order3_updated.payment_status}"

# 7. Payment with different methods
puts
puts "7. Supported payment methods:"
payment_methods = ['credit_card', 'debit_card', 'paypal', 'bank_transfer', 'apple_pay', 'google_pay', 'crypto']
payment_methods.each do |method|
  puts "   - #{method}"
end

# 8. Summary
puts
puts "=== Payment Summary ==="
orders = commerce.orders.list
total_collected = 0.0

orders.each do |order|
  status_icon = order.payment_status.downcase.include?('paid') ? '✓' : '○'
  puts "#{status_icon} #{order.order_number}: $#{order.total_amount} - #{order.payment_status}"
  total_collected += order.total_amount.to_f if order.payment_status.downcase.include?('paid')
end

puts
puts "Total orders: #{orders.size}"
puts "Total collected: $#{total_collected.round(2)}"

puts
puts "✓ Payments example completed successfully!"
