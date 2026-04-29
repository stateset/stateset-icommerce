import Foundation

final class StateSetSwiftStore {
    var nextId = 0
    var nextIntId = 0
    var customers: [Customer] = []
    var products: [Product] = []
    var inventoryItems: [String: InventoryItem] = [:]
    var stockLevels: [String: StockLevel] = [:]
    var carts: [Cart] = []
    var orders: [Order] = []
    var returns: [Return] = []
    var payments: [Payment] = []
    var shipments: [Shipment] = []
    var rates: [String: ExchangeRate] = [:]
    var subscriptionPlans: [SubscriptionPlan] = []
    var subscriptions: [Subscription] = []
    var promotions: [Promotion] = []
    var coupons: [Coupon] = []
    var warehouses: [Warehouse] = []
    var glAccounts: [GlAccount] = []

    func id(_ prefix: String) -> String {
        nextId += 1
        return "\(prefix)_\(nextId)"
    }

    func intId() -> Int {
        nextIntId += 1
        return nextIntId
    }
}

private func swiftStore(_ commerce: StateSetCommerce?) throws -> StateSetSwiftStore {
    guard let commerce = commerce else {
        throw StateSetError.invalidHandle
    }
    return commerce.store
}

private func timestamp() -> String {
    ISO8601DateFormatter().string(from: Date())
}

private func amountString(_ amount: Double) -> String {
    String(format: "%.2f", amount)
}

private func rateKey(_ fromCurrency: Currency, _ toCurrency: Currency) -> String {
    "\(fromCurrency.rawValue):\(toCurrency.rawValue)"
}

// MARK: - Customers API

public final class CustomersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(email: String, firstName: String, lastName: String, phone: String? = nil) throws -> Customer {
        let store = try swiftStore(commerce)
        let customer = Customer(
            id: store.id("cust"),
            email: email,
            firstName: firstName,
            lastName: lastName,
            phone: phone,
            createdAt: timestamp(),
            updatedAt: timestamp()
        )
        store.customers.append(customer)
        return customer
    }

    public func get(id: String) throws -> Customer? {
        try swiftStore(commerce).customers.first { $0.id == id }
    }

    public func list() throws -> [Customer] {
        try swiftStore(commerce).customers
    }

    public func delete(id: String) throws -> Bool {
        let store = try swiftStore(commerce)
        let count = store.customers.count
        store.customers.removeAll { $0.id == id }
        return store.customers.count != count
    }
}

// MARK: - Products API

public final class ProductsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(name: String, sku: String, price: Double, description: String? = nil) throws -> Product {
        let store = try swiftStore(commerce)
        let product = Product(
            id: store.id("prod"),
            name: name,
            sku: sku,
            price: price,
            slug: nil,
            description: description,
            isActive: true,
            createdAt: timestamp(),
            updatedAt: timestamp()
        )
        store.products.append(product)
        return product
    }

    public func get(id: String) throws -> Product? {
        try swiftStore(commerce).products.first { $0.id == id }
    }

    public func list() throws -> [Product] {
        try swiftStore(commerce).products
    }
}

// MARK: - Orders API

public final class OrdersAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(customerId: String, items: [OrderItem], currency: String = "USD") throws -> Order {
        let store = try swiftStore(commerce)
        let total = items.reduce(0.0) { $0 + Double($1.quantity) * $1.unitPrice }
        let id = store.id("ord")
        let order = Order(
            id: id,
            orderNumber: id.uppercased(),
            customerId: customerId,
            status: OrderStatus.pending.rawValue,
            totalAmount: amountString(total),
            currency: currency,
            createdAt: timestamp(),
            updatedAt: timestamp()
        )
        store.orders.append(order)
        return order
    }

    public func get(id: String) throws -> Order? {
        try swiftStore(commerce).orders.first { $0.id == id }
    }

    public func list() throws -> [Order] {
        try swiftStore(commerce).orders
    }

    public func updateStatus(id: String, status: OrderStatus) throws -> Order {
        try replaceOrder(id: id, status: status.rawValue)
    }

    public func ship(id: String) throws -> Order {
        try replaceOrder(id: id, status: OrderStatus.shipped.rawValue)
    }

    public func cancel(id: String) throws -> Order {
        try replaceOrder(id: id, status: OrderStatus.cancelled.rawValue)
    }

    private func replaceOrder(id: String, status: String) throws -> Order {
        let store = try swiftStore(commerce)
        guard let index = store.orders.firstIndex(where: { $0.id == id }) else {
            throw StateSetError.operationFailed("Order not found")
        }
        let current = store.orders[index]
        let updated = Order(
            id: current.id,
            orderNumber: current.orderNumber,
            customerId: current.customerId,
            status: status,
            totalAmount: current.totalAmount,
            currency: current.currency,
            createdAt: current.createdAt,
            updatedAt: timestamp()
        )
        store.orders[index] = updated
        return updated
    }
}

