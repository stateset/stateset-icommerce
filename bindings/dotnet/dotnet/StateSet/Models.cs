using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace StateSet.Embedded;

// ============================================================================
// These records mirror the JSON the Rust engine (stateset-embedded) emits
// through stateset-ffi's JSON call surface. Property names map to the engine's
// snake_case fields; every money/quantity field is `decimal` and crosses the
// boundary as an exact decimal string, never a float.
// ============================================================================

#region Enums

/// <summary>Customer lifecycle status.</summary>
public enum CustomerStatus { Active, Inactive, Suspended, Deleted }

/// <summary>Product lifecycle status.</summary>
public enum ProductStatus { Draft, Active, Archived }

/// <summary>Product type.</summary>
public enum ProductType { Simple, Variable, Bundle, Digital }

/// <summary>Order status.</summary>
public enum OrderStatus { Pending, Confirmed, Processing, PartiallyShipped, Shipped, Delivered, Cancelled, Refunded }

/// <summary>Payment status of an order.</summary>
public enum OrderPaymentStatus { Pending, Authorized, Paid, PartiallyPaid, Refunded, PartiallyRefunded, Failed }

/// <summary>Fulfillment status of an order.</summary>
public enum FulfillmentStatus { Unfulfilled, PartiallyFulfilled, Fulfilled, Shipped, Delivered }

/// <summary>Cart status.</summary>
public enum CartStatus { Active, ReadyForPayment, PaymentPending, Completed, Abandoned, Cancelled, Expired }

/// <summary>Payment status of a cart.</summary>
public enum CartPaymentStatus { None, MethodSelected, Authorized, Captured, Failed, Refunded }

/// <summary>Payment method type.</summary>
public enum PaymentMethod
{
    CreditCard, DebitCard, BankTransfer, PayPal, ApplePay, GooglePay, Crypto, Stablecoin,
    StoreCredit, GiftCard, CashOnDelivery, Invoice, Other,
}

/// <summary>Status of a payment transaction.</summary>
public enum PaymentStatus
{
    Pending, Processing, RequiresAction, Completed, Failed, Cancelled, Refunded, PartiallyRefunded, Disputed,
}

/// <summary>Refund status.</summary>
public enum RefundStatus { Pending, Processing, Completed, Failed, Cancelled }

/// <summary>Reason for a return.</summary>
public enum ReturnReason { Defective, WrongItem, NotAsDescribed, ChangedMind, BetterPriceFound, NoLongerNeeded, Damaged, Other }

/// <summary>Return status.</summary>
public enum ReturnStatus { Requested, Approved, Rejected, InTransit, Received, Inspecting, Completed, Cancelled }

/// <summary>What happens to a returned item.</summary>
public enum ReturnDisposition { Restock, Refurbish, Scrap, ReturnToVendor, Quarantine }

/// <summary>Condition of a returned item.</summary>
public enum ItemCondition { New, Opened, Used, Damaged, Defective }

/// <summary>Shipment status.</summary>
public enum ShipmentStatus
{
    Pending, Processing, ReadyToShip, Shipped, InTransit, OutForDelivery, Delivered, Failed, Returned, Cancelled, OnHold,
}

/// <summary>Shipping carrier.</summary>
public enum ShippingCarrier { Other, Ups, FedEx, Usps, Dhl, OnTrac, LaserShip }

/// <summary>Shipping method / service level.</summary>
public enum ShippingMethod { Standard, Express, Overnight, TwoDay, Ground, International, SameDay, Freight }

/// <summary>Inventory reservation status.</summary>
public enum ReservationStatus { Pending, Confirmed, Allocated, Cancelled, Released, Expired, Fulfilled }

#endregion

#region Customers

/// <summary>A customer record.</summary>
public sealed record Customer
{
    public string Id { get; init; } = "";
    public string Email { get; init; } = "";
    public string FirstName { get; init; } = "";
    public string LastName { get; init; } = "";
    public string? Phone { get; init; }
    public CustomerStatus Status { get; init; }
    public bool AcceptsMarketing { get; init; }
    public bool EmailVerified { get; init; }
    public List<string> Tags { get; init; } = new();
    public JsonNode? Metadata { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }

    /// <summary>"First Last".</summary>
    [JsonIgnore]
    public string FullName => $"{FirstName} {LastName}".Trim();
}

/// <summary>Fields to change on a customer; null leaves a field unchanged.</summary>
public sealed record UpdateCustomer
{
    public string? Email { get; init; }
    public string? FirstName { get; init; }
    public string? LastName { get; init; }
    public string? Phone { get; init; }
    public CustomerStatus? Status { get; init; }
    public bool? AcceptsMarketing { get; init; }
    public List<string>? Tags { get; init; }
}

