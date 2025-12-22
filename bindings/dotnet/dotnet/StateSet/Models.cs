using System.Text.Json.Serialization;

namespace StateSet.Embedded;

/// <summary>
/// Represents a customer in the commerce system
/// </summary>
public record Customer
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("email")]
    public string Email { get; init; } = string.Empty;

    [JsonPropertyName("first_name")]
    public string FirstName { get; init; } = string.Empty;

    [JsonPropertyName("last_name")]
    public string LastName { get; init; } = string.Empty;

    [JsonPropertyName("phone")]
    public string? Phone { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents a product in the catalog
/// </summary>
public record Product
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("slug")]
    public string? Slug { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("is_active")]
    public bool IsActive { get; init; } = true;

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents a product variant (SKU)
/// </summary>
public record ProductVariant
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("product_id")]
    public string ProductId { get; init; } = string.Empty;

    [JsonPropertyName("sku")]
    public string Sku { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("price")]
    public string Price { get; init; } = "0";

    [JsonPropertyName("compare_at_price")]
    public string? CompareAtPrice { get; init; }

    [JsonPropertyName("is_default")]
    public bool IsDefault { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Represents an order
/// </summary>
public record Order
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("order_number")]
    public string OrderNumber { get; init; } = string.Empty;

    [JsonPropertyName("customer_id")]
    public string CustomerId { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("total_amount")]
    public string TotalAmount { get; init; } = "0";

    [JsonPropertyName("currency")]
    public string Currency { get; init; } = "USD";

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents an order item for order creation
/// </summary>
public record OrderItem
{
    [JsonPropertyName("sku")]
    public string Sku { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("quantity")]
    public int Quantity { get; init; }

    [JsonPropertyName("unit_price")]
    public double UnitPrice { get; init; }

    [JsonPropertyName("product_id")]
    public string ProductId { get; init; } = "00000000-0000-0000-0000-000000000000";
}

