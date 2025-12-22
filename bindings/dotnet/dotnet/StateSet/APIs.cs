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

    /// <summary>
    /// Ship an order
    /// </summary>
    public Order Ship(string id)
    {
        var ptr = NativeMethods.stateset_order_ship(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Order>(ptr);
    }

    /// <summary>
    /// Cancel an order
    /// </summary>
    public Order Cancel(string id)
    {
        var ptr = NativeMethods.stateset_order_cancel(_commerce.Handle, id);
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

    /// <summary>
    /// Get a return by ID
    /// </summary>
    public Return? Get(string id)
    {
        var ptr = NativeMethods.stateset_return_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Return>(ptr);
    }

    /// <summary>
    /// Approve a return
    /// </summary>
    public Return Approve(string id)
    {
        var ptr = NativeMethods.stateset_return_approve(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Return>(ptr);
    }

    /// <summary>
    /// Reject a return
    /// </summary>
    public Return Reject(string id, string reason)
    {
        var ptr = NativeMethods.stateset_return_reject(_commerce.Handle, id, reason);
        return StateSetCommerce.ParseJsonRequired<Return>(ptr);
    }

    /// <summary>
    /// Complete a return
    /// </summary>
    public Return Complete(string id)
    {
        var ptr = NativeMethods.stateset_return_complete(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Return>(ptr);
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

    /// <summary>
    /// Get a payment by ID
    /// </summary>
    public Payment? Get(string id)
    {
        var ptr = NativeMethods.stateset_payment_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Payment>(ptr);
    }

    /// <summary>
    /// List all payments
    /// </summary>
    public List<Payment> List()
    {
        var ptr = NativeMethods.stateset_payment_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Payment>(ptr);
    }

    /// <summary>
    /// Complete a payment
    /// </summary>
    public Payment Complete(string id)
    {
        var ptr = NativeMethods.stateset_payment_complete(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Payment>(ptr);
    }

    /// <summary>
    /// Fail a payment
    /// </summary>
    public Payment Fail(string id, string reason)
    {
        var ptr = NativeMethods.stateset_payment_fail(_commerce.Handle, id, reason);
        return StateSetCommerce.ParseJsonRequired<Payment>(ptr);
    }

    /// <summary>
    /// Refund a payment
    /// </summary>
    public Refund Refund(string paymentId, decimal amount, string reason)
    {
        var ptr = NativeMethods.stateset_payment_refund(_commerce.Handle, paymentId, (double)amount, reason);
        return StateSetCommerce.ParseJsonRequired<Refund>(ptr);
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

/// <summary>
/// Shipments API
/// </summary>
public sealed class ShipmentsApi
{
    private readonly StateSetCommerce _commerce;
    internal ShipmentsApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a shipment
    /// </summary>
    public Shipment Create(string orderId, string recipientName, string shippingAddress, string carrier)
    {
        var ptr = NativeMethods.stateset_shipment_create(_commerce.Handle, orderId, recipientName, shippingAddress, carrier);
        return StateSetCommerce.ParseJsonRequired<Shipment>(ptr);
    }

    /// <summary>
    /// Get a shipment by ID
    /// </summary>
    public Shipment? Get(string id)
    {
        var ptr = NativeMethods.stateset_shipment_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Shipment>(ptr);
    }

    /// <summary>
    /// List all shipments
    /// </summary>
    public List<Shipment> List()
    {
        var ptr = NativeMethods.stateset_shipment_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Shipment>(ptr);
    }

    /// <summary>
    /// Ship a shipment with tracking number
    /// </summary>
    public Shipment Ship(string id, string trackingNumber)
    {
        var ptr = NativeMethods.stateset_shipment_ship(_commerce.Handle, id, trackingNumber);
        return StateSetCommerce.ParseJsonRequired<Shipment>(ptr);
    }

    /// <summary>
    /// Mark shipment as delivered
    /// </summary>
    public Shipment Deliver(string id)
    {
        var ptr = NativeMethods.stateset_shipment_deliver(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Shipment>(ptr);
    }

    /// <summary>
    /// Cancel a shipment
    /// </summary>
    public Shipment Cancel(string id)
    {
        var ptr = NativeMethods.stateset_shipment_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Shipment>(ptr);
    }
}

/// <summary>
/// Warranties API
/// </summary>
public sealed class WarrantiesApi
{
    private readonly StateSetCommerce _commerce;
    internal WarrantiesApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a warranty
    /// </summary>
    public Warranty Create(string customerId, string productId, WarrantyType warrantyType, int durationMonths)
    {
        var typeStr = warrantyType.ToString().ToLowerInvariant();
        var ptr = NativeMethods.stateset_warranty_create(_commerce.Handle, customerId, productId, typeStr, durationMonths);
        return StateSetCommerce.ParseJsonRequired<Warranty>(ptr);
    }

    /// <summary>
    /// Get a warranty by ID
    /// </summary>
    public Warranty? Get(string id)
    {
        var ptr = NativeMethods.stateset_warranty_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Warranty>(ptr);
    }

    /// <summary>
    /// List all warranties
    /// </summary>
    public List<Warranty> List()
    {
        var ptr = NativeMethods.stateset_warranty_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Warranty>(ptr);
    }

    /// <summary>
    /// Create a warranty claim
    /// </summary>
    public WarrantyClaim CreateClaim(string warrantyId, string issueDescription)
    {
        var ptr = NativeMethods.stateset_warranty_create_claim(_commerce.Handle, warrantyId, issueDescription);
        return StateSetCommerce.ParseJsonRequired<WarrantyClaim>(ptr);
    }

    /// <summary>
    /// Approve a warranty claim
    /// </summary>
    public WarrantyClaim ApproveClaim(string claimId)
    {
        var ptr = NativeMethods.stateset_warranty_approve_claim(_commerce.Handle, claimId);
        return StateSetCommerce.ParseJsonRequired<WarrantyClaim>(ptr);
    }

    /// <summary>
    /// Deny a warranty claim
    /// </summary>
    public WarrantyClaim DenyClaim(string claimId, string reason)
    {
        var ptr = NativeMethods.stateset_warranty_deny_claim(_commerce.Handle, claimId, reason);
        return StateSetCommerce.ParseJsonRequired<WarrantyClaim>(ptr);
    }

    /// <summary>
    /// Complete a warranty claim with resolution
    /// </summary>
    public WarrantyClaim CompleteClaim(string claimId, ClaimResolution resolution)
    {
        var resolutionStr = resolution switch
        {
            ClaimResolution.Repair => "repair",
            ClaimResolution.Replacement => "replacement",
            ClaimResolution.Refund => "refund",
            ClaimResolution.StoreCredit => "store_credit",
            _ => "repair"
        };
        var ptr = NativeMethods.stateset_warranty_complete_claim(_commerce.Handle, claimId, resolutionStr);
        return StateSetCommerce.ParseJsonRequired<WarrantyClaim>(ptr);
    }
}

/// <summary>
/// Suppliers API
/// </summary>
public sealed class SuppliersApi
{
    private readonly StateSetCommerce _commerce;
    internal SuppliersApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a supplier
    /// </summary>
    public Supplier Create(string name, string email, string phone)
    {
        var ptr = NativeMethods.stateset_supplier_create(_commerce.Handle, name, email, phone);
        return StateSetCommerce.ParseJsonRequired<Supplier>(ptr);
    }

    /// <summary>
    /// Get a supplier by ID
    /// </summary>
    public Supplier? Get(string id)
    {
        var ptr = NativeMethods.stateset_supplier_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Supplier>(ptr);
    }

    /// <summary>
    /// List all suppliers
    /// </summary>
    public List<Supplier> List()
    {
        var ptr = NativeMethods.stateset_supplier_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Supplier>(ptr);
    }
}

/// <summary>
/// Purchase Orders API
/// </summary>
public sealed class PurchaseOrdersApi
{
    private readonly StateSetCommerce _commerce;
    private static readonly JsonSerializerOptions SerializerOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
    };

    internal PurchaseOrdersApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a purchase order
    /// </summary>
    public PurchaseOrder Create(string supplierId, IEnumerable<PurchaseOrderItem> items)
    {
        var itemsJson = JsonSerializer.Serialize(items, SerializerOptions);
        var ptr = NativeMethods.stateset_purchase_order_create(_commerce.Handle, supplierId, itemsJson);
        return StateSetCommerce.ParseJsonRequired<PurchaseOrder>(ptr);
    }

    /// <summary>
    /// Get a purchase order by ID
    /// </summary>
    public PurchaseOrder? Get(string id)
    {
        var ptr = NativeMethods.stateset_purchase_order_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<PurchaseOrder>(ptr);
    }

    /// <summary>
    /// List all purchase orders
    /// </summary>
    public List<PurchaseOrder> List()
    {
        var ptr = NativeMethods.stateset_purchase_order_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<PurchaseOrder>(ptr);
    }

    /// <summary>
    /// Submit a purchase order for approval
    /// </summary>
    public PurchaseOrder Submit(string id)
    {
        var ptr = NativeMethods.stateset_purchase_order_submit(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<PurchaseOrder>(ptr);
    }

    /// <summary>
    /// Approve a purchase order
    /// </summary>
    public PurchaseOrder Approve(string id, string approvedBy)
    {
        var ptr = NativeMethods.stateset_purchase_order_approve(_commerce.Handle, id, approvedBy);
        return StateSetCommerce.ParseJsonRequired<PurchaseOrder>(ptr);
    }

    /// <summary>
    /// Send a purchase order to supplier
    /// </summary>
    public PurchaseOrder Send(string id)
    {
        var ptr = NativeMethods.stateset_purchase_order_send(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<PurchaseOrder>(ptr);
    }

    /// <summary>
    /// Cancel a purchase order
    /// </summary>
    public PurchaseOrder Cancel(string id)
    {
        var ptr = NativeMethods.stateset_purchase_order_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<PurchaseOrder>(ptr);
    }
}

/// <summary>
/// Invoices API
/// </summary>
public sealed class InvoicesApi
{
    private readonly StateSetCommerce _commerce;
    private static readonly JsonSerializerOptions SerializerOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
    };

    internal InvoicesApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create an invoice
    /// </summary>
    public Invoice Create(string customerId, IEnumerable<InvoiceItem> items, string billingEmail)
    {
        var itemsJson = JsonSerializer.Serialize(items, SerializerOptions);
        var ptr = NativeMethods.stateset_invoice_create(_commerce.Handle, customerId, itemsJson, billingEmail);
        return StateSetCommerce.ParseJsonRequired<Invoice>(ptr);
    }

    /// <summary>
    /// Get an invoice by ID
    /// </summary>
    public Invoice? Get(string id)
    {
        var ptr = NativeMethods.stateset_invoice_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Invoice>(ptr);
    }

    /// <summary>
    /// List all invoices
    /// </summary>
    public List<Invoice> List()
    {
        var ptr = NativeMethods.stateset_invoice_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Invoice>(ptr);
    }

    /// <summary>
    /// Send an invoice
    /// </summary>
    public Invoice Send(string id)
    {
        var ptr = NativeMethods.stateset_invoice_send(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Invoice>(ptr);
    }

    /// <summary>
    /// Void an invoice
    /// </summary>
    public Invoice Void(string id)
    {
        var ptr = NativeMethods.stateset_invoice_void(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Invoice>(ptr);
    }

    /// <summary>
    /// Record payment against invoice
    /// </summary>
    public Invoice RecordPayment(string id, decimal amount, string paymentMethod)
    {
        var ptr = NativeMethods.stateset_invoice_record_payment(_commerce.Handle, id, (double)amount, paymentMethod);
        return StateSetCommerce.ParseJsonRequired<Invoice>(ptr);
    }

    /// <summary>
    /// Get overdue invoices
    /// </summary>
    public List<Invoice> GetOverdue()
    {
        var ptr = NativeMethods.stateset_invoice_get_overdue(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Invoice>(ptr);
    }
}

/// <summary>
/// Bill of Materials API
/// </summary>
public sealed class BomApi
{
    private readonly StateSetCommerce _commerce;
    internal BomApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a BOM
    /// </summary>
    public BillOfMaterials Create(string productId, string name, string? description = null)
    {
        var ptr = NativeMethods.stateset_bom_create(_commerce.Handle, productId, name, description);
        return StateSetCommerce.ParseJsonRequired<BillOfMaterials>(ptr);
    }

    /// <summary>
    /// Get a BOM by ID
    /// </summary>
    public BillOfMaterials? Get(string id)
    {
        var ptr = NativeMethods.stateset_bom_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<BillOfMaterials>(ptr);
    }

    /// <summary>
    /// List all BOMs
    /// </summary>
    public List<BillOfMaterials> List()
    {
        var ptr = NativeMethods.stateset_bom_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<BillOfMaterials>(ptr);
    }

    /// <summary>
    /// Add component to BOM
    /// </summary>
    public BomComponent AddComponent(string bomId, string name, string componentSku, decimal quantity)
    {
        var ptr = NativeMethods.stateset_bom_add_component(_commerce.Handle, bomId, name, componentSku, (double)quantity);
        return StateSetCommerce.ParseJsonRequired<BomComponent>(ptr);
    }

    /// <summary>
    /// Get BOM components
    /// </summary>
    public List<BomComponent> GetComponents(string bomId)
    {
        var ptr = NativeMethods.stateset_bom_get_components(_commerce.Handle, bomId);
        return StateSetCommerce.ParseJsonList<BomComponent>(ptr);
    }

    /// <summary>
    /// Activate a BOM
    /// </summary>
    public BillOfMaterials Activate(string id)
    {
        var ptr = NativeMethods.stateset_bom_activate(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<BillOfMaterials>(ptr);
    }
}

/// <summary>
/// Work Orders API
/// </summary>
public sealed class WorkOrdersApi
{
    private readonly StateSetCommerce _commerce;
    internal WorkOrdersApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Create a work order
    /// </summary>
    public WorkOrder Create(string productId, decimal quantityToBuild, string? bomId = null)
    {
        var ptr = NativeMethods.stateset_work_order_create(_commerce.Handle, productId, (double)quantityToBuild, bomId);
        return StateSetCommerce.ParseJsonRequired<WorkOrder>(ptr);
    }

    /// <summary>
    /// Get a work order by ID
    /// </summary>
    public WorkOrder? Get(string id)
    {
        var ptr = NativeMethods.stateset_work_order_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<WorkOrder>(ptr);
    }

    /// <summary>
    /// List all work orders
    /// </summary>
    public List<WorkOrder> List()
    {
        var ptr = NativeMethods.stateset_work_order_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<WorkOrder>(ptr);
    }

    /// <summary>
    /// Start a work order
    /// </summary>
    public WorkOrder Start(string id)
    {
        var ptr = NativeMethods.stateset_work_order_start(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<WorkOrder>(ptr);
    }

    /// <summary>
    /// Complete a work order
    /// </summary>
    public WorkOrder Complete(string id, decimal quantityCompleted)
    {
        var ptr = NativeMethods.stateset_work_order_complete(_commerce.Handle, id, (double)quantityCompleted);
        return StateSetCommerce.ParseJsonRequired<WorkOrder>(ptr);
    }

    /// <summary>
    /// Cancel a work order
    /// </summary>
    public WorkOrder Cancel(string id)
    {
        var ptr = NativeMethods.stateset_work_order_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<WorkOrder>(ptr);
    }
}

/// <summary>
/// Currency API
/// </summary>
public sealed class CurrencyApi
{
    private readonly StateSetCommerce _commerce;
    internal CurrencyApi(StateSetCommerce commerce) => _commerce = commerce;

    /// <summary>
    /// Set exchange rate
    /// </summary>
    public ExchangeRate SetRate(CurrencyCode fromCurrency, CurrencyCode toCurrency, decimal rate)
    {
        var ptr = NativeMethods.stateset_currency_set_rate(
            _commerce.Handle, fromCurrency.ToString(), toCurrency.ToString(), (double)rate);
        return StateSetCommerce.ParseJsonRequired<ExchangeRate>(ptr);
    }

    /// <summary>
    /// Get exchange rate
    /// </summary>
    public ExchangeRate? GetRate(CurrencyCode fromCurrency, CurrencyCode toCurrency)
    {
        var ptr = NativeMethods.stateset_currency_get_rate(
            _commerce.Handle, fromCurrency.ToString(), toCurrency.ToString());
        return StateSetCommerce.ParseJson<ExchangeRate>(ptr);
    }

    /// <summary>
    /// Convert currency
    /// </summary>
    public ConversionResult Convert(decimal amount, CurrencyCode fromCurrency, CurrencyCode toCurrency)
    {
        var ptr = NativeMethods.stateset_currency_convert(
            _commerce.Handle, (double)amount, fromCurrency.ToString(), toCurrency.ToString());
        return StateSetCommerce.ParseJsonRequired<ConversionResult>(ptr);
    }

    /// <summary>
    /// Get currency settings
    /// </summary>
    public StoreCurrencySettings GetSettings()
    {
        var ptr = NativeMethods.stateset_currency_get_settings(_commerce.Handle);
        return StateSetCommerce.ParseJsonRequired<StoreCurrencySettings>(ptr);
    }
}
