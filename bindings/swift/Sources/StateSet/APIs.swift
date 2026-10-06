import Foundation

// Each API is a thin, typed facade over one engine domain. Every method is a
// single native call to the Rust engine; the quoted method names are the
// stateset-ffi JSON surface (crates/stateset-ffi/src/json_api.rs). Nothing
// here keeps state.

/// Customers.
public final class CustomersAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    public func create(email: String, firstName: String, lastName: String, phone: String? = nil,
                       acceptsMarketing: Bool? = nil, tags: [String]? = nil) throws -> Customer {
        try c.call("customers.create", ["email": email, "first_name": firstName, "last_name": lastName,
                                        "phone": phone, "accepts_marketing": acceptsMarketing, "tags": tags])
    }

    public func get(id: String) throws -> Customer? { try c.callOptional("customers.get", ["id": id]) }

    public func get(email: String) throws -> Customer? {
        try c.callOptional("customers.get_by_email", ["email": email])
    }

    /// Update fields; `nil` leaves a field unchanged.
    public func update(id: String, email: String? = nil, firstName: String? = nil, lastName: String? = nil,
                       phone: String? = nil, status: CustomerStatus? = nil) throws -> Customer {
        try c.call("customers.update", ["id": id, "input": [
            "email": email, "first_name": firstName, "last_name": lastName, "phone": phone,
            "status": status?.rawValue,
        ] as [String: Any?]])
    }

    public func list(limit: Int? = nil, offset: Int? = nil) throws -> [Customer] {
        try c.call("customers.list", ["limit": limit, "offset": offset])
    }

    public func count() throws -> Int64 { try c.call("customers.count", nil) }

    public func delete(id: String) throws { try c.callVoid("customers.delete", ["id": id]) }
}

/// Products and variants.
public final class ProductsAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    /// Create a product with one default variant carrying `sku` and `price`.
    public func create(name: String, sku: String, price: Decimal, description: String? = nil) throws -> Product {
        try create(name: name, variants: [CreateProductVariant(sku: sku, price: price, isDefault: true)],
                   description: description)
    }

    public func create(name: String, variants: [CreateProductVariant], description: String? = nil,
                       slug: String? = nil, productType: ProductType? = nil) throws -> Product {
        try c.call("products.create", ["name": name, "slug": slug, "description": description,
                                       "product_type": productType?.rawValue, "variants": variants])
    }

    public func get(id: String) throws -> Product? { try c.callOptional("products.get", ["id": id]) }

    public func get(slug: String) throws -> Product? { try c.callOptional("products.get_by_slug", ["slug": slug]) }

    public func list(limit: Int? = nil, offset: Int? = nil) throws -> [Product] {
        try c.call("products.list", ["limit": limit, "offset": offset])
    }

    public func count() throws -> Int64 { try c.call("products.count", nil) }

    public func search(_ query: String) throws -> [Product] { try c.call("products.search", ["query": query]) }

    public func activate(id: String) throws -> Product { try c.call("products.activate", ["id": id]) }

    public func archive(id: String) throws -> Product { try c.call("products.archive", ["id": id]) }

    public func delete(id: String) throws { try c.callVoid("products.delete", ["id": id]) }

    public func addVariant(productId: String, variant: CreateProductVariant) throws -> ProductVariant {
        try c.call("products.add_variant", ["product_id": productId, "variant": variant])
    }

    public func variants(productId: String) throws -> [ProductVariant] {
        try c.call("products.get_variants", ["product_id": productId])
    }

    public func variant(sku: String) throws -> ProductVariant? {
        try c.callOptional("products.get_variant_by_sku", ["sku": sku])
    }
}

