#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - Shopping Cart Example
#
# Demonstrates the full cart workflow:
# - Cart creation (guest and customer carts)
# - Adding items to cart
# - Cart management
# - Checkout process
#
# Run with: ruby carts_example.rb

require 'stateset_embedded'

puts "=== StateSet iCommerce - Shopping Cart Example ==="
puts

# Initialize commerce with in-memory database
commerce = StateSet::Commerce.new(':memory:')
puts "✓ Commerce initialized"
puts

# 1. Create a customer for the cart
puts "1. Creating customer..."
customer = commerce.customers.create(
  email: 'shopper@example.com',
  first_name: 'Jane',
  last_name: 'Doe',
  phone: '+1-555-0199'
)
puts "   Created customer: #{customer.full_name} (#{customer.email})"

# 2. Create products to add to cart
puts
puts "2. Creating products..."
laptop = commerce.products.create(
  name: 'Laptop Pro 15',
  sku: 'LAPTOP-PRO-15',
  price: 1299.99,
  description: 'High-performance laptop with 15-inch display'
)
puts "   Created: #{laptop.name} - $#{laptop.price}"

mouse = commerce.products.create(
  name: 'Wireless Mouse',
  sku: 'MOUSE-WIRELESS',
  price: 49.99,
  description: 'Ergonomic wireless mouse'
)
puts "   Created: #{mouse.name} - $#{mouse.price}"

keyboard = commerce.products.create(
  name: 'Mechanical Keyboard',
  sku: 'KB-MECH-RGB',
  price: 149.99,
  description: 'RGB mechanical keyboard with Cherry MX switches'
)
puts "   Created: #{keyboard.name} - $#{keyboard.price}"

# 3. Create a guest cart (no customer attached)
puts
puts "3. Creating guest cart..."
guest_cart = commerce.carts.create(
  customer_id: nil,
  currency: 'USD'
)
puts "   Created guest cart: #{guest_cart.id}"
puts "   Status: #{guest_cart.status}"
puts "   Currency: #{guest_cart.currency}"

# 4. Create a customer cart
puts
puts "4. Creating customer cart..."
cart = commerce.carts.create(
  customer_id: customer.id,
  currency: 'USD'
)
puts "   Created cart: #{cart.id}"
puts "   Customer: #{customer.full_name}"
puts "   Status: #{cart.status}"

# 5. Add items to the cart
puts
puts "5. Adding items to cart..."

cart = commerce.carts.add_item(
  cart.id,
  'LAPTOP-PRO-15',
  'Laptop Pro 15',
  1,
  1299.99
)
puts "   Added: 1x Laptop Pro 15 @ $1299.99"

cart = commerce.carts.add_item(
  cart.id,
  'MOUSE-WIRELESS',
  'Wireless Mouse',
  2,
  49.99
)
puts "   Added: 2x Wireless Mouse @ $49.99 each"

cart = commerce.carts.add_item(
  cart.id,
  'KB-MECH-RGB',
  'Mechanical Keyboard',
  1,
  149.99
)
puts "   Added: 1x Mechanical Keyboard @ $149.99"

# 6. Review cart contents
puts
puts "6. Cart summary..."
puts "   Items in cart: #{cart.items.size}"
cart.items.each do |item|
  puts "   - #{item.name}: #{item.quantity}x @ $#{item.unit_price} = $#{item.total}"
end
puts "   ---"
puts "   Subtotal: $#{cart.subtotal}"
puts "   Total: $#{cart.total} #{cart.currency}"

# 7. List all carts
puts
puts "7. Listing all carts..."
all_carts = commerce.carts.list
puts "   Total carts: #{all_carts.size}"
all_carts.each do |c|
  customer_info = c.customer_id ? "Customer: #{c.customer_id[0..7]}..." : "Guest"
  puts "   - Cart #{c.id[0..7]}... | #{customer_info} | Items: #{c.items.size} | Total: $#{c.total}"
end

# 8. Checkout the cart
puts
puts "8. Checking out cart..."
order = commerce.carts.checkout(cart.id)
puts "   ✓ Cart converted to order!"
puts "   Order Number: #{order.order_number}"
puts "   Order Total: $#{order.total_amount}"
puts "   Order Status: #{order.status}"

# 9. Verify cart is now checked out
puts
puts "9. Verifying cart status after checkout..."
updated_cart = commerce.carts.get(cart.id)
if updated_cart
  puts "   Cart status: #{updated_cart.status}"
else
  puts "   Cart no longer active (converted to order)"
end

# Summary
puts
puts "=== Summary ==="
puts "Created #{all_carts.size} carts"
puts "Completed 1 checkout"
puts "Generated order: #{order.order_number}"

puts
puts "✓ Cart example completed successfully!"
