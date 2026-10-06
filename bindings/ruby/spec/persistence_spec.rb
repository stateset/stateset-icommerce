# frozen_string_literal: true

# The test that proves this binding is the real engine and not an in-memory
# stand-in: data written through one Commerce instance must be durable in the
# SQLite file and readable by a NEW instance -- and by a separate Ruby
# process -- that opens the same path. (Until 1.37 the gem ignored db_path
# and kept everything in a Ruby-side hash, so this test could not pass.)

require 'spec_helper'
require 'json'
require 'open3'
require 'rbconfig'

RSpec.describe 'persistence to the SQLite file' do
  around do |example|
    Dir.mktmpdir('stateset-ruby') do |dir|
      @db_path = File.join(dir, 'store.db')
      example.run
    end
  end

  let(:db_path) { @db_path }

  it 'creates the database file at db_path' do
    expect(File.exist?(db_path)).to be(false)
    StateSet::Commerce.new(db_path)
    expect(File.size(db_path)).to be > 0
  end

  it 'reads back, from a fresh instance, everything the first instance wrote' do
    writer = StateSet::Commerce.new(db_path)
    customer = create_customer(writer, email: 'durable@example.com')
    order = create_order(writer, customer: customer, price: '19.99', quantity: 3)
    payment = writer.payments.create(
      order_id: order.id, customer_id: customer.id,
      amount: order.total_amount, payment_method: 'credit_card'
    )
    writer.inventory.create_item(sku: 'PERSIST-1', name: 'Persisted', initial_quantity: 7)

    reader = StateSet::Commerce.new(db_path)
    expect(reader.customers.get(customer.id)).to eq(customer)
    expect(reader.customers.get_by_email('durable@example.com').id).to eq(customer.id)

    reread = reader.orders.get(order.id)
    expect(reread.order_number).to eq(order.order_number)
    expect(reread.total_amount).to eq(BigDecimal('59.97'))
    expect(reread.items.map(&:quantity)).to eq([3])

    expect(reader.payments.get(payment.id).amount).to eq(BigDecimal('59.97'))
    expect(reader.inventory.get_stock('PERSIST-1').total_on_hand).to eq(BigDecimal('7'))
  end

  it 'is visible to a separate Ruby process opening the same file' do
    writer = StateSet::Commerce.new(db_path)
    customer = create_customer(writer, email: 'cross-process@example.com')

    lib = File.expand_path('../lib', __dir__)
    script = <<~RUBY
      require 'stateset_embedded'
      c = StateSet::Commerce.new(ARGV.fetch(0))
      found = c.customers.get(ARGV.fetch(1))
      puts JSON.generate(email: found&.email, count: c.customers.count)
    RUBY
    out, status = Open3.capture2(RbConfig.ruby, '-I', lib, '-e', script, db_path, customer.id)

    expect(status).to be_success
    expect(JSON.parse(out)).to eq('email' => 'cross-process@example.com', 'count' => 1)
  end

  it 'keeps each :memory: store private and ephemeral' do
    first = memory_store
    create_customer(first)
    expect(first.customers.count).to eq(1)
    expect(memory_store.customers.count).to eq(0)
  end
end
