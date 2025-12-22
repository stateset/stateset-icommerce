package com.stateset.embedded

import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

/**
 * Customers API
 */
class Customers internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        email: String,
        firstName: String,
        lastName: String,
        phone: String? = null
    ): Customer {
        val json = commerce.nativeCustomerCreate(
            commerce.getPtr(),
            email,
            firstName,
            lastName,
            phone ?: ""
        ) ?: throw StateSetException("Failed to create customer")

        return StateSetCommerce.json.decodeFromString(Customer.serializer(), json)
    }

    fun get(id: String): Customer? {
        val json = commerce.nativeCustomerGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Customer.serializer(), json)
    }

    fun list(): List<Customer> {
        val json = commerce.nativeCustomerList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Customer.serializer()), json)
    }

    fun delete(id: String): Boolean {
        return commerce.nativeCustomerDelete(commerce.getPtr(), id) == 1
    }
}

/**
 * Products API
 */
class Products internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        name: String,
        sku: String,
        price: Double,
        description: String? = null
    ): Product {
        val json = commerce.nativeProductCreate(
            commerce.getPtr(),
            name,
            sku,
            price,
            description ?: ""
        ) ?: throw StateSetException("Failed to create product")

        return StateSetCommerce.json.decodeFromString(Product.serializer(), json)
    }

    fun get(id: String): Product? {
        val json = commerce.nativeProductGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Product.serializer(), json)
    }

    fun list(): List<Product> {
        val json = commerce.nativeProductList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Product.serializer()), json)
    }
}

/**
 * Orders API
 */
class Orders internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        customerId: String,
        items: List<OrderItem>,
        currency: String = "USD"
    ): Order {
        val itemsJson = StateSetCommerce.json.encodeToString(ListSerializer(OrderItem.serializer()), items)

        val json = commerce.nativeOrderCreate(
            commerce.getPtr(),
            customerId,
            itemsJson,
            currency
        ) ?: throw StateSetException("Failed to create order")

        return StateSetCommerce.json.decodeFromString(Order.serializer(), json)
    }

    fun get(id: String): Order? {
        val json = commerce.nativeOrderGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Order.serializer(), json)
    }

    fun list(): List<Order> {
        val json = commerce.nativeOrderList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Order.serializer()), json)
    }

    fun updateStatus(id: String, status: OrderStatus): Order {
        val json = commerce.nativeOrderUpdateStatus(
            commerce.getPtr(),
            id,
            status.name.lowercase()
        ) ?: throw StateSetException("Failed to update order status")

        return StateSetCommerce.json.decodeFromString(Order.serializer(), json)
    }

    fun ship(id: String): Order {
        val json = commerce.nativeOrderShip(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to ship order")
        return StateSetCommerce.json.decodeFromString(Order.serializer(), json)
    }

    fun cancel(id: String): Order {
        val json = commerce.nativeOrderCancel(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to cancel order")
        return StateSetCommerce.json.decodeFromString(Order.serializer(), json)
    }
}

/**
 * Inventory API
 */
class Inventory internal constructor(private val commerce: StateSetCommerce) {

    fun createItem(
        sku: String,
        name: String,
        initialQuantity: Double = 0.0
    ): InventoryItem {
        val json = commerce.nativeInventoryCreateItem(
            commerce.getPtr(),
            sku,
            name,
            initialQuantity
        ) ?: throw StateSetException("Failed to create inventory item")

        return StateSetCommerce.json.decodeFromString(InventoryItem.serializer(), json)
    }

    fun adjust(
        sku: String,
        quantityDelta: Double,
        reason: String = "manual adjustment"
    ): Boolean {
        return commerce.nativeInventoryAdjust(
            commerce.getPtr(),
            sku,
            quantityDelta,
            reason
        ) == 1
    }

    fun getLevel(sku: String): StockLevel? {
        val json = commerce.nativeInventoryGetLevel(commerce.getPtr(), sku) ?: return null
        return StateSetCommerce.json.decodeFromString(StockLevel.serializer(), json)
    }
}

/**
 * Carts API
 */
class Carts internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        customerId: String? = null,
        currency: String = "USD"
    ): Cart {
        val json = commerce.nativeCartCreate(
            commerce.getPtr(),
            customerId ?: "",
            currency
        ) ?: throw StateSetException("Failed to create cart")

        return StateSetCommerce.json.decodeFromString(Cart.serializer(), json)
    }

    fun addItem(
        cartId: String,
        variantId: String,
        quantity: Int = 1
    ): Cart {
        val json = commerce.nativeCartAddItem(
            commerce.getPtr(),
            cartId,
            variantId,
            quantity
        ) ?: throw StateSetException("Failed to add item to cart")

        return StateSetCommerce.json.decodeFromString(Cart.serializer(), json)
    }

    fun get(cartId: String): Cart? {
        val json = commerce.nativeCartGet(commerce.getPtr(), cartId) ?: return null
        return StateSetCommerce.json.decodeFromString(Cart.serializer(), json)
    }
}

