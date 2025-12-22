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
