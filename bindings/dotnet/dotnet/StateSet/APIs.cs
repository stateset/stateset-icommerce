using System.Text.Json;

namespace StateSet.Embedded;

/// <summary>
/// Customers API
/// </summary>
public sealed class CustomersApi
{
    private readonly StateSetCommerce _commerce;

    internal CustomersApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a new customer
    /// </summary>
    public Customer Create(string email, string firstName, string lastName, string? phone = null)
    {
        var ptr = NativeMethods.stateset_customer_create(
            _commerce.Handle, email, firstName, lastName, phone);
        return StateSetCommerce.ParseJsonRequired<Customer>(ptr);
    }

    /// <summary>
    /// Get a customer by ID
    /// </summary>
    public Customer? Get(string id)
    {
        var ptr = NativeMethods.stateset_customer_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Customer>(ptr);
    }

    /// <summary>
    /// List all customers
    /// </summary>
    public List<Customer> List()
    {
        var ptr = NativeMethods.stateset_customer_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Customer>(ptr);
    }

    /// <summary>
    /// Delete a customer by ID
    /// </summary>
    public bool Delete(string id)
    {
        return NativeMethods.stateset_customer_delete(_commerce.Handle, id) == 1;
    }

    /// <summary>
    /// Get customer count
    /// </summary>
    public int Count()
    {
        var result = NativeMethods.stateset_customer_count(_commerce.Handle);
        return result >= 0 ? result : 0;
    }
}

/// <summary>
/// Products API
/// </summary>
public sealed class ProductsApi
{
    private readonly StateSetCommerce _commerce;

    internal ProductsApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a new product
    /// </summary>
    public Product Create(string name, string sku, decimal price, string? description = null)
    {
        var ptr = NativeMethods.stateset_product_create(
            _commerce.Handle, name, sku, (double)price, description);
        return StateSetCommerce.ParseJsonRequired<Product>(ptr);
    }

    /// <summary>
    /// Get a product by ID
    /// </summary>
    public Product? Get(string id)
    {
        var ptr = NativeMethods.stateset_product_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Product>(ptr);
    }

    /// <summary>
    /// List all products
    /// </summary>
    public List<Product> List()
    {
        var ptr = NativeMethods.stateset_product_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Product>(ptr);
    }
}

/// <summary>
/// Orders API
/// </summary>
public sealed class OrdersApi
{
    private readonly StateSetCommerce _commerce;
    private static readonly JsonSerializerOptions SerializerOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
    };

    internal OrdersApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a new order
    /// </summary>
    public Order Create(string customerId, IEnumerable<OrderItem> items, string currency = "USD")
    {
        var itemsJson = JsonSerializer.Serialize(items, SerializerOptions);
        var ptr = NativeMethods.stateset_order_create(
            _commerce.Handle, customerId, itemsJson, currency);
        return StateSetCommerce.ParseJsonRequired<Order>(ptr);
    }

    /// <summary>
    /// Get an order by ID
    /// </summary>
    public Order? Get(string id)
    {
        var ptr = NativeMethods.stateset_order_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Order>(ptr);
    }

    /// <summary>
    /// List all orders
    /// </summary>
    public List<Order> List()
    {
        var ptr = NativeMethods.stateset_order_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Order>(ptr);
    }

    /// <summary>
    /// Update order status
    /// </summary>
    public Order UpdateStatus(string id, OrderStatus status)
    {
        var statusStr = status.ToString().ToLowerInvariant();
        var ptr = NativeMethods.stateset_order_update_status(_commerce.Handle, id, statusStr);
        return StateSetCommerce.ParseJsonRequired<Order>(ptr);
    }
}

/// <summary>
/// Inventory API
/// </summary>
public sealed class InventoryApi
{
    private readonly StateSetCommerce _commerce;

    internal InventoryApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a new inventory item
    /// </summary>
    public InventoryItem CreateItem(string sku, string name, decimal initialQuantity = 0)
    {
        var ptr = NativeMethods.stateset_inventory_create_item(
            _commerce.Handle, sku, name, (double)initialQuantity);
        return StateSetCommerce.ParseJsonRequired<InventoryItem>(ptr);
    }

    /// <summary>
    /// Adjust inventory quantity
    /// </summary>
    public bool Adjust(string sku, decimal quantityDelta, string reason = "manual adjustment")
    {
        return NativeMethods.stateset_inventory_adjust(
            _commerce.Handle, sku, (double)quantityDelta, reason) == 1;
    }

    /// <summary>
    /// Get stock level for SKU
    /// </summary>
    public StockLevel? GetLevel(string sku)
    {
        var ptr = NativeMethods.stateset_inventory_get_level(_commerce.Handle, sku);
        return StateSetCommerce.ParseJson<StockLevel>(ptr);
    }
}