/**
 * Returns API
 */
class Returns internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        orderId: String,
        reason: ReturnReason,
        notes: String? = null
    ): Return {
        val json = commerce.nativeReturnCreate(
            commerce.getPtr(),
            orderId,
            reason.name.lowercase(),
            notes ?: ""
        ) ?: throw StateSetException("Failed to create return")

        return StateSetCommerce.json.decodeFromString(Return.serializer(), json)
    }

    fun list(): List<Return> {
        val json = commerce.nativeReturnList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Return.serializer()), json)
    }

    fun get(id: String): Return? {
        val json = commerce.nativeReturnGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Return.serializer(), json)
    }

    fun approve(id: String): Return {
        val json = commerce.nativeReturnApprove(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to approve return")
        return StateSetCommerce.json.decodeFromString(Return.serializer(), json)
    }

    fun reject(id: String, reason: String): Return {
        val json = commerce.nativeReturnReject(commerce.getPtr(), id, reason)
            ?: throw StateSetException("Failed to reject return")
        return StateSetCommerce.json.decodeFromString(Return.serializer(), json)
    }

    fun complete(id: String): Return {
        val json = commerce.nativeReturnComplete(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to complete return")
        return StateSetCommerce.json.decodeFromString(Return.serializer(), json)
    }
}

/**
 * Payments API
 */
class Payments internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        orderId: String,
        amount: Double,
        currency: String = "USD",
        method: PaymentMethod = PaymentMethod.CreditCard
    ): Payment {
        val json = commerce.nativePaymentCreate(
            commerce.getPtr(),
            orderId,
            amount,
            currency,
            method.name.lowercase()
        ) ?: throw StateSetException("Failed to create payment")

        return StateSetCommerce.json.decodeFromString(Payment.serializer(), json)
    }

    fun get(id: String): Payment? {
        val json = commerce.nativePaymentGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Payment.serializer(), json)
    }

    fun list(): List<Payment> {
        val json = commerce.nativePaymentList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Payment.serializer()), json)
    }

    fun complete(id: String): Payment {
        val json = commerce.nativePaymentComplete(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to complete payment")
        return StateSetCommerce.json.decodeFromString(Payment.serializer(), json)
    }

    fun fail(id: String, reason: String): Payment {
        val json = commerce.nativePaymentFail(commerce.getPtr(), id, reason)
            ?: throw StateSetException("Failed to fail payment")
        return StateSetCommerce.json.decodeFromString(Payment.serializer(), json)
    }

    fun refund(paymentId: String, amount: Double, reason: String): Refund {
        val json = commerce.nativePaymentRefund(commerce.getPtr(), paymentId, amount, reason)
            ?: throw StateSetException("Failed to refund payment")
        return StateSetCommerce.json.decodeFromString(Refund.serializer(), json)
    }
}

/**
 * Analytics API
 */
class Analytics internal constructor(private val commerce: StateSetCommerce) {

    fun salesSummary(period: TimePeriod = TimePeriod.ThisMonth): SalesSummary {
        val periodStr = when (period) {
            TimePeriod.Today -> "today"
            TimePeriod.ThisWeek -> "week"
            TimePeriod.ThisMonth -> "month"
            TimePeriod.ThisQuarter -> "quarter"
            TimePeriod.ThisYear -> "year"
            TimePeriod.AllTime -> "all"
        }

        val json = commerce.nativeAnalyticsSalesSummary(
            commerce.getPtr(),
            periodStr
        ) ?: throw StateSetException("Failed to get sales summary")

        return StateSetCommerce.json.decodeFromString(SalesSummary.serializer(), json)
    }

    fun topProducts(limit: Int = 10): List<TopProduct> {
        val json = commerce.nativeAnalyticsTopProducts(commerce.getPtr(), limit) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(TopProduct.serializer()), json)
    }

    fun topCustomers(limit: Int = 10): List<TopCustomer> {
        val json = commerce.nativeAnalyticsTopCustomers(commerce.getPtr(), limit) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(TopCustomer.serializer()), json)
    }
}

