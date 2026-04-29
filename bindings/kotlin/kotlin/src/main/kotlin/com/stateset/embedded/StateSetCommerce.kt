package com.stateset.embedded

import kotlinx.serialization.json.Json
import java.io.Closeable

/**
 * StateSet Embedded Commerce - The SQLite of Commerce
 *
 * A zero-dependency, local-first commerce engine for Kotlin/Android applications.
 *
 * Example usage:
 * ```kotlin
 * StateSetCommerce("store.db").use { commerce ->
 *     val customer = commerce.customers.create(
 *         email = "alice@example.com",
 *         firstName = "Alice",
 *         lastName = "Smith"
 *     )
 *
 *     val product = commerce.products.create(
 *         name = "Premium Widget",
 *         sku = "WIDGET-001",
 *         price = 29.99
 *     )
 *
 *     val order = commerce.orders.create(
 *         customerId = customer.id,
 *         items = listOf(OrderItem("WIDGET-001", "Widget", 2, 29.99)),
 *         currency = "USD"
 *     )
 * }
 * ```
 */
class StateSetCommerce(dbPath: String) : Closeable {

    companion object {
        init {
            NativeLoader.load()
        }

        internal val json = Json {
            ignoreUnknownKeys = true
            coerceInputValues = true
            isLenient = true
        }
    }

    private var nativePtr: Long = nativeCreate(dbPath)

    init {
        if (nativePtr == 0L) {
            throw StateSetException("Failed to create commerce instance")
        }
    }

    /** Customers API */
    val customers: Customers = Customers(this)

    /** Products API */
    val products: Products = Products(this)

    /** Orders API */
    val orders: Orders = Orders(this)

    /** Inventory API */
    val inventory: Inventory = Inventory(this)

    /** Carts API */
    val carts: Carts = Carts(this)

    /** Returns API */
    val returns: Returns = Returns(this)

    /** Payments API */
    val payments: Payments = Payments(this)

    /** Analytics API */
    val analytics: Analytics = Analytics(this)

    /** Shipments API */
    val shipments: Shipments = Shipments(this)

    /** Warranties API */
    val warranties: Warranties = Warranties(this)

    /** Suppliers API */
    val suppliers: Suppliers = Suppliers(this)

    /** Purchase Orders API */
    val purchaseOrders: PurchaseOrders = PurchaseOrders(this)

    /** Invoices API */
    val invoices: Invoices = Invoices(this)

    /** Bill of Materials API */
    val bom: BOM = BOM(this)

    /** Work Orders API */
    val workOrders: WorkOrders = WorkOrders(this)

    /** Currency API */
    val currency: CurrencyApi = CurrencyApi(this)

    internal fun getPtr(): Long = nativePtr

    override fun close() {
        if (nativePtr != 0L) {
            nativeDestroy(nativePtr)
            nativePtr = 0L
        }
    }

    // Native methods
    private external fun nativeCreate(dbPath: String): Long
    private external fun nativeDestroy(ptr: Long)

    // Customer natives
    @JvmName("nativeCustomerCreate")
    internal external fun nativeCustomerCreate(ptr: Long, email: String, firstName: String, lastName: String, phone: String?): String?
    @JvmName("nativeCustomerGet")
    internal external fun nativeCustomerGet(ptr: Long, id: String): String?
    @JvmName("nativeCustomerList")
    internal external fun nativeCustomerList(ptr: Long): String?
    @JvmName("nativeCustomerDelete")
    internal external fun nativeCustomerDelete(ptr: Long, id: String): Int

    // Product natives
    @JvmName("nativeProductCreate")
    internal external fun nativeProductCreate(ptr: Long, name: String, sku: String, price: Double, description: String?): String?
    @JvmName("nativeProductGet")
    internal external fun nativeProductGet(ptr: Long, id: String): String?
    @JvmName("nativeProductList")
    internal external fun nativeProductList(ptr: Long): String?

    // Order natives
    @JvmName("nativeOrderCreate")
    internal external fun nativeOrderCreate(ptr: Long, customerId: String, itemsJson: String, currency: String): String?
    @JvmName("nativeOrderGet")
    internal external fun nativeOrderGet(ptr: Long, id: String): String?
    @JvmName("nativeOrderList")
    internal external fun nativeOrderList(ptr: Long): String?
    @JvmName("nativeOrderUpdateStatus")
    internal external fun nativeOrderUpdateStatus(ptr: Long, id: String, status: String): String?

