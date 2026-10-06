# frozen_string_literal: true

module StateSet
  # Base for the per-domain APIs returned by {Commerce}. Every method is one
  # call into the engine; there is no Ruby-side state.
  #
  # Conventions:
  # * create/update methods take the engine input's fields as keywords
  #   (`customers.create(email: ..., first_name: ..., last_name: ...)`).
  #   Unknown keywords are refused with {ValidationError}.
  # * list/count take the engine filter's fields as keywords
  #   (`orders.list(status: 'pending', limit: 10)`).
  # * get-style methods return `nil` when nothing matches; state changes on a
  #   missing record raise {NotFoundError}.
  # * Money and quantities are BigDecimal on the way out and must be
  #   BigDecimal, Integer or a decimal String on the way in (Float raises).
  class Api
    def initialize(native)
      @native = native
    end

    private

    def call(op, args = {})
      Wire.call(@native, op, args)
    end

    def one(klass, op, args = {})
      klass.from(call(op, args))
    end

    def many(klass, op, args = {})
      klass.from_list(call(op, args))
    end

    def done(op, args = {})
      call(op, args)
      true
    end
  end

  # Customer accounts.
  class Customers < Api
    def create(**input) = one(Customer, 'customers.create', input: input)
    def get(id) = one(Customer, 'customers.get', id: id)
    def get_by_email(email) = one(Customer, 'customers.get_by_email', email: email)
    def update(id, **input) = one(Customer, 'customers.update', id: id, input: input)
    def list(**filter) = many(Customer, 'customers.list', filter: filter)
    def count(**filter) = call('customers.count', filter: filter)
    def delete(id) = done('customers.delete', id: id)
  end

  # Product catalog and variants.
  class Products < Api
    def create(**input) = one(Product, 'products.create', input: input)
    def get(id) = one(Product, 'products.get', id: id)
    def get_by_slug(slug) = one(Product, 'products.get_by_slug', slug: slug)
    def update(id, **input) = one(Product, 'products.update', id: id, input: input)
    def list(**filter) = many(Product, 'products.list', filter: filter)
    def count(**filter) = call('products.count', filter: filter)
    def search(query) = many(Product, 'products.search', query: query)
    def delete(id) = done('products.delete', id: id)
    def activate(id) = one(Product, 'products.activate', id: id)
    def archive(id) = one(Product, 'products.archive', id: id)

    def add_variant(product_id, **input)
      one(ProductVariant, 'products.add_variant', product_id: product_id, input: input)
    end

    def get_variant(id) = one(ProductVariant, 'products.get_variant', id: id)
    def get_variant_by_sku(sku) = one(ProductVariant, 'products.get_variant_by_sku', sku: sku)
    def get_variants(product_id) = many(ProductVariant, 'products.get_variants', product_id: product_id)
  end

  # Stock items, balances and reservations.
  class Inventory < Api
    def create_item(**input) = one(InventoryItem, 'inventory.create_item', input: input)
    def get_item(id) = one(InventoryItem, 'inventory.get_item', id: id)
    def get_item_by_sku(sku) = one(InventoryItem, 'inventory.get_item_by_sku', sku: sku)
    def get_stock(sku) = one(StockLevel, 'inventory.get_stock', sku: sku)
    def list(**filter) = many(InventoryItem, 'inventory.list', filter: filter)

    # Adjust on-hand stock by a signed delta.
    def adjust(sku, quantity, reason)
      one(InventoryTransaction, 'inventory.adjust', sku: sku, quantity: quantity, reason: reason)
    end

    def reserve(sku, quantity, reference_type:, reference_id:, expires_in_seconds: nil)
      one(InventoryReservation, 'inventory.reserve',
          sku: sku, quantity: quantity, reference_type: reference_type,
          reference_id: reference_id, expires_in_seconds: expires_in_seconds)
    end

    def get_reservation(id) = one(InventoryReservation, 'inventory.get_reservation', id: id)
    def confirm_reservation(id) = done('inventory.confirm_reservation', id: id)
    def release_reservation(id) = done('inventory.release_reservation', id: id)

    def get_transactions(item_id, limit: 100)
      many(InventoryTransaction, 'inventory.get_transactions', item_id: item_id, limit: limit)
    end

    def has_stock?(sku, quantity) = call('inventory.has_stock', sku: sku, quantity: quantity)
  end

  # Shopping carts and checkout.
  class Carts < Api
    def create(**input) = one(Cart, 'carts.create', input: input)
    def get(id) = one(Cart, 'carts.get', id: id)
    def get_by_number(cart_number) = one(Cart, 'carts.get_by_number', cart_number: cart_number)
    def list(**filter) = many(Cart, 'carts.list', filter: filter)
    def count(**filter) = call('carts.count', filter: filter)
    def for_customer(customer_id) = many(Cart, 'carts.for_customer', customer_id: customer_id)
    def delete(id) = done('carts.delete', id: id)
    def add_item(cart_id, **item) = one(CartItem, 'carts.add_item', cart_id: cart_id, input: item)
    def update_item(item_id, **input) = one(CartItem, 'carts.update_item', item_id: item_id, input: input)
    def remove_item(item_id) = done('carts.remove_item', item_id: item_id)
    def get_items(cart_id) = many(CartItem, 'carts.get_items', cart_id: cart_id)
    def clear_items(cart_id) = done('carts.clear_items', cart_id: cart_id)
    def set_shipping_address(id, **address) = one(Cart, 'carts.set_shipping_address', id: id, address: address)
    def set_billing_address(id, **address) = one(Cart, 'carts.set_billing_address', id: id, address: address)

    # `address:` is a shipping address hash (first_name, last_name, line1,
    # city, postal_code, country, ...).
    def set_shipping(id, address:, **rest)
      one(Cart, 'carts.set_shipping', id: id, input: { shipping_address: address, **rest })
    end

    def get_shipping_rates(id) = many(ShippingRate, 'carts.get_shipping_rates', id: id)
    def set_payment(id, **input) = one(Cart, 'carts.set_payment', id: id, input: input)
    def set_tax(id, tax_amount) = one(Cart, 'carts.set_tax', id: id, tax_amount: tax_amount)
    def apply_discount(id, coupon_code) = one(Cart, 'carts.apply_discount', id: id, coupon_code: coupon_code)
    def remove_discount(id) = one(Cart, 'carts.remove_discount', id: id)
    def recalculate(id) = one(Cart, 'carts.recalculate', id: id)
    def reserve_inventory(id) = one(Cart, 'carts.reserve_inventory', id: id)
    def release_inventory(id) = one(Cart, 'carts.release_inventory', id: id)
    def mark_ready_for_payment(id) = one(Cart, 'carts.mark_ready_for_payment', id: id)
    def begin_checkout(id) = one(Cart, 'carts.begin_checkout', id: id)

    # Complete checkout: mints a Confirmed order with payment Pending. Record
    # the payment with `commerce.payments`.
    def complete(id) = one(CheckoutResult, 'carts.complete', id: id)
    def cancel(id) = one(Cart, 'carts.cancel', id: id)
    def abandon(id) = one(Cart, 'carts.abandon', id: id)
  end

  # Orders and their fulfillment lifecycle.
  class Orders < Api
    def create(**input) = one(Order, 'orders.create', input: input)
    def get(id) = one(Order, 'orders.get', id: id)
    def get_by_number(order_number) = one(Order, 'orders.get_by_number', order_number: order_number)
    def list(**filter) = many(Order, 'orders.list', filter: filter)
    def count(**filter) = call('orders.count', filter: filter)
    def list_for_customer(customer_id) = many(Order, 'orders.list_for_customer', customer_id: customer_id)
    def update_status(id, status) = one(Order, 'orders.update_status', id: id, status: status)
    def cancel(id) = one(Order, 'orders.cancel', id: id)

    # Ship the order (confirming/processing it first if needed). `lines:` is
    # an optional partial shipment: `[{ order_item_id:, quantity: }]`.
    def ship(id, tracking_number: nil, lines: nil)
      one(Order, 'orders.ship', id: id, tracking_number: tracking_number, lines: lines)
    end

    def deliver(id) = one(Order, 'orders.deliver', id: id)
  end

  # Payments and refunds.
  class Payments < Api
    def create(**input) = one(Payment, 'payments.create', input: input)
    def get(id) = one(Payment, 'payments.get', id: id)
    def get_by_number(payment_number) = one(Payment, 'payments.get_by_number', payment_number: payment_number)
    def list(**filter) = many(Payment, 'payments.list', filter: filter)
    def count(**filter) = call('payments.count', filter: filter)
    def for_order(order_id) = many(Payment, 'payments.for_order', order_id: order_id)
    def mark_processing(id) = one(Payment, 'payments.mark_processing', id: id)
    def mark_completed(id) = one(Payment, 'payments.mark_completed', id: id)
    alias complete mark_completed

    def mark_failed(id, reason, code: nil)
      one(Payment, 'payments.mark_failed', id: id, reason: reason, code: code)
    end

    def cancel(id) = one(Payment, 'payments.cancel', id: id)

    # `payment_id:` is required; omit `amount:` to refund the remaining balance.
    def create_refund(**input) = one(Refund, 'payments.create_refund', input: input)
    def get_refund(id) = one(Refund, 'payments.get_refund', id: id)
    def get_refunds(payment_id) = many(Refund, 'payments.get_refunds', payment_id: payment_id)
    def complete_refund(id) = one(Refund, 'payments.complete_refund', id: id)
    def fail_refund(id, reason) = one(Refund, 'payments.fail_refund', id: id, reason: reason)
  end

  # Return merchandise authorizations.
  class Returns < Api
    def create(**input) = one(Return, 'returns.create', input: input)
    def get(id) = one(Return, 'returns.get', id: id)
    def list(**filter) = many(Return, 'returns.list', filter: filter)
    def count(**filter) = call('returns.count', filter: filter)
    def list_for_order(order_id) = many(Return, 'returns.list_for_order', order_id: order_id)
    def approve(id) = one(Return, 'returns.approve', id: id)
    def reject(id, reason) = one(Return, 'returns.reject', id: id, reason: reason)
    def mark_received(id) = one(Return, 'returns.mark_received', id: id)
    def complete(id) = one(Return, 'returns.complete', id: id)
    def cancel(id) = one(Return, 'returns.cancel', id: id)
    def add_tracking(id, tracking_number) = one(Return, 'returns.add_tracking', id: id, tracking_number: tracking_number)

    # Record a received item's disposition (`'restock'`, `'refurbish'`, `'scrap'`,
    # `'return_to_vendor'`, `'quarantine'`) and apply its stock effects. Every
    # item needs one before {#complete}.
    def set_item_disposition(id, item_id, **input)
      one(ReturnItem, 'returns.set_item_disposition', id: id, item_id: item_id, input: input)
    end
  end

  # Outbound shipments.
  class Shipments < Api
    def create(**input) = one(Shipment, 'shipments.create', input: input)
    def get(id) = one(Shipment, 'shipments.get', id: id)
    def get_by_number(shipment_number) = one(Shipment, 'shipments.get_by_number', shipment_number: shipment_number)
    def get_by_tracking(tracking_number) = one(Shipment, 'shipments.get_by_tracking', tracking_number: tracking_number)
    def list(**filter) = many(Shipment, 'shipments.list', filter: filter)
    def count(**filter) = call('shipments.count', filter: filter)
    def for_order(order_id) = many(Shipment, 'shipments.for_order', order_id: order_id)
    def get_items(id) = many(ShipmentItem, 'shipments.get_items', id: id)
    def add_item(id, **item) = one(ShipmentItem, 'shipments.add_item', id: id, input: item)
    def remove_item(item_id) = done('shipments.remove_item', item_id: item_id)
    def mark_processing(id) = one(Shipment, 'shipments.mark_processing', id: id)
    def mark_ready(id) = one(Shipment, 'shipments.mark_ready', id: id)
    def ship(id, tracking_number: nil) = one(Shipment, 'shipments.ship', id: id, tracking_number: tracking_number)
    def mark_in_transit(id) = one(Shipment, 'shipments.mark_in_transit', id: id)
    def mark_out_for_delivery(id) = one(Shipment, 'shipments.mark_out_for_delivery', id: id)
    def mark_delivered(id) = one(Shipment, 'shipments.mark_delivered', id: id)
    def mark_failed(id) = one(Shipment, 'shipments.mark_failed', id: id)
    def hold(id) = one(Shipment, 'shipments.hold', id: id)
    def cancel(id) = one(Shipment, 'shipments.cancel', id: id)
  end
end