/// <summary>
/// Carts API
/// </summary>
public sealed class CartsApi
{
    private readonly StateSetCommerce _commerce;

    internal CartsApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a new cart
    /// </summary>
    public Cart Create(string? customerId = null, string currency = "USD")
    {
        var ptr = NativeMethods.stateset_cart_create(_commerce.Handle, customerId, currency);
        return StateSetCommerce.ParseJsonRequired<Cart>(ptr);
    }

    /// <summary>
    /// Add item to cart
    /// </summary>
    public Cart AddItem(string cartId, string variantId, int quantity = 1)
    {
        var ptr = NativeMethods.stateset_cart_add_item(_commerce.Handle, cartId, variantId, quantity);
        return StateSetCommerce.ParseJsonRequired<Cart>(ptr);
    }

    /// <summary>
    /// Get cart by ID
    /// </summary>
    public Cart? Get(string cartId)
    {
        var ptr = NativeMethods.stateset_cart_get(_commerce.Handle, cartId);
        return StateSetCommerce.ParseJson<Cart>(ptr);
    }
}

/// <summary>
/// Returns API
/// </summary>
public sealed class ReturnsApi
{
    private readonly StateSetCommerce _commerce;

    internal ReturnsApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a return request
    /// </summary>
    public Return Create(string orderId, ReturnReason reason, string? notes = null)
    {
        var reasonStr = reason switch
        {
            ReturnReason.Defective => "defective",
            ReturnReason.WrongItem => "wrong_item",
            ReturnReason.NotAsDescribed => "not_as_described",
            ReturnReason.ChangedMind => "changed_mind",
            ReturnReason.Damaged => "damaged",
            _ => "other"
        };
        var ptr = NativeMethods.stateset_return_create(_commerce.Handle, orderId, reasonStr, notes);
        return StateSetCommerce.ParseJsonRequired<Return>(ptr);
    }

    /// <summary>
    /// List all returns
    /// </summary>
    public List<Return> List()
    {
        var ptr = NativeMethods.stateset_return_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Return>(ptr);
    }
}

/// <summary>
/// Payments API
/// </summary>
public sealed class PaymentsApi
{
    private readonly StateSetCommerce _commerce;

    internal PaymentsApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a payment
    /// </summary>
    public Payment Create(string orderId, decimal amount, string currency = "USD", PaymentMethod method = PaymentMethod.CreditCard)
    {
        var methodStr = method switch
        {
            PaymentMethod.CreditCard => "credit_card",
            PaymentMethod.DebitCard => "debit_card",
            PaymentMethod.BankTransfer => "bank_transfer",
            PaymentMethod.PayPal => "paypal",
            PaymentMethod.ApplePay => "apple_pay",
            PaymentMethod.GooglePay => "google_pay",
            PaymentMethod.Crypto => "crypto",
            _ => "other"
        };
        var ptr = NativeMethods.stateset_payment_create(
            _commerce.Handle, orderId, (double)amount, currency, methodStr);
        return StateSetCommerce.ParseJsonRequired<Payment>(ptr);
    }
}

/// <summary>
/// Analytics API
/// </summary>
public sealed class AnalyticsApi
{
    private readonly StateSetCommerce _commerce;

    internal AnalyticsApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Get sales summary for a time period
    /// </summary>
    public SalesSummary GetSalesSummary(TimePeriod period = TimePeriod.Month)
    {
        var periodStr = period switch
        {
            TimePeriod.Today => "today",
            TimePeriod.Week => "week",
            TimePeriod.Month => "month",
            TimePeriod.Quarter => "quarter",
            TimePeriod.Year => "year",
            TimePeriod.AllTime => "all",
            _ => "month"
        };
        var ptr = NativeMethods.stateset_analytics_sales_summary(_commerce.Handle, periodStr);
        return StateSetCommerce.ParseJsonRequired<SalesSummary>(ptr);
    }

    /// <summary>
    /// Get top selling products
    /// </summary>
    public List<TopProduct> GetTopProducts(int limit = 10)
    {
        var ptr = NativeMethods.stateset_analytics_top_products(_commerce.Handle, limit);
        return StateSetCommerce.ParseJsonList<TopProduct>(ptr);
    }

    /// <summary>
    /// Get top customers by spend
    /// </summary>
    public List<TopCustomer> GetTopCustomers(int limit = 10)
    {
        var ptr = NativeMethods.stateset_analytics_top_customers(_commerce.Handle, limit);
        return StateSetCommerce.ParseJsonList<TopCustomer>(ptr);
    }
}
