package com.stateset.embedded

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class Customer(
    val id: String,
    val email: String,
    @SerialName("first_name") val firstName: String,
    @SerialName("last_name") val lastName: String,
    val phone: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class Product(
    val id: String,
    val name: String,
    val slug: String? = null,
    val description: String? = null,
    @SerialName("is_active") val isActive: Boolean = true,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class ProductVariant(
    val id: String,
    @SerialName("product_id") val productId: String,
    val sku: String,
    val name: String? = null,
    val price: String, // Decimal as string
    @SerialName("compare_at_price") val compareAtPrice: String? = null,
    @SerialName("is_default") val isDefault: Boolean = false,
    @SerialName("created_at") val createdAt: String? = null
)

@Serializable
data class Order(
    val id: String,
    @SerialName("order_number") val orderNumber: String,
    @SerialName("customer_id") val customerId: String,
    val status: String,
    @SerialName("total_amount") val totalAmount: String,
    val currency: String,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class OrderItem(
    val sku: String,
    val name: String,
    val quantity: Int,
    @SerialName("unit_price") val unitPrice: Double
)

@Serializable
data class InventoryItem(
    val id: String,
    val sku: String,
    val name: String,
    val description: String? = null,
    @SerialName("unit_of_measure") val unitOfMeasure: String? = null,
    @SerialName("created_at") val createdAt: String? = null
)

@Serializable
data class StockLevel(
    val id: String,
    @SerialName("inventory_item_id") val inventoryItemId: String,
    @SerialName("location_id") val locationId: String? = null,
    val available: String, // Decimal as string
    val reserved: String,
    val incoming: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class Cart(
    val id: String,
    @SerialName("customer_id") val customerId: String? = null,
    val status: String,
    @SerialName("grand_total") val grandTotal: String,
    val currency: String,
    @SerialName("created_at") val createdAt: String? = null
)

@Serializable
data class CartItem(
    val id: String,
    @SerialName("cart_id") val cartId: String,
    @SerialName("variant_id") val variantId: String,
    val quantity: Int,
    @SerialName("unit_price") val unitPrice: String,
    @SerialName("line_total") val lineTotal: String
)

@Serializable
data class Return(
    val id: String,
    @SerialName("order_id") val orderId: String,
    val reason: String,
    val status: String,
    @SerialName("refund_amount") val refundAmount: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null
)

@Serializable
data class Payment(
    val id: String,
    @SerialName("order_id") val orderId: String,
    val amount: String,
    val currency: String,
    val method: String,
    val status: String,
    @SerialName("created_at") val createdAt: String? = null
)

@Serializable
data class SalesSummary(
    @SerialName("total_revenue") val totalRevenue: String,
    @SerialName("order_count") val orderCount: Int,
    @SerialName("average_order_value") val averageOrderValue: String
)

@Serializable
data class TopProduct(
    @SerialName("product_id") val productId: String,
    @SerialName("product_name") val productName: String,
    @SerialName("total_quantity") val totalQuantity: Int,
    @SerialName("total_revenue") val totalRevenue: String
)

@Serializable
data class TopCustomer(
    @SerialName("customer_id") val customerId: String,
    @SerialName("customer_name") val customerName: String,
    @SerialName("order_count") val orderCount: Int,
    @SerialName("total_spent") val totalSpent: String
)

// Enums
enum class OrderStatus {
    Pending,
    Confirmed,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded
}

enum class ReturnReason {
    Defective,
    WrongItem,
    NotAsDescribed,
    ChangedMind,
    ArrivedLate,
    Other
}

enum class PaymentMethod {
    CreditCard,
    DebitCard,
    BankTransfer,
    PayPal,
    Stripe,
    Crypto,
    Cash,
    Other
}

enum class TimePeriod {
    Today,
    ThisWeek,
    ThisMonth,
    ThisQuarter,
    ThisYear,
    AllTime
}
