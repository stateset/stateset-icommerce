# frozen_string_literal: true

require 'spec_helper'

RSpec.describe StateSet::Commerce do
  let(:commerce) { StateSet::Commerce.new(':memory:') }

  describe '#new' do
    it 'creates a commerce instance with in-memory database' do
      expect(commerce).to be_a(StateSet::Commerce)
    end

    it 'creates a commerce instance with file database' do
      db_path = '/tmp/test_stateset.db'
      File.delete(db_path) if File.exist?(db_path)

      c = StateSet::Commerce.new(db_path)
      expect(c).to be_a(StateSet::Commerce)

      File.delete(db_path) if File.exist?(db_path)
    end
  end

  describe '#customers' do
    it 'returns a Customers API instance' do
      expect(commerce.customers).to be_a(StateSet::Customers)
    end

    describe 'create' do
      it 'creates a customer' do
        customer = commerce.customers.create(
          'alice@example.com',
          'Alice',
          'Smith',
          nil,
          nil
        )

        expect(customer).to be_a(StateSet::Customer)
        expect(customer.email).to eq('alice@example.com')
        expect(customer.first_name).to eq('Alice')
        expect(customer.last_name).to eq('Smith')
        expect(customer.full_name).to eq('Alice Smith')
      end

      it 'creates a customer with phone' do
        customer = commerce.customers.create(
          'bob@example.com',
          'Bob',
          'Jones',
          '+1-555-1234',
          true
        )

        expect(customer.phone).to eq('+1-555-1234')
        expect(customer.accepts_marketing).to be true
      end
    end

    describe 'get' do
      it 'retrieves a customer by ID' do
        created = commerce.customers.create('test@example.com', 'Test', 'User', nil, nil)
        found = commerce.customers.get(created.id)

        expect(found).not_to be_nil
        expect(found.id).to eq(created.id)
      end

      it 'returns nil for non-existent customer' do
        found = commerce.customers.get('00000000-0000-0000-0000-000000000000')
        expect(found).to be_nil
      end
    end

    describe 'get_by_email' do
      it 'retrieves a customer by email' do
        created = commerce.customers.create('unique@example.com', 'Unique', 'User', nil, nil)
        found = commerce.customers.get_by_email('unique@example.com')

        expect(found).not_to be_nil
        expect(found.email).to eq('unique@example.com')
      end
    end

    describe 'list' do
      it 'returns all customers' do
        commerce.customers.create('list1@example.com', 'List', 'One', nil, nil)
        commerce.customers.create('list2@example.com', 'List', 'Two', nil, nil)

        customers = commerce.customers.list
        expect(customers.length).to be >= 2
      end
    end

    describe 'count' do
      it 'returns customer count' do
        initial_count = commerce.customers.count
        commerce.customers.create('count@example.com', 'Count', 'Test', nil, nil)

        expect(commerce.customers.count).to eq(initial_count + 1)
      end
    end
  end

  describe '#orders' do
    let(:customer) { commerce.customers.create('order@example.com', 'Order', 'Test', nil, nil) }

    it 'returns an Orders API instance' do
      expect(commerce.orders).to be_a(StateSet::Orders)
    end

    describe 'create' do
      it 'creates an order with items' do
        items = [
          { sku: 'WIDGET-001', name: 'Widget', quantity: 2, unit_price: 29.99 }
        ]

        order = commerce.orders.create(customer.id, items, 'USD', nil)

        expect(order).to be_a(StateSet::Order)
        expect(order.customer_id).to eq(customer.id)
        expect(order.currency).to eq('USD')
        expect(order.item_count).to eq(1)
      end
    end

    describe 'list' do
      it 'returns all orders' do
        orders = commerce.orders.list
        expect(orders).to be_an(Array)
      end
    end
  end

  describe '#products' do
    it 'returns a Products API instance' do
      expect(commerce.products).to be_a(StateSet::Products)
    end

    describe 'create' do
      it 'creates a product' do
        product = commerce.products.create('Test Product', 'A test product', 'TestVendor', 'Widget')

        expect(product).to be_a(StateSet::Product)
        expect(product.name).to eq('Test Product')
        expect(product.description).to eq('A test product')
        expect(product.vendor).to eq('TestVendor')
      end
    end

    describe 'list' do
      it 'returns all products' do
        products = commerce.products.list
        expect(products).to be_an(Array)
      end
    end
  end

  describe '#inventory' do
    it 'returns an Inventory API instance' do
      expect(commerce.inventory).to be_a(StateSet::Inventory)
    end

    describe 'create' do
      it 'creates an inventory item' do
        item = commerce.inventory.create('SKU-001', 100, 10, 50)

        expect(item).to be_a(StateSet::InventoryItem)
        expect(item.sku).to eq('SKU-001')
        expect(item.quantity_on_hand).to eq(100)
        expect(item.quantity_available).to eq(100)
      end
    end

    describe 'adjust' do
      it 'adjusts inventory quantity' do
        item = commerce.inventory.create('SKU-ADJ', 50, nil, nil)
        adjusted = commerce.inventory.adjust(item.id, -10, 'Sold 10 units')

        expect(adjusted.quantity_on_hand).to eq(40)
      end
    end

    describe 'reserve and release' do
      it 'reserves and releases inventory' do
        item = commerce.inventory.create('SKU-RES', 100, nil, nil)

        reserved = commerce.inventory.reserve(item.id, 20, nil)
        expect(reserved.quantity_reserved).to eq(20)
        expect(reserved.quantity_available).to eq(80)

        released = commerce.inventory.release(item.id, 10)
        expect(released.quantity_reserved).to eq(10)
        expect(released.quantity_available).to eq(90)
      end
    end
  end

  describe '#carts' do
    it 'returns a Carts API instance' do
      expect(commerce.carts).to be_a(StateSet::Carts)
    end

    describe 'create' do
      it 'creates a cart' do
        cart = commerce.carts.create(nil, 'USD')

        expect(cart).to be_a(StateSet::Cart)
        expect(cart.currency).to eq('USD')
        expect(cart.items).to be_empty
      end
    end

    describe 'add_item' do
      it 'adds an item to the cart' do
        cart = commerce.carts.create(nil, 'USD')
        updated = commerce.carts.add_item(cart.id, 'SKU-001', 'Test Item', 2, 19.99)

        expect(updated.items.length).to eq(1)
        expect(updated.items.first.sku).to eq('SKU-001')
        expect(updated.items.first.quantity).to eq(2)
      end
    end
  end

  describe '#analytics' do
    it 'returns an Analytics API instance' do
      expect(commerce.analytics).to be_a(StateSet::Analytics)
    end

    describe 'sales_summary' do
      it 'returns a sales summary' do
        summary = commerce.analytics.sales_summary(30)

        expect(summary).to be_a(StateSet::SalesSummary)
        expect(summary.total_orders).to be >= 0
        expect(summary.total_revenue).to be >= 0
      end
    end
  end

  describe '#returns' do
    it 'returns a Returns API instance' do
      expect(commerce.returns).to be_a(StateSet::Returns)
    end

    it 'creates and approves a return' do
      customer = commerce.customers.create('returns@example.com', 'Return', 'User', nil, nil)
      items = [
        { sku: 'RET-001', name: 'Return Item', quantity: 1, unit_price: 19.99 }
      ]
      order = commerce.orders.create(customer.id, items, 'USD', nil)

      ret = commerce.returns.create(order.id, 'defective')
      expect(ret).to be_a(StateSet::Return)
      expect(ret.order_id).to eq(order.id)
      expect(ret.status).to eq('requested')

      approved = commerce.returns.approve(ret.id, nil)
      expect(approved.status).to eq('approved')
    end
  end

  describe '#payments' do
    it 'records a payment' do
      customer = commerce.customers.create('pay@example.com', 'Pay', 'User', nil, nil)
      items = [
        { sku: 'PAY-001', name: 'Payment Item', quantity: 1, unit_price: 49.99 }
      ]
      order = commerce.orders.create(customer.id, items, 'USD', nil)

      recorded = commerce.payments.record(order.id, 49.99, 'credit_card')
      expect(recorded).to be true
    end
  end

  describe '#shipments' do
    it 'creates and ships a shipment' do
      customer = commerce.customers.create('ship@example.com', 'Ship', 'User', nil, nil)
      items = [
        { sku: 'SHIP-001', name: 'Ship Item', quantity: 1, unit_price: 12.50 }
      ]
      order = commerce.orders.create(customer.id, items, 'USD', nil)

      shipment = commerce.shipments.create(order.id, 'ups', nil)
      expect(shipment).to be_a(StateSet::Shipment)
      expect(shipment.order_id).to eq(order.id)

      shipped = commerce.shipments.ship(shipment.id, 'TRACK123')
      expect(shipped.status).to eq('shipped')
    end
  end

  describe 'all API accessors' do
    it 'provides access to all 17 APIs' do
      expect(commerce.customers).to be_a(StateSet::Customers)
      expect(commerce.orders).to be_a(StateSet::Orders)
      expect(commerce.products).to be_a(StateSet::Products)
      expect(commerce.inventory).to be_a(StateSet::Inventory)
      expect(commerce.returns).to be_a(StateSet::Returns)
      expect(commerce.payments).to be_a(StateSet::Payments)
      expect(commerce.shipments).to be_a(StateSet::Shipments)
      expect(commerce.warranties).to be_a(StateSet::Warranties)
      expect(commerce.purchase_orders).to be_a(StateSet::PurchaseOrders)
      expect(commerce.invoices).to be_a(StateSet::Invoices)
      expect(commerce.bom).to be_a(StateSet::BomApi)
      expect(commerce.work_orders).to be_a(StateSet::WorkOrders)
      expect(commerce.carts).to be_a(StateSet::Carts)
      expect(commerce.analytics).to be_a(StateSet::Analytics)
      expect(commerce.currency).to be_a(StateSet::CurrencyOps)
      expect(commerce.subscriptions).to be_a(StateSet::Subscriptions)
      expect(commerce.promotions).to be_a(StateSet::Promotions)
      expect(commerce.tax).to be_a(StateSet::Tax)
    end
  end
end

RSpec.describe StateSet do
  it 'has a version number' do
    expect(StateSet::VERSION).to eq('1.4.0')
  end
end
