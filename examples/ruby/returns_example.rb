#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - Returns Management Example
#
# Demonstrates the full returns workflow:
# - Creating orders (prerequisite for returns)
# - Initiating returns
# - Approving returns with refunds
# - Rejecting returns
# - Listing and managing returns
#
# Run with: ruby returns_example.rb

require 'stateset_embedded'

puts "=== StateSet iCommerce - Returns Management Example ==="
puts

# Initialize commerce with in-memory database
commerce = StateSet::Commerce.new(':memory:')
puts "✓ Commerce initialized"
puts

# 1. Create a customer
puts "1. Creating customer..."
customer = commerce.customers.create(
  email: 'returner@example.com',
  first_name: 'Bob',
  last_name: 'Wilson',
  phone: '+1-555-0234'
)
puts "   Created customer: #{customer.full_name}"

# 2. Create products
puts
puts "2. Creating products..."
headphones = commerce.products.create(
  name: 'Premium Headphones',
  sku: 'HEADPHONES-PRO',
  price: 299.99,
  description: 'Noise-cancelling wireless headphones'
)
puts "   Created: #{headphones.name}"

speaker = commerce.products.create(
  name: 'Bluetooth Speaker',
  sku: 'SPEAKER-BT',
  price: 89.99,
  description: 'Portable Bluetooth speaker'
)
puts "   Created: #{speaker.name}"

charger = commerce.products.create(
  name: 'Fast Charger',
  sku: 'CHARGER-FAST',
  price: 39.99,
  description: '65W USB-C fast charger'
)
puts "   Created: #{charger.name}"

# 3. Create orders (we need orders to create returns)
puts
puts "3. Creating orders..."

order1 = commerce.orders.create(
  customer_id: customer.id,
  items: [
    {
      product_id: headphones.id,
      sku: 'HEADPHONES-PRO',
      name: 'Premium Headphones',
      quantity: 1,
      unit_price: '299.99'
    }
  ],
  currency: 'USD'
)
puts "   Order 1: #{order1.order_number} - $#{order1.total_amount}"

order2 = commerce.orders.create(
  customer_id: customer.id,
  items: [
    {
      product_id: speaker.id,
      sku: 'SPEAKER-BT',
      name: 'Bluetooth Speaker',
      quantity: 2,
      unit_price: '89.99'
    }
  ],
  currency: 'USD'
)
puts "   Order 2: #{order2.order_number} - $#{order2.total_amount}"

order3 = commerce.orders.create(
  customer_id: customer.id,
  items: [
    {
      product_id: charger.id,
      sku: 'CHARGER-FAST',
      name: 'Fast Charger',
      quantity: 1,
      unit_price: '39.99'
    }
  ],
  currency: 'USD'
)
puts "   Order 3: #{order3.order_number} - $#{order3.total_amount}"

# 4. Initiate a return for order 1 (defective product)
puts
puts "4. Creating return for defective headphones..."
return1 = commerce.returns.create(
  order1.id,
  'Product arrived with audio defect in left ear cup'
)
puts "   Return ID: #{return1.id[0..7]}..."
puts "   Order: #{return1.order_id[0..7]}..."
puts "   Status: #{return1.status}"
puts "   Reason: #{return1.reason}"

# 5. Create another return for order 2 (wrong item)
puts
puts "5. Creating return for wrong item..."
return2 = commerce.returns.create(
  order2.id,
  'Received wrong color - ordered black, received white'
)
puts "   Return ID: #{return2.id[0..7]}..."
puts "   Status: #{return2.status}"
puts "   Reason: #{return2.reason}"

# 6. Create a return for order 3 (changed mind)
puts
puts "6. Creating return for change of mind..."
return3 = commerce.returns.create(
  order3.id,
  'No longer needed - found alternative solution'
)
puts "   Return ID: #{return3.id[0..7]}..."
puts "   Status: #{return3.status}"

# 7. List all returns
puts
puts "7. Listing all pending returns..."
all_returns = commerce.returns.list
puts "   Total returns: #{all_returns.size}"
all_returns.each do |r|
  puts "   - Return #{r.id[0..7]}... | Status: #{r.status} | Reason: #{r.reason[0..30]}..."
end

# 8. Approve return 1 with full refund (defective product)
puts
puts "8. Approving return for defective product (full refund)..."
return1 = commerce.returns.approve(return1.id, 299.99)
puts "   Return approved!"
puts "   Status: #{return1.status}"
puts "   Refund amount: $#{return1.refund_amount}"

# 9. Approve return 2 with partial refund (restocking fee)
puts
puts "9. Approving return with restocking fee..."
# Original order was $179.98 (2x $89.99), apply 15% restocking fee
refund_amount = 179.98 * 0.85  # $152.98
return2 = commerce.returns.approve(return2.id, refund_amount)
puts "   Return approved with restocking fee!"
puts "   Status: #{return2.status}"
puts "   Original: $179.98"
puts "   Refund (85%): $#{return2.refund_amount.round(2)}"

# 10. Reject return 3 (outside return policy for change of mind)
puts
puts "10. Rejecting return (outside policy)..."
return3 = commerce.returns.reject(
  return3.id,
  'Return request is outside our 14-day return policy for change of mind returns'
)
puts "   Return rejected!"
puts "   Status: #{return3.status}"

# 11. Get a specific return
puts
puts "11. Retrieving return details..."
fetched_return = commerce.returns.get(return1.id)
if fetched_return
  puts "   Return found:"
  puts "   - ID: #{fetched_return.id}"
  puts "   - Order ID: #{fetched_return.order_id}"
  puts "   - Customer ID: #{fetched_return.customer_id}"
  puts "   - Status: #{fetched_return.status}"
  puts "   - Reason: #{fetched_return.reason}"
  puts "   - Refund: $#{fetched_return.refund_amount}"
  puts "   - Created: #{fetched_return.created_at}"
end

# 12. Final summary
puts
puts "=== Returns Summary ==="
final_returns = commerce.returns.list
approved = final_returns.select { |r| r.status.downcase.include?('approved') }
rejected = final_returns.select { |r| r.status.downcase.include?('rejected') }
pending = final_returns.select { |r| r.status.downcase.include?('pending') }

puts "Total returns: #{final_returns.size}"
puts "  - Approved: #{approved.size}"
puts "  - Rejected: #{rejected.size}"
puts "  - Pending: #{pending.size}"

total_refunded = approved.sum { |r| r.refund_amount }
puts "Total refunded: $#{total_refunded.round(2)}"

puts
puts "✓ Returns example completed successfully!"
