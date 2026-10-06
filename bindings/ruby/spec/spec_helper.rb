# frozen_string_literal: true

require 'stateset_embedded'
require 'securerandom'
require 'tmpdir'

# Shared fixtures. Everything here goes through the public API into the real
# engine; nothing is stubbed.
module CommerceFixtures
  def memory_store
    StateSet::Commerce.new(':memory:')
  end

  def unique_email(prefix = 'shopper')
    "#{prefix}-#{SecureRandom.hex(4)}@example.com"
  end

  def create_customer(commerce, email: unique_email)
    commerce.customers.create(email: email, first_name: 'Ada', last_name: 'Lovelace')
  end

  def create_product(commerce, sku: "SKU-#{SecureRandom.hex(3)}", price: '29.99')
    product = commerce.products.create(
      name: "Widget Pro #{sku}", # the slug derives from the name and must be unique
      description: 'The best widget',
      variants: [{ sku: sku, price: price, name: 'Default' }]
    )
    product = commerce.products.activate(product.id) # draft products are not purchasable
    [product, commerce.products.get_variant_by_sku(sku)]
  end

  def order_line(product, sku:, price:, quantity: 1)
    { product_id: product.id, sku: sku, name: 'Widget Pro', quantity: quantity, unit_price: price }
  end

  def create_order(commerce, customer:, price: '29.99', quantity: 2)
    product, variant = create_product(commerce, price: price)
    commerce.orders.create(
      customer_id: customer.id,
      items: [order_line(product, sku: variant.sku, price: price, quantity: quantity)]
    )
  end

  def address
    {
      first_name: 'Ada', last_name: 'Lovelace', line1: '1 Analytical Way',
      city: 'London', postal_code: 'N1 9GU', country: 'GB'
    }
  end
end

RSpec.configure do |config|
  config.include CommerceFixtures

  config.expect_with :rspec do |expectations|
    expectations.include_chain_clauses_in_custom_matcher_descriptions = true
  end

  config.mock_with :rspec do |mocks|
    mocks.verify_partial_doubles = true
  end

  config.shared_context_metadata_behavior = :apply_to_host_groups
  config.filter_run_when_matching :focus
  config.disable_monkey_patching!
  config.warnings = true

  config.default_formatter = 'doc' if config.files_to_run.one?

  config.order = :random
  Kernel.srand config.seed
end