#endregion

#region Products

/// <summary>A product (catalog entry). Prices live on its variants.</summary>
public sealed record Product
{
    public string Id { get; init; } = "";
    public string Name { get; init; } = "";
    public string Slug { get; init; } = "";
    public string Description { get; init; } = "";
    public ProductStatus Status { get; init; }
    public ProductType ProductType { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>A sellable variant of a product (SKU + price).</summary>
public sealed record ProductVariant
{
    public string Id { get; init; } = "";
    public string ProductId { get; init; } = "";
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public decimal Price { get; init; }
    public decimal? CompareAtPrice { get; init; }
    public decimal? Cost { get; init; }
    public string? Barcode { get; init; }
    public decimal? Weight { get; init; }
    public string? WeightUnit { get; init; }
    public bool IsDefault { get; init; }
    public bool IsActive { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>Input for a product variant.</summary>
public sealed record CreateProductVariant
{
    public string Sku { get; init; } = "";
    public string? Name { get; init; }
    public decimal Price { get; init; }
    public decimal? CompareAtPrice { get; init; }
    public decimal? Cost { get; init; }
    public string? Barcode { get; init; }
    public decimal? Weight { get; init; }
    public string? WeightUnit { get; init; }
    public bool? IsDefault { get; init; }
}

/// <summary>Fields to change on a product; null leaves a field unchanged.</summary>
public sealed record UpdateProduct
{
    public string? Name { get; init; }
    public string? Slug { get; init; }
    public string? Description { get; init; }
    public ProductStatus? Status { get; init; }
}

#endregion

#region Inventory

/// <summary>An inventory item (one per SKU).</summary>
public sealed record InventoryItem
{
    public long Id { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public string? Description { get; init; }
    public string UnitOfMeasure { get; init; } = "";
    public bool IsActive { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>Aggregated stock for a SKU across locations.</summary>
public sealed record StockLevel
{
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public decimal TotalOnHand { get; init; }
    public decimal TotalAllocated { get; init; }
    public decimal TotalAvailable { get; init; }
    public List<LocationStock> Locations { get; init; } = new();
}

/// <summary>Stock at one location.</summary>
public sealed record LocationStock
{
    public int LocationId { get; init; }
    public string? LocationName { get; init; }
    public decimal OnHand { get; init; }
    public decimal Allocated { get; init; }
    public decimal Available { get; init; }
}

/// <summary>A recorded inventory movement.</summary>
public sealed record InventoryTransaction
{
    public long Id { get; init; }
    public long ItemId { get; init; }
    public int LocationId { get; init; }
    public string TransactionType { get; init; } = "";
    public decimal Quantity { get; init; }
    public string? ReferenceType { get; init; }
    public string? ReferenceId { get; init; }
    public string? Reason { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
}

/// <summary>A hold on stock for a reference (cart, order, ...).</summary>
public sealed record InventoryReservation
{
    public string Id { get; init; } = "";
    public long ItemId { get; init; }
    public int LocationId { get; init; }
    public decimal Quantity { get; init; }
    public ReservationStatus Status { get; init; }
    public string ReferenceType { get; init; } = "";
    public string ReferenceId { get; init; } = "";
    public DateTimeOffset? ExpiresAt { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
}

#endregion

#region Carts

/// <summary>A postal address on a cart.</summary>
public sealed record CartAddress
{
    public string FirstName { get; init; } = "";
    public string LastName { get; init; } = "";
    public string? Company { get; init; }
    public string Line1 { get; init; } = "";
    public string? Line2 { get; init; }
    public string City { get; init; } = "";
    public string? State { get; init; }
    public string PostalCode { get; init; } = "";
    public string Country { get; init; } = "";
    public string? Phone { get; init; }
    public string? Email { get; init; }
}

/// <summary>A shopping cart.</summary>
public sealed record Cart
{
    public string Id { get; init; } = "";
    public string CartNumber { get; init; } = "";
    public string? CustomerId { get; init; }
    public CartStatus Status { get; init; }
    public string Currency { get; init; } = "";
    public List<CartItem> Items { get; init; } = new();
    public decimal Subtotal { get; init; }
    public decimal TaxAmount { get; init; }
    public decimal ShippingAmount { get; init; }
    public decimal DiscountAmount { get; init; }
    public decimal GrandTotal { get; init; }
    public string? CustomerEmail { get; init; }
    public string? CustomerName { get; init; }
    public CartAddress? ShippingAddress { get; init; }
    public CartAddress? BillingAddress { get; init; }
    public string? ShippingMethod { get; init; }
    public string? ShippingCarrier { get; init; }
    public string? PaymentMethod { get; init; }
    public CartPaymentStatus PaymentStatus { get; init; }
    public string? CouponCode { get; init; }
    public string? OrderId { get; init; }
    public string? OrderNumber { get; init; }
    public string? Notes { get; init; }
    public bool InventoryReserved { get; init; }
    public DateTimeOffset? ExpiresAt { get; init; }
    public DateTimeOffset? CompletedAt { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>A line in a cart.</summary>
public sealed record CartItem
{
    public string Id { get; init; } = "";
    public string CartId { get; init; } = "";
    public string? ProductId { get; init; }
    public string? VariantId { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public int Quantity { get; init; }
    public decimal UnitPrice { get; init; }
    public decimal? OriginalPrice { get; init; }
    public decimal DiscountAmount { get; init; }
    public decimal TaxAmount { get; init; }
    public decimal Total { get; init; }
    public bool RequiresShipping { get; init; }
}

/// <summary>Input for a cart line.</summary>
public sealed record AddCartItem
{
    public string? ProductId { get; init; }
    public string? VariantId { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public string? Description { get; init; }
    public int Quantity { get; init; } = 1;
    public decimal UnitPrice { get; init; }
    public decimal? OriginalPrice { get; init; }
    public bool? RequiresShipping { get; init; }
}

/// <summary>Outcome of completing checkout.</summary>
public sealed record CheckoutResult
{
    public string CartId { get; init; } = "";
    public string OrderId { get; init; } = "";
    public string OrderNumber { get; init; } = "";
    public string? PaymentId { get; init; }
    public decimal TotalCharged { get; init; }
    public string Currency { get; init; } = "";
}

#endregion

#region Orders

/// <summary>A postal address on an order.</summary>
public sealed record Address
{
    public string Line1 { get; init; } = "";
    public string? Line2 { get; init; }
    public string City { get; init; } = "";
    public string? State { get; init; }
    public string PostalCode { get; init; } = "";
    public string Country { get; init; } = "";
}

/// <summary>An order.</summary>
public sealed record Order
{
    public string Id { get; init; } = "";
    public string OrderNumber { get; init; } = "";
    public string CustomerId { get; init; } = "";
    public OrderStatus Status { get; init; }
    public DateTimeOffset OrderDate { get; init; }
    public decimal TotalAmount { get; init; }
    public decimal TaxAmount { get; init; }
    public decimal ShippingAmount { get; init; }
    public decimal DiscountAmount { get; init; }
    public string Currency { get; init; } = "";
    public OrderPaymentStatus PaymentStatus { get; init; }
    public FulfillmentStatus FulfillmentStatus { get; init; }
    public string? PaymentMethod { get; init; }
    public string? ShippingMethod { get; init; }
    public string? TrackingNumber { get; init; }
    public string? Notes { get; init; }
    public Address? ShippingAddress { get; init; }
    public Address? BillingAddress { get; init; }
    public List<OrderItem> Items { get; init; } = new();
    public int Version { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>A line on an order.</summary>
public sealed record OrderItem
{
    public string Id { get; init; } = "";
    public string OrderId { get; init; } = "";
    public string ProductId { get; init; } = "";
    public string? VariantId { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public int Quantity { get; init; }
    public int ShippedQuantity { get; init; }
    public decimal UnitPrice { get; init; }
    public decimal Discount { get; init; }
    public decimal TaxAmount { get; init; }
    public decimal Total { get; init; }
}

/// <summary>Input for an order line.</summary>
public sealed record CreateOrderItem
{
    public string ProductId { get; init; } = "";
    public string? VariantId { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public int Quantity { get; init; } = 1;
    public decimal UnitPrice { get; init; }
    public decimal? Discount { get; init; }
    public decimal? TaxAmount { get; init; }
}

#endregion

#region Payments

/// <summary>A payment.</summary>
public sealed record Payment
{
    public string Id { get; init; } = "";
    public string PaymentNumber { get; init; } = "";
    public string? OrderId { get; init; }
    public string? CustomerId { get; init; }
    public PaymentStatus Status { get; init; }
    public PaymentMethod PaymentMethod { get; init; }
    public decimal Amount { get; init; }
    public string Currency { get; init; } = "";
    public decimal AmountRefunded { get; init; }
    public string? ExternalId { get; init; }
    public string? Processor { get; init; }
    public string? Description { get; init; }
    public string? FailureReason { get; init; }
    public string? FailureCode { get; init; }
    public DateTimeOffset? PaidAt { get; init; }
    public int Version { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>A refund against a payment.</summary>
public sealed record Refund
{
    public string Id { get; init; } = "";
    public string RefundNumber { get; init; } = "";
    public string PaymentId { get; init; } = "";
    public RefundStatus Status { get; init; }
    public decimal Amount { get; init; }
    public string Currency { get; init; } = "";
    public string? Reason { get; init; }
    public string? FailureReason { get; init; }
    public DateTimeOffset? RefundedAt { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

#endregion

#region Returns

/// <summary>A return (RMA).</summary>
public sealed record Return
{
    public string Id { get; init; } = "";
    public string OrderId { get; init; } = "";
    public string CustomerId { get; init; } = "";
    public ReturnStatus Status { get; init; }
    public ReturnReason Reason { get; init; }
    public string? ReasonDetails { get; init; }
    public decimal? RefundAmount { get; init; }
    public string? RefundMethod { get; init; }
    public string? TrackingNumber { get; init; }
    public List<ReturnItem> Items { get; init; } = new();
    public string? Notes { get; init; }
    public int Version { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>A line on a return.</summary>
public sealed record ReturnItem
{
    public string Id { get; init; } = "";
    public string ReturnId { get; init; } = "";
    public string OrderItemId { get; init; } = "";
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public int Quantity { get; init; }
    public ItemCondition Condition { get; init; }
    public decimal RefundAmount { get; init; }
    public ReturnDisposition? Disposition { get; init; }
}

/// <summary>Input for a return line.</summary>
public sealed record CreateReturnItem
{
    public string OrderItemId { get; init; } = "";
    public int Quantity { get; init; } = 1;
    public ItemCondition? Condition { get; init; }
}

#endregion

#region Shipments

/// <summary>A shipment.</summary>
public sealed record Shipment
{
    public string Id { get; init; } = "";
    public string ShipmentNumber { get; init; } = "";
    public string OrderId { get; init; } = "";
    public ShipmentStatus Status { get; init; }
    public ShippingCarrier Carrier { get; init; }
    public ShippingMethod ShippingMethod { get; init; }
    public string? TrackingNumber { get; init; }
    public string? TrackingUrl { get; init; }
    public string RecipientName { get; init; } = "";
    public string? RecipientEmail { get; init; }
    public string? RecipientPhone { get; init; }
    public string ShippingAddress { get; init; } = "";
    public decimal? WeightKg { get; init; }
    public decimal? ShippingCost { get; init; }
    public bool SignatureRequired { get; init; }
    public DateTimeOffset? ShippedAt { get; init; }
    public DateTimeOffset? DeliveredAt { get; init; }
    public string? Notes { get; init; }
    public List<ShipmentItem> Items { get; init; } = new();
    public int Version { get; init; }
    public DateTimeOffset CreatedAt { get; init; }
    public DateTimeOffset UpdatedAt { get; init; }
}

/// <summary>A line in a shipment.</summary>
public sealed record ShipmentItem
{
    public string Id { get; init; } = "";
    public string ShipmentId { get; init; } = "";
    public string? OrderItemId { get; init; }
    public string? ProductId { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public int Quantity { get; init; }
}

/// <summary>Input for a shipment line.</summary>
public sealed record CreateShipmentItem
{
    public string? OrderItemId { get; init; }
    public string? ProductId { get; init; }
    public string Sku { get; init; } = "";
    public string Name { get; init; } = "";
    public int Quantity { get; init; } = 1;
}

#endregion

#region Errors

/// <summary>
/// Stable error codes reported by the native engine (the values of
/// <c>FfiErrorCode</c> in <c>crates/stateset-ffi</c>).
/// </summary>
public enum StateSetErrorCode
{
    Ok = 0,
    NotFound = 1,
    InvalidArgument = 2,
    InternalError = 3,
    DatabaseError = 4,
    SerializationError = 5,
    NullPointer = 6,
    Utf8Error = 7,
    BufferTooSmall = 8,
}

/// <summary>An error raised by the StateSet engine.</summary>
public class StateSetException : Exception
{
    /// <summary>The engine's error code.</summary>
    public StateSetErrorCode Code { get; }

    /// <summary>The engine's error kind, e.g. <c>"not_found"</c>.</summary>
    public string Kind { get; }

    public StateSetException(string message)
        : this(StateSetErrorCode.InternalError, "internal_error", message) { }

    public StateSetException(StateSetErrorCode code, string kind, string message)
        : base(message)
    {
        Code = code;
        Kind = kind;
    }
}

/// <summary>Raised when an operation names an entity that does not exist.</summary>
public sealed class StateSetNotFoundException : StateSetException
{
    public StateSetNotFoundException(string kind, string message)
        : base(StateSetErrorCode.NotFound, kind, message) { }
}

/// <summary>Raised when the engine refuses an argument or a state transition.</summary>
public sealed class StateSetValidationException : StateSetException
{
    public StateSetValidationException(string kind, string message)
        : base(StateSetErrorCode.InvalidArgument, kind, message) { }
}

#endregion
