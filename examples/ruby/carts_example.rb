#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - cart and checkout
#
# Run with: ruby carts_example.rb

require 'stateset_embedded'

commerce = StateSet::Commerce.new(':memory:')

customer = commerce.customers.create(email: 'shopper@example.com', first_name: 'Sam', last_name: 'Shopper')
cart = commerce.carts.create(customer_id: customer.id, customer_email: customer.email, currency: 'USD')
puts "Cart #{cart.cart_number} created"

commerce.carts.add_item(cart.id, sku: 'TEE-M', name: 'T-Shirt (M)', quantity: 2, unit_price: '19.99')
mug = commerce.carts.add_item(cart.id, sku: 'MUG', name: 'Mug', quantity: 1, unit_price: BigDecimal('12.50'))
commerce.carts.update_item(mug.id, quantity: 2)

address = {
  first_name: 'Sam', last_name: 'Shopper', line1: '1 Main St',
  city: 'Springfield', state: 'IL', postal_code: '62701', country: 'US'
}
commerce.carts.set_shipping(cart.id, address: address, shipping_method: 'standard', shipping_amount: '5.00')
commerce.carts.set_payment(cart.id, payment_method: 'credit_card', payment_token: 'tok_demo')
commerce.carts.set_tax(cart.id, '5.20')

ready = commerce.carts.mark_ready_for_payment(cart.id)
puts "Subtotal #{ready.subtotal.to_s('F')}, shipping #{ready.shipping_amount.to_s('F')}, " \
     "tax #{ready.tax_amount.to_s('F')}, total #{ready.grand_total.to_s('F')}"

commerce.carts.begin_checkout(cart.id)
result = commerce.carts.complete(cart.id)
puts "Checkout complete: order #{result.order_number}, charged #{result.total_charged.to_s('F')} #{result.currency}"

# The order is Confirmed with payment Pending: record the payment.
payment = commerce.payments.create(
  order_id: result.order_id, customer_id: customer.id,
  amount: result.total_charged, currency: 'USD', payment_method: 'credit_card'
)
commerce.payments.mark_completed(payment.id)
puts "Payment #{payment.payment_number} completed"
