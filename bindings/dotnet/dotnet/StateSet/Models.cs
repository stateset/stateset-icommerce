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