/// Inventory.
public final class InventoryAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    public func createItem(sku: String, name: String, initialQuantity: Decimal? = nil,
                           description: String? = nil, reorderPoint: Decimal? = nil) throws -> InventoryItem {
        try c.call("inventory.create_item", ["sku": sku, "name": name, "description": description,
                                             "initial_quantity": initialQuantity, "reorder_point": reorderPoint])
    }

    public func item(sku: String) throws -> InventoryItem? {
        try c.callOptional("inventory.get_item_by_sku", ["sku": sku])
    }

    public func list() throws -> [InventoryItem] { try c.call("inventory.list", nil) }

    /// Aggregated stock for a SKU, or `nil` when the SKU is unknown.
    public func getStock(sku: String) throws -> StockLevel? {
        try c.callOptional("inventory.get_stock", ["sku": sku])
    }

    /// Adjust on-hand stock by `quantityDelta` (negative to remove).
    @discardableResult
    public func adjust(sku: String, quantityDelta: Decimal, reason: String = "manual adjustment") throws
        -> InventoryTransaction {
        try c.call("inventory.adjust", ["sku": sku, "quantity": quantityDelta, "reason": reason])
    }

    public func hasStock(sku: String, quantity: Decimal) throws -> Bool {
        try c.call("inventory.has_stock", ["sku": sku, "quantity": quantity])
    }

    public func reserve(sku: String, quantity: Decimal, referenceType: String, referenceId: String,
                        expiresInSeconds: Int64? = nil) throws -> InventoryReservation {
        try c.call("inventory.reserve", ["sku": sku, "quantity": quantity, "reference_type": referenceType,
                                         "reference_id": referenceId, "expires_in_seconds": expiresInSeconds])
    }

    public func releaseReservation(id: String) throws {
        try c.callVoid("inventory.release_reservation", ["id": id])
    }

    public func confirmReservation(id: String) throws {
        try c.callVoid("inventory.confirm_reservation", ["id": id])
    }
}

/// Carts and checkout.
public final class CartsAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    /// Create a cart; `currency` nil means the store default.
    public func create(customerId: String? = nil, currency: String? = nil, customerEmail: String? = nil,
                       customerName: String? = nil) throws -> Cart {
        try c.call("carts.create", ["customer_id": customerId, "currency": currency,
                                    "customer_email": customerEmail, "customer_name": customerName])
    }

    public func get(id: String) throws -> Cart? { try c.callOptional("carts.get", ["id": id]) }

    public func list() throws -> [Cart] { try c.call("carts.list", nil) }

    @discardableResult
    public func addItem(cartId: String, item: AddCartItem) throws -> CartItem {
        try c.call("carts.add_item", ["cart_id": cartId, "item": item])
    }

    @discardableResult
    public func addItem(cartId: String, sku: String, name: String, quantity: Int32, unitPrice: Decimal,
                        productId: String? = nil) throws -> CartItem {
        try addItem(cartId: cartId, item: AddCartItem(sku: sku, name: name, quantity: quantity,
                                                      unitPrice: unitPrice, productId: productId))
    }

    public func updateItemQuantity(itemId: String, quantity: Int32) throws -> CartItem {
        try c.call("carts.update_item", ["item_id": itemId, "input": ["quantity": Int(quantity)]])
    }

    public func removeItem(itemId: String) throws { try c.callVoid("carts.remove_item", ["item_id": itemId]) }

    public func items(cartId: String) throws -> [CartItem] { try c.call("carts.get_items", ["id": cartId]) }

    public func clearItems(cartId: String) throws { try c.callVoid("carts.clear_items", ["id": cartId]) }

    @discardableResult
    public func setShippingAddress(cartId: String, address: CartAddress) throws -> Cart {
        try c.call("carts.set_shipping_address", ["id": cartId, "address": address])
    }

    @discardableResult
    public func setBillingAddress(cartId: String, address: CartAddress) throws -> Cart {
        try c.call("carts.set_billing_address", ["id": cartId, "address": address])
    }

    @discardableResult
    public func setShipping(cartId: String, address: CartAddress, method: String? = nil, carrier: String? = nil,
                            amount: Decimal? = nil) throws -> Cart {
        try c.call("carts.set_shipping", ["id": cartId, "shipping": [
            "shipping_address": address, "shipping_method": method, "shipping_carrier": carrier,
            "shipping_amount": amount,
        ] as [String: Any?]])
    }

    @discardableResult
    public func setPayment(cartId: String, paymentMethod: String, paymentToken: String? = nil) throws -> Cart {
        try c.call("carts.set_payment", ["id": cartId, "payment": [
            "payment_method": paymentMethod, "payment_token": paymentToken,
        ] as [String: Any?]])
    }

    public func applyDiscount(cartId: String, couponCode: String) throws -> Cart {
        try c.call("carts.apply_discount", ["id": cartId, "coupon_code": couponCode])
    }

    public func recalculate(cartId: String) throws -> Cart { try c.call("carts.recalculate", ["id": cartId]) }

    /// Complete checkout: creates the order (and payment record).
    public func complete(cartId: String) throws -> CheckoutResult { try c.call("carts.complete", ["id": cartId]) }

    public func cancel(cartId: String) throws -> Cart { try c.call("carts.cancel", ["id": cartId]) }

    public func abandon(cartId: String) throws -> Cart { try c.call("carts.abandon", ["id": cartId]) }
}

