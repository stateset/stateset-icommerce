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
    /// Compatibility overload for cart item calls that pass SKU details.
    /// </summary>
    public Cart AddItem(string cartId, string sku, string name, int quantity, decimal unitPrice)
        => AddItem(cartId, sku, quantity);

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

    public SalesSummary SalesSummary(TimePeriod period = TimePeriod.Month) => GetSalesSummary(period);

    /// <summary>
    /// Get top selling products
    /// </summary>
    public List<TopProduct> GetTopProducts(int limit = 10)
    {
        var ptr = NativeMethods.stateset_analytics_top_products(_commerce.Handle, limit);
        return StateSetCommerce.ParseJsonList<TopProduct>(ptr);
    }

    public List<TopProduct> TopProducts(int limit = 10) => GetTopProducts(limit);

    /// <summary>
    /// Get top customers by spend
    /// </summary>
    public List<TopCustomer> GetTopCustomers(int limit = 10)
    {
        var ptr = NativeMethods.stateset_analytics_top_customers(_commerce.Handle, limit);
        return StateSetCommerce.ParseJsonList<TopCustomer>(ptr);
    }

    public List<TopCustomer> TopCustomers(int limit = 10) => GetTopCustomers(limit);
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
    public Shipment Create(string orderId, string recipientName, string shippingAddress, string carrier = "")
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
    public Supplier Create(string name, string email, string phone = "")
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
    public Invoice Create(string customerId, IEnumerable<InvoiceItem> items, string billingEmail = "")
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

/// <summary>
/// Subscriptions API
/// </summary>
public sealed class SubscriptionsApi
{
    private readonly StateSetCommerce _commerce;
    internal SubscriptionsApi(StateSetCommerce commerce) => _commerce = commerce;

    public SubscriptionPlan CreatePlan(string code, string name, string interval, int intervalCount, decimal price, string currency = "USD")
    {
        var ptr = NativeMethods.stateset_subscription_plan_create(_commerce.Handle, code, name, interval, intervalCount, (double)price, currency);
        return StateSetCommerce.ParseJsonRequired<SubscriptionPlan>(ptr);
    }

    public SubscriptionPlan? GetPlan(string id)
    {
        var ptr = NativeMethods.stateset_subscription_plan_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<SubscriptionPlan>(ptr);
    }

    public List<SubscriptionPlan> ListPlans()
    {
        var ptr = NativeMethods.stateset_subscription_plan_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<SubscriptionPlan>(ptr);
    }

