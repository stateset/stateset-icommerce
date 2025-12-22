using System.Runtime.InteropServices;
using System.Text.Json;

namespace StateSet.Embedded;

/// <summary>
/// StateSet Embedded Commerce - The SQLite of Commerce
///
/// A zero-dependency, local-first commerce engine for .NET applications.
/// </summary>
/// <example>
/// <code>
/// using var commerce = new StateSetCommerce("store.db");
///
/// var customer = commerce.Customers.Create(
///     email: "alice@example.com",
///     firstName: "Alice",
///     lastName: "Smith"
/// );
///
/// var product = commerce.Products.Create(
///     name: "Premium Widget",
///     sku: "WIDGET-001",
///     price: 29.99m
/// );
///
/// var order = commerce.Orders.Create(
///     customerId: customer.Id,
///     items: new[] { new OrderItem { Sku = "WIDGET-001", Name = "Widget", Quantity = 2, UnitPrice = 29.99 } },
///     currency: "USD"
/// );
/// </code>
/// </example>
public sealed class StateSetCommerce : IDisposable
{
    private IntPtr _handle;
    private bool _disposed;

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
    };

    /// <summary>
    /// Customers API
    /// </summary>
    public CustomersApi Customers { get; }

    /// <summary>
    /// Products API
    /// </summary>
    public ProductsApi Products { get; }

    /// <summary>
    /// Orders API
    /// </summary>
    public OrdersApi Orders { get; }

    /// <summary>
    /// Inventory API
    /// </summary>
    public InventoryApi Inventory { get; }

    /// <summary>
    /// Carts API
    /// </summary>
    public CartsApi Carts { get; }

    /// <summary>
    /// Returns API
    /// </summary>
    public ReturnsApi Returns { get; }

    /// <summary>
    /// Payments API
    /// </summary>
    public PaymentsApi Payments { get; }

    /// <summary>
    /// Analytics API
    /// </summary>
    public AnalyticsApi Analytics { get; }

    /// <summary>
    /// Create a new Commerce instance
    /// </summary>
    /// <param name="dbPath">Path to SQLite database file, or ":memory:" for in-memory database</param>
    /// <exception cref="StateSetException">Thrown if database initialization fails</exception>
    public StateSetCommerce(string dbPath)
    {
        _handle = NativeMethods.stateset_commerce_new(dbPath);
        if (_handle == IntPtr.Zero)
        {
            throw new StateSetException("Failed to create commerce instance");
        }

        Customers = new CustomersApi(this);
        Products = new ProductsApi(this);
        Orders = new OrdersApi(this);
        Inventory = new InventoryApi(this);
        Carts = new CartsApi(this);
        Returns = new ReturnsApi(this);
        Payments = new PaymentsApi(this);
        Analytics = new AnalyticsApi(this);
    }

    internal IntPtr Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            return _handle;
        }
    }

    internal static T? ParseJson<T>(IntPtr ptr) where T : class
    {
        if (ptr == IntPtr.Zero)
            return null;

        try
        {
            var json = Marshal.PtrToStringUTF8(ptr);
            if (string.IsNullOrEmpty(json))
                return null;

            return JsonSerializer.Deserialize<T>(json, JsonOptions);
        }
        finally
        {
            NativeMethods.stateset_string_free(ptr);
        }
    }

    internal static T ParseJsonRequired<T>(IntPtr ptr) where T : class
    {
        var result = ParseJson<T>(ptr);
        return result ?? throw new StateSetException("Failed to parse response");
    }

    internal static List<T> ParseJsonList<T>(IntPtr ptr) where T : class
    {
        if (ptr == IntPtr.Zero)
            return new List<T>();

        try
        {
            var json = Marshal.PtrToStringUTF8(ptr);
            if (string.IsNullOrEmpty(json))
                return new List<T>();

            return JsonSerializer.Deserialize<List<T>>(json, JsonOptions) ?? new List<T>();
        }
        finally
        {
            NativeMethods.stateset_string_free(ptr);
        }
    }

    /// <summary>
    /// Dispose of the commerce instance and release native resources
    /// </summary>
    public void Dispose()
    {
        if (_disposed)
            return;

        if (_handle != IntPtr.Zero)
        {
            NativeMethods.stateset_commerce_free(_handle);
            _handle = IntPtr.Zero;
        }

        _disposed = true;
    }
}

/// <summary>
/// Exception thrown when a StateSet operation fails
/// </summary>
public class StateSetException : Exception
{
    public StateSetException(string message) : base(message) { }
    public StateSetException(string message, Exception inner) : base(message, inner) { }
}
