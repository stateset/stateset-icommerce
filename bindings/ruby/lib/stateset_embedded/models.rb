# frozen_string_literal: true

module StateSet
  # Engine records. Field names are the engine's own (snake_case); the
  # declarations below only say which fields are exact decimals (BigDecimal)
  # and which hold nested records. Keep them in step with the engine structs
  # in `crates/stateset-core/src/models/` -- spec/models_spec.rb checks every
  # declared field is actually returned by the engine.

  # ---- Customers -----------------------------------------------------------

  class Customer < Model
    def full_name
      "#{self[:first_name]} #{self[:last_name]}".strip
    end
  end

  # ---- Products ------------------------------------------------------------

  class Product < Model; end

  class ProductVariant < Model
    decimal :price, :compare_at_price, :cost, :weight
  end

  # ---- Inventory -----------------------------------------------------------

  class InventoryItem < Model; end

  class LocationStock < Model
    decimal :on_hand, :allocated, :available
  end

  class StockLevel < Model
    decimal :total_on_hand, :total_allocated, :total_available
    nested locations: LocationStock
  end

  class InventoryReservation < Model
    decimal :quantity
  end

  class InventoryTransaction < Model
    decimal :quantity
  end

  # ---- Carts & checkout ----------------------------------------------------

  class CartItem < Model
    decimal :unit_price, :original_price, :discount_amount, :tax_amount, :total, :weight
  end

  class Cart < Model
    decimal :subtotal, :tax_amount, :shipping_amount, :discount_amount, :grand_total
    nested items: CartItem
    time :estimated_delivery
  end

  class ShippingRate < Model
    decimal :price
    time :estimated_delivery
  end

  class CheckoutResult < Model
    decimal :total_charged
  end

  # ---- Orders --------------------------------------------------------------

  class OrderItem < Model
    decimal :unit_price, :discount, :tax_amount, :total
  end

  class Order < Model
    decimal :total_amount, :tax_amount, :shipping_amount, :discount_amount
    nested items: OrderItem
    time :order_date

    def item_count
      self[:items].size
    end
  end

  # ---- Payments & refunds --------------------------------------------------

  class Payment < Model
    decimal :amount, :amount_refunded
  end

  class Refund < Model
    decimal :amount
  end

  # ---- Returns -------------------------------------------------------------

  class ReturnItem < Model
    decimal :refund_amount
  end

  class Return < Model
    decimal :refund_amount
    nested items: ReturnItem
  end

  # ---- Shipments -----------------------------------------------------------

  class ShipmentItem < Model; end

  class ShipmentEvent < Model
    time :event_time
  end

  class Shipment < Model
    decimal :weight_kg, :shipping_cost, :insurance_amount
    nested items: ShipmentItem, events: ShipmentEvent
    time :estimated_delivery
  end
end