    // Inventory natives
    @JvmName("nativeInventoryCreateItem")
    internal external fun nativeInventoryCreateItem(ptr: Long, sku: String, name: String, initialQuantity: Double): String?
    @JvmName("nativeInventoryAdjust")
    internal external fun nativeInventoryAdjust(ptr: Long, sku: String, quantityDelta: Double, reason: String): Int
    @JvmName("nativeInventoryGetLevel")
    internal external fun nativeInventoryGetLevel(ptr: Long, sku: String): String?

    // Cart natives
    @JvmName("nativeCartCreate")
    internal external fun nativeCartCreate(ptr: Long, customerId: String?, currency: String?): String?
    @JvmName("nativeCartAddItem")
    internal external fun nativeCartAddItem(ptr: Long, cartId: String, variantId: String, quantity: Int): String?
    @JvmName("nativeCartGet")
    internal external fun nativeCartGet(ptr: Long, cartId: String): String?

    // Return natives
    @JvmName("nativeReturnCreate")
    internal external fun nativeReturnCreate(ptr: Long, orderId: String, reason: String, notes: String?): String?
    @JvmName("nativeReturnList")
    internal external fun nativeReturnList(ptr: Long): String?

    // Payment natives
    @JvmName("nativePaymentCreate")
    internal external fun nativePaymentCreate(ptr: Long, orderId: String, amount: Double, currency: String, method: String): String?

    // Analytics natives
    @JvmName("nativeAnalyticsSalesSummary")
    internal external fun nativeAnalyticsSalesSummary(ptr: Long, period: String): String?
    @JvmName("nativeAnalyticsTopProducts")
    internal external fun nativeAnalyticsTopProducts(ptr: Long, limit: Int): String?
    @JvmName("nativeAnalyticsTopCustomers")
    internal external fun nativeAnalyticsTopCustomers(ptr: Long, limit: Int): String?

    // Order natives - additional
    @JvmName("nativeOrderShip")
    internal external fun nativeOrderShip(ptr: Long, id: String): String?
    @JvmName("nativeOrderCancel")
    internal external fun nativeOrderCancel(ptr: Long, id: String): String?

    // Return natives - additional
    @JvmName("nativeReturnGet")
    internal external fun nativeReturnGet(ptr: Long, id: String): String?
    @JvmName("nativeReturnApprove")
    internal external fun nativeReturnApprove(ptr: Long, id: String): String?
    @JvmName("nativeReturnReject")
    internal external fun nativeReturnReject(ptr: Long, id: String, reason: String): String?
    @JvmName("nativeReturnComplete")
    internal external fun nativeReturnComplete(ptr: Long, id: String): String?

    // Payment natives - additional
    @JvmName("nativePaymentGet")
    internal external fun nativePaymentGet(ptr: Long, id: String): String?
    @JvmName("nativePaymentList")
    internal external fun nativePaymentList(ptr: Long): String?
    @JvmName("nativePaymentComplete")
    internal external fun nativePaymentComplete(ptr: Long, id: String): String?
    @JvmName("nativePaymentFail")
    internal external fun nativePaymentFail(ptr: Long, id: String, reason: String): String?
    @JvmName("nativePaymentRefund")
    internal external fun nativePaymentRefund(ptr: Long, paymentId: String, amount: Double, reason: String): String?

    // Shipment natives
    @JvmName("nativeShipmentCreate")
    internal external fun nativeShipmentCreate(ptr: Long, orderId: String, recipientName: String, shippingAddress: String, carrier: String): String?
    @JvmName("nativeShipmentGet")
    internal external fun nativeShipmentGet(ptr: Long, id: String): String?
    @JvmName("nativeShipmentList")
    internal external fun nativeShipmentList(ptr: Long): String?
    @JvmName("nativeShipmentShip")
    internal external fun nativeShipmentShip(ptr: Long, id: String, trackingNumber: String): String?
    @JvmName("nativeShipmentDeliver")
    internal external fun nativeShipmentDeliver(ptr: Long, id: String): String?
    @JvmName("nativeShipmentCancel")
    internal external fun nativeShipmentCancel(ptr: Long, id: String): String?