/// <summary>
/// Represents an inventory item
/// </summary>
public record InventoryItem
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("sku")]
    public string Sku { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("unit_of_measure")]
    public string? UnitOfMeasure { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Represents a stock level for an inventory item
/// </summary>
public record StockLevel
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("inventory_item_id")]
    public string InventoryItemId { get; init; } = string.Empty;

    [JsonPropertyName("location_id")]
    public string? LocationId { get; init; }

    [JsonPropertyName("available")]
    public string Available { get; init; } = "0";

    [JsonPropertyName("reserved")]
    public string Reserved { get; init; } = "0";

    [JsonPropertyName("incoming")]
    public string? Incoming { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents a shopping cart
/// </summary>
public record Cart
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("customer_id")]
    public string? CustomerId { get; init; }

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("grand_total")]
    public string GrandTotal { get; init; } = "0";

    [JsonPropertyName("currency")]
    public string Currency { get; init; } = "USD";

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Represents a return request
/// </summary>
public record Return
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("order_id")]
    public string OrderId { get; init; } = string.Empty;

    [JsonPropertyName("reason")]
    public string Reason { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("refund_amount")]
    public string? RefundAmount { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Represents a payment
/// </summary>
public record Payment
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("order_id")]
    public string OrderId { get; init; } = string.Empty;

    [JsonPropertyName("amount")]
    public string Amount { get; init; } = "0";

    [JsonPropertyName("currency")]
    public string Currency { get; init; } = "USD";

    [JsonPropertyName("method")]
    public string Method { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Represents a sales summary from analytics
/// </summary>
public record SalesSummary
{
    [JsonPropertyName("total_revenue")]
    public string TotalRevenue { get; init; } = "0";

    [JsonPropertyName("order_count")]
    public int OrderCount { get; init; }

    [JsonPropertyName("average_order_value")]
    public string AverageOrderValue { get; init; } = "0";
}

/// <summary>
/// Represents a top-selling product from analytics
/// </summary>
public record TopProduct
{
    [JsonPropertyName("product_id")]
    public string ProductId { get; init; } = string.Empty;

    [JsonPropertyName("product_name")]
    public string ProductName { get; init; } = string.Empty;

    [JsonPropertyName("total_quantity")]
    public int TotalQuantity { get; init; }

    [JsonPropertyName("total_revenue")]
    public string TotalRevenue { get; init; } = "0";
}

/// <summary>
/// Represents a top customer from analytics
/// </summary>
public record TopCustomer
{
    [JsonPropertyName("customer_id")]
    public string CustomerId { get; init; } = string.Empty;

    [JsonPropertyName("customer_name")]
    public string CustomerName { get; init; } = string.Empty;

    [JsonPropertyName("order_count")]
    public int OrderCount { get; init; }

    [JsonPropertyName("total_spent")]
    public string TotalSpent { get; init; } = "0";
}

// =============================================================================
// Enums
// =============================================================================

/// <summary>
/// Order status values
/// </summary>
public enum OrderStatus
{
    Pending,
    Confirmed,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded
}

/// <summary>
/// Return reason values
/// </summary>
public enum ReturnReason
{
    Defective,
    WrongItem,
    NotAsDescribed,
    ChangedMind,
    Damaged,
    Other
}

/// <summary>
/// Payment method values
/// </summary>
public enum PaymentMethod
{
    CreditCard,
    DebitCard,
    BankTransfer,
    PayPal,
    ApplePay,
    GooglePay,
    Crypto,
    Other
}

/// <summary>
/// Analytics time period values
/// </summary>
public enum TimePeriod
{
    Today,
    Week,
    Month,
    Quarter,
    Year,
    AllTime
}

// =============================================================================
// Shipment Models
// =============================================================================

/// <summary>
/// Represents a shipment
/// </summary>
public record Shipment
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("shipment_number")]
    public string ShipmentNumber { get; init; } = string.Empty;

    [JsonPropertyName("order_id")]
    public string OrderId { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("carrier")]
    public string? Carrier { get; init; }

    [JsonPropertyName("tracking_number")]
    public string? TrackingNumber { get; init; }

    [JsonPropertyName("tracking_url")]
    public string? TrackingUrl { get; init; }

    [JsonPropertyName("recipient_name")]
    public string RecipientName { get; init; } = string.Empty;

    [JsonPropertyName("recipient_email")]
    public string? RecipientEmail { get; init; }

    [JsonPropertyName("shipping_address")]
    public string ShippingAddress { get; init; } = string.Empty;

    [JsonPropertyName("shipped_at")]
    public string? ShippedAt { get; init; }

    [JsonPropertyName("delivered_at")]
    public string? DeliveredAt { get; init; }

    [JsonPropertyName("estimated_delivery")]
    public string? EstimatedDelivery { get; init; }

    [JsonPropertyName("weight")]
    public string? Weight { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Shipment status values
/// </summary>
public enum ShipmentStatus
{
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

/// <summary>
/// Shipping carrier values
/// </summary>
public enum ShippingCarrier
{
    UPS,
    FedEx,
    USPS,
    DHL,
    Other
}

// =============================================================================
// Warranty Models
// =============================================================================

/// <summary>
/// Represents a warranty
/// </summary>
public record Warranty
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("warranty_number")]
    public string WarrantyNumber { get; init; } = string.Empty;

    [JsonPropertyName("customer_id")]
    public string CustomerId { get; init; } = string.Empty;

    [JsonPropertyName("product_id")]
    public string? ProductId { get; init; }

    [JsonPropertyName("order_id")]
    public string? OrderId { get; init; }

    [JsonPropertyName("order_item_id")]
    public string? OrderItemId { get; init; }

    [JsonPropertyName("serial_number")]
    public string? SerialNumber { get; init; }

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("warranty_type")]
    public string WarrantyType { get; init; } = string.Empty;

    [JsonPropertyName("duration_months")]
    public int DurationMonths { get; init; }

    [JsonPropertyName("coverage_description")]
    public string? CoverageDescription { get; init; }

    [JsonPropertyName("start_date")]
    public string StartDate { get; init; } = string.Empty;

    [JsonPropertyName("end_date")]
    public string EndDate { get; init; } = string.Empty;

    [JsonPropertyName("purchase_date")]
    public string? PurchaseDate { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents a warranty claim
/// </summary>
public record WarrantyClaim
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("claim_number")]
    public string ClaimNumber { get; init; } = string.Empty;

    [JsonPropertyName("warranty_id")]
    public string WarrantyId { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("issue_description")]
    public string IssueDescription { get; init; } = string.Empty;

    [JsonPropertyName("resolution")]
    public string? Resolution { get; init; }

    [JsonPropertyName("resolution_notes")]
    public string? ResolutionNotes { get; init; }

    [JsonPropertyName("contact_email")]
    public string? ContactEmail { get; init; }

    [JsonPropertyName("contact_phone")]
    public string? ContactPhone { get; init; }

    [JsonPropertyName("denial_reason")]
    public string? DenialReason { get; init; }

    [JsonPropertyName("resolved_at")]
    public string? ResolvedAt { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Warranty type values
/// </summary>
public enum WarrantyType
{
    Standard,
    Extended,
    Limited,
    Lifetime
}

/// <summary>
/// Warranty status values
/// </summary>
public enum WarrantyStatus
{
    Active,
    Expired,
    Voided
}

/// <summary>
/// Claim status values
/// </summary>
public enum ClaimStatus
{
    Pending,
    Approved,
    Denied,
    Completed,
    Cancelled
}

/// <summary>
/// Claim resolution values
/// </summary>
public enum ClaimResolution
{
    Repair,
    Replacement,
    Refund,
    StoreCredit
}

// =============================================================================
// Supplier Models
// =============================================================================

/// <summary>
/// Represents a supplier
/// </summary>
public record Supplier
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("supplier_code")]
    public string? SupplierCode { get; init; }

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("email")]
    public string? Email { get; init; }

    [JsonPropertyName("phone")]
    public string? Phone { get; init; }

    [JsonPropertyName("address")]
    public string? Address { get; init; }

    [JsonPropertyName("contact_name")]
    public string? ContactName { get; init; }

    [JsonPropertyName("payment_terms")]
    public string? PaymentTerms { get; init; }

    [JsonPropertyName("lead_time_days")]
    public int? LeadTimeDays { get; init; }

    [JsonPropertyName("is_active")]
    public bool IsActive { get; init; } = true;

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