/// Orders.
public final class OrdersAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    /// Create an order. `currency` nil means the store default; an explicit
    /// value must be a valid ISO 4217 code.
    public func create(customerId: String, items: [CreateOrderItem], currency: String? = nil,
                       shippingAddress: Address? = nil, notes: String? = nil) throws -> Order {
        try c.call("orders.create", ["customer_id": customerId, "items": items, "currency": currency,
                                     "shipping_address": shippingAddress, "notes": notes])
    }

    public func get(id: String) throws -> Order? { try c.callOptional("orders.get", ["id": id]) }

    public func get(orderNumber: String) throws -> Order? {
        try c.callOptional("orders.get_by_number", ["order_number": orderNumber])
    }

    public func list(limit: Int? = nil, offset: Int? = nil) throws -> [Order] {
        try c.call("orders.list", ["limit": limit, "offset": offset])
    }

    public func list(customerId: String) throws -> [Order] {
        try c.call("orders.list_for_customer", ["customer_id": customerId])
    }

    public func count() throws -> Int64 { try c.call("orders.count", nil) }

    /// Transition status; the engine enforces the state machine.
    public func updateStatus(id: String, status: OrderStatus) throws -> Order {
        try c.call("orders.update_status", ["id": id, "status": status.rawValue])
    }

    public func ship(id: String, trackingNumber: String? = nil) throws -> Order {
        try c.call("orders.ship", ["id": id, "tracking_number": trackingNumber])
    }

    public func deliver(id: String) throws -> Order { try c.call("orders.deliver", ["id": id]) }

    public func cancel(id: String) throws -> Order { try c.call("orders.cancel", ["id": id]) }
}

/// Payments and refunds.
public final class PaymentsAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    public func create(orderId: String, amount: Decimal, currency: String? = nil,
                       method: PaymentMethod = .creditCard, customerId: String? = nil,
                       externalId: String? = nil, idempotencyKey: String? = nil) throws -> Payment {
        try c.call("payments.create", ["order_id": orderId, "amount": amount, "currency": currency,
                                       "payment_method": method.rawValue, "customer_id": customerId,
                                       "external_id": externalId, "idempotency_key": idempotencyKey])
    }

    public func get(id: String) throws -> Payment? { try c.callOptional("payments.get", ["id": id]) }

    public func list() throws -> [Payment] { try c.call("payments.list", nil) }

    public func list(orderId: String) throws -> [Payment] {
        try c.call("payments.for_order", ["order_id": orderId])
    }

    public func markProcessing(id: String) throws -> Payment { try c.call("payments.mark_processing", ["id": id]) }

    /// Mark a payment completed (captured).
    public func complete(id: String) throws -> Payment { try c.call("payments.mark_completed", ["id": id]) }

    public func fail(id: String, reason: String, code: String? = nil) throws -> Payment {
        try c.call("payments.mark_failed", ["id": id, "reason": reason, "code": code])
    }

    public func cancel(id: String) throws -> Payment { try c.call("payments.cancel", ["id": id]) }

    /// Create a refund; `amount` nil refunds the remaining balance.
    public func refund(paymentId: String, amount: Decimal?, reason: String? = nil,
                       idempotencyKey: String? = nil) throws -> Refund {
        try c.call("payments.create_refund", ["payment_id": paymentId, "amount": amount, "reason": reason,
                                              "idempotency_key": idempotencyKey])
    }

    public func refund(id: String) throws -> Refund? { try c.callOptional("payments.get_refund", ["id": id]) }

    public func refunds(paymentId: String) throws -> [Refund] {
        try c.call("payments.get_refunds", ["payment_id": paymentId])
    }

    public func completeRefund(id: String) throws -> Refund { try c.call("payments.complete_refund", ["id": id]) }

    public func failRefund(id: String, reason: String) throws -> Refund {
        try c.call("payments.fail_refund", ["id": id, "reason": reason])
    }
}

