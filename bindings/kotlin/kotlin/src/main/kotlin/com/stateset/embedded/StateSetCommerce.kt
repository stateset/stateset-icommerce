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
    internal external fun nativeCustomerCreate(ptr: Long, email: String, firstName: String, lastName: String, phone: String?): String?
    internal external fun nativeCustomerGet(ptr: Long, id: String): String?
    internal external fun nativeCustomerList(ptr: Long): String?
    internal external fun nativeCustomerDelete(ptr: Long, id: String): Int

    // Product natives
    internal external fun nativeProductCreate(ptr: Long, name: String, sku: String, price: Double, description: String?): String?
    internal external fun nativeProductGet(ptr: Long, id: String): String?
    internal external fun nativeProductList(ptr: Long): String?

    // Order natives
    internal external fun nativeOrderCreate(ptr: Long, customerId: String, itemsJson: String, currency: String): String?
    internal external fun nativeOrderGet(ptr: Long, id: String): String?
    internal external fun nativeOrderList(ptr: Long): String?
    internal external fun nativeOrderUpdateStatus(ptr: Long, id: String, status: String): String?

    // Inventory natives
    internal external fun nativeInventoryCreateItem(ptr: Long, sku: String, name: String, initialQuantity: Double): String?
    internal external fun nativeInventoryAdjust(ptr: Long, sku: String, quantityDelta: Double, reason: String): Int
    internal external fun nativeInventoryGetLevel(ptr: Long, sku: String): String?

    // Cart natives
    internal external fun nativeCartCreate(ptr: Long, customerId: String?, currency: String?): String?
    internal external fun nativeCartAddItem(ptr: Long, cartId: String, variantId: String, quantity: Int): String?
    internal external fun nativeCartGet(ptr: Long, cartId: String): String?

    // Return natives
    internal external fun nativeReturnCreate(ptr: Long, orderId: String, reason: String, notes: String?): String?
    internal external fun nativeReturnList(ptr: Long): String?

    // Payment natives
    internal external fun nativePaymentCreate(ptr: Long, orderId: String, amount: Double, currency: String, method: String): String?

    // Analytics natives
    internal external fun nativeAnalyticsSalesSummary(ptr: Long, period: String): String?
    internal external fun nativeAnalyticsTopProducts(ptr: Long, limit: Int): String?
    internal external fun nativeAnalyticsTopCustomers(ptr: Long, limit: Int): String?
}

/**
 * Exception thrown when a StateSet operation fails
 */
class StateSetException(message: String) : RuntimeException(message)