// MARK: - Inventory API

public final class InventoryAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createItem(sku: String, name: String, initialQuantity: Double = 0) throws -> InventoryItem {
        let store = try swiftStore(commerce)
        let item = InventoryItem(
            id: store.id("inv"),
            sku: sku,
            name: name,
            description: nil,
            unitOfMeasure: nil,
            createdAt: timestamp()
        )
        store.inventoryItems[sku] = item
        store.stockLevels[sku] = StockLevel(
            id: store.id("stock"),
            inventoryItemId: item.id,
            locationId: nil,
            available: Int(initialQuantity),
            reserved: 0,
            incoming: nil,
            updatedAt: timestamp()
        )
        return item
    }

    public func adjust(sku: String, quantityDelta: Double, reason: String = "manual adjustment") throws -> Bool {
        let store = try swiftStore(commerce)
        guard let current = store.stockLevels[sku] else {
            return false
        }
        store.stockLevels[sku] = StockLevel(
            id: current.id,
            inventoryItemId: current.inventoryItemId,
            locationId: current.locationId,
            available: current.available + Int(quantityDelta),
            reserved: current.reserved,
            incoming: current.incoming,
            updatedAt: timestamp()
        )
        return true
    }

    public func getLevel(sku: String) throws -> StockLevel? {
        try swiftStore(commerce).stockLevels[sku]
    }
}

// MARK: - Carts API

public final class CartsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(customerId: String? = nil, currency: String = "USD") throws -> Cart {
        let store = try swiftStore(commerce)
        let cart = Cart(
            id: store.id("cart"),
            customerId: customerId,
            status: "active",
            grandTotal: "0.00",
            currency: currency,
            createdAt: timestamp()
        )
        store.carts.append(cart)
        return cart
    }
}

// MARK: - Returns API

public final class ReturnsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(orderId: String, reason: ReturnReason, notes: String? = nil) throws -> Return {
        let store = try swiftStore(commerce)
        let ret = Return(
            id: store.id("ret"),
            orderId: orderId,
            reason: reason.rawValue,
            status: ReturnStatus.requested.rawValue,
            refundAmount: nil,
            notes: notes,
            createdAt: timestamp()
        )
        store.returns.append(ret)
        return ret
    }

    public func list() throws -> [Return] {
        try swiftStore(commerce).returns
    }

    public func get(id: String) throws -> Return? {
        try swiftStore(commerce).returns.first { $0.id == id }
    }

    public func approve(id: String) throws -> Return {
        try replaceReturn(id: id, status: ReturnStatus.approved.rawValue)
    }

    public func reject(id: String, reason: String) throws -> Return {
        try replaceReturn(id: id, status: ReturnStatus.rejected.rawValue)
    }

    public func complete(id: String) throws -> Return {
        try replaceReturn(id: id, status: ReturnStatus.completed.rawValue)
    }

    private func replaceReturn(id: String, status: String) throws -> Return {
        let store = try swiftStore(commerce)
        guard let index = store.returns.firstIndex(where: { $0.id == id }) else {
            throw StateSetError.operationFailed("Return not found")
        }
        let current = store.returns[index]
        let updated = Return(
            id: current.id,
            orderId: current.orderId,
            reason: current.reason,
            status: status,
            refundAmount: current.refundAmount,
            notes: current.notes,
            createdAt: current.createdAt
        )
        store.returns[index] = updated
        return updated
    }
}

// MARK: - Payments API

public final class PaymentsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(orderId: String, amount: Double, currency: String = "USD", method: PaymentMethod = .creditCard) throws -> Payment {
        let store = try swiftStore(commerce)
        let payment = Payment(
            id: store.id("pay"),
            orderId: orderId,
            amount: amountString(amount),
            currency: currency,
            method: method.rawValue,
            status: "pending",
            createdAt: timestamp()
        )
        store.payments.append(payment)
        return payment
    }

    public func get(id: String) throws -> Payment? {
        try swiftStore(commerce).payments.first { $0.id == id }
    }

    public func list() throws -> [Payment] {
        try swiftStore(commerce).payments
    }
}

// MARK: - Analytics API