/**
 * Shipments API
 */
class Shipments internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        orderId: String,
        recipientName: String,
        shippingAddress: String,
        carrier: String
    ): Shipment {
        val json = commerce.nativeShipmentCreate(
            commerce.getPtr(),
            orderId,
            recipientName,
            shippingAddress,
            carrier
        ) ?: throw StateSetException("Failed to create shipment")

        return StateSetCommerce.json.decodeFromString(Shipment.serializer(), json)
    }

    fun get(id: String): Shipment? {
        val json = commerce.nativeShipmentGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Shipment.serializer(), json)
    }

    fun list(): List<Shipment> {
        val json = commerce.nativeShipmentList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Shipment.serializer()), json)
    }

    fun ship(id: String, trackingNumber: String): Shipment {
        val json = commerce.nativeShipmentShip(commerce.getPtr(), id, trackingNumber)
            ?: throw StateSetException("Failed to ship shipment")
        return StateSetCommerce.json.decodeFromString(Shipment.serializer(), json)
    }

    fun deliver(id: String): Shipment {
        val json = commerce.nativeShipmentDeliver(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to deliver shipment")
        return StateSetCommerce.json.decodeFromString(Shipment.serializer(), json)
    }

    fun cancel(id: String): Shipment {
        val json = commerce.nativeShipmentCancel(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to cancel shipment")
        return StateSetCommerce.json.decodeFromString(Shipment.serializer(), json)
    }
}

/**
 * Warranties API
 */
class Warranties internal constructor(private val commerce: StateSetCommerce) {

    fun create(
        customerId: String,
        productId: String,
        warrantyType: WarrantyType,
        durationMonths: Int
    ): Warranty {
        val json = commerce.nativeWarrantyCreate(
            commerce.getPtr(),
            customerId,
            productId,
            warrantyType.name.lowercase(),
            durationMonths
        ) ?: throw StateSetException("Failed to create warranty")

        return StateSetCommerce.json.decodeFromString(Warranty.serializer(), json)
    }

    fun get(id: String): Warranty? {
        val json = commerce.nativeWarrantyGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Warranty.serializer(), json)
    }

    fun list(): List<Warranty> {
        val json = commerce.nativeWarrantyList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Warranty.serializer()), json)
    }

    fun createClaim(warrantyId: String, issueDescription: String): WarrantyClaim {
        val json = commerce.nativeWarrantyCreateClaim(commerce.getPtr(), warrantyId, issueDescription)
            ?: throw StateSetException("Failed to create warranty claim")
        return StateSetCommerce.json.decodeFromString(WarrantyClaim.serializer(), json)
    }

    fun approveClaim(claimId: String): WarrantyClaim {
        val json = commerce.nativeWarrantyApproveClaim(commerce.getPtr(), claimId)
            ?: throw StateSetException("Failed to approve warranty claim")
        return StateSetCommerce.json.decodeFromString(WarrantyClaim.serializer(), json)
    }

    fun denyClaim(claimId: String, reason: String): WarrantyClaim {
        val json = commerce.nativeWarrantyDenyClaim(commerce.getPtr(), claimId, reason)
            ?: throw StateSetException("Failed to deny warranty claim")
        return StateSetCommerce.json.decodeFromString(WarrantyClaim.serializer(), json)
    }

    fun completeClaim(claimId: String, resolution: ClaimResolution): WarrantyClaim {
        val resolutionStr = when (resolution) {
            ClaimResolution.Repair -> "repair"
            ClaimResolution.Replacement -> "replacement"
            ClaimResolution.Refund -> "refund"
            ClaimResolution.StoreCredit -> "store_credit"
        }
        val json = commerce.nativeWarrantyCompleteClaim(commerce.getPtr(), claimId, resolutionStr)
            ?: throw StateSetException("Failed to complete warranty claim")
        return StateSetCommerce.json.decodeFromString(WarrantyClaim.serializer(), json)
    }
}

/**
 * Suppliers API
 */
class Suppliers internal constructor(private val commerce: StateSetCommerce) {

    fun create(name: String, email: String, phone: String): Supplier {
        val json = commerce.nativeSupplierCreate(commerce.getPtr(), name, email, phone)
            ?: throw StateSetException("Failed to create supplier")
        return StateSetCommerce.json.decodeFromString(Supplier.serializer(), json)
    }

