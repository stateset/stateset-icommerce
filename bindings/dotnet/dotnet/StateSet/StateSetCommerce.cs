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
    internal StateSetDotNetStore Store { get; } = new();

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
    /// Shipments API
    /// </summary>
    public ShipmentsApi Shipments { get; }

    /// <summary>
    /// Warranties API
    /// </summary>
    public WarrantiesApi Warranties { get; }

    /// <summary>
    /// Suppliers API
    /// </summary>
    public SuppliersApi Suppliers { get; }

    /// <summary>
    /// Purchase Orders API
    /// </summary>
    public PurchaseOrdersApi PurchaseOrders { get; }

    /// <summary>
    /// Invoices API
    /// </summary>
    public InvoicesApi Invoices { get; }

    /// <summary>
    /// Bill of Materials API
    /// </summary>
    public BomApi Bom { get; }

    /// <summary>
    /// Work Orders API
    /// </summary>
    public WorkOrdersApi WorkOrders { get; }

    /// <summary>
    /// Currency API
    /// </summary>
    public CurrencyApi Currency { get; }

    /// <summary>
    /// Subscriptions API
    /// </summary>
    public SubscriptionsApi Subscriptions { get; }

    /// <summary>
    /// Promotions API
    /// </summary>
    public PromotionsApi Promotions { get; }

    /// <summary>
    /// Tax API
    /// </summary>
    public TaxApi Tax { get; }

    /// <summary>
    /// Quality API
    /// </summary>
    public QualityApi Quality { get; }

    /// <summary>
    /// Lots API
    /// </summary>
    public LotsApi Lots { get; }

    /// <summary>
    /// Serials API
    /// </summary>
    public SerialsApi Serials { get; }

    /// <summary>
    /// Warehouse API
    /// </summary>
    public WarehouseApi Warehouse { get; }

    /// <summary>
    /// Receiving API
    /// </summary>
    public ReceivingApi Receiving { get; }

    /// <summary>
    /// Fulfillment API
    /// </summary>
    public FulfillmentApi Fulfillment { get; }

    /// <summary>
    /// Accounts Payable API
    /// </summary>
    public AccountsPayableApi AccountsPayable { get; }

    /// <summary>
    /// Accounts Receivable API
    /// </summary>
    public AccountsReceivableApi AccountsReceivable { get; }

    /// <summary>
    /// Cost Accounting API
    /// </summary>
    public CostAccountingApi CostAccounting { get; }

    /// <summary>
    /// Credit API
    /// </summary>
    public CreditApi Credit { get; }

    /// <summary>
    /// Backorders API
    /// </summary>
    public BackordersApi Backorders { get; }

    /// <summary>
    /// General Ledger API
    /// </summary>
    public GeneralLedgerApi GeneralLedger { get; }

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
        Shipments = new ShipmentsApi(this);
        Warranties = new WarrantiesApi(this);
        Suppliers = new SuppliersApi(this);
        PurchaseOrders = new PurchaseOrdersApi(this);
        Invoices = new InvoicesApi(this);
        Bom = new BomApi(this);
        WorkOrders = new WorkOrdersApi(this);
        Currency = new CurrencyApi(this);
        Subscriptions = new SubscriptionsApi(this);
        Promotions = new PromotionsApi(this);
        Tax = new TaxApi(this);
        Quality = new QualityApi(this);
        Lots = new LotsApi(this);
        Serials = new SerialsApi(this);
        Warehouse = new WarehouseApi(this);
        Receiving = new ReceivingApi(this);
        Fulfillment = new FulfillmentApi(this);
        AccountsPayable = new AccountsPayableApi(this);
        AccountsReceivable = new AccountsReceivableApi(this);
        CostAccounting = new CostAccountingApi(this);
        Credit = new CreditApi(this);
        Backorders = new BackordersApi(this);
        GeneralLedger = new GeneralLedgerApi(this);
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
