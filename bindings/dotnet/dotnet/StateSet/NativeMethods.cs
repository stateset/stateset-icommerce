using System.Runtime.InteropServices;

namespace StateSet.Embedded;

/// <summary>
/// P/Invoke declarations for the native StateSet library
/// </summary>
internal static partial class NativeMethods
{
    private const string LibraryName = "stateset_dotnet";

    // =============================================================================
    // Memory Management
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void stateset_string_free(IntPtr s);

    // =============================================================================
    // Commerce Lifecycle
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_commerce_new(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string dbPath);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void stateset_commerce_free(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_get_last_error();

    // =============================================================================
    // Customers API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_customer_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string email,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string firstName,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string lastName,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? phone);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_customer_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_customer_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int stateset_customer_delete(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int stateset_customer_count(IntPtr handle);

    // =============================================================================
    // Products API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_product_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string sku,
        double price,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? description);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_product_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_product_list(IntPtr handle);

    // =============================================================================
    // Orders API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_order_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string customerId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string itemsJson,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string currency);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_order_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_order_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_order_update_status(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string status);

    // =============================================================================
    // Inventory API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_inventory_create_item(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string sku,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        double initialQuantity);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int stateset_inventory_adjust(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string sku,
        double quantityDelta,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string reason);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_inventory_get_level(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string sku);

    // =============================================================================
    // Carts API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_cart_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? customerId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? currency);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_cart_add_item(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string cartId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string variantId,
        int quantity);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_cart_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string cartId);

    // =============================================================================
    // Returns API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_return_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string orderId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string reason,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? notes);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_return_list(IntPtr handle);

    // =============================================================================
    // Payments API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_payment_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string orderId,
        double amount,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string currency,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string method);

    // =============================================================================
    // Analytics API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_analytics_sales_summary(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string period);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_analytics_top_products(
        IntPtr handle,
        int limit);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_analytics_top_customers(
        IntPtr handle,
        int limit);

    // =============================================================================
    // Orders API - Additional Methods
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_order_ship(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_order_cancel(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    // =============================================================================
    // Returns API - Additional Methods
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_return_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_return_approve(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_return_reject(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string reason);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_return_complete(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    // =============================================================================
    // Payments API - Additional Methods
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_payment_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_payment_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_payment_complete(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_payment_fail(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string reason);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_payment_refund(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string paymentId,
        double amount,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string reason);

    // =============================================================================
    // Shipments API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_shipment_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string orderId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string recipientName,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string shippingAddress,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string carrier);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_shipment_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_shipment_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_shipment_ship(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string trackingNumber);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_shipment_deliver(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_shipment_cancel(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    // =============================================================================
    // Warranties API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string customerId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string productId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string warrantyType,
        int durationMonths);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_create_claim(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string warrantyId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string issueDescription);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_approve_claim(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string claimId);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_deny_claim(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string claimId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string reason);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_warranty_complete_claim(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string claimId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string resolution);

    // =============================================================================
    // Suppliers API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_supplier_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string email,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string phone);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_supplier_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_supplier_list(IntPtr handle);

    // =============================================================================
    // Purchase Orders API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string supplierId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string itemsJson);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_submit(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_approve(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string approvedBy);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_send(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_purchase_order_cancel(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    // =============================================================================
    // Invoices API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string customerId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string itemsJson,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string billingEmail);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_send(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_void(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_record_payment(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        double amount,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string paymentMethod);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_invoice_get_overdue(IntPtr handle);

    // =============================================================================
    // BOM API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_bom_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string productId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? description);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_bom_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_bom_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_bom_add_component(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string bomId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string componentSku,
        double quantity);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_bom_get_components(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string bomId);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_bom_activate(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    // =============================================================================
    // Work Orders API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_work_order_create(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string productId,
        double quantityToBuild,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? bomId);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_work_order_get(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_work_order_list(IntPtr handle);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_work_order_start(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_work_order_complete(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
        double quantityCompleted);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_work_order_cancel(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    // =============================================================================
    // Currency API
    // =============================================================================

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_currency_set_rate(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string fromCurrency,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string toCurrency,
        double rate);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_currency_get_rate(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string fromCurrency,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string toCurrency);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_currency_convert(
        IntPtr handle,
        double amount,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string fromCurrency,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string toCurrency);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr stateset_currency_get_settings(IntPtr handle);
}
