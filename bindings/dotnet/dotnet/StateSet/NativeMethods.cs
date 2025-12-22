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
}