    public SubscriptionPlan ActivatePlan(string id)
    {
        var ptr = NativeMethods.stateset_subscription_plan_activate(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<SubscriptionPlan>(ptr);
    }

    public SubscriptionPlan ArchivePlan(string id)
    {
        var ptr = NativeMethods.stateset_subscription_plan_archive(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<SubscriptionPlan>(ptr);
    }

    public Subscription Subscribe(string customerId, string planId)
    {
        var ptr = NativeMethods.stateset_subscription_subscribe(_commerce.Handle, customerId, planId);
        return StateSetCommerce.ParseJsonRequired<Subscription>(ptr);
    }

    public Subscription? Get(string id)
    {
        var ptr = NativeMethods.stateset_subscription_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Subscription>(ptr);
    }

    public List<Subscription> List()
    {
        var ptr = NativeMethods.stateset_subscription_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Subscription>(ptr);
    }

    public Subscription Pause(string id)
    {
        var ptr = NativeMethods.stateset_subscription_pause(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Subscription>(ptr);
    }

    public Subscription Resume(string id)
    {
        var ptr = NativeMethods.stateset_subscription_resume(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Subscription>(ptr);
    }

    public Subscription Cancel(string id)
    {
        var ptr = NativeMethods.stateset_subscription_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Subscription>(ptr);
    }
}

/// <summary>
/// Promotions API
/// </summary>
public sealed class PromotionsApi
{
    private readonly StateSetCommerce _commerce;
    internal PromotionsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Promotion Create(string code, string name, string discountType, decimal discountValue)
    {
        var ptr = NativeMethods.stateset_promotion_create(_commerce.Handle, code, name, discountType, (double)discountValue);
        return StateSetCommerce.ParseJsonRequired<Promotion>(ptr);
    }

    public Promotion? Get(string id)
    {
        var ptr = NativeMethods.stateset_promotion_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Promotion>(ptr);
    }

    public Promotion? GetByCode(string code)
    {
        var ptr = NativeMethods.stateset_promotion_get_by_code(_commerce.Handle, code);
        return StateSetCommerce.ParseJson<Promotion>(ptr);
    }

    public List<Promotion> List()
    {
        var ptr = NativeMethods.stateset_promotion_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Promotion>(ptr);
    }

    public Promotion Activate(string id)
    {
        var ptr = NativeMethods.stateset_promotion_activate(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Promotion>(ptr);
    }

    public Promotion Deactivate(string id)
    {
        var ptr = NativeMethods.stateset_promotion_deactivate(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Promotion>(ptr);
    }

    public bool Delete(string id) => NativeMethods.stateset_promotion_delete(_commerce.Handle, id) == 1;

    public List<Promotion> GetActive()
    {
        var ptr = NativeMethods.stateset_promotion_get_active(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Promotion>(ptr);
    }

    public Coupon CreateCoupon(string promotionId, string code, int maxUses = -1)
    {
        var ptr = NativeMethods.stateset_coupon_create(_commerce.Handle, promotionId, code, maxUses);
        return StateSetCommerce.ParseJsonRequired<Coupon>(ptr);
    }

    public Coupon? GetCouponByCode(string code)
    {
        var ptr = NativeMethods.stateset_coupon_get_by_code(_commerce.Handle, code);
        return StateSetCommerce.ParseJson<Coupon>(ptr);
    }

    public Coupon? ValidateCoupon(string code)
    {
        var ptr = NativeMethods.stateset_coupon_validate(_commerce.Handle, code);
        return StateSetCommerce.ParseJson<Coupon>(ptr);
    }
}

/// <summary>
/// Tax API
/// </summary>
public sealed class TaxApi
{
    private readonly StateSetCommerce _commerce;
    internal TaxApi(StateSetCommerce commerce) => _commerce = commerce;

    public TaxCalculation Calculate(string lineItemsJson, string shippingCountry, string? shippingState = null)
    {
        var ptr = NativeMethods.stateset_tax_calculate(_commerce.Handle, lineItemsJson, shippingCountry, shippingState);
        return StateSetCommerce.ParseJsonRequired<TaxCalculation>(ptr);
    }

    public double GetEffectiveRate(string country, string? state = null, string? category = null)
        => NativeMethods.stateset_tax_get_effective_rate(_commerce.Handle, country, state, category);

    public TaxJurisdiction CreateJurisdiction(string name, string code, string countryCode, string? stateCode = null)
    {
        var ptr = NativeMethods.stateset_tax_jurisdiction_create(_commerce.Handle, name, code, countryCode, stateCode);
        return StateSetCommerce.ParseJsonRequired<TaxJurisdiction>(ptr);
    }

    public TaxJurisdiction? GetJurisdiction(string id)
    {
        var ptr = NativeMethods.stateset_tax_jurisdiction_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<TaxJurisdiction>(ptr);
    }

    public List<TaxJurisdiction> ListJurisdictions()
    {
        var ptr = NativeMethods.stateset_tax_jurisdiction_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<TaxJurisdiction>(ptr);
    }

    public TaxRate CreateRate(string jurisdictionId, string name, decimal rate)
    {
        var ptr = NativeMethods.stateset_tax_rate_create(_commerce.Handle, jurisdictionId, name, (double)rate);
        return StateSetCommerce.ParseJsonRequired<TaxRate>(ptr);
    }

    public TaxRate CreateRate(string country, decimal rate)
    {
        var jurisdiction = CreateJurisdiction($"{country} Tax", country, country);
        return CreateRate(jurisdiction.Id, $"{country} Tax Rate", rate) with { Country = country };
    }

    public TaxRate? GetRate(string id)
    {
        var ptr = NativeMethods.stateset_tax_rate_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<TaxRate>(ptr);
    }

    public List<TaxRate> ListRates()
    {
        var ptr = NativeMethods.stateset_tax_rate_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<TaxRate>(ptr);
    }

    public TaxExemption CreateExemption(string customerId, string exemptionType, string effectiveFrom)
    {
        var ptr = NativeMethods.stateset_tax_exemption_create(_commerce.Handle, customerId, exemptionType, effectiveFrom);
        return StateSetCommerce.ParseJsonRequired<TaxExemption>(ptr);
    }

    public List<TaxExemption> GetCustomerExemptions(string customerId)
    {
        var ptr = NativeMethods.stateset_tax_exemption_get_customer(_commerce.Handle, customerId);
        return StateSetCommerce.ParseJsonList<TaxExemption>(ptr);
    }

    public bool CustomerIsExempt(string customerId)
        => NativeMethods.stateset_tax_customer_is_exempt(_commerce.Handle, customerId) == 1;

    public TaxSettings GetSettings()
    {
        var ptr = NativeMethods.stateset_tax_get_settings(_commerce.Handle);
        return StateSetCommerce.ParseJsonRequired<TaxSettings>(ptr);
    }

    public TaxSettings SetEnabled(bool enabled)
    {
        var ptr = NativeMethods.stateset_tax_set_enabled(_commerce.Handle, enabled ? 1 : 0);
        return StateSetCommerce.ParseJsonRequired<TaxSettings>(ptr);
    }
}

/// <summary>
/// Quality API
/// </summary>
public sealed class QualityApi
{
    private readonly StateSetCommerce _commerce;
    internal QualityApi(StateSetCommerce commerce) => _commerce = commerce;

    public Inspection CreateInspection(string inspectionType, string referenceType, string referenceId)
    {
        var ptr = NativeMethods.stateset_quality_inspection_create(_commerce.Handle, inspectionType, referenceType, referenceId);
        return StateSetCommerce.ParseJsonRequired<Inspection>(ptr);
    }

    public Inspection? GetInspection(string id)
    {
        var ptr = NativeMethods.stateset_quality_inspection_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Inspection>(ptr);
    }

    public List<Inspection> ListInspections()
    {
        var ptr = NativeMethods.stateset_quality_inspection_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Inspection>(ptr);
    }

    public Inspection StartInspection(string id)
    {
        var ptr = NativeMethods.stateset_quality_inspection_start(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Inspection>(ptr);
    }

    public Inspection CompleteInspection(string id)
    {
        var ptr = NativeMethods.stateset_quality_inspection_complete(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Inspection>(ptr);
    }

    public Ncr CreateNcr(string source, string severity, string sku, int quantityAffected, string description)
    {
        var ptr = NativeMethods.stateset_quality_ncr_create(_commerce.Handle, source, severity, sku, quantityAffected, description);
        return StateSetCommerce.ParseJsonRequired<Ncr>(ptr);
    }

    public Ncr? GetNcr(string id)
    {
        var ptr = NativeMethods.stateset_quality_ncr_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Ncr>(ptr);
    }

    public List<Ncr> ListNcrs()
    {
        var ptr = NativeMethods.stateset_quality_ncr_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Ncr>(ptr);
    }

    public Ncr CloseNcr(string id)
    {
        var ptr = NativeMethods.stateset_quality_ncr_close(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Ncr>(ptr);
    }

    public QualityHold CreateHold(string sku, int quantityHeld, string reason, string holdType)
    {
        var ptr = NativeMethods.stateset_quality_hold_create(_commerce.Handle, sku, quantityHeld, reason, holdType);
        return StateSetCommerce.ParseJsonRequired<QualityHold>(ptr);
    }

    public QualityHold? GetHold(string id)
    {
        var ptr = NativeMethods.stateset_quality_hold_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<QualityHold>(ptr);
    }

    public List<QualityHold> ListHolds()
    {
        var ptr = NativeMethods.stateset_quality_hold_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<QualityHold>(ptr);
    }

    public QualityHold ReleaseHold(string id, string releasedBy)
    {
        var ptr = NativeMethods.stateset_quality_hold_release(_commerce.Handle, id, releasedBy);
        return StateSetCommerce.ParseJsonRequired<QualityHold>(ptr);
    }

    public List<QualityHold> GetActiveHolds()
    {
        var ptr = NativeMethods.stateset_quality_hold_get_active(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<QualityHold>(ptr);
    }
}

/// <summary>
/// Lots API
/// </summary>
public sealed class LotsApi
{
    private readonly StateSetCommerce _commerce;
    internal LotsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Lot Create(string sku, int quantityProduced)
    {
        var ptr = NativeMethods.stateset_lot_create(_commerce.Handle, sku, quantityProduced);
        return StateSetCommerce.ParseJsonRequired<Lot>(ptr);
    }

    public Lot? Get(string id)
    {
        var ptr = NativeMethods.stateset_lot_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Lot>(ptr);
    }

    public Lot? GetByNumber(string lotNumber)
    {
        var ptr = NativeMethods.stateset_lot_get_by_number(_commerce.Handle, lotNumber);
        return StateSetCommerce.ParseJson<Lot>(ptr);
    }

    public List<Lot> List()
    {
        var ptr = NativeMethods.stateset_lot_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Lot>(ptr);
    }

    public Lot CreateLot(string lotNumber, string sku, int quantity)
        => Create(sku, quantity) with { LotNumber = lotNumber };

    public List<Lot> ListLots() => List();

    public List<Lot> GetActiveLots(string sku)
    {
        var ptr = NativeMethods.stateset_lot_get_active(_commerce.Handle, sku);
        return StateSetCommerce.ParseJsonList<Lot>(ptr);
    }

    public Lot Quarantine(string id, string reason)
    {
        var ptr = NativeMethods.stateset_lot_quarantine(_commerce.Handle, id, reason);
        return StateSetCommerce.ParseJsonRequired<Lot>(ptr);
    }

    public Lot ReleaseQuarantine(string id)
    {
        var ptr = NativeMethods.stateset_lot_release_quarantine(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Lot>(ptr);
    }

    public List<Lot> GetExpiringLots(int days)
    {
        var ptr = NativeMethods.stateset_lot_get_expiring(_commerce.Handle, days);
        return StateSetCommerce.ParseJsonList<Lot>(ptr);
    }

    public List<Lot> GetExpiredLots()
    {
        var ptr = NativeMethods.stateset_lot_get_expired(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Lot>(ptr);
    }

    public List<Lot> GetQuarantined()
    {
        var ptr = NativeMethods.stateset_lot_get_quarantined(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Lot>(ptr);
    }
}

/// <summary>
/// Serials API
/// </summary>
public sealed class SerialsApi
{
    private readonly StateSetCommerce _commerce;
    internal SerialsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Serial Create(string sku, string? lotNumber = null)
    {
        var ptr = NativeMethods.stateset_serial_create(_commerce.Handle, sku, lotNumber);
        return StateSetCommerce.ParseJsonRequired<Serial>(ptr);
    }

    public Serial? Get(string id)
    {
        var ptr = NativeMethods.stateset_serial_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Serial>(ptr);
    }

    public Serial? GetBySerial(string serial)
    {
        var ptr = NativeMethods.stateset_serial_get_by_serial(_commerce.Handle, serial);
        return StateSetCommerce.ParseJson<Serial>(ptr);
    }

    public List<Serial> List()
    {
        var ptr = NativeMethods.stateset_serial_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Serial>(ptr);
    }

    public Serial RegisterSerial(string serialNumber, string sku)
        => Create(sku) with { SerialNumber = serialNumber };

    public List<Serial> ListSerials() => List();

    public List<Serial> GetAvailable(string sku, int limit)
    {
        var ptr = NativeMethods.stateset_serial_get_available(_commerce.Handle, sku, limit);
        return StateSetCommerce.ParseJsonList<Serial>(ptr);
    }

    public Serial MarkSold(string id, string customerId, string? orderId = null)
    {
        var ptr = NativeMethods.stateset_serial_mark_sold(_commerce.Handle, id, customerId, orderId);
        return StateSetCommerce.ParseJsonRequired<Serial>(ptr);
    }

    public Serial Quarantine(string id, string reason)
    {
        var ptr = NativeMethods.stateset_serial_quarantine(_commerce.Handle, id, reason);
        return StateSetCommerce.ParseJsonRequired<Serial>(ptr);
    }

    public bool IsAvailable(string serial) => NativeMethods.stateset_serial_is_available(_commerce.Handle, serial) == 1;
}

/// <summary>
/// Warehouse API
/// </summary>
public sealed class WarehouseApi
{
    private readonly StateSetCommerce _commerce;
    internal WarehouseApi(StateSetCommerce commerce) => _commerce = commerce;

    public Warehouse CreateWarehouse(string code, string name, string warehouseType = "standard")
    {
        var ptr = NativeMethods.stateset_warehouse_create(_commerce.Handle, code, name, warehouseType);
        return StateSetCommerce.ParseJsonRequired<Warehouse>(ptr);
    }

    public Warehouse? GetWarehouse(int id)
    {
        var ptr = NativeMethods.stateset_warehouse_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Warehouse>(ptr);
    }

    public Warehouse? GetWarehouseByCode(string code)
    {
        var ptr = NativeMethods.stateset_warehouse_get_by_code(_commerce.Handle, code);
        return StateSetCommerce.ParseJson<Warehouse>(ptr);
    }

    public List<Warehouse> ListWarehouses()
    {
        var ptr = NativeMethods.stateset_warehouse_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Warehouse>(ptr);
    }

    public Location CreateLocation(int warehouseId, string locationType, string? zone = null, string? aisle = null)
    {
        var ptr = NativeMethods.stateset_location_create(_commerce.Handle, warehouseId, locationType, zone, aisle);
        return StateSetCommerce.ParseJsonRequired<Location>(ptr);
    }

    public Location? GetLocation(int id)
    {
        var ptr = NativeMethods.stateset_location_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Location>(ptr);
    }

    public List<Location> ListLocations(int? warehouseId = null)
    {
        var ptr = NativeMethods.stateset_location_list(_commerce.Handle, warehouseId ?? -1);
        return StateSetCommerce.ParseJsonList<Location>(ptr);
    }

    public List<Location> GetPickableLocations(int warehouseId, string sku)
    {
        var ptr = NativeMethods.stateset_location_get_pickable(_commerce.Handle, warehouseId, sku);
        return StateSetCommerce.ParseJsonList<Location>(ptr);
    }

    public int GetTotalAvailable(int warehouseId, string sku)
        => NativeMethods.stateset_warehouse_get_total_available(_commerce.Handle, warehouseId, sku);
}

/// <summary>
/// Receiving API
/// </summary>
public sealed class ReceivingApi
{
    private readonly StateSetCommerce _commerce;
    internal ReceivingApi(StateSetCommerce commerce) => _commerce = commerce;

    public Receipt CreateReceipt(string receiptType, int warehouseId, string? purchaseOrderId = null)
    {
        var ptr = NativeMethods.stateset_receipt_create(_commerce.Handle, receiptType, warehouseId, purchaseOrderId);
        return StateSetCommerce.ParseJsonRequired<Receipt>(ptr);
    }

    public Receipt? GetReceipt(string id)
    {
        var ptr = NativeMethods.stateset_receipt_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Receipt>(ptr);
    }

    public Receipt? GetReceiptByNumber(string number)
    {
        var ptr = NativeMethods.stateset_receipt_get_by_number(_commerce.Handle, number);
        return StateSetCommerce.ParseJson<Receipt>(ptr);
    }

    public List<Receipt> ListReceipts()
    {
        var ptr = NativeMethods.stateset_receipt_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Receipt>(ptr);
    }

    public Receipt StartReceiving(string id)
    {
        var ptr = NativeMethods.stateset_receipt_start(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Receipt>(ptr);
    }

    public Receipt CompleteReceiving(string id)
    {
        var ptr = NativeMethods.stateset_receipt_complete(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Receipt>(ptr);
    }

    public Receipt CancelReceipt(string id)
    {
        var ptr = NativeMethods.stateset_receipt_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Receipt>(ptr);
    }

    public Receipt CreateReceiptFromPo(string poId, int warehouseId)
    {
        var ptr = NativeMethods.stateset_receipt_create_from_po(_commerce.Handle, poId, warehouseId);
        return StateSetCommerce.ParseJsonRequired<Receipt>(ptr);
    }
}

/// <summary>
/// Fulfillment API
/// </summary>
public sealed class FulfillmentApi
{
    private readonly StateSetCommerce _commerce;
    internal FulfillmentApi(StateSetCommerce commerce) => _commerce = commerce;

    public Wave CreateWave(int warehouseId, IEnumerable<string> orderIds, int priority = 0)
    {
        var orderIdsJson = JsonSerializer.Serialize(orderIds);
        var ptr = NativeMethods.stateset_wave_create(_commerce.Handle, warehouseId, orderIdsJson, priority);
        return StateSetCommerce.ParseJsonRequired<Wave>(ptr);
    }

    public Wave? GetWave(string id)
    {
        var ptr = NativeMethods.stateset_wave_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Wave>(ptr);
    }

    public List<Wave> ListWaves()
    {
        var ptr = NativeMethods.stateset_wave_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Wave>(ptr);
    }

    public Wave ReleaseWave(string id)
    {
        var ptr = NativeMethods.stateset_wave_release(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Wave>(ptr);
    }

    public Wave CompleteWave(string id)
    {
        var ptr = NativeMethods.stateset_wave_complete(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Wave>(ptr);
    }

    public Wave CancelWave(string id)
    {
        var ptr = NativeMethods.stateset_wave_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Wave>(ptr);
    }

    public PickTask? GetPick(string id)
    {
        var ptr = NativeMethods.stateset_pick_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<PickTask>(ptr);
    }

    public List<PickTask> ListPicks()
    {
        var ptr = NativeMethods.stateset_pick_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<PickTask>(ptr);
    }

    public List<PickTask> ListPickLists() => ListPicks();

    public PickTask AssignPick(string id, string assignedTo)
    {
        var ptr = NativeMethods.stateset_pick_assign(_commerce.Handle, id, assignedTo);
        return StateSetCommerce.ParseJsonRequired<PickTask>(ptr);
    }

    public PickTask StartPick(string id)
    {
        var ptr = NativeMethods.stateset_pick_start(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<PickTask>(ptr);
    }

    public PickTask CancelPick(string id)
    {
        var ptr = NativeMethods.stateset_pick_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<PickTask>(ptr);
    }

    public bool IsOrderReadyToPack(string orderId)
        => NativeMethods.stateset_fulfillment_is_ready_to_pack(_commerce.Handle, orderId) == 1;

    public bool IsOrderReadyToShip(string orderId)
        => NativeMethods.stateset_fulfillment_is_ready_to_ship(_commerce.Handle, orderId) == 1;
}

/// <summary>
/// Accounts Payable API
/// </summary>
public sealed class AccountsPayableApi
{
    private readonly StateSetCommerce _commerce;
    internal AccountsPayableApi(StateSetCommerce commerce) => _commerce = commerce;

    public Bill CreateBill(string supplierId, string dueDate, string? paymentTerms = null)
    {
        var ptr = NativeMethods.stateset_ap_bill_create(_commerce.Handle, supplierId, dueDate, paymentTerms);
        return StateSetCommerce.ParseJsonRequired<Bill>(ptr);
    }

    public Bill? GetBill(string id)
    {
        var ptr = NativeMethods.stateset_ap_bill_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Bill>(ptr);
    }

    public Bill? GetBillByNumber(string number)
    {
        var ptr = NativeMethods.stateset_ap_bill_get_by_number(_commerce.Handle, number);
        return StateSetCommerce.ParseJson<Bill>(ptr);
    }

    public List<Bill> ListBills()
    {
        var ptr = NativeMethods.stateset_ap_bill_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Bill>(ptr);
    }

    public Bill ApproveBill(string id)
    {
        var ptr = NativeMethods.stateset_ap_bill_approve(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Bill>(ptr);
    }

    public Bill CancelBill(string id)
    {
        var ptr = NativeMethods.stateset_ap_bill_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Bill>(ptr);
    }

    public List<Bill> GetOverdueBills()
    {
        var ptr = NativeMethods.stateset_ap_bill_get_overdue(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Bill>(ptr);
    }

    public List<Bill> GetBillsDueSoon(int days)
    {
        var ptr = NativeMethods.stateset_ap_bill_get_due_soon(_commerce.Handle, days);
        return StateSetCommerce.ParseJsonList<Bill>(ptr);
    }

    public ApAgingSummary GetAgingSummary()
    {
        var ptr = NativeMethods.stateset_ap_aging_summary(_commerce.Handle);
        return StateSetCommerce.ParseJsonRequired<ApAgingSummary>(ptr);
    }

    public double GetTotalOutstanding() => NativeMethods.stateset_ap_total_outstanding(_commerce.Handle);
}

/// <summary>
/// Accounts Receivable API
/// </summary>
public sealed class AccountsReceivableApi
{
    private readonly StateSetCommerce _commerce;
    internal AccountsReceivableApi(StateSetCommerce commerce) => _commerce = commerce;

    public ArAgingSummary GetAgingSummary()
    {
        var ptr = NativeMethods.stateset_ar_aging_summary(_commerce.Handle);
        return StateSetCommerce.ParseJsonRequired<ArAgingSummary>(ptr);
    }

    public double GetTotalOutstanding() => NativeMethods.stateset_ar_total_outstanding(_commerce.Handle);

    public double GetDso(int days) => NativeMethods.stateset_ar_get_dso(_commerce.Handle, days);

    public CreditMemo CreateCreditMemo(string customerId, decimal amount, string reason)
    {
        var ptr = NativeMethods.stateset_ar_credit_memo_create(_commerce.Handle, customerId, (double)amount, reason);
        return StateSetCommerce.ParseJsonRequired<CreditMemo>(ptr);
    }

    public CreditMemo? GetCreditMemo(string id)
    {
        var ptr = NativeMethods.stateset_ar_credit_memo_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<CreditMemo>(ptr);
    }

    public List<CreditMemo> ListCreditMemos()
    {
        var ptr = NativeMethods.stateset_ar_credit_memo_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<CreditMemo>(ptr);
    }

    public List<CreditMemo> ListReceivables() => ListCreditMemos();

    public CreditMemo VoidCreditMemo(string id)
    {
        var ptr = NativeMethods.stateset_ar_credit_memo_void(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<CreditMemo>(ptr);
    }

    public List<CreditMemo> GetUnappliedCredits(string customerId)
    {
        var ptr = NativeMethods.stateset_ar_get_unapplied_credits(_commerce.Handle, customerId);
        return StateSetCommerce.ParseJsonList<CreditMemo>(ptr);
    }
}

/// <summary>
/// Cost Accounting API
/// </summary>
public sealed class CostAccountingApi
{
    private readonly StateSetCommerce _commerce;
    internal CostAccountingApi(StateSetCommerce commerce) => _commerce = commerce;

    public ItemCost? GetItemCost(string sku)
    {
        var ptr = NativeMethods.stateset_cost_get_item_cost(_commerce.Handle, sku);
        return StateSetCommerce.ParseJson<ItemCost>(ptr);
    }

    public ItemCost SetItemCost(string sku, decimal standardCost, decimal? currentCost = null)
    {
        var ptr = NativeMethods.stateset_cost_set_item_cost(_commerce.Handle, sku, (double)standardCost, (double)(currentCost ?? standardCost));
        return StateSetCommerce.ParseJsonRequired<ItemCost>(ptr);
    }

    public List<ItemCost> ListItemCosts()
    {
        var ptr = NativeMethods.stateset_cost_list_item_costs(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<ItemCost>(ptr);
    }

    public List<ItemCost> ListCostEntries() => ListItemCosts();

    public ItemCost UpdateAverageCost(string sku, int quantity, decimal unitCost)
    {
        var ptr = NativeMethods.stateset_cost_update_average(_commerce.Handle, sku, quantity, (double)unitCost);
        return StateSetCommerce.ParseJsonRequired<ItemCost>(ptr);
    }

    public double GetTotalInventoryValue() => NativeMethods.stateset_cost_total_inventory_value(_commerce.Handle);
}

/// <summary>
/// Credit API
/// </summary>
public sealed class CreditApi
{
    private readonly StateSetCommerce _commerce;
    internal CreditApi(StateSetCommerce commerce) => _commerce = commerce;

    public CreditAccount CreateCreditAccount(string customerId, decimal creditLimit)
    {
        var ptr = NativeMethods.stateset_credit_account_create(_commerce.Handle, customerId, (double)creditLimit);
        return StateSetCommerce.ParseJsonRequired<CreditAccount>(ptr);
    }

    public CreditAccount SetCreditLimit(string customerId, decimal limit, string currency = "USD")
        => CreateCreditAccount(customerId, limit);

    public CreditAccount? GetCreditAccount(string id)
    {
        var ptr = NativeMethods.stateset_credit_account_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<CreditAccount>(ptr);
    }

    public CreditAccount? GetCreditAccountByCustomer(string customerId)
    {
        var ptr = NativeMethods.stateset_credit_account_get_by_customer(_commerce.Handle, customerId);
        return StateSetCommerce.ParseJson<CreditAccount>(ptr);
    }

    public CreditAccount? GetCreditLimit(string customerId) => GetCreditAccountByCustomer(customerId);

    public List<CreditAccount> ListCreditAccounts()
    {
        var ptr = NativeMethods.stateset_credit_account_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<CreditAccount>(ptr);
    }

    public CreditCheck CheckCredit(string customerId, decimal orderAmount)
    {
        var ptr = NativeMethods.stateset_credit_check(_commerce.Handle, customerId, (double)orderAmount);
        return StateSetCommerce.ParseJsonRequired<CreditCheck>(ptr);
    }

    public CreditAccount AdjustCreditLimit(string customerId, decimal newLimit, string reason)
    {
        var ptr = NativeMethods.stateset_credit_adjust_limit(_commerce.Handle, customerId, (double)newLimit, reason);
        return StateSetCommerce.ParseJsonRequired<CreditAccount>(ptr);
    }

    public CreditAccount SuspendCreditAccount(string customerId, string reason)
    {
        var ptr = NativeMethods.stateset_credit_account_suspend(_commerce.Handle, customerId, reason);
        return StateSetCommerce.ParseJsonRequired<CreditAccount>(ptr);
    }

    public CreditAccount ReactivateCreditAccount(string customerId)
    {
        var ptr = NativeMethods.stateset_credit_account_reactivate(_commerce.Handle, customerId);
        return StateSetCommerce.ParseJsonRequired<CreditAccount>(ptr);
    }

    public List<CreditAccount> GetOverLimitCustomers()
    {
        var ptr = NativeMethods.stateset_credit_get_over_limit(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<CreditAccount>(ptr);
    }
}

/// <summary>
/// Backorders API
/// </summary>
public sealed class BackordersApi
{
    private readonly StateSetCommerce _commerce;
    internal BackordersApi(StateSetCommerce commerce) => _commerce = commerce;

    public Backorder CreateBackorder(string orderId, string sku, int quantity, string? expectedDate = null)
    {
        var ptr = NativeMethods.stateset_backorder_create(_commerce.Handle, orderId, sku, quantity, expectedDate);
        return StateSetCommerce.ParseJsonRequired<Backorder>(ptr);
    }

    public Backorder? GetBackorder(string id)
    {
        var ptr = NativeMethods.stateset_backorder_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<Backorder>(ptr);
    }

    public Backorder? GetBackorderByNumber(string number)
    {
        var ptr = NativeMethods.stateset_backorder_get_by_number(_commerce.Handle, number);
        return StateSetCommerce.ParseJson<Backorder>(ptr);
    }

    public List<Backorder> ListBackorders()
    {
        var ptr = NativeMethods.stateset_backorder_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Backorder>(ptr);
    }

    public Backorder CancelBackorder(string id)
    {
        var ptr = NativeMethods.stateset_backorder_cancel(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<Backorder>(ptr);
    }

    public List<Backorder> GetBackordersForOrder(string orderId)
    {
        var ptr = NativeMethods.stateset_backorder_get_for_order(_commerce.Handle, orderId);
        return StateSetCommerce.ParseJsonList<Backorder>(ptr);
    }

    public List<Backorder> GetBackordersForSku(string sku)
    {
        var ptr = NativeMethods.stateset_backorder_get_for_sku(_commerce.Handle, sku);
        return StateSetCommerce.ParseJsonList<Backorder>(ptr);
    }

    public List<Backorder> GetOverdueBackorders()
    {
        var ptr = NativeMethods.stateset_backorder_get_overdue(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<Backorder>(ptr);
    }

    public BackorderSummary GetSummary()
    {
        var ptr = NativeMethods.stateset_backorder_summary(_commerce.Handle);
        return StateSetCommerce.ParseJsonRequired<BackorderSummary>(ptr);
    }

    public int CountPending() => NativeMethods.stateset_backorder_count_pending(_commerce.Handle);
}

/// <summary>
/// General Ledger API
/// </summary>
public sealed class GeneralLedgerApi
{
    private readonly StateSetCommerce _commerce;
    internal GeneralLedgerApi(StateSetCommerce commerce) => _commerce = commerce;

    public GlAccount CreateAccount(string accountNumber, string name, string accountType)
    {
        var ptr = NativeMethods.stateset_gl_account_create(_commerce.Handle, accountNumber, name, accountType);
        return StateSetCommerce.ParseJsonRequired<GlAccount>(ptr);
    }

    public GlAccount? GetAccount(string id)
    {
        var ptr = NativeMethods.stateset_gl_account_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<GlAccount>(ptr);
    }

    public GlAccount? GetAccountByNumber(string accountNumber)
    {
        var ptr = NativeMethods.stateset_gl_account_get_by_number(_commerce.Handle, accountNumber);
        return StateSetCommerce.ParseJson<GlAccount>(ptr);
    }

    public List<GlAccount> ListAccounts()
    {
        var ptr = NativeMethods.stateset_gl_account_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<GlAccount>(ptr);
    }

    public List<GlAccount> InitializeChartOfAccounts()
    {
        var ptr = NativeMethods.stateset_gl_initialize_coa(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<GlAccount>(ptr);
    }

    public JournalEntry? GetJournalEntry(string id)
    {
        var ptr = NativeMethods.stateset_gl_journal_entry_get(_commerce.Handle, id);
        return StateSetCommerce.ParseJson<JournalEntry>(ptr);
    }

    public List<JournalEntry> ListJournalEntries()
    {
        var ptr = NativeMethods.stateset_gl_journal_entry_list(_commerce.Handle);
        return StateSetCommerce.ParseJsonList<JournalEntry>(ptr);
    }

    public JournalEntry PostJournalEntry(string id, string postedBy)
    {
        var ptr = NativeMethods.stateset_gl_journal_entry_post(_commerce.Handle, id, postedBy);
        return StateSetCommerce.ParseJsonRequired<JournalEntry>(ptr);
    }

    public JournalEntry VoidJournalEntry(string id)
    {
        var ptr = NativeMethods.stateset_gl_journal_entry_void(_commerce.Handle, id);
        return StateSetCommerce.ParseJsonRequired<JournalEntry>(ptr);
    }

    public TrialBalance GetTrialBalance(string asOfDate)
    {
        var ptr = NativeMethods.stateset_gl_trial_balance(_commerce.Handle, asOfDate);
        return StateSetCommerce.ParseJsonRequired<TrialBalance>(ptr);
    }

    public BalanceSheet GetBalanceSheet(string asOfDate)
    {
        var ptr = NativeMethods.stateset_gl_balance_sheet(_commerce.Handle, asOfDate);
        return StateSetCommerce.ParseJsonRequired<BalanceSheet>(ptr);
    }

    public IncomeStatement GetIncomeStatement(string startDate, string endDate)
    {
        var ptr = NativeMethods.stateset_gl_income_statement(_commerce.Handle, startDate, endDate);
        return StateSetCommerce.ParseJsonRequired<IncomeStatement>(ptr);
    }

    public double GetAccountBalance(string accountId, string? asOfDate = null)
        => NativeMethods.stateset_gl_account_balance(_commerce.Handle, accountId, asOfDate);
}