public final class AnalyticsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func salesSummary(period: TimePeriod = .thisMonth) throws -> SalesSummary {
        let orders = try swiftStore(commerce).orders
        let revenue = orders.reduce(0.0) { $0 + (Double($1.totalAmount) ?? 0.0) }
        let average = orders.isEmpty ? 0.0 : revenue / Double(orders.count)
        return SalesSummary(
            totalRevenue: amountString(revenue),
            orderCount: orders.count,
            averageOrderValue: amountString(average)
        )
    }

    public func topProducts(limit: Int = 10) throws -> [TopProduct] {
        []
    }

    public func topCustomers(limit: Int = 10) throws -> [TopCustomer] {
        []
    }
}

// MARK: - Shipments API

public final class ShipmentsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(orderId: String, recipientName: String, shippingAddress: String, carrier: String) throws -> Shipment {
        let store = try swiftStore(commerce)
        let id = store.id("ship")
        let shipment = Shipment(
            id: id,
            shipmentNumber: id.uppercased(),
            orderId: orderId,
            status: ShipmentStatus.pending.rawValue,
            carrier: carrier,
            trackingNumber: nil,
            trackingUrl: nil,
            recipientName: recipientName,
            recipientEmail: nil,
            shippingAddress: shippingAddress,
            shippedAt: nil,
            deliveredAt: nil,
            estimatedDelivery: nil,
            weight: nil,
            notes: nil,
            createdAt: timestamp(),
            updatedAt: timestamp()
        )
        store.shipments.append(shipment)
        return shipment
    }

    public func get(id: String) throws -> Shipment? {
        try swiftStore(commerce).shipments.first { $0.id == id }
    }

    public func list() throws -> [Shipment] {
        try swiftStore(commerce).shipments
    }

    public func ship(id: String, trackingNumber: String) throws -> Shipment {
        try replaceShipment(id: id, status: ShipmentStatus.shipped.rawValue, trackingNumber: trackingNumber)
    }

    public func deliver(id: String) throws -> Shipment {
        try replaceShipment(id: id, status: ShipmentStatus.delivered.rawValue, deliveredAt: timestamp())
    }

    public func cancel(id: String) throws -> Shipment {
        try replaceShipment(id: id, status: ShipmentStatus.cancelled.rawValue)
    }

    private func replaceShipment(
        id: String,
        status: String,
        trackingNumber: String? = nil,
        deliveredAt: String? = nil
    ) throws -> Shipment {
        let store = try swiftStore(commerce)
        guard let index = store.shipments.firstIndex(where: { $0.id == id }) else {
            throw StateSetError.operationFailed("Shipment not found")
        }
        let current = store.shipments[index]
        let updated = Shipment(
            id: current.id,
            shipmentNumber: current.shipmentNumber,
            orderId: current.orderId,
            status: status,
            carrier: current.carrier,
            trackingNumber: trackingNumber ?? current.trackingNumber,
            trackingUrl: current.trackingUrl,
            recipientName: current.recipientName,
            recipientEmail: current.recipientEmail,
            shippingAddress: current.shippingAddress,
            shippedAt: status == ShipmentStatus.shipped.rawValue ? timestamp() : current.shippedAt,
            deliveredAt: deliveredAt ?? current.deliveredAt,
            estimatedDelivery: current.estimatedDelivery,
            weight: current.weight,
            notes: current.notes,
            createdAt: current.createdAt,
            updatedAt: timestamp()
        )
        store.shipments[index] = updated
        return updated
    }
}

// MARK: - Currency API

public final class CurrencyAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func setRate(from fromCurrency: Currency, to toCurrency: Currency, rate: Double) throws -> ExchangeRate {
        let store = try swiftStore(commerce)
        let exchangeRate = ExchangeRate(
            id: store.id("rate"),
            fromCurrency: fromCurrency.rawValue,
            toCurrency: toCurrency.rawValue,
            rate: rate,
            source: nil,
            validFrom: timestamp(),
            validTo: nil,
            createdAt: timestamp()
        )
        store.rates[rateKey(fromCurrency, toCurrency)] = exchangeRate
        return exchangeRate
    }

    public func getRate(from fromCurrency: Currency, to toCurrency: Currency) throws -> ExchangeRate? {
        try swiftStore(commerce).rates[rateKey(fromCurrency, toCurrency)]
    }

    public func convert(amount: Double, from fromCurrency: Currency, to toCurrency: Currency) throws -> ConversionResult {
        let rate = try getRate(from: fromCurrency, to: toCurrency)?.rate ?? 1.0
        return ConversionResult(
            fromCurrency: fromCurrency.rawValue,
            toCurrency: toCurrency.rawValue,
            originalAmount: amount,
            convertedAmount: amount * rate,
            rate: rate,
            rateAt: timestamp()
        )
    }

    public func getSettings() throws -> StoreCurrencySettings {
        StoreCurrencySettings(
            baseCurrency: Currency.usd.rawValue,
            enabledCurrencies: Currency.allCases.map(\.rawValue),
            autoConvert: false,
            roundingMode: "standard"
        )
    }
}

