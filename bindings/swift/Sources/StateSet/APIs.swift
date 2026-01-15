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

// MARK: - Subscriptions API

public final class SubscriptionsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createPlan(code: String, name: String, interval: String, intervalCount: Int, price: Double, currency: String = "USD") throws -> SubscriptionPlan {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_plan_create(handle, code, name, interval, Int32(intervalCount), price, currency)
        return try commerce!.parseJSON(result)
    }

    public func getPlan(id: String) throws -> SubscriptionPlan? {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_plan_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listPlans() throws -> [SubscriptionPlan] {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_plan_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func activatePlan(id: String) throws -> SubscriptionPlan {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_plan_activate(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func archivePlan(id: String) throws -> SubscriptionPlan {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_plan_archive(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func subscribe(customerId: String, planId: String) throws -> Subscription {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_subscribe(handle, customerId, planId)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Subscription? {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Subscription] {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func pause(id: String) throws -> Subscription {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_pause(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func resume(id: String) throws -> Subscription {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_resume(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancel(id: String) throws -> Subscription {
        let handle = try commerce!.getHandle()
        let result = stateset_subscription_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Promotions API

public final class PromotionsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(code: String, name: String, discountType: String, discountValue: Double) throws -> Promotion {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_create(handle, code, name, discountType, discountValue)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Promotion? {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getByCode(code: String) throws -> Promotion? {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_get_by_code(handle, code)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Promotion] {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func activate(id: String) throws -> Promotion {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_activate(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func deactivate(id: String) throws -> Promotion {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_deactivate(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func delete(id: String) throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_promotion_delete(handle, id) == 1
    }

    public func getActive() throws -> [Promotion] {
        let handle = try commerce!.getHandle()
        let result = stateset_promotion_get_active(handle)
        return try commerce!.parseJSON(result)
    }

    public func createCoupon(promotionId: String, code: String, maxUses: Int? = nil) throws -> Coupon {
        let handle = try commerce!.getHandle()
        let result = stateset_coupon_create(handle, promotionId, code, maxUses.map { Int32($0) } ?? -1)
        return try commerce!.parseJSON(result)
    }

    public func getCouponByCode(code: String) throws -> Coupon? {
        let handle = try commerce!.getHandle()
        let result = stateset_coupon_get_by_code(handle, code)
        return commerce!.parseOptionalJSON(result)
    }

    public func validateCoupon(code: String) throws -> Coupon? {
        let handle = try commerce!.getHandle()
        let result = stateset_coupon_validate(handle, code)
        return commerce!.parseOptionalJSON(result)
    }
}

// MARK: - Tax API

public final class TaxAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func calculate(lineItemsJSON: String, shippingCountry: String, shippingState: String? = nil) throws -> TaxCalculation {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_calculate(handle, lineItemsJSON, shippingCountry, shippingState)
        return try commerce!.parseJSON(result)
    }

    public func getEffectiveRate(country: String, state: String? = nil, category: String? = nil) throws -> Double {
        let handle = try commerce!.getHandle()
        return stateset_tax_get_effective_rate(handle, country, state, category)
    }

    public func createJurisdiction(name: String, code: String, countryCode: String, stateCode: String? = nil) throws -> TaxJurisdiction {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_jurisdiction_create(handle, name, code, countryCode, stateCode)
        return try commerce!.parseJSON(result)
    }

    public func getJurisdiction(id: String) throws -> TaxJurisdiction? {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_jurisdiction_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listJurisdictions() throws -> [TaxJurisdiction] {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_jurisdiction_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func createRate(jurisdictionId: String, name: String, rate: Double) throws -> TaxRate {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_rate_create(handle, jurisdictionId, name, rate)
        return try commerce!.parseJSON(result)
    }

    public func getRate(id: String) throws -> TaxRate? {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_rate_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listRates() throws -> [TaxRate] {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_rate_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func createExemption(customerId: String, exemptionType: String, effectiveFrom: String) throws -> TaxExemption {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_exemption_create(handle, customerId, exemptionType, effectiveFrom)
        return try commerce!.parseJSON(result)
    }

    public func getCustomerExemptions(customerId: String) throws -> [TaxExemption] {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_exemption_get_customer(handle, customerId)
        return try commerce!.parseJSON(result)
    }

    public func customerIsExempt(customerId: String) throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_tax_customer_is_exempt(handle, customerId) == 1
    }

    public func getSettings() throws -> TaxSettings {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_get_settings(handle)
        return try commerce!.parseJSON(result)
    }

    public func setEnabled(enabled: Bool) throws -> TaxSettings {
        let handle = try commerce!.getHandle()
        let result = stateset_tax_set_enabled(handle, enabled ? 1 : 0)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Quality API

public final class QualityAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createInspection(inspectionType: String, referenceType: String, referenceId: String) throws -> Inspection {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_inspection_create(handle, inspectionType, referenceType, referenceId)
        return try commerce!.parseJSON(result)
    }

    public func getInspection(id: String) throws -> Inspection? {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_inspection_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listInspections() throws -> [Inspection] {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_inspection_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func startInspection(id: String) throws -> Inspection {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_inspection_start(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func completeInspection(id: String) throws -> Inspection {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_inspection_complete(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func createNcr(source: String, severity: String, sku: String, quantityAffected: Int, description: String) throws -> Ncr {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_ncr_create(handle, source, severity, sku, Int32(quantityAffected), description)
        return try commerce!.parseJSON(result)
    }

    public func getNcr(id: String) throws -> Ncr? {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_ncr_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listNcrs() throws -> [Ncr] {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_ncr_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func closeNcr(id: String) throws -> Ncr {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_ncr_close(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func createHold(sku: String, quantityHeld: Int, reason: String, holdType: String) throws -> QualityHold {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_hold_create(handle, sku, Int32(quantityHeld), reason, holdType)
        return try commerce!.parseJSON(result)
    }

    public func getHold(id: String) throws -> QualityHold? {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_hold_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listHolds() throws -> [QualityHold] {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_hold_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func releaseHold(id: String, releasedBy: String) throws -> QualityHold {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_hold_release(handle, id, releasedBy)
        return try commerce!.parseJSON(result)
    }

    public func getActiveHolds() throws -> [QualityHold] {
        let handle = try commerce!.getHandle()
        let result = stateset_quality_hold_get_active(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Lots API

public final class LotsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(sku: String, quantityProduced: Int) throws -> Lot {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_create(handle, sku, Int32(quantityProduced))
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Lot? {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getByNumber(lotNumber: String) throws -> Lot? {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_get_by_number(handle, lotNumber)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Lot] {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func getActiveLots(sku: String) throws -> [Lot] {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_get_active(handle, sku)
        return try commerce!.parseJSON(result)
    }

    public func quarantine(id: String, reason: String) throws -> Lot {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_quarantine(handle, id, reason)
        return try commerce!.parseJSON(result)
    }

    public func releaseQuarantine(id: String) throws -> Lot {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_release_quarantine(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func getExpiringLots(days: Int) throws -> [Lot] {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_get_expiring(handle, Int32(days))
        return try commerce!.parseJSON(result)
    }

    public func getExpiredLots() throws -> [Lot] {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_get_expired(handle)
        return try commerce!.parseJSON(result)
    }

    public func getQuarantined() throws -> [Lot] {
        let handle = try commerce!.getHandle()
        let result = stateset_lot_get_quarantined(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Serials API

public final class SerialsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(sku: String, lotNumber: String? = nil) throws -> Serial {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_create(handle, sku, lotNumber)
        return try commerce!.parseJSON(result)
    }

    public func get(id: String) throws -> Serial? {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getBySerial(serial: String) throws -> Serial? {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_get_by_serial(handle, serial)
        return commerce!.parseOptionalJSON(result)
    }

    public func list() throws -> [Serial] {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func getAvailable(sku: String, limit: Int) throws -> [Serial] {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_get_available(handle, sku, Int32(limit))
        return try commerce!.parseJSON(result)
    }

    public func markSold(id: String, customerId: String, orderId: String? = nil) throws -> Serial {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_mark_sold(handle, id, customerId, orderId)
        return try commerce!.parseJSON(result)
    }

    public func quarantine(id: String, reason: String) throws -> Serial {
        let handle = try commerce!.getHandle()
        let result = stateset_serial_quarantine(handle, id, reason)
        return try commerce!.parseJSON(result)
    }

    public func isAvailable(serial: String) throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_serial_is_available(handle, serial) == 1
    }
}

// MARK: - Warehouse API

public final class WarehouseAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createWarehouse(code: String, name: String, warehouseType: String = "standard") throws -> Warehouse {
        let handle = try commerce!.getHandle()
        let result = stateset_warehouse_create(handle, code, name, warehouseType)
        return try commerce!.parseJSON(result)
    }

    public func getWarehouse(id: Int) throws -> Warehouse? {
        let handle = try commerce!.getHandle()
        let result = stateset_warehouse_get(handle, Int32(id))
        return commerce!.parseOptionalJSON(result)
    }

    public func getWarehouseByCode(code: String) throws -> Warehouse? {
        let handle = try commerce!.getHandle()
        let result = stateset_warehouse_get_by_code(handle, code)
        return commerce!.parseOptionalJSON(result)
    }

    public func listWarehouses() throws -> [Warehouse] {
        let handle = try commerce!.getHandle()
        let result = stateset_warehouse_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func createLocation(warehouseId: Int, locationType: String, zone: String? = nil, aisle: String? = nil) throws -> Location {
        let handle = try commerce!.getHandle()
        let result = stateset_location_create(handle, Int32(warehouseId), locationType, zone, aisle)
        return try commerce!.parseJSON(result)
    }

    public func getLocation(id: Int) throws -> Location? {
        let handle = try commerce!.getHandle()
        let result = stateset_location_get(handle, Int32(id))
        return commerce!.parseOptionalJSON(result)
    }

    public func listLocations(warehouseId: Int? = nil) throws -> [Location] {
        let handle = try commerce!.getHandle()
        let result = stateset_location_list(handle, warehouseId.map { Int32($0) } ?? -1)
        return try commerce!.parseJSON(result)
    }

    public func getPickableLocations(warehouseId: Int, sku: String) throws -> [Location] {
        let handle = try commerce!.getHandle()
        let result = stateset_location_get_pickable(handle, Int32(warehouseId), sku)
        return try commerce!.parseJSON(result)
    }

    public func getTotalAvailable(warehouseId: Int, sku: String) throws -> Int {
        let handle = try commerce!.getHandle()
        return Int(stateset_warehouse_get_total_available(handle, Int32(warehouseId), sku))
    }
}

// MARK: - Receiving API

public final class ReceivingAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createReceipt(receiptType: String, warehouseId: Int, purchaseOrderId: String? = nil) throws -> Receipt {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_create(handle, receiptType, Int32(warehouseId), purchaseOrderId)
        return try commerce!.parseJSON(result)
    }

    public func getReceipt(id: String) throws -> Receipt? {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getReceiptByNumber(number: String) throws -> Receipt? {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_get_by_number(handle, number)
        return commerce!.parseOptionalJSON(result)
    }

    public func listReceipts() throws -> [Receipt] {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func startReceiving(id: String) throws -> Receipt {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_start(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func completeReceiving(id: String) throws -> Receipt {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_complete(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancelReceipt(id: String) throws -> Receipt {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func createReceiptFromPo(poId: String, warehouseId: Int) throws -> Receipt {
        let handle = try commerce!.getHandle()
        let result = stateset_receipt_create_from_po(handle, poId, Int32(warehouseId))
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Fulfillment API

public final class FulfillmentAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createWave(warehouseId: Int, orderIds: [String], priority: Int = 0) throws -> Wave {
        let handle = try commerce!.getHandle()
        let orderIdsJSON = try JSONEncoder().encode(orderIds)
        let orderIdsString = String(data: orderIdsJSON, encoding: .utf8) ?? "[]"
        let result = stateset_wave_create(handle, Int32(warehouseId), orderIdsString, Int32(priority))
        return try commerce!.parseJSON(result)
    }

    public func getWave(id: String) throws -> Wave? {
        let handle = try commerce!.getHandle()
        let result = stateset_wave_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listWaves() throws -> [Wave] {
        let handle = try commerce!.getHandle()
        let result = stateset_wave_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func releaseWave(id: String) throws -> Wave {
        let handle = try commerce!.getHandle()
        let result = stateset_wave_release(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func completeWave(id: String) throws -> Wave {
        let handle = try commerce!.getHandle()
        let result = stateset_wave_complete(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancelWave(id: String) throws -> Wave {
        let handle = try commerce!.getHandle()
        let result = stateset_wave_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func getPick(id: String) throws -> PickTask? {
        let handle = try commerce!.getHandle()
        let result = stateset_pick_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listPicks() throws -> [PickTask] {
        let handle = try commerce!.getHandle()
        let result = stateset_pick_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func assignPick(id: String, assignedTo: String) throws -> PickTask {
        let handle = try commerce!.getHandle()
        let result = stateset_pick_assign(handle, id, assignedTo)
        return try commerce!.parseJSON(result)
    }

    public func startPick(id: String) throws -> PickTask {
        let handle = try commerce!.getHandle()
        let result = stateset_pick_start(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancelPick(id: String) throws -> PickTask {
        let handle = try commerce!.getHandle()
        let result = stateset_pick_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func isOrderReadyToPack(orderId: String) throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_fulfillment_is_ready_to_pack(handle, orderId) == 1
    }

    public func isOrderReadyToShip(orderId: String) throws -> Bool {
        let handle = try commerce!.getHandle()
        return stateset_fulfillment_is_ready_to_ship(handle, orderId) == 1
    }
}

// MARK: - Accounts Payable API

public final class AccountsPayableAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createBill(supplierId: String, dueDate: String, paymentTerms: String? = nil) throws -> Bill {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_create(handle, supplierId, dueDate, paymentTerms)
        return try commerce!.parseJSON(result)
    }

    public func getBill(id: String) throws -> Bill? {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getBillByNumber(number: String) throws -> Bill? {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_get_by_number(handle, number)
        return commerce!.parseOptionalJSON(result)
    }

    public func listBills() throws -> [Bill] {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func approveBill(id: String) throws -> Bill {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_approve(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func cancelBill(id: String) throws -> Bill {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func getOverdueBills() throws -> [Bill] {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_get_overdue(handle)
        return try commerce!.parseJSON(result)
    }

    public func getBillsDueSoon(days: Int) throws -> [Bill] {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_bill_get_due_soon(handle, Int32(days))
        return try commerce!.parseJSON(result)
    }

    public func getAgingSummary() throws -> ApAgingSummary {
        let handle = try commerce!.getHandle()
        let result = stateset_ap_aging_summary(handle)
        return try commerce!.parseJSON(result)
    }

    public func getTotalOutstanding() throws -> Double {
        let handle = try commerce!.getHandle()
        return stateset_ap_total_outstanding(handle)
    }
}

// MARK: - Accounts Receivable API

public final class AccountsReceivableAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func getAgingSummary() throws -> ArAgingSummary {
        let handle = try commerce!.getHandle()
        let result = stateset_ar_aging_summary(handle)
        return try commerce!.parseJSON(result)
    }

    public func getTotalOutstanding() throws -> Double {
        let handle = try commerce!.getHandle()
        return stateset_ar_total_outstanding(handle)
    }

    public func getDso(days: Int) throws -> Double {
        let handle = try commerce!.getHandle()
        return stateset_ar_get_dso(handle, Int32(days))
    }

    public func createCreditMemo(customerId: String, amount: Double, reason: String) throws -> CreditMemo {
        let handle = try commerce!.getHandle()
        let result = stateset_ar_credit_memo_create(handle, customerId, amount, reason)
        return try commerce!.parseJSON(result)
    }

    public func getCreditMemo(id: String) throws -> CreditMemo? {
        let handle = try commerce!.getHandle()
        let result = stateset_ar_credit_memo_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listCreditMemos() throws -> [CreditMemo] {
        let handle = try commerce!.getHandle()
        let result = stateset_ar_credit_memo_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func voidCreditMemo(id: String) throws -> CreditMemo {
        let handle = try commerce!.getHandle()
        let result = stateset_ar_credit_memo_void(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func getUnappliedCredits(customerId: String) throws -> [CreditMemo] {
        let handle = try commerce!.getHandle()
        let result = stateset_ar_get_unapplied_credits(handle, customerId)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Cost Accounting API

public final class CostAccountingAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func getItemCost(sku: String) throws -> ItemCost? {
        let handle = try commerce!.getHandle()
        let result = stateset_cost_get_item_cost(handle, sku)
        return commerce!.parseOptionalJSON(result)
    }

    public func setItemCost(sku: String, standardCost: Double, currentCost: Double? = nil) throws -> ItemCost {
        let handle = try commerce!.getHandle()
        let result = stateset_cost_set_item_cost(handle, sku, standardCost, currentCost ?? standardCost)
        return try commerce!.parseJSON(result)
    }

    public func listItemCosts() throws -> [ItemCost] {
        let handle = try commerce!.getHandle()
        let result = stateset_cost_list_item_costs(handle)
        return try commerce!.parseJSON(result)
    }

    public func updateAverageCost(sku: String, quantity: Int, unitCost: Double) throws -> ItemCost {
        let handle = try commerce!.getHandle()
        let result = stateset_cost_update_average(handle, sku, Int32(quantity), unitCost)
        return try commerce!.parseJSON(result)
    }

    public func getTotalInventoryValue() throws -> Double {
        let handle = try commerce!.getHandle()
        return stateset_cost_total_inventory_value(handle)
    }
}

// MARK: - Credit API

public final class CreditAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createCreditAccount(customerId: String, creditLimit: Double) throws -> CreditAccount {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_account_create(handle, customerId, creditLimit)
        return try commerce!.parseJSON(result)
    }

    public func getCreditAccount(id: String) throws -> CreditAccount? {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_account_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getCreditAccountByCustomer(customerId: String) throws -> CreditAccount? {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_account_get_by_customer(handle, customerId)
        return commerce!.parseOptionalJSON(result)
    }

    public func listCreditAccounts() throws -> [CreditAccount] {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_account_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func checkCredit(customerId: String, orderAmount: Double) throws -> CreditCheck {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_check(handle, customerId, orderAmount)
        return try commerce!.parseJSON(result)
    }

    public func adjustCreditLimit(customerId: String, newLimit: Double, reason: String) throws -> CreditAccount {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_adjust_limit(handle, customerId, newLimit, reason)
        return try commerce!.parseJSON(result)
    }

    public func suspendCreditAccount(customerId: String, reason: String) throws -> CreditAccount {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_account_suspend(handle, customerId, reason)
        return try commerce!.parseJSON(result)
    }

    public func reactivateCreditAccount(customerId: String) throws -> CreditAccount {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_account_reactivate(handle, customerId)
        return try commerce!.parseJSON(result)
    }

    public func getOverLimitCustomers() throws -> [CreditAccount] {
        let handle = try commerce!.getHandle()
        let result = stateset_credit_get_over_limit(handle)
        return try commerce!.parseJSON(result)
    }
}

// MARK: - Backorders API

public final class BackordersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createBackorder(orderId: String, sku: String, quantity: Int, expectedDate: String? = nil) throws -> Backorder {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_create(handle, orderId, sku, Int32(quantity), expectedDate)
        return try commerce!.parseJSON(result)
    }

    public func getBackorder(id: String) throws -> Backorder? {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getBackorderByNumber(number: String) throws -> Backorder? {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_get_by_number(handle, number)
        return commerce!.parseOptionalJSON(result)
    }

    public func listBackorders() throws -> [Backorder] {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func cancelBackorder(id: String) throws -> Backorder {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_cancel(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func getBackordersForOrder(orderId: String) throws -> [Backorder] {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_get_for_order(handle, orderId)
        return try commerce!.parseJSON(result)
    }

    public func getBackordersForSku(sku: String) throws -> [Backorder] {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_get_for_sku(handle, sku)
        return try commerce!.parseJSON(result)
    }

    public func getOverdueBackorders() throws -> [Backorder] {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_get_overdue(handle)
        return try commerce!.parseJSON(result)
    }

    public func getSummary() throws -> BackorderSummary {
        let handle = try commerce!.getHandle()
        let result = stateset_backorder_summary(handle)
        return try commerce!.parseJSON(result)
    }

    public func countPending() throws -> Int {
        let handle = try commerce!.getHandle()
        return Int(stateset_backorder_count_pending(handle))
    }
}

// MARK: - General Ledger API

public final class GeneralLedgerAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createAccount(accountNumber: String, name: String, accountType: String) throws -> GlAccount {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_account_create(handle, accountNumber, name, accountType)
        return try commerce!.parseJSON(result)
    }

    public func getAccount(id: String) throws -> GlAccount? {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_account_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func getAccountByNumber(accountNumber: String) throws -> GlAccount? {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_account_get_by_number(handle, accountNumber)
        return commerce!.parseOptionalJSON(result)
    }

    public func listAccounts() throws -> [GlAccount] {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_account_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func initializeChartOfAccounts() throws -> [GlAccount] {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_initialize_coa(handle)
        return try commerce!.parseJSON(result)
    }

    public func getJournalEntry(id: String) throws -> JournalEntry? {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_journal_entry_get(handle, id)
        return commerce!.parseOptionalJSON(result)
    }

    public func listJournalEntries() throws -> [JournalEntry] {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_journal_entry_list(handle)
        return try commerce!.parseJSON(result)
    }

    public func postJournalEntry(id: String, postedBy: String) throws -> JournalEntry {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_journal_entry_post(handle, id, postedBy)
        return try commerce!.parseJSON(result)
    }

    public func voidJournalEntry(id: String) throws -> JournalEntry {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_journal_entry_void(handle, id)
        return try commerce!.parseJSON(result)
    }

    public func getTrialBalance(asOfDate: String) throws -> TrialBalance {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_trial_balance(handle, asOfDate)
        return try commerce!.parseJSON(result)
    }

    public func getBalanceSheet(asOfDate: String) throws -> BalanceSheet {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_balance_sheet(handle, asOfDate)
        return try commerce!.parseJSON(result)
    }

    public func getIncomeStatement(startDate: String, endDate: String) throws -> IncomeStatement {
        let handle = try commerce!.getHandle()
        let result = stateset_gl_income_statement(handle, startDate, endDate)
        return try commerce!.parseJSON(result)
    }

    public func getAccountBalance(accountId: String, asOfDate: String? = nil) throws -> Double {
        let handle = try commerce!.getHandle()
        return stateset_gl_account_balance(handle, accountId, asOfDate)
    }
}
