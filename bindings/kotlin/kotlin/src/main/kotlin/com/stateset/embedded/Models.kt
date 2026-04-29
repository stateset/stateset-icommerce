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
    @SerialName("payment_method") val method: String,
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

// =============================================================================
// Shipment Models
// =============================================================================

@Serializable
data class Shipment(
    val id: String,
    @SerialName("shipment_number") val shipmentNumber: String,
    @SerialName("order_id") val orderId: String,
    val status: String,
    val carrier: String? = null,
    @SerialName("tracking_number") val trackingNumber: String? = null,
    @SerialName("tracking_url") val trackingUrl: String? = null,
    @SerialName("recipient_name") val recipientName: String,
    @SerialName("recipient_email") val recipientEmail: String? = null,
    @SerialName("shipping_address") val shippingAddress: String,
    @SerialName("shipped_at") val shippedAt: String? = null,
    @SerialName("delivered_at") val deliveredAt: String? = null,
    @SerialName("estimated_delivery") val estimatedDelivery: String? = null,
    val weight: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

enum class ShipmentStatus {
    Pending,
    Processing,
    Ready,
    Shipped,
    InTransit,
    OutForDelivery,
    Delivered,
    Failed,
    Cancelled
}

enum class ShippingCarrier {
    UPS,
    FedEx,
    USPS,
    DHL,
    Other
}

// =============================================================================
// Warranty Models
// =============================================================================

@Serializable
data class Warranty(
    val id: String,
    @SerialName("warranty_number") val warrantyNumber: String,
    @SerialName("customer_id") val customerId: String,
    @SerialName("product_id") val productId: String? = null,
    @SerialName("order_id") val orderId: String? = null,
    @SerialName("order_item_id") val orderItemId: String? = null,
    @SerialName("serial_number") val serialNumber: String? = null,
    val status: String,
    @SerialName("warranty_type") val warrantyType: String,
    @SerialName("duration_months") val durationMonths: Int,
    @SerialName("coverage_description") val coverageDescription: String? = null,
    @SerialName("start_date") val startDate: String,
    @SerialName("end_date") val endDate: String,
    @SerialName("purchase_date") val purchaseDate: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class WarrantyClaim(
    val id: String,
    @SerialName("claim_number") val claimNumber: String,
    @SerialName("warranty_id") val warrantyId: String,
    val status: String,
    @SerialName("issue_description") val issueDescription: String,
    val resolution: String? = null,
    @SerialName("resolution_notes") val resolutionNotes: String? = null,
    @SerialName("contact_email") val contactEmail: String? = null,
    @SerialName("contact_phone") val contactPhone: String? = null,
    @SerialName("denial_reason") val denialReason: String? = null,
    @SerialName("resolved_at") val resolvedAt: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

enum class WarrantyType {
    Standard,
    Extended,
    Limited,
    Lifetime
}

enum class WarrantyStatus {
    Active,
    Expired,
    Voided
}

enum class ClaimStatus {
    Pending,
    Approved,
    Denied,
    Completed,
    Cancelled
}

enum class ClaimResolution {
    Repair,
    Replacement,
    Refund,
    StoreCredit
}

// =============================================================================
// Supplier Models
// =============================================================================

@Serializable
data class Supplier(
    val id: String,
    @SerialName("supplier_code") val supplierCode: String? = null,
    val name: String,
    val email: String? = null,
    val phone: String? = null,
    val address: String? = null,
    @SerialName("contact_name") val contactName: String? = null,
    @SerialName("payment_terms") val paymentTerms: String? = null,
    @SerialName("lead_time_days") val leadTimeDays: Int? = null,
    @SerialName("is_active") val isActive: Boolean = true,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

// =============================================================================
// Purchase Order Models
// =============================================================================

@Serializable
data class PurchaseOrder(
    val id: String,
    @SerialName("po_number") val poNumber: String,
    @SerialName("supplier_id") val supplierId: String,
    val status: String,
    val subtotal: String,
    @SerialName("tax_amount") val taxAmount: String,
    @SerialName("shipping_cost") val shippingCost: String,
    val total: String,
    val currency: String,
    @SerialName("ship_to_address") val shipToAddress: String? = null,
    @SerialName("expected_date") val expectedDate: String? = null,
    @SerialName("received_date") val receivedDate: String? = null,
    @SerialName("approved_by") val approvedBy: String? = null,
    @SerialName("approved_at") val approvedAt: String? = null,
    @SerialName("supplier_reference") val supplierReference: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class PurchaseOrderItem(
    val sku: String,
    val name: String,
    val quantity: Double,
    @SerialName("unit_cost") val unitCost: Double
)

enum class PurchaseOrderStatus {
    Draft,
    PendingApproval,
    Approved,
    Sent,
    Acknowledged,
    PartiallyReceived,
    Received,
    Completed,
    Cancelled,
    OnHold
}

// =============================================================================
// Invoice Models
// =============================================================================

@Serializable
data class Invoice(
    val id: String,
    @SerialName("invoice_number") val invoiceNumber: String,
    @SerialName("customer_id") val customerId: String,
    @SerialName("order_id") val orderId: String? = null,
    val status: String,
    @SerialName("invoice_type") val invoiceType: String,
    val subtotal: String,
    @SerialName("tax_amount") val taxAmount: String,
    val total: String,
    @SerialName("amount_paid") val amountPaid: String,
    val currency: String,
    @SerialName("billing_email") val billingEmail: String? = null,
    @SerialName("billing_name") val billingName: String? = null,
    @SerialName("billing_address") val billingAddress: String? = null,
    @SerialName("due_date") val dueDate: String? = null,
    @SerialName("sent_at") val sentAt: String? = null,
    @SerialName("viewed_at") val viewedAt: String? = null,
    @SerialName("paid_at") val paidAt: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class InvoiceItem(
    val description: String,
    val quantity: Double,
    @SerialName("unit_price") val unitPrice: Double,
    val sku: String? = null
)

enum class InvoiceStatus {
    Draft,
    Sent,
    Viewed,
    PartiallyPaid,
    Paid,
    Overdue,
    Voided,
    WrittenOff,
    Disputed
}

// =============================================================================
// Bill of Materials Models
// =============================================================================

@Serializable
data class BillOfMaterials(
    val id: String,
    @SerialName("bom_number") val bomNumber: String,
    @SerialName("product_id") val productId: String,
    val name: String,
    val description: String? = null,
    val version: String,
    val status: String,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

@Serializable
data class BOMComponent(
    val id: String,
    @SerialName("bom_id") val bomId: String,
    @SerialName("component_sku") val componentSku: String? = null,
    val name: String,
    val description: String? = null,
    val quantity: String,
    @SerialName("unit_of_measure") val unitOfMeasure: String? = null,
    val position: String? = null,
    @SerialName("is_optional") val isOptional: Boolean = false,
    val notes: String? = null
)

enum class BOMStatus {
    Draft,
    Active,
    Obsolete
}

// =============================================================================
// Work Order Models
// =============================================================================

@Serializable
data class WorkOrder(
    val id: String,
    @SerialName("work_order_number") val workOrderNumber: String,
    @SerialName("product_id") val productId: String,
    @SerialName("bom_id") val bomId: String? = null,
    val status: String,
    val priority: String,
    @SerialName("quantity_to_build") val quantityToBuild: String,
    @SerialName("quantity_completed") val quantityCompleted: String,
    @SerialName("planned_start") val plannedStart: String? = null,
    @SerialName("planned_end") val plannedEnd: String? = null,
    @SerialName("actual_start") val actualStart: String? = null,
    @SerialName("actual_end") val actualEnd: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
    @SerialName("updated_at") val updatedAt: String? = null
)

enum class WorkOrderStatus {
    Planned,
    InProgress,
    OnHold,
    Completed,
    PartiallyCompleted,
    Cancelled
}

enum class WorkOrderPriority {
    Low,
    Normal,
    High,
    Urgent
}

// =============================================================================
// Currency Models
// =============================================================================

@Serializable
data class ExchangeRate(
    val id: String,
    @SerialName("base_currency") val baseCurrency: String,
    @SerialName("quote_currency") val quoteCurrency: String,
    val rate: String,
    val source: String? = null,
    @SerialName("valid_from") val validFrom: String,
    @SerialName("valid_to") val validTo: String? = null,
    @SerialName("created_at") val createdAt: String? = null
)

@Serializable
data class ConversionResult(
    @SerialName("from_currency") val fromCurrency: String,
    @SerialName("to_currency") val toCurrency: String,
    @SerialName("original_amount") val originalAmount: String,
    @SerialName("converted_amount") val convertedAmount: String,
    val rate: String,
    @SerialName("rate_at") val rateAt: String
)

@Serializable
data class StoreCurrencySettings(
    @SerialName("base_currency") val baseCurrency: String,
    @SerialName("enabled_currencies") val enabledCurrencies: List<String>,
    @SerialName("auto_convert") val autoConvert: Boolean,
    @SerialName("rounding_mode") val roundingMode: String
)

enum class Currency {
    USD,
    EUR,
    GBP,
    JPY,
    CAD,
    AUD,
    CHF,
    CNY
}

// =============================================================================
// Refund Models
// =============================================================================

@Serializable
data class Refund(
    val id: String,
    @SerialName("refund_number") val refundNumber: String,
    @SerialName("payment_id") val paymentId: String,
    val amount: String,
    val currency: String,
    val status: String,
    val reason: String? = null,
    @SerialName("external_id") val externalId: String? = null,
    @SerialName("failure_reason") val failureReason: String? = null,
    @SerialName("refunded_at") val refundedAt: String? = null,
    @SerialName("created_at") val createdAt: String? = null
)

enum class RefundStatus {
    Pending,
    Completed,
    Failed
}

// =============================================================================
// Return Status
// =============================================================================

enum class ReturnStatus {
    Requested,
    Approved,
    Rejected,
    InTransit,
    Received,
    Completed,
    Cancelled
}
