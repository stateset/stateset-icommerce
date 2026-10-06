# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'carts, checkout, payments and refunds' do
  let(:commerce) { memory_store }
  let(:customer) { create_customer(commerce) }

  def build_cart
    product, variant = create_product(commerce, sku: 'CART-1', price: '10.00')
    cart = commerce.carts.create(customer_id: customer.id, customer_email: customer.email, currency: 'USD')
    item = commerce.carts.add_item(
      cart.id, product_id: product.id, variant_id: variant.id, sku: 'CART-1',
               name: 'Widget Pro', quantity: 3, unit_price: BigDecimal('10.00')
    )
    [cart, item]
  end

  it 'computes exact cart totals and manages items' do
    cart, item = build_cart
    expect(item).to be_a(StateSet::CartItem)
    expect(item.total).to eq(BigDecimal('30.00'))

    item = commerce.carts.update_item(item.id, quantity: 2)
    expect(item.quantity).to eq(2)

    reread = commerce.carts.get(cart.id)
    expect(reread.subtotal).to eq(BigDecimal('20.00'))
    expect(reread.items.map(&:sku)).to eq(['CART-1'])
    expect(commerce.carts.get_by_number(cart.cart_number).id).to eq(cart.id)
    expect(commerce.carts.for_customer(customer.id).map(&:id)).to eq([cart.id])
    expect(commerce.carts.get_items(cart.id).size).to eq(1)
    expect(commerce.carts.count).to eq(1)

    expect(commerce.carts.remove_item(item.id)).to be(true)
    expect(commerce.carts.get_items(cart.id)).to be_empty
  end

  it 'checks out a cart into an order, pays it, and refunds part of it' do
    cart, = build_cart
    commerce.carts.set_shipping_address(cart.id, **address)
    commerce.carts.set_shipping(cart.id, address: address, shipping_method: 'standard', shipping_amount: '5.00')
    commerce.carts.set_payment(cart.id, payment_method: 'credit_card', payment_token: 'tok_test')
    commerce.carts.set_tax(cart.id, '2.40')
    ready = commerce.carts.mark_ready_for_payment(cart.id)
    expect(ready.grand_total).to eq(BigDecimal('37.40'))

    commerce.carts.begin_checkout(cart.id)
    result = commerce.carts.complete(cart.id)
    expect(result).to be_a(StateSet::CheckoutResult)
    expect(result.total_charged).to eq(BigDecimal('37.40'))

    order = commerce.orders.get(result.order_id)
    expect(order.total_amount).to eq(BigDecimal('37.40'))
    expect(commerce.carts.get(cart.id).status).to eq('completed')

    payment = commerce.payments.create(
      order_id: order.id, customer_id: customer.id,
      amount: order.total_amount, currency: 'USD', payment_method: 'credit_card'
    )
    expect(payment.amount).to eq(BigDecimal('37.40'))
    expect(commerce.payments.mark_completed(payment.id).status).to eq('completed')
    expect(commerce.payments.for_order(order.id).map(&:id)).to eq([payment.id])
    expect(commerce.payments.get_by_number(payment.payment_number).id).to eq(payment.id)

    refund = commerce.payments.create_refund(payment_id: payment.id, amount: '7.40', reason: 'goodwill')
    expect(refund.amount).to eq(BigDecimal('7.40'))
    expect(commerce.payments.complete_refund(refund.id).status).to eq('completed')
    expect(commerce.payments.get_refunds(payment.id).map(&:id)).to eq([refund.id])
    expect(commerce.payments.get(payment.id).amount_refunded).to eq(BigDecimal('7.40'))

    expect { commerce.payments.create_refund(payment_id: payment.id, amount: '30.01') }
      .to raise_error(StateSet::ValidationError) { |e| expect(e.code).to eq('commerce.refund.exceeds_captured') }
  end

  it 'cancels and abandons carts' do
    cart, = build_cart
    expect(commerce.carts.cancel(cart.id).status).to eq('cancelled')
    other = commerce.carts.create(customer_email: unique_email)
    expect(commerce.carts.abandon(other.id).status).to eq('abandoned')
  end

  it 'records failed and cancelled payments' do
    failed = commerce.payments.create(amount: '5.00', payment_method: 'credit_card')
    failed = commerce.payments.mark_failed(failed.id, 'card declined', code: 'card_declined')
    expect(failed.status).to eq('failed')
    expect(failed.failure_code).to eq('card_declined')

    pending = commerce.payments.create(amount: '5.00', payment_method: 'bank_transfer')
    expect(commerce.payments.cancel(pending.id).status).to eq('cancelled')
    expect(commerce.payments.count).to eq(2)
  end
end
