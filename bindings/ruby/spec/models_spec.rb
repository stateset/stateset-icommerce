# frozen_string_literal: true

# The Ruby models only declare how to TYPE engine fields (BigDecimal, Time,
# nested records). This checks every declared field is really returned by the
# engine, so a renamed engine field cannot silently turn into a nil reader.

require 'spec_helper'

RSpec.describe 'model declarations' do
  let(:commerce) { memory_store }

  def check(record)
    klass = record.class
    declared = klass.decimal_fields + klass.time_fields + klass.nested_fields.keys
    missing = declared.reject { |field| record.key?(field) }
    expect(missing).to be_empty, "#{klass} declares fields the engine does not return: #{missing}"
    klass.decimal_fields.each do |field|
      expect(record[field]).to(satisfy { |v| v.nil? || v.is_a?(BigDecimal) }, "#{klass}##{field}")
    end
    klass.nested_fields.each do |field, nested|
      Array(record[field]).each { |child| check(child).tap { expect(child).to be_a(nested) } }
    end
  end

  it 'matches what the engine returns for every record type' do
    customer = create_customer(commerce)
    product, variant = create_product(commerce, sku: 'M-1', price: '5.00')
    item = commerce.inventory.create_item(sku: 'M-1', name: 'M', initial_quantity: 10)
    reservation = commerce.inventory.reserve('M-1', 1, reference_type: 'order', reference_id: 'R')
    txn = commerce.inventory.adjust('M-1', 1, 'recount')

    cart = commerce.carts.create(customer_id: customer.id, currency: 'USD')
    commerce.carts.add_item(cart.id, sku: 'M-1', name: 'M', quantity: 1, unit_price: '5.00')
    commerce.carts.set_shipping(cart.id, address: address, shipping_amount: '1.00')
    rates = commerce.carts.get_shipping_rates(cart.id)
    commerce.carts.set_payment(cart.id, payment_method: 'credit_card')
    commerce.carts.mark_ready_for_payment(cart.id)
    commerce.carts.begin_checkout(cart.id)
    checkout = commerce.carts.complete(cart.id)

    order = create_order(commerce, customer: customer, price: '5.00', quantity: 2)
    payment = commerce.payments.create(order_id: order.id, amount: order.total_amount, payment_method: 'credit_card')
    commerce.payments.mark_completed(payment.id)
    refund = commerce.payments.create_refund(payment_id: payment.id, amount: '1.00')
    shipment = commerce.shipments.create(order_id: order.id, recipient_name: 'A', shipping_address: 'X')
    commerce.orders.ship(order.id)
    commerce.orders.deliver(order.id)
    ret = commerce.returns.create(order_id: order.id, reason: 'other',
                                  items: [{ order_item_id: order.items.first.id, quantity: 1 }])

    [customer, product, variant, item, reservation, txn, commerce.inventory.get_stock('M-1'),
     commerce.carts.get(cart.id), checkout, order, payment, refund, shipment, ret, *rates]
      .each { |record| check(record) }
  end
end