// MARK: - Subscriptions API

public final class SubscriptionsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createPlan(code: String, name: String, interval: String, intervalCount: Int, price: Double, currency: String = "USD") throws -> SubscriptionPlan {
        let store = try swiftStore(commerce)
        let plan = SubscriptionPlan(
            id: store.id("plan"),
            code: code,
            name: name,
            interval: interval,
            intervalCount: intervalCount,
            price: price,
            currency: currency,
            status: "active",
            createdAt: timestamp()
        )
        store.subscriptionPlans.append(plan)
        return plan
    }

    public func getPlan(id: String) throws -> SubscriptionPlan? {
        try swiftStore(commerce).subscriptionPlans.first { $0.id == id }
    }

    public func listPlans() throws -> [SubscriptionPlan] {
        try swiftStore(commerce).subscriptionPlans
    }

    public func activatePlan(id: String) throws -> SubscriptionPlan {
        try replacePlan(id: id, status: "active")
    }

    public func archivePlan(id: String) throws -> SubscriptionPlan {
        try replacePlan(id: id, status: "archived")
    }

    public func subscribe(customerId: String, planId: String) throws -> Subscription {
        let store = try swiftStore(commerce)
        let subscription = Subscription(
            id: store.id("sub"),
            customerId: customerId,
            planId: planId,
            status: "active",
            createdAt: timestamp()
        )
        store.subscriptions.append(subscription)
        return subscription
    }

    public func get(id: String) throws -> Subscription? {
        try swiftStore(commerce).subscriptions.first { $0.id == id }
    }

    public func list() throws -> [Subscription] {
        try swiftStore(commerce).subscriptions
    }

    public func pause(id: String) throws -> Subscription {
        try replaceSubscription(id: id, status: "paused")
    }

    public func resume(id: String) throws -> Subscription {
        try replaceSubscription(id: id, status: "active")
    }

    public func cancel(id: String) throws -> Subscription {
        try replaceSubscription(id: id, status: "cancelled")
    }

    private func replacePlan(id: String, status: String) throws -> SubscriptionPlan {
        let store = try swiftStore(commerce)
        guard let index = store.subscriptionPlans.firstIndex(where: { $0.id == id }) else {
            throw StateSetError.operationFailed("Subscription plan not found")
        }
        let current = store.subscriptionPlans[index]
        let updated = SubscriptionPlan(
            id: current.id,
            code: current.code,
            name: current.name,
            interval: current.interval,
            intervalCount: current.intervalCount,
            price: current.price,
            currency: current.currency,
            status: status,
            createdAt: current.createdAt
        )
        store.subscriptionPlans[index] = updated
        return updated
    }

    private func replaceSubscription(id: String, status: String) throws -> Subscription {
        let store = try swiftStore(commerce)
        guard let index = store.subscriptions.firstIndex(where: { $0.id == id }) else {
            throw StateSetError.operationFailed("Subscription not found")
        }
        let current = store.subscriptions[index]
        let updated = Subscription(
            id: current.id,
            customerId: current.customerId,
            planId: current.planId,
            status: status,
            createdAt: current.createdAt
        )
        store.subscriptions[index] = updated
        return updated
    }
}

// MARK: - Promotions API

