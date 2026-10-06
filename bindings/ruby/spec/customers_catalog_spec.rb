# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'customers, products and inventory' do
  let(:commerce) { memory_store }

  describe StateSet::Customers do
    it 'creates, reads, updates, lists, counts and deletes' do
      customer = commerce.customers.create(
        email: 'alice@example.com', first_name: 'Alice', last_name: 'Smith',
        phone: '+1-555-0100', accepts_marketing: true, tags: %w[vip]
      )
      expect(customer).to be_a(StateSet::Customer)
      expect(customer.id).to match(/\A\h{8}-\h{4}-\h{4}-\h{4}-\h{12}\z/)
      expect(customer.full_name).to eq('Alice Smith')
      expect(customer.accepts_marketing).to be(true)
      expect(customer.tags).to eq(%w[vip])
      expect(customer.created_at).to be_a(Time)

      expect(commerce.customers.get(customer.id)).to eq(customer)
      expect(commerce.customers.get_by_email('alice@example.com').id).to eq(customer.id)
      expect(commerce.customers.get(SecureRandom.uuid)).to be_nil

      updated = commerce.customers.update(customer.id, first_name: 'Alicia')
      expect(updated.first_name).to eq('Alicia')

      create_customer(commerce)
      expect(commerce.customers.count).to eq(2)
      expect(commerce.customers.list(limit: 1).size).to eq(1)
      expect(commerce.customers.list(email: 'alice@example.com').map(&:id)).to eq([customer.id])

      expect(commerce.customers.delete(customer.id)).to be(true)
    end

    it 'refuses a duplicate email with ConflictError' do
      create_customer(commerce, email: 'dup@example.com')
      expect { create_customer(commerce, email: 'dup@example.com') }
        .to raise_error(StateSet::ConflictError)
    end

    it 'refuses an unknown keyword instead of dropping it' do
      expect do
        commerce.customers.create(email: unique_email, first_name: 'A', last_name: 'B', frist_name: 'typo')
      end.to raise_error(StateSet::ValidationError, /frist_name/)
    end
  end

  describe StateSet::Products do
    it 'manages products and exact-priced variants' do
      product, variant = create_product(commerce, sku: 'WIDGET-1', price: '19.99')
      expect(product.name).to eq('Widget Pro WIDGET-1')
      expect(variant).to be_a(StateSet::ProductVariant)
      expect(variant.price).to eq(BigDecimal('19.99'))
      expect(variant.price).to be_a(BigDecimal)

      extra = commerce.products.add_variant(product.id, sku: 'WIDGET-2', price: BigDecimal('24.50'))
      expect(commerce.products.get_variants(product.id).map(&:sku)).to contain_exactly('WIDGET-1', 'WIDGET-2')
      expect(commerce.products.get_variant(extra.id).price).to eq(BigDecimal('24.50'))

      expect(commerce.products.get(product.id).id).to eq(product.id)
      expect(commerce.products.get_by_slug(product.slug).id).to eq(product.id)
      expect(commerce.products.update(product.id, name: 'Widget Max').name).to eq('Widget Max')
      expect(commerce.products.activate(product.id).status).to eq('active')
      expect(commerce.products.list(status: 'active').map(&:id)).to include(product.id)
      expect(commerce.products.count).to eq(1)
      expect(commerce.products.search('Widget').map(&:id)).to include(product.id)
      expect(commerce.products.archive(product.id).status).to eq('archived')
    end
  end

  describe StateSet::Inventory do
    it 'tracks on-hand stock as exact decimals' do
      item = commerce.inventory.create_item(sku: 'BOLT', name: 'Bolt', initial_quantity: 10, reorder_point: '2')
      expect(item).to be_a(StateSet::InventoryItem)
      expect(commerce.inventory.get_item(item.id).sku).to eq('BOLT')
      expect(commerce.inventory.get_item_by_sku('BOLT').id).to eq(item.id)

      txn = commerce.inventory.adjust('BOLT', BigDecimal('-2.5'), 'damaged')
      expect(txn.quantity).to eq(BigDecimal('-2.5'))

      stock = commerce.inventory.get_stock('BOLT')
      expect(stock.total_on_hand).to eq(BigDecimal('7.5'))
      expect(stock.total_available).to be_a(BigDecimal)
      expect(commerce.inventory.has_stock?('BOLT', 7)).to be(true)
      expect(commerce.inventory.has_stock?('BOLT', 8)).to be(false)
      expect(commerce.inventory.get_transactions(item.id).size).to be >= 1
      expect(commerce.inventory.list.map(&:sku)).to include('BOLT')
    end

    it 'reserves, confirms and releases stock' do
      commerce.inventory.create_item(sku: 'NUT', name: 'Nut', initial_quantity: 5)
      reservation = commerce.inventory.reserve('NUT', 3, reference_type: 'order', reference_id: 'ORD-1')
      expect(reservation.quantity).to eq(BigDecimal('3'))
      expect(commerce.inventory.get_stock('NUT').total_available).to eq(BigDecimal('2'))
      expect(commerce.inventory.get_reservation(reservation.id).id).to eq(reservation.id)

      expect(commerce.inventory.release_reservation(reservation.id)).to be(true)
      expect(commerce.inventory.get_stock('NUT').total_available).to eq(BigDecimal('5'))

      held = commerce.inventory.reserve('NUT', 1, reference_type: 'order', reference_id: 'ORD-2')
      expect(commerce.inventory.confirm_reservation(held.id)).to be(true)
    end

    it 'raises InsufficientStockError when a reservation exceeds availability' do
      commerce.inventory.create_item(sku: 'RARE', name: 'Rare', initial_quantity: 1)
      expect { commerce.inventory.reserve('RARE', 2, reference_type: 'order', reference_id: 'X') }
        .to raise_error(StateSet::InsufficientStockError)
    end
  end
end
