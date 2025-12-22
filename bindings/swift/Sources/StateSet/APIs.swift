import Foundation
import StateSetC

// MARK: - Customers API

public final class CustomersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(email: String, firstName: String, lastName: String, phone: String? = nil) throws -> Customer {
        let handle = try commerce!.getHandle()
        let result = stateset_customer_create(handle, email, firstName, lastName, phone)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Customer? {
        let handle = try commerce!.getHandle()
        let result = stateset_customer_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Customer] {
        let handle = try commerce!.getHandle()
        let result = stateset_customer_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func delete(id: String) throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_customer_delete(handle, id) == 1
    }
}

// MARK: - Products API

public final class ProductsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(name: String, sku: String, price: Double, description: String? = nil) throws -> Product {
        let handle = try commerce!.getHandle()
        let result = stateset_product_create(handle, name, sku, price, description)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Product? {
        let handle = try commerce!.getHandle()
        let result = stateset_product_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Product] {
        let handle = try commerce!.getHandle()
        let result = stateset_product_list(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Orders API

public final class OrdersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(customerId: String, items: [OrderItem], currency: String = "USD") throws -> Order {
        let handle = try commerce!.getHandle()

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let itemsData = try encoder.encode(items)
        let itemsJSON = String(data: itemsData, encoding: .utf8) ?? "[]"

        let result = stateset_order_create(handle, customerId, itemsJSON, currency)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Order? {
        let handle = try commerce!.getHandle()
        let result = stateset_order_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Order] {
        let handle = try commerce!.getHandle()
        let result = stateset_order_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func updateStatus(id: String, status: OrderStatus) throws -> Order {
        let handle = try commerce!.getHandle()
        let result = stateset_order_update_status(handle, id, status.rawValue)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Inventory API

public final class InventoryAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createItem(sku: String, name: String, initialQuantity: Double = 0) throws -> InventoryItem {
        let handle = try commerce!.getHandle()
        let result = stateset_inventory_create_item(handle, sku, name, initialQuantity)
        return try commerce!.parseJSON(result)
    }

    public func adjust(sku: String, quantityDelta: Double, reason: String = "manual adjustment") throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_inventory_adjust(handle, sku, quantityDelta, reason) == 1
    }

    public func getLevel(sku: String) throws -> StockLevel? {
        let handle = try commerce!.getHandle()
        let result = stateset_inventory_get_level(handle, sku)
        return commerce!.parseOptionalJSON(result)
    }
}

// MARK: - Carts API

public final class CartsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(customerId: String? = nil, currency: String = "USD") throws -> Cart {
        let handle = try commerce!.getHandle()
        let result = stateset_cart_create(handle, customerId, currency)
        return try commerce!.parseJSON(result)
    }

    public func addItem(cartId: String, variantId: String, quantity: Int = 1) throws -> Cart {
        let handle = try commerce!.getHandle()
        let result = stateset_cart_add_item(handle, cartId, variantId, Int32(quantity))
        return try commerce!.parseJSON(result)
    }

    public func get(cartId: String) throws -> Cart? {
        let handle = try commerce!.getHandle()
        let result = stateset_cart_get(handle, cartId)
        return commerce!.parseOptionalJSON(result)
    }
}

// MARK: - Returns API

public final class ReturnsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(orderId: String, reason: ReturnReason, notes: String? = nil) throws -> Return {
        let handle = try commerce!.getHandle()
        let result = stateset_return_create(handle, orderId, reason.rawValue, notes)
        return try commerce!.parseJSON(result)
    }

    public func list() throws -> [Return] {
        let handle = try commerce!.getHandle()
        let result = stateset_return_list(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Payments API

public final class PaymentsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(orderId: String, amount: Double, currency: String = "USD", method: PaymentMethod = .creditCard) throws -> Payment {
        let handle = try commerce!.getHandle()
        let result = stateset_payment_create(handle, orderId, amount, currency, method.rawValue)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Analytics API

public final class AnalyticsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func salesSummary(period: TimePeriod = .thisMonth) throws -> SalesSummary {
        let handle = try commerce!.getHandle()
        let result = stateset_analytics_sales_summary(handle, period.rawValue)
        return try commerce!.parseJSON(result)
    }

    public func topProducts(limit: Int = 10) throws -> [TopProduct] {
        let handle = try commerce!.getHandle()
        let result = stateset_analytics_top_products(handle, Int32(limit))
        return try commerce!.parseJSON(result)
    }

    public func topCustomers(limit: Int = 10) throws -> [TopCustomer] {
        let handle = try commerce!.getHandle()
        let result = stateset_analytics_top_customers(handle, Int32(limit))
        return try commerce!.parseJSON(result)
    }
}