/// Returns (RMAs).
public final class ReturnsAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    public func create(orderId: String, reason: ReturnReason, items: [CreateReturnItem],
                       reasonDetails: String? = nil, notes: String? = nil) throws -> Return {
        try c.call("returns.create", ["order_id": orderId, "reason": reason.rawValue, "items": items,
                                      "reason_details": reasonDetails, "notes": notes])
    }

    public func get(id: String) throws -> Return? { try c.callOptional("returns.get", ["id": id]) }

    public func list() throws -> [Return] { try c.call("returns.list", nil) }

    public func list(orderId: String) throws -> [Return] {
        try c.call("returns.list_for_order", ["order_id": orderId])
    }

    public func approve(id: String) throws -> Return { try c.call("returns.approve", ["id": id]) }

    public func reject(id: String, reason: String) throws -> Return {
        try c.call("returns.reject", ["id": id, "reason": reason])
    }

    public func markReceived(id: String) throws -> Return { try c.call("returns.mark_received", ["id": id]) }

    /// Decide what happens to a received item; every item needs one before `complete`.
    public func setItemDisposition(id: String, itemId: String, disposition: ReturnDisposition,
                                   warehouseId: Int? = nil, dispositionBy: String? = nil) throws -> ReturnItem {
        try c.call("returns.set_item_disposition", ["id": id, "item_id": itemId,
                                                    "disposition": disposition.rawValue,
                                                    "warehouse_id": warehouseId, "disposition_by": dispositionBy])
    }

    public func complete(id: String) throws -> Return { try c.call("returns.complete", ["id": id]) }

    public func cancel(id: String) throws -> Return { try c.call("returns.cancel", ["id": id]) }

    public func addTracking(id: String, trackingNumber: String) throws -> Return {
        try c.call("returns.add_tracking", ["id": id, "tracking_number": trackingNumber])
    }
}

/// Shipments.
public final class ShipmentsAPI: @unchecked Sendable {
    private unowned let c: StateSetCommerce
    init(commerce: StateSetCommerce) { c = commerce }

    public func create(orderId: String, recipientName: String, shippingAddress: String,
                       carrier: ShippingCarrier? = nil, method: ShippingMethod? = nil,
                       trackingNumber: String? = nil, items: [CreateShipmentItem]? = nil,
                       shippingCost: Decimal? = nil) throws -> Shipment {
        try c.call("shipments.create", ["order_id": orderId, "recipient_name": recipientName,
                                        "shipping_address": shippingAddress, "carrier": carrier?.rawValue,
                                        "shipping_method": method?.rawValue, "tracking_number": trackingNumber,
                                        "items": items, "shipping_cost": shippingCost])
    }

    public func get(id: String) throws -> Shipment? { try c.callOptional("shipments.get", ["id": id]) }

    public func get(trackingNumber: String) throws -> Shipment? {
        try c.callOptional("shipments.get_by_tracking", ["tracking_number": trackingNumber])
    }

    public func list() throws -> [Shipment] { try c.call("shipments.list", nil) }

    public func list(orderId: String) throws -> [Shipment] {
        try c.call("shipments.for_order", ["order_id": orderId])
    }

    public func markProcessing(id: String) throws -> Shipment { try c.call("shipments.mark_processing", ["id": id]) }

    public func markReady(id: String) throws -> Shipment { try c.call("shipments.mark_ready", ["id": id]) }

    public func ship(id: String, trackingNumber: String? = nil) throws -> Shipment {
        try c.call("shipments.ship", ["id": id, "tracking_number": trackingNumber])
    }

    public func markInTransit(id: String) throws -> Shipment { try c.call("shipments.mark_in_transit", ["id": id]) }

    public func markOutForDelivery(id: String) throws -> Shipment {
        try c.call("shipments.mark_out_for_delivery", ["id": id])
    }

    /// Mark delivered (from out-for-delivery).
    public func deliver(id: String) throws -> Shipment { try c.call("shipments.mark_delivered", ["id": id]) }

    public func cancel(id: String) throws -> Shipment { try c.call("shipments.cancel", ["id": id]) }
}
