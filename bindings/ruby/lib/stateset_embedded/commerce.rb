# frozen_string_literal: true

module StateSet
  # An open commerce store, backed by the Rust engine.
  #
  #   commerce = StateSet::Commerce.new('store.db')  # SQLite file, created if missing
  #   commerce = StateSet::Commerce.new(':memory:')  # ephemeral, gone when collected
  #
  # Data written through one instance is durable in the file and visible to
  # any later instance (or process) that opens the same path.
  class Commerce
    MEMORY = ':memory:'

    attr_reader :db_path

    def initialize(db_path = MEMORY)
      @db_path = db_path.to_s
      raise ValidationError.new('db_path must not be empty', code: 'binding.invalid_argument', status: 400) if @db_path.empty?

      opened = Native.open(@db_path)
      Wire.unwrap(opened) if opened.is_a?(String) # raises the engine's open error
      @native = opened
    end

    def in_memory?
      db_path == MEMORY
    end

    def customers = @customers ||= Customers.new(@native)
    def products = @products ||= Products.new(@native)
    def inventory = @inventory ||= Inventory.new(@native)
    def carts = @carts ||= Carts.new(@native)
    def orders = @orders ||= Orders.new(@native)
    def payments = @payments ||= Payments.new(@native)
    def returns = @returns ||= Returns.new(@native)
    def shipments = @shipments ||= Shipments.new(@native)

    def inspect
      "#<#{self.class.name} db_path=#{db_path.inspect}>"
    end
  end
end
