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

    public func ship(id: String) throws -> Order {
        let handle = try commerce!.getHandle()
        let result = stateset_order_ship(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancel(id: String) throws -> Order {
        let handle = try commerce!.getHandle()
        let result = stateset_order_cancel(handle, id)
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

    public func get(id: String) throws -> Return? {
        let handle = try commerce!.getHandle()
        let result = stateset_return_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func approve(id: String) throws -> Return {
        let handle = try commerce!.getHandle()
        let result = stateset_return_approve(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func reject(id: String, reason: String) throws -> Return {
        let handle = try commerce!.getHandle()
        let result = stateset_return_reject(handle, id, reason)
        return try commerce!.parseJSON(result)
    }

    public func complete(id: String) throws -> Return {
        let handle = try commerce!.getHandle()
        let result = stateset_return_complete(handle, id)
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

    public func get(id: String) throws -> Payment? {
        let handle = try commerce!.getHandle()
        let result = stateset_payment_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Payment] {
        let handle = try commerce!.getHandle()
        let result = stateset_payment_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func complete(id: String) throws -> Payment {
        let handle = try commerce!.getHandle()
        let result = stateset_payment_complete(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func fail(id: String, reason: String) throws -> Payment {
        let handle = try commerce!.getHandle()
        let result = stateset_payment_fail(handle, id, reason)
        return try commerce!.parseJSON(result)
    }

    public func refund(paymentId: String, amount: Double, reason: String) throws -> Refund {
        let handle = try commerce!.getHandle()
        let result = stateset_payment_refund(handle, paymentId, amount, reason)
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

// MARK: - Shipments API

public final class ShipmentsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(orderId: String, recipientName: String, shippingAddress: String, carrier: String) throws -> Shipment {
        let handle = try commerce!.getHandle()
        let result = stateset_shipment_create(handle, orderId, recipientName, shippingAddress, carrier)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Shipment? {
        let handle = try commerce!.getHandle()
        let result = stateset_shipment_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Shipment] {
        let handle = try commerce!.getHandle()
        let result = stateset_shipment_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func ship(id: String, trackingNumber: String) throws -> Shipment {
        let handle = try commerce!.getHandle()
        let result = stateset_shipment_ship(handle, id, trackingNumber)
        return try commerce!.parseJSON(result)
    }

    public func deliver(id: String) throws -> Shipment {
        let handle = try commerce!.getHandle()
        let result = stateset_shipment_deliver(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancel(id: String) throws -> Shipment {
        let handle = try commerce!.getHandle()
        let result = stateset_shipment_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Warranties API

public final class WarrantiesAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(customerId: String, productId: String, warrantyType: WarrantyType, durationMonths: Int) throws -> Warranty {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_create(handle, customerId, productId, warrantyType.rawValue, Int32(durationMonths))
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Warranty? {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Warranty] {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func createClaim(warrantyId: String, issueDescription: String) throws -> WarrantyClaim {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_create_claim(handle, warrantyId, issueDescription)
        return try commerce!.parseJSON(result)
    }

    public func approveClaim(claimId: String) throws -> WarrantyClaim {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_approve_claim(handle, claimId)
        return try commerce!.parseJSON(result)
    }

    public func denyClaim(claimId: String, reason: String) throws -> WarrantyClaim {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_deny_claim(handle, claimId, reason)
        return try commerce!.parseJSON(result)
    }

    public func completeClaim(claimId: String, resolution: ClaimResolution) throws -> WarrantyClaim {
        let handle = try commerce!.getHandle()
        let result = stateset_warranty_complete_claim(handle, claimId, resolution.rawValue)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Suppliers API

public final class SuppliersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(name: String, email: String, phone: String) throws -> Supplier {
        let handle = try commerce!.getHandle()
        let result = stateset_supplier_create(handle, name, email, phone)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Supplier? {
        let handle = try commerce!.getHandle()
        let result = stateset_supplier_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Supplier] {
        let handle = try commerce!.getHandle()
        let result = stateset_supplier_list(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Purchase Orders API

public final class PurchaseOrdersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(supplierId: String, items: [PurchaseOrderItem]) throws -> PurchaseOrder {
        let handle = try commerce!.getHandle()

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let itemsData = try encoder.encode(items)
        let itemsJSON = String(data: itemsData, encoding: .utf8) ?? "[]"

        let result = stateset_purchase_order_create(handle, supplierId, itemsJSON)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> PurchaseOrder? {
        let handle = try commerce!.getHandle()
        let result = stateset_purchase_order_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [PurchaseOrder] {
        let handle = try commerce!.getHandle()
        let result = stateset_purchase_order_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func submit(id: String) throws -> PurchaseOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_purchase_order_submit(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func approve(id: String, approvedBy: String) throws -> PurchaseOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_purchase_order_approve(handle, id, approvedBy)
        return try commerce!.parseJSON(result)
    }

    public func send(id: String) throws -> PurchaseOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_purchase_order_send(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancel(id: String) throws -> PurchaseOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_purchase_order_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Invoices API

public final class InvoicesAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(customerId: String, items: [InvoiceItem], billingEmail: String) throws -> Invoice {
        let handle = try commerce!.getHandle()

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let itemsData = try encoder.encode(items)
        let itemsJSON = String(data: itemsData, encoding: .utf8) ?? "[]"

        let result = stateset_invoice_create(handle, customerId, itemsJSON, billingEmail)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Invoice? {
        let handle = try commerce!.getHandle()
        let result = stateset_invoice_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Invoice] {
        let handle = try commerce!.getHandle()
        let result = stateset_invoice_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func send(id: String) throws -> Invoice {
        let handle = try commerce!.getHandle()
        let result = stateset_invoice_send(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func void(id: String) throws -> Invoice {
        let handle = try commerce!.getHandle()
        let result = stateset_invoice_void(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func recordPayment(id: String, amount: Double, paymentMethod: String) throws -> Invoice {
        let handle = try commerce!.getHandle()
        let result = stateset_invoice_record_payment(handle, id, amount, paymentMethod)
        return try commerce!.parseJSON(result)
    }

    public func getOverdue() throws -> [Invoice] {
        let handle = try commerce!.getHandle()
        let result = stateset_invoice_get_overdue(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - BOM (Bill of Materials) API

public final class BOMAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(productId: String, name: String, description: String? = nil) throws -> BillOfMaterials {
        let handle = try commerce!.getHandle()
        let result = stateset_bom_create(handle, productId, name, description)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> BillOfMaterials? {
        let handle = try commerce!.getHandle()
        let result = stateset_bom_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [BillOfMaterials] {
        let handle = try commerce!.getHandle()
        let result = stateset_bom_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func addComponent(bomId: String, name: String, componentSku: String, quantity: Double) throws -> BOMComponent {
        let handle = try commerce!.getHandle()
        let result = stateset_bom_add_component(handle, bomId, name, componentSku, quantity)
        return try commerce!.parseJSON(result)
    }

    public func getComponents(bomId: String) throws -> [BOMComponent] {
        let handle = try commerce!.getHandle()
        let result = stateset_bom_get_components(handle, bomId)
        return try commerce!.parseJSON(result)
    }

    public func activate(id: String) throws -> BillOfMaterials {
        let handle = try commerce!.getHandle()
        let result = stateset_bom_activate(handle, id)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Work Orders API

public final class WorkOrdersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(productId: String, quantityToBuild: Double, bomId: String? = nil) throws -> WorkOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_work_order_create(handle, productId, quantityToBuild, bomId)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> WorkOrder? {
        let handle = try commerce!.getHandle()
        let result = stateset_work_order_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [WorkOrder] {
        let handle = try commerce!.getHandle()
        let result = stateset_work_order_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func start(id: String) throws -> WorkOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_work_order_start(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func complete(id: String, quantityCompleted: Double) throws -> WorkOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_work_order_complete(handle, id, quantityCompleted)
        return try commerce!.parseJSON(result)
    }

    public func cancel(id: String) throws -> WorkOrder {
        let handle = try commerce!.getHandle()
        let result = stateset_work_order_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Currency API

public final class CurrencyAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func setRate(from fromCurrency: Currency, to toCurrency: Currency, rate: Double) throws -> ExchangeRate {
        let handle = try commerce!.getHandle()
        let result = stateset_currency_set_rate(handle, fromCurrency.rawValue, toCurrency.rawValue, rate)
        return try commerce!.parseJSON(result)
    }

    public func getRate(from fromCurrency: Currency, to toCurrency: Currency) throws -> ExchangeRate? {
        let handle = try commerce!.getHandle()
        let result = stateset_currency_get_rate(handle, fromCurrency.rawValue, toCurrency.rawValue)
        return commerce!.parseOptionalJSON(result)
    }

    public func convert(amount: Double, from fromCurrency: Currency, to toCurrency: Currency) throws -> ConversionResult {
        let handle = try commerce!.getHandle()
        let result = stateset_currency_convert(handle, amount, fromCurrency.rawValue, toCurrency.rawValue)
        return try commerce!.parseJSON(result)
    }

    public func getSettings() throws -> StoreCurrencySettings {
        let handle = try commerce!.getHandle()
        let result = stateset_currency_get_settings(handle)
        return try commerce!.parseJSON(result)
    }
}
