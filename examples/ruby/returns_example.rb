#!/usr/bin/env ruby
# frozen_string_literal: true

# StateSet iCommerce - shipping an order and processing a return
#
# Run with: ruby returns_example.rb

require 'stateset_embedded'

commerce = StateSet::Commerce.new(':memory:')

commerce.inventory.create_item(sku: 'BOOT-42', name: 'Boots', initial_quantity: 10)
customer = commerce.customers.create(email: 'returner@example.com', first_name: 'Robin', last_name: 'Returner')
order = commerce.orders.create(
  customer_id: customer.id,
  items: [{ product_id: customer.id, sku: 'BOOT-42', name: 'Boots', quantity: 2, unit_price: '89.00' }]
)

shipment = commerce.shipments.create(
  order_id: order.id, recipient_name: 'Robin Returner',
  shipping_address: '1 Main St, Springfield', carrier: 'ups'
)
commerce.shipments.mark_processing(shipment.id)
commerce.shipments.mark_ready(shipment.id)
commerce.shipments.ship(shipment.id, tracking_number: '1ZRETURN')
commerce.shipments.mark_in_transit(shipment.id)
commerce.shipments.mark_out_for_delivery(shipment.id)
commerce.shipments.mark_delivered(shipment.id)
commerce.orders.ship(order.id, tracking_number: '1ZRETURN')
commerce.orders.deliver(order.id)
puts "Order #{order.order_number} delivered"

# Returns are only accepted for what actually shipped.
ret = commerce.returns.create(
  order_id: order.id, reason: 'defective', reason_details: 'sole detached',
  items: [{ order_item_id: order.items.first.id, quantity: 1 }]
)
puts "Return #{ret.id}: #{ret.status}"

commerce.returns.approve(ret.id)
commerce.returns.add_tracking(ret.id, '1ZBACK')
commerce.returns.mark_received(ret.id)
# Every received item needs a warehouse disposition before completion.
commerce.returns.set_item_disposition(ret.id, ret.items.first.id, disposition: 'restock')
done = commerce.returns.complete(ret.id)
puts "Return #{done.status}; refund amount #{done.refund_amount&.to_s('F') || 'n/a'}"

begin
  commerce.returns.create(order_id: order.id, reason: 'defective',
                          items: [{ order_item_id: order.items.first.id, quantity: 5 }])
rescue StateSet::Error => e
  puts "Refused (#{e.class.name.split('::').last}): #{e.code}"
end