public final class PromotionsAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func create(code: String, name: String, discountType: String, discountValue: Double) throws -> Promotion {
        let store = try swiftStore(commerce)
        let promotion = Promotion(
            id: store.id("promo"),
            code: code,
            name: name,
            discountType: discountType,
            discountValue: discountValue,
            isActive: false,
            createdAt: timestamp()
        )
        store.promotions.append(promotion)
        return promotion
    }

    public func get(id: String) throws -> Promotion? {
        try swiftStore(commerce).promotions.first { $0.id == id }
    }

    public func getByCode(code: String) throws -> Promotion? {
        try swiftStore(commerce).promotions.first { $0.code == code }
    }

    public func list() throws -> [Promotion] {
        try swiftStore(commerce).promotions
    }

    public func activate(id: String) throws -> Promotion {
        try replacePromotion(id: id, isActive: true)
    }

    public func deactivate(id: String) throws -> Promotion {
        try replacePromotion(id: id, isActive: false)
    }

    public func delete(id: String) throws -> Bool {
        let store = try swiftStore(commerce)
        let count = store.promotions.count
        store.promotions.removeAll { $0.id == id }
        return store.promotions.count != count
    }

    public func getActive() throws -> [Promotion] {
        try swiftStore(commerce).promotions.filter(\.isActive)
    }

    public func createCoupon(promotionId: String, code: String, maxUses: Int? = nil) throws -> Coupon {
        let store = try swiftStore(commerce)
        let coupon = Coupon(
            id: store.id("coupon"),
            promotionId: promotionId,
            code: code,
            maxUses: maxUses,
            usedCount: 0,
            isActive: true,
            createdAt: timestamp()
        )
        store.coupons.append(coupon)
        return coupon
    }

    public func getCouponByCode(code: String) throws -> Coupon? {
        try swiftStore(commerce).coupons.first { $0.code == code }
    }

    public func validateCoupon(code: String) throws -> Coupon? {
        try swiftStore(commerce).coupons.first { $0.code == code && $0.isActive }
    }

    private func replacePromotion(id: String, isActive: Bool) throws -> Promotion {
        let store = try swiftStore(commerce)
        guard let index = store.promotions.firstIndex(where: { $0.id == id }) else {
            throw StateSetError.operationFailed("Promotion not found")
        }
        let current = store.promotions[index]
        let updated = Promotion(
            id: current.id,
            code: current.code,
            name: current.name,
            discountType: current.discountType,
            discountValue: current.discountValue,
            isActive: isActive,
            createdAt: current.createdAt
        )
        store.promotions[index] = updated
        return updated
    }
}

// MARK: - Tax API

public final class TaxAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}

    public func calculate(lineItemsJSON: String, shippingCountry: String, shippingState: String? = nil) throws -> TaxCalculation {
        TaxCalculation(subtotal: 0, taxAmount: 0, total: 0, currency: Currency.usd.rawValue)
    }

    public func getEffectiveRate(country: String, state: String? = nil, category: String? = nil) throws -> Double {
        0
    }

    public func getSettings() throws -> TaxSettings {
        TaxSettings(enabled: false, defaultCountry: "US", pricesIncludeTax: false)
    }

    public func setEnabled(enabled: Bool) throws -> TaxSettings {
        TaxSettings(enabled: enabled, defaultCountry: "US", pricesIncludeTax: false)
    }
}

// MARK: - Warehouse API

public final class WarehouseAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createWarehouse(code: String, name: String, warehouseType: String = "standard") throws -> Warehouse {
        let store = try swiftStore(commerce)
        let warehouse = Warehouse(
            id: store.intId(),
            code: code,
            name: name,
            warehouseType: warehouseType,
            isActive: true,
            createdAt: timestamp()
        )
        store.warehouses.append(warehouse)
        return warehouse
    }

    public func getWarehouse(id: Int) throws -> Warehouse? {
        try swiftStore(commerce).warehouses.first { $0.id == id }
    }

    public func getWarehouseByCode(code: String) throws -> Warehouse? {
        try swiftStore(commerce).warehouses.first { $0.code == code }
    }

    public func listWarehouses() throws -> [Warehouse] {
        try swiftStore(commerce).warehouses
    }
}

// MARK: - General Ledger API

public final class GeneralLedgerAPI: @unchecked Sendable {
    private weak var commerce: StateSetCommerce?

    internal init(commerce: StateSetCommerce) {
        self.commerce = commerce
    }

    public func createAccount(accountNumber: String, name: String, accountType: String) throws -> GlAccount {
        let store = try swiftStore(commerce)
        let account = GlAccount(
            id: store.id("gl"),
            accountNumber: accountNumber,
            name: name,
            accountType: accountType,
            isActive: true,
            createdAt: timestamp()
        )
        store.glAccounts.append(account)
        return account
    }

    public func listAccounts() throws -> [GlAccount] {
        try swiftStore(commerce).glAccounts
    }
}

// MARK: - v1 Availability APIs

public final class WarrantiesAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class SuppliersAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class PurchaseOrdersAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class InvoicesAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class BOMAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class WorkOrdersAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class QualityAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class LotsAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class SerialsAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class ReceivingAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class FulfillmentAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class AccountsPayableAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class AccountsReceivableAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class CostAccountingAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class CreditAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}

public final class BackordersAPI: @unchecked Sendable {
    internal init(commerce: StateSetCommerce) {}
}