// =============================================================================
// Purchase Order Models
// =============================================================================

/// <summary>
/// Represents a purchase order
/// </summary>
public record PurchaseOrder
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("po_number")]
    public string PoNumber { get; init; } = string.Empty;

    [JsonPropertyName("supplier_id")]
    public string SupplierId { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("subtotal")]
    public string Subtotal { get; init; } = "0";

    [JsonPropertyName("tax_amount")]
    public string TaxAmount { get; init; } = "0";

    [JsonPropertyName("shipping_cost")]
    public string ShippingCost { get; init; } = "0";

    [JsonPropertyName("total")]
    public string Total { get; init; } = "0";

    [JsonPropertyName("currency")]
    public string Currency { get; init; } = "USD";

    [JsonPropertyName("ship_to_address")]
    public string? ShipToAddress { get; init; }

    [JsonPropertyName("expected_date")]
    public string? ExpectedDate { get; init; }

    [JsonPropertyName("received_date")]
    public string? ReceivedDate { get; init; }

    [JsonPropertyName("approved_by")]
    public string? ApprovedBy { get; init; }

    [JsonPropertyName("approved_at")]
    public string? ApprovedAt { get; init; }

    [JsonPropertyName("supplier_reference")]
    public string? SupplierReference { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents a purchase order item
/// </summary>
public record PurchaseOrderItem
{
    [JsonPropertyName("sku")]
    public string Sku { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("quantity")]
    public double Quantity { get; init; }

    [JsonPropertyName("unit_cost")]
    public double UnitCost { get; init; }
}

/// <summary>
/// Purchase order status values
/// </summary>
public enum PurchaseOrderStatus
{
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

/// <summary>
/// Represents an invoice
/// </summary>
public record Invoice
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("invoice_number")]
    public string InvoiceNumber { get; init; } = string.Empty;

    [JsonPropertyName("customer_id")]
    public string CustomerId { get; init; } = string.Empty;

    [JsonPropertyName("order_id")]
    public string? OrderId { get; init; }

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("invoice_type")]
    public string InvoiceType { get; init; } = string.Empty;

    [JsonPropertyName("subtotal")]
    public string Subtotal { get; init; } = "0";

    [JsonPropertyName("tax_amount")]
    public string TaxAmount { get; init; } = "0";

    [JsonPropertyName("total")]
    public string Total { get; init; } = "0";

    [JsonPropertyName("amount_paid")]
    public string AmountPaid { get; init; } = "0";

    [JsonPropertyName("currency")]
    public string Currency { get; init; } = "USD";

    [JsonPropertyName("billing_email")]
    public string? BillingEmail { get; init; }

    [JsonPropertyName("billing_name")]
    public string? BillingName { get; init; }

    [JsonPropertyName("billing_address")]
    public string? BillingAddress { get; init; }

    [JsonPropertyName("due_date")]
    public string? DueDate { get; init; }

    [JsonPropertyName("sent_at")]
    public string? SentAt { get; init; }

    [JsonPropertyName("viewed_at")]
    public string? ViewedAt { get; init; }

    [JsonPropertyName("paid_at")]
    public string? PaidAt { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents an invoice item
/// </summary>
public record InvoiceItem
{
    [JsonPropertyName("description")]
    public string Description { get; init; } = string.Empty;

    [JsonPropertyName("quantity")]
    public double Quantity { get; init; }

    [JsonPropertyName("unit_price")]
    public double UnitPrice { get; init; }

    [JsonPropertyName("sku")]
    public string? Sku { get; init; }
}

/// <summary>
/// Invoice status values
/// </summary>
public enum InvoiceStatus
{
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

/// <summary>
/// Represents a bill of materials
/// </summary>
public record BillOfMaterials
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("bom_number")]
    public string BomNumber { get; init; } = string.Empty;

    [JsonPropertyName("product_id")]
    public string ProductId { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("version")]
    public string Version { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Represents a BOM component
/// </summary>
public record BomComponent
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("bom_id")]
    public string BomId { get; init; } = string.Empty;

    [JsonPropertyName("component_sku")]
    public string? ComponentSku { get; init; }

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("quantity")]
    public string Quantity { get; init; } = "0";

    [JsonPropertyName("unit_of_measure")]
    public string? UnitOfMeasure { get; init; }

    [JsonPropertyName("position")]
    public string? Position { get; init; }

    [JsonPropertyName("is_optional")]
    public bool IsOptional { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }
}

