#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - payments and refunds, with exact money
#
# Run with: ruby payments_example.rb

require 'stateset_embedded'

commerce = StateSet::Commerce.new(':memory:')

customer = commerce.customers.create(email: 'payer@example.com', first_name: 'Pat', last_name: 'Payer')
order = commerce.orders.create(
  customer_id: customer.id,
  items: [{ product_id: customer.id, sku: 'SVC-1', name: 'Service', quantity: 3, unit_price: '0.10' }]
)
puts "Order total: #{order.total_amount.to_s('F')} (exact: 3 x 0.10)"

payment = commerce.payments.create(
  order_id: order.id, customer_id: customer.id,
  amount: order.total_amount, payment_method: 'credit_card'
)
payment = commerce.payments.mark_completed(payment.id)
puts "Payment #{payment.payment_number}: #{payment.status}, #{payment.amount.to_s('F')}"

refund = commerce.payments.create_refund(payment_id: payment.id, amount: '0.10', reason: 'one item missing')
commerce.payments.complete_refund(refund.id)
puts "Refunded #{refund.amount.to_s('F')}; payment now shows #{commerce.payments.get(payment.id).amount_refunded.to_s('F')} refunded"

# Over-refunding is refused by the engine.
begin
  commerce.payments.create_refund(payment_id: payment.id, amount: '1.00')
rescue StateSet::ValidationError => e
  puts "Refused: #{e.code}"
end

# Floats never reach the engine.
begin
  commerce.payments.create(amount: 0.1, payment_method: 'credit_card')
rescue StateSet::ValidationError => e
  puts "Refused: #{e.code}"
end

# A declined card is recorded, not thrown away.
declined = commerce.payments.create(amount: '5.00', payment_method: 'credit_card')
declined = commerce.payments.mark_failed(declined.id, 'card declined', code: 'card_declined')
puts "Payment #{declined.payment_number}: #{declined.status} (#{declined.failure_code})"
