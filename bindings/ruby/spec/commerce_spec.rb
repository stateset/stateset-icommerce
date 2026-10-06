# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'money, errors and the API surface' do
  let(:commerce) { memory_store }
  let(:customer) { create_customer(commerce) }

  describe 'money is exact' do
    it 'refuses Float amounts anywhere in the arguments' do
      expect { commerce.payments.create(amount: 0.1, payment_method: 'credit_card') }
        .to raise_error(StateSet::ValidationError, /Float 0.1 refused/) { |e| expect(e.code).to eq('binding.float_refused') }
      expect { commerce.inventory.adjust('X', 1.5, 'r') }.to raise_error(StateSet::ValidationError, /Float/)
      expect(commerce.payments.count).to eq(0)
    end

    it 'accepts BigDecimal, Integer and decimal String and returns BigDecimal' do
      [BigDecimal('0.30'), '0.30'].each do |amount|
        payment = commerce.payments.create(amount: amount, payment_method: 'credit_card')
        expect(payment.amount).to be_a(BigDecimal)
        expect(payment.amount).to eq(BigDecimal('0.3'))
      end
      expect(commerce.payments.create(amount: 3, payment_method: 'credit_card').amount).to eq(BigDecimal('3'))
    end

    it 'adds 0.1 + 0.2 to exactly 0.3' do
      product, variant = create_product(commerce)
      order = commerce.orders.create(
        customer_id: customer.id,
        items: [order_line(product, sku: variant.sku, price: '0.1'), order_line(product, sku: variant.sku, price: '0.2')]
      )
      expect(order.total_amount).to eq(BigDecimal('0.3'))
    end
  end

  # Shared cross-binding SEMANTIC corpus (bindings/test-vectors/semantics-v1.json),
  # kept honest against the engine by crates/stateset-embedded/tests/semantics_vectors.rs.
  describe 'semantic vectors' do
    corpus = JSON.parse(File.read(File.expand_path('../../test-vectors/semantics-v1.json', __dir__)))
    not_reachable = {
      'currency_decimals' => 'the Ruby surface exposes no currency-scale accessor',
      'canadian_tax_rates' => 'the Ruby surface exposes no Canadian tax lookup'
    }
    asserted = %w[decimal_render money_scale_enforced rejected_inputs accepted_inputs]
    rows = ->(name) { corpus.fetch('categories').fetch(name).fetch('rows') }

    def vector_order(price, quantity: 1, currency: nil, customer_id: customer.id)
      product_id = customer.id # orders do not require a catalog product
      input = {
        customer_id: customer_id,
        items: [{ product_id: product_id, sku: 'SKU-1', name: 'Widget', quantity: quantity, unit_price: price }]
      }
      input[:currency] = currency unless currency.nil?
      commerce.orders.create(**input)
    end

    it 'accounts for every category in the corpus' do
      expect(corpus['version']).to eq(1)
      expect(corpus['categories'].keys.sort).to eq((asserted + not_reachable.keys).sort)
    end

    it 'renders decimals exactly (decimal_render)' do
      rows.call('decimal_render').select { |r| r['money_scale_ok'] }.each do |row|
        a, b = row['operands']
        order =
          case row['op']
          when 'add'
            product_id = customer.id
            commerce.orders.create(
              customer_id: customer.id,
              items: [a, b].map { |p| { product_id: product_id, sku: 'S', name: 'W', quantity: 1, unit_price: p } }
            )
          when 'mul' then vector_order(a, quantity: Integer(b))
          end
        expect(order.total_amount).to eq(BigDecimal(row['expected'])), row['id']
      end
    end

    it 'enforces currency money scale (money_scale_enforced)' do
      rows.call('money_scale_enforced').select { |r| r['currency'] == 'USD' }.each do |row|
        if row['must_reject']
          expect { vector_order(row['amount']) }.to raise_error(StateSet::ValidationError), row['id']
        else
          expect(vector_order(row['amount']).total_amount).to eq(BigDecimal(row['amount']))
        end
      end
    end

    it 'refuses malformed currencies and ids without writing (rejected_inputs)' do
      rows.call('rejected_inputs').each do |row|
        case row['kind']
        when 'currency'
          expect { vector_order('10.00', currency: row['value']) }.to raise_error(StateSet::ValidationError), row['id']
        when 'uuid'
          expect { vector_order('10.00', customer_id: row['value']) }.to raise_error(StateSet::ValidationError), row['id']
        end
      end
      expect(commerce.orders.count).to eq(0)
    end

    it 'keeps accepting case-insensitive currency codes (accepted_inputs)' do
      rows.call('accepted_inputs').select { |r| r['kind'] == 'currency' }.each do |row|
        expect(vector_order('10.00', currency: row['value']).currency).to eq(row['normalizes_to']), row['id']
      end
    end
  end

  describe 'errors' do
    it 'maps engine failures onto a StateSet::Error hierarchy' do
      expect(StateSet::InsufficientStockError.ancestors).to include(StateSet::InvalidOperationError, StateSet::Error)
      error = begin
        commerce.payments.mark_completed(SecureRandom.uuid)
      rescue StateSet::Error => e
        e
      end
      expect(error).to be_a(StateSet::NotFoundError)
      expect(error.status).to eq(404)
      expect(error.code).to be_a(String)
    end

    it 'reports malformed ids as ValidationError, not a crash' do
      expect { commerce.orders.get('not-a-uuid') }
        .to raise_error(StateSet::ValidationError) { |e| expect(e.code).to eq('binding.invalid_argument') }
    end

    it 'raises when the database path cannot be opened' do
      expect { StateSet::Commerce.new('/nonexistent-dir/deeper/store.db') }.to raise_error(StateSet::Error)
    end
  end

  describe 'surface' do
    it 'exposes only engine-backed domains' do
      domains = %i[customers products inventory carts orders payments returns shipments]
      domains.each { |d| expect(commerce).to respond_to(d) }
      # The pre-1.37 gem advertised these, backed by nothing; they are gone.
      %i[warranties purchase_orders invoices bom work_orders analytics currency subscriptions promotions tax]
        .each { |d| expect(commerce).not_to respond_to(d) }
    end

    it 'reports the workspace version' do
      expect(StateSet::VERSION).to eq('1.37.0')
    end
  end
end