/// <summary>
/// BOM status values
/// </summary>
public enum BomStatus
{
    Draft,
    Active,
    Obsolete
}

// =============================================================================
// Work Order Models
// =============================================================================

/// <summary>
/// Represents a work order
/// </summary>
public record WorkOrder
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("work_order_number")]
    public string WorkOrderNumber { get; init; } = string.Empty;

    [JsonPropertyName("product_id")]
    public string ProductId { get; init; } = string.Empty;

    [JsonPropertyName("bom_id")]
    public string? BomId { get; init; }

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("priority")]
    public string Priority { get; init; } = string.Empty;

    [JsonPropertyName("quantity_to_build")]
    public string QuantityToBuild { get; init; } = "0";

    [JsonPropertyName("quantity_completed")]
    public string QuantityCompleted { get; init; } = "0";

    [JsonPropertyName("planned_start")]
    public string? PlannedStart { get; init; }

    [JsonPropertyName("planned_end")]
    public string? PlannedEnd { get; init; }

    [JsonPropertyName("actual_start")]
    public string? ActualStart { get; init; }

    [JsonPropertyName("actual_end")]
    public string? ActualEnd { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("updated_at")]
    public string? UpdatedAt { get; init; }
}

/// <summary>
/// Work order status values
/// </summary>
public enum WorkOrderStatus
{
    Planned,
    InProgress,
    OnHold,
    Completed,
    PartiallyCompleted,
    Cancelled
}