    fun get(id: String): Supplier? {
        val json = commerce.nativeSupplierGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Supplier.serializer(), json)
    }

    fun list(): List<Supplier> {
        val json = commerce.nativeSupplierList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Supplier.serializer()), json)
    }
}

/**
 * Purchase Orders API
 */
class PurchaseOrders internal constructor(private val commerce: StateSetCommerce) {

    fun create(supplierId: String, items: List<PurchaseOrderItem>): PurchaseOrder {
        val itemsJson = StateSetCommerce.json.encodeToString(ListSerializer(PurchaseOrderItem.serializer()), items)
        val json = commerce.nativePurchaseOrderCreate(commerce.getPtr(), supplierId, itemsJson)
            ?: throw StateSetException("Failed to create purchase order")
        return StateSetCommerce.json.decodeFromString(PurchaseOrder.serializer(), json)
    }

    fun get(id: String): PurchaseOrder? {
        val json = commerce.nativePurchaseOrderGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(PurchaseOrder.serializer(), json)
    }

    fun list(): List<PurchaseOrder> {
        val json = commerce.nativePurchaseOrderList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(PurchaseOrder.serializer()), json)
    }

    fun submit(id: String): PurchaseOrder {
        val json = commerce.nativePurchaseOrderSubmit(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to submit purchase order")
        return StateSetCommerce.json.decodeFromString(PurchaseOrder.serializer(), json)
    }

    fun approve(id: String, approvedBy: String): PurchaseOrder {
        val json = commerce.nativePurchaseOrderApprove(commerce.getPtr(), id, approvedBy)
            ?: throw StateSetException("Failed to approve purchase order")
        return StateSetCommerce.json.decodeFromString(PurchaseOrder.serializer(), json)
    }

    fun send(id: String): PurchaseOrder {
        val json = commerce.nativePurchaseOrderSend(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to send purchase order")
        return StateSetCommerce.json.decodeFromString(PurchaseOrder.serializer(), json)
    }

    fun cancel(id: String): PurchaseOrder {
        val json = commerce.nativePurchaseOrderCancel(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to cancel purchase order")
        return StateSetCommerce.json.decodeFromString(PurchaseOrder.serializer(), json)
    }
}

/**
 * Invoices API
 */
class Invoices internal constructor(private val commerce: StateSetCommerce) {

    fun create(customerId: String, items: List<InvoiceItem>, billingEmail: String): Invoice {
        val itemsJson = StateSetCommerce.json.encodeToString(ListSerializer(InvoiceItem.serializer()), items)
        val json = commerce.nativeInvoiceCreate(commerce.getPtr(), customerId, itemsJson, billingEmail)
            ?: throw StateSetException("Failed to create invoice")
        return StateSetCommerce.json.decodeFromString(Invoice.serializer(), json)
    }

    fun get(id: String): Invoice? {
        val json = commerce.nativeInvoiceGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(Invoice.serializer(), json)
    }

    fun list(): List<Invoice> {
        val json = commerce.nativeInvoiceList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Invoice.serializer()), json)
    }

    fun send(id: String): Invoice {
        val json = commerce.nativeInvoiceSend(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to send invoice")
        return StateSetCommerce.json.decodeFromString(Invoice.serializer(), json)
    }

    fun void(id: String): Invoice {
        val json = commerce.nativeInvoiceVoid(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to void invoice")
        return StateSetCommerce.json.decodeFromString(Invoice.serializer(), json)
    }

    fun recordPayment(id: String, amount: Double, paymentMethod: String): Invoice {
        val json = commerce.nativeInvoiceRecordPayment(commerce.getPtr(), id, amount, paymentMethod)
            ?: throw StateSetException("Failed to record payment")
        return StateSetCommerce.json.decodeFromString(Invoice.serializer(), json)
    }

    fun getOverdue(): List<Invoice> {
        val json = commerce.nativeInvoiceGetOverdue(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(Invoice.serializer()), json)
    }
}

/**
 * Bill of Materials (BOM) API
 */
class BOM internal constructor(private val commerce: StateSetCommerce) {

    fun create(productId: String, name: String, description: String? = null): BillOfMaterials {
        val json = commerce.nativeBOMCreate(commerce.getPtr(), productId, name, description ?: "")
            ?: throw StateSetException("Failed to create BOM")
        return StateSetCommerce.json.decodeFromString(BillOfMaterials.serializer(), json)
    }

    fun get(id: String): BillOfMaterials? {
        val json = commerce.nativeBOMGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(BillOfMaterials.serializer(), json)
    }

    fun list(): List<BillOfMaterials> {
        val json = commerce.nativeBOMList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(BillOfMaterials.serializer()), json)
    }

    fun addComponent(bomId: String, name: String, componentSku: String, quantity: Double): BOMComponent {
        val json = commerce.nativeBOMAddComponent(commerce.getPtr(), bomId, name, componentSku, quantity)
            ?: throw StateSetException("Failed to add component to BOM")
        return StateSetCommerce.json.decodeFromString(BOMComponent.serializer(), json)
    }

    fun getComponents(bomId: String): List<BOMComponent> {
        val json = commerce.nativeBOMGetComponents(commerce.getPtr(), bomId) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(BOMComponent.serializer()), json)
    }

    fun activate(id: String): BillOfMaterials {
        val json = commerce.nativeBOMActivate(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to activate BOM")
        return StateSetCommerce.json.decodeFromString(BillOfMaterials.serializer(), json)
    }
}

/**
 * Work Orders API
 */
class WorkOrders internal constructor(private val commerce: StateSetCommerce) {

    fun create(productId: String, quantityToBuild: Double, bomId: String? = null): WorkOrder {
        val json = commerce.nativeWorkOrderCreate(commerce.getPtr(), productId, quantityToBuild, bomId ?: "")
            ?: throw StateSetException("Failed to create work order")
        return StateSetCommerce.json.decodeFromString(WorkOrder.serializer(), json)
    }

    fun get(id: String): WorkOrder? {
        val json = commerce.nativeWorkOrderGet(commerce.getPtr(), id) ?: return null
        return StateSetCommerce.json.decodeFromString(WorkOrder.serializer(), json)
    }

    fun list(): List<WorkOrder> {
        val json = commerce.nativeWorkOrderList(commerce.getPtr()) ?: return emptyList()
        return StateSetCommerce.json.decodeFromString(ListSerializer(WorkOrder.serializer()), json)
    }

    fun start(id: String): WorkOrder {
        val json = commerce.nativeWorkOrderStart(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to start work order")
        return StateSetCommerce.json.decodeFromString(WorkOrder.serializer(), json)
    }

    fun complete(id: String, quantityCompleted: Double): WorkOrder {
        val json = commerce.nativeWorkOrderComplete(commerce.getPtr(), id, quantityCompleted)
            ?: throw StateSetException("Failed to complete work order")
        return StateSetCommerce.json.decodeFromString(WorkOrder.serializer(), json)
    }

    fun cancel(id: String): WorkOrder {
        val json = commerce.nativeWorkOrderCancel(commerce.getPtr(), id)
            ?: throw StateSetException("Failed to cancel work order")
        return StateSetCommerce.json.decodeFromString(WorkOrder.serializer(), json)
    }
}

/**
 * Currency API
 */
class CurrencyApi internal constructor(private val commerce: StateSetCommerce) {

    fun setRate(fromCurrency: Currency, toCurrency: Currency, rate: Double): ExchangeRate {
        val json = commerce.nativeCurrencySetRate(
            commerce.getPtr(),
            fromCurrency.name,
            toCurrency.name,
            rate
        ) ?: throw StateSetException("Failed to set exchange rate")
        return StateSetCommerce.json.decodeFromString(ExchangeRate.serializer(), json)
    }

    fun getRate(fromCurrency: Currency, toCurrency: Currency): ExchangeRate? {
        val json = commerce.nativeCurrencyGetRate(
            commerce.getPtr(),
            fromCurrency.name,
            toCurrency.name
        ) ?: return null
        return StateSetCommerce.json.decodeFromString(ExchangeRate.serializer(), json)
    }

    fun convert(amount: Double, fromCurrency: Currency, toCurrency: Currency): ConversionResult {
        val json = commerce.nativeCurrencyConvert(
            commerce.getPtr(),
            amount,
            fromCurrency.name,
            toCurrency.name
        ) ?: throw StateSetException("Failed to convert currency")
        return StateSetCommerce.json.decodeFromString(ConversionResult.serializer(), json)
    }

    fun getSettings(): StoreCurrencySettings {
        val json = commerce.nativeCurrencyGetSettings(commerce.getPtr())
            ?: throw StateSetException("Failed to get currency settings")
        return StateSetCommerce.json.decodeFromString(StoreCurrencySettings.serializer(), json)
    }
}
