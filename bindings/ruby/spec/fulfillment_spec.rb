# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'orders, shipments and returns' do
  let(:commerce) { memory_store }
  let(:customer) { create_customer(commerce) }

  describe StateSet::Orders do
    it 'creates an order with exact totals and walks its lifecycle' do
      order = create_order(commerce, customer: customer, price: '29.99', quantity: 2)
      expect(order).to be_a(StateSet::Order)
      expect(order.status).to eq('pending')
      expect(order.total_amount).to eq(BigDecimal('59.98'))
      expect(order.items.first).to be_a(StateSet::OrderItem)
      expect(order.items.first.unit_price).to eq(BigDecimal('29.99'))
      expect(order.item_count).to eq(1)

      expect(commerce.orders.get_by_number(order.order_number).id).to eq(order.id)
      expect(commerce.orders.list_for_customer(customer.id).map(&:id)).to eq([order.id])
      expect(commerce.orders.list(status: 'pending').map(&:id)).to eq([order.id])
      expect(commerce.orders.count).to eq(1)

      expect(commerce.orders.update_status(order.id, :confirmed).status).to eq('confirmed')
      shipped = commerce.orders.ship(order.id, tracking_number: '1Z999AA10123456784')
      expect(shipped.status).to eq('shipped')
      expect(shipped.tracking_number).to eq('1Z999AA10123456784')
      expect(commerce.orders.deliver(order.id).status).to eq('delivered')
    end

    it 'ships part of an order' do
      order = create_order(commerce, customer: customer, quantity: 3)
      line = order.items.first
      partial = commerce.orders.ship(order.id, lines: [{ order_item_id: line.id, quantity: 1 }])
      expect(partial.status).to eq('partially_shipped')
      expect(partial.items.first.shipped_quantity).to eq(1)
    end

    it 'cancels a pending order and refuses an invalid transition' do
      order = create_order(commerce, customer: customer)
      expect(commerce.orders.cancel(order.id).status).to eq('cancelled')
      expect { commerce.orders.deliver(order.id) }.to raise_error(StateSet::Error) do |error|
        expect(error).not_to be_a(StateSet::InternalError)
      end
    end

    it 'raises NotFoundError for a state change on a missing order' do
      expect { commerce.orders.cancel(SecureRandom.uuid) }.to raise_error(StateSet::NotFoundError)
    end
  end

  describe StateSet::Shipments do
    it 'creates a shipment and tracks it to delivery' do
      order = create_order(commerce, customer: customer)
      shipment = commerce.shipments.create(
        order_id: order.id, recipient_name: 'Ada Lovelace',
        shipping_address: '1 Analytical Way, London', carrier: 'ups',
        shipping_cost: BigDecimal('7.25')
      )
      expect(shipment).to be_a(StateSet::Shipment)
      expect(shipment.shipping_cost).to eq(BigDecimal('7.25'))

      item = commerce.shipments.add_item(shipment.id, sku: order.items.first.sku, name: 'Widget Pro', quantity: 1)
      expect(commerce.shipments.get_items(shipment.id).map(&:id)).to eq([item.id])

      commerce.shipments.mark_processing(shipment.id)
      commerce.shipments.mark_ready(shipment.id)
      shipped = commerce.shipments.ship(shipment.id, tracking_number: 'TRACK-1')
      expect(shipped.status).to eq('shipped')
      expect(commerce.shipments.get_by_tracking('TRACK-1').id).to eq(shipment.id)
      expect(commerce.shipments.mark_in_transit(shipment.id).status).to eq('in_transit')
      expect(commerce.shipments.mark_out_for_delivery(shipment.id).status).to eq('out_for_delivery')
      expect(commerce.shipments.mark_delivered(shipment.id).status).to eq('delivered')

      expect(commerce.shipments.for_order(order.id).map(&:id)).to eq([shipment.id])
      expect(commerce.shipments.get_by_number(shipment.shipment_number).id).to eq(shipment.id)
      expect(commerce.shipments.list(status: 'delivered').map(&:id)).to eq([shipment.id])
      expect(commerce.shipments.count).to eq(1)
    end

    it 'holds and cancels shipments' do
      order = create_order(commerce, customer: customer)
      shipment = commerce.shipments.create(order_id: order.id, recipient_name: 'A', shipping_address: 'X')
      expect(commerce.shipments.hold(shipment.id).status).to eq('on_hold')
      other = commerce.shipments.create(order_id: order.id, recipient_name: 'B', shipping_address: 'Y')
      expect(commerce.shipments.cancel(other.id).status).to eq('cancelled')
    end
  end

  describe StateSet::Returns do
    def delivered_order
      order = create_order(commerce, customer: customer, price: '15.00', quantity: 2)
      commerce.orders.ship(order.id)
      commerce.orders.deliver(order.id)
    end

    it 'runs a return from request to completion' do
      order = delivered_order
      ret = commerce.returns.create(
        order_id: order.id, reason: 'defective', reason_details: 'cracked',
        items: [{ order_item_id: order.items.first.id, quantity: 1 }]
      )
      expect(ret).to be_a(StateSet::Return)
      expect(ret.status).to eq('requested')
      expect(ret.items.first).to be_a(StateSet::ReturnItem)
      expect(ret.items.first.refund_amount).to be_a(BigDecimal)

      expect(commerce.returns.approve(ret.id).status).to eq('approved')
      expect(commerce.returns.add_tracking(ret.id, 'RET-TRACK').tracking_number).to eq('RET-TRACK')
      expect(commerce.returns.mark_received(ret.id).status).to eq('received')
      expect { commerce.returns.complete(ret.id) }.to raise_error(StateSet::NotPermittedError, /disposition/)
      item = commerce.returns.set_item_disposition(ret.id, ret.items.first.id, disposition: 'scrap')
      expect(item.disposition).to eq('scrap')
      expect(commerce.returns.complete(ret.id).status).to eq('completed')

      expect(commerce.returns.get(ret.id).status).to eq('completed')
      expect(commerce.returns.list_for_order(order.id).map(&:id)).to eq([ret.id])
      expect(commerce.returns.list(status: 'completed').map(&:id)).to eq([ret.id])
      expect(commerce.returns.count).to eq(1)
    end

    it 'rejects and cancels returns' do
      order = delivered_order
      line = order.items.first
      rejected = commerce.returns.create(order_id: order.id, reason: 'changed_mind',
                                         items: [{ order_item_id: line.id, quantity: 1 }])
      expect(commerce.returns.reject(rejected.id, 'outside policy').status).to eq('rejected')
      cancelled = commerce.returns.create(order_id: order.id, reason: 'other',
                                          items: [{ order_item_id: line.id, quantity: 1 }])
      expect(commerce.returns.cancel(cancelled.id).status).to eq('cancelled')
    end

    it 'refuses to return an order that never shipped' do
      order = create_order(commerce, customer: customer)
      expect do
        commerce.returns.create(order_id: order.id, reason: 'defective',
                                items: [{ order_item_id: order.items.first.id, quantity: 1 }])
      end.to raise_error(StateSet::ValidationError) { |e| expect(e.code).to eq('commerce.return.order_not_shipped') }
    end
  end
end