/// <summary>
/// Work order priority values
/// </summary>
public enum WorkOrderPriority
{
    Low,
    Normal,
    High,
    Urgent
}

// =============================================================================
// Currency Models
// =============================================================================

/// <summary>
/// Represents an exchange rate
/// </summary>
public record ExchangeRate
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("base_currency")]
    public string BaseCurrency { get; init; } = string.Empty;

    [JsonPropertyName("quote_currency")]
    public string QuoteCurrency { get; init; } = string.Empty;

    [JsonPropertyName("rate")]
    public string Rate { get; init; } = "0";

    [JsonPropertyName("source")]
    public string? Source { get; init; }

    [JsonPropertyName("valid_from")]
    public string ValidFrom { get; init; } = string.Empty;

    [JsonPropertyName("valid_to")]
    public string? ValidTo { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Represents a currency conversion result
/// </summary>
public record ConversionResult
{
    [JsonPropertyName("from_currency")]
    public string FromCurrency { get; init; } = string.Empty;

    [JsonPropertyName("to_currency")]
    public string ToCurrency { get; init; } = string.Empty;

    [JsonPropertyName("original_amount")]
    public string OriginalAmount { get; init; } = "0";

    [JsonPropertyName("converted_amount")]
    public string ConvertedAmount { get; init; } = "0";

    [JsonPropertyName("rate")]
    public string Rate { get; init; } = "0";

    [JsonPropertyName("rate_at")]
    public string RateAt { get; init; } = string.Empty;
}

/// <summary>
/// Represents store currency settings
/// </summary>
public record StoreCurrencySettings
{
    [JsonPropertyName("base_currency")]
    public string BaseCurrency { get; init; } = string.Empty;

    [JsonPropertyName("enabled_currencies")]
    public List<string> EnabledCurrencies { get; init; } = new();

    [JsonPropertyName("auto_convert")]
    public bool AutoConvert { get; init; }

    [JsonPropertyName("rounding_mode")]
    public string RoundingMode { get; init; } = string.Empty;
}

/// <summary>
/// Currency codes
/// </summary>
public enum CurrencyCode
{
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

/// <summary>
/// Represents a refund
/// </summary>
public record Refund
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("refund_number")]
    public string RefundNumber { get; init; } = string.Empty;

    [JsonPropertyName("payment_id")]
    public string PaymentId { get; init; } = string.Empty;

    [JsonPropertyName("amount")]
    public string Amount { get; init; } = "0";

    [JsonPropertyName("currency")]
    public string Currency { get; init; } = "USD";

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("reason")]
    public string? Reason { get; init; }

    [JsonPropertyName("external_id")]
    public string? ExternalId { get; init; }

    [JsonPropertyName("failure_reason")]
    public string? FailureReason { get; init; }

    [JsonPropertyName("refunded_at")]
    public string? RefundedAt { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }
}

/// <summary>
/// Refund status values
/// </summary>
public enum RefundStatus
{
    Pending,
    Completed,
    Failed
}

/// <summary>
/// Return status values
/// </summary>
public enum ReturnStatus
{
    Requested,
    Approved,
    Rejected,
    InTransit,
    Received,
    Completed,
    Cancelled
}