    // Warranty natives
    internal external fun nativeWarrantyCreate(ptr: Long, customerId: String, productId: String, warrantyType: String, durationMonths: Int): String?
    internal external fun nativeWarrantyGet(ptr: Long, id: String): String?
    internal external fun nativeWarrantyList(ptr: Long): String?
    internal external fun nativeWarrantyCreateClaim(ptr: Long, warrantyId: String, issueDescription: String): String?
    internal external fun nativeWarrantyApproveClaim(ptr: Long, claimId: String): String?
    internal external fun nativeWarrantyDenyClaim(ptr: Long, claimId: String, reason: String): String?
    internal external fun nativeWarrantyCompleteClaim(ptr: Long, claimId: String, resolution: String): String?

    // Supplier natives
    internal external fun nativeSupplierCreate(ptr: Long, name: String, email: String, phone: String): String?
    internal external fun nativeSupplierGet(ptr: Long, id: String): String?
    internal external fun nativeSupplierList(ptr: Long): String?

    // Purchase Order natives
    internal external fun nativePurchaseOrderCreate(ptr: Long, supplierId: String, itemsJson: String): String?
    internal external fun nativePurchaseOrderGet(ptr: Long, id: String): String?
    internal external fun nativePurchaseOrderList(ptr: Long): String?
    internal external fun nativePurchaseOrderSubmit(ptr: Long, id: String): String?
    internal external fun nativePurchaseOrderApprove(ptr: Long, id: String, approvedBy: String): String?
    internal external fun nativePurchaseOrderSend(ptr: Long, id: String): String?
    internal external fun nativePurchaseOrderCancel(ptr: Long, id: String): String?

    // Invoice natives
    internal external fun nativeInvoiceCreate(ptr: Long, customerId: String, itemsJson: String, billingEmail: String): String?
    internal external fun nativeInvoiceGet(ptr: Long, id: String): String?
    internal external fun nativeInvoiceList(ptr: Long): String?
    internal external fun nativeInvoiceSend(ptr: Long, id: String): String?
    internal external fun nativeInvoiceVoid(ptr: Long, id: String): String?
    internal external fun nativeInvoiceRecordPayment(ptr: Long, id: String, amount: Double, paymentMethod: String): String?
    internal external fun nativeInvoiceGetOverdue(ptr: Long): String?

    // BOM natives
    internal external fun nativeBOMCreate(ptr: Long, productId: String, name: String, description: String?): String?
    internal external fun nativeBOMGet(ptr: Long, id: String): String?
    internal external fun nativeBOMList(ptr: Long): String?
    internal external fun nativeBOMAddComponent(ptr: Long, bomId: String, name: String, componentSku: String, quantity: Double): String?
    internal external fun nativeBOMGetComponents(ptr: Long, bomId: String): String?
    internal external fun nativeBOMActivate(ptr: Long, id: String): String?

    // Work Order natives
    internal external fun nativeWorkOrderCreate(ptr: Long, productId: String, quantityToBuild: Double, bomId: String?): String?
    internal external fun nativeWorkOrderGet(ptr: Long, id: String): String?
    internal external fun nativeWorkOrderList(ptr: Long): String?
    internal external fun nativeWorkOrderStart(ptr: Long, id: String): String?
    internal external fun nativeWorkOrderComplete(ptr: Long, id: String, quantityCompleted: Double): String?
    internal external fun nativeWorkOrderCancel(ptr: Long, id: String): String?

    // Currency natives
    internal external fun nativeCurrencySetRate(ptr: Long, fromCurrency: String, toCurrency: String, rate: Double): String?
    internal external fun nativeCurrencyGetRate(ptr: Long, fromCurrency: String, toCurrency: String): String?
    internal external fun nativeCurrencyConvert(ptr: Long, amount: Double, fromCurrency: String, toCurrency: String): String?
    internal external fun nativeCurrencyGetSettings(ptr: Long): String?
}

/**
 * Exception thrown when a StateSet operation fails
 */
class StateSetException(message: String) : RuntimeException(message)
