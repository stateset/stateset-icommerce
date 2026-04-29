using System.Globalization;

namespace StateSet.Embedded;

internal sealed class StateSetDotNetStore
{
    private int _nextId;
    private int _nextIntId;

    internal List<Customer> Customers { get; } = new();
    internal List<Product> Products { get; } = new();
    internal Dictionary<string, InventoryItem> InventoryItems { get; } = new();
    internal Dictionary<string, StockLevel> StockLevels { get; } = new();
    internal List<Cart> Carts { get; } = new();
    internal List<Order> Orders { get; } = new();
    internal List<Return> Returns { get; } = new();
    internal List<Payment> Payments { get; } = new();
    internal List<Refund> Refunds { get; } = new();
    internal List<Shipment> Shipments { get; } = new();
    internal List<Warranty> Warranties { get; } = new();
    internal List<WarrantyClaim> WarrantyClaims { get; } = new();
    internal List<Supplier> Suppliers { get; } = new();
    internal List<PurchaseOrder> PurchaseOrders { get; } = new();
    internal List<Invoice> Invoices { get; } = new();
    internal List<BillOfMaterials> Boms { get; } = new();
    internal Dictionary<string, List<BomComponent>> BomComponents { get; } = new();
    internal List<WorkOrder> WorkOrders { get; } = new();
    internal Dictionary<string, ExchangeRate> ExchangeRates { get; } = new();
    internal List<SubscriptionPlan> SubscriptionPlans { get; } = new();
    internal List<Subscription> Subscriptions { get; } = new();
    internal List<Promotion> Promotions { get; } = new();
    internal List<Coupon> Coupons { get; } = new();
    internal List<TaxJurisdiction> TaxJurisdictions { get; } = new();
    internal List<TaxRate> TaxRates { get; } = new();
    internal List<TaxExemption> TaxExemptions { get; } = new();
    internal List<Inspection> Inspections { get; } = new();
    internal List<Ncr> Ncrs { get; } = new();
    internal List<QualityHold> QualityHolds { get; } = new();
    internal List<Lot> Lots { get; } = new();
    internal List<Serial> Serials { get; } = new();
    internal List<Warehouse> Warehouses { get; } = new();
    internal List<Location> Locations { get; } = new();
    internal List<Receipt> Receipts { get; } = new();
    internal List<Wave> Waves { get; } = new();
    internal List<PickTask> Picks { get; } = new();
    internal List<Bill> Bills { get; } = new();
    internal List<CreditMemo> CreditMemos { get; } = new();
    internal List<ItemCost> ItemCosts { get; } = new();
    internal List<CreditAccount> CreditAccounts { get; } = new();
    internal List<Backorder> Backorders { get; } = new();
    internal List<GlAccount> GlAccounts { get; } = new();
    internal List<JournalEntry> JournalEntries { get; } = new();

    internal string Id(string prefix) => $"{prefix}_{++_nextId}";
    internal int IntId() => ++_nextIntId;
}

internal static class StateSetDotNetApiSupport
{
    internal static string Now() => DateTimeOffset.UtcNow.ToString("O", CultureInfo.InvariantCulture);

    internal static string Amount(decimal amount) =>
        amount.ToString("0.00", CultureInfo.InvariantCulture);

    internal static string Amount(double amount) =>
        amount.ToString("0.00", CultureInfo.InvariantCulture);

    internal static string PaymentMethodName(PaymentMethod method) => method switch
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

    internal static string ReturnReasonName(ReturnReason reason) => reason switch
    {
        ReturnReason.Defective => "defective",
        ReturnReason.WrongItem => "wrong_item",
        ReturnReason.NotAsDescribed => "not_as_described",
        ReturnReason.ChangedMind => "changed_mind",
        ReturnReason.Damaged => "damaged",
        _ => "other"
    };
}

/// <summary>Customers API.</summary>
public sealed class CustomersApi
{
    private readonly StateSetCommerce _commerce;

    internal CustomersApi(StateSetCommerce commerce) => _commerce = commerce;

    public Customer Create(string email, string firstName, string lastName, string? phone = null)
    {
        var customer = new Customer
        {
            Id = _commerce.Store.Id("cust"),
            Email = email,
            FirstName = firstName,
            LastName = lastName,
            Phone = phone,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Customers.Add(customer);
        return customer;
    }

    public Customer? Get(string id) => _commerce.Store.Customers.FirstOrDefault(customer => customer.Id == id);

    public List<Customer> List() => _commerce.Store.Customers.ToList();

    public bool Delete(string id)
    {
        var removed = _commerce.Store.Customers.RemoveAll(customer => customer.Id == id);
        return removed > 0;
    }

    public int Count() => _commerce.Store.Customers.Count;
}

/// <summary>Products API.</summary>
public sealed class ProductsApi
{
    private readonly StateSetCommerce _commerce;

    internal ProductsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Product Create(string name, string sku, decimal price, string? description = null)
    {
        var product = new Product
        {
            Id = _commerce.Store.Id("prod"),
            Name = name,
            Description = description,
            IsActive = true,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Products.Add(product);
        return product;
    }

    public Product? Get(string id) => _commerce.Store.Products.FirstOrDefault(product => product.Id == id);

    public List<Product> List() => _commerce.Store.Products.ToList();
}

/// <summary>Orders API.</summary>
public sealed class OrdersApi
{
    private readonly StateSetCommerce _commerce;

    internal OrdersApi(StateSetCommerce commerce) => _commerce = commerce;

    public Order Create(string customerId, IEnumerable<OrderItem> items, string currency = "USD")
    {
        var orderItems = items.ToList();
        var total = orderItems.Sum(item => item.Quantity * (decimal)item.UnitPrice);
        var id = _commerce.Store.Id("ord");
        var order = new Order
        {
            Id = id,
            OrderNumber = id.ToUpperInvariant(),
            CustomerId = customerId,
            Status = "pending",
            TotalAmount = StateSetDotNetApiSupport.Amount(total),
            Currency = currency,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Orders.Add(order);
        return order;
    }

    public Order? Get(string id) => _commerce.Store.Orders.FirstOrDefault(order => order.Id == id);

    public List<Order> List() => _commerce.Store.Orders.ToList();

    public Order UpdateStatus(string id, OrderStatus status) =>
        Replace(id, status.ToString().ToLowerInvariant());

    public Order Ship(string id) => Replace(id, "shipped");

    public Order Cancel(string id) => Replace(id, "cancelled");

    private Order Replace(string id, string status)
    {
        var index = _commerce.Store.Orders.FindIndex(order => order.Id == id);
        if (index < 0) throw new StateSetException($"Order not found: {id}");
        var updated = _commerce.Store.Orders[index] with
        {
            Status = status,
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Orders[index] = updated;
        return updated;
    }
}

/// <summary>Inventory API.</summary>
public sealed class InventoryApi
{
    private readonly StateSetCommerce _commerce;

    internal InventoryApi(StateSetCommerce commerce) => _commerce = commerce;

    public InventoryItem CreateItem(string sku, string name, decimal initialQuantity = 0)
    {
        var item = new InventoryItem
        {
            Id = _commerce.Store.Id("inv"),
            Sku = sku,
            Name = name,
            CreatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.InventoryItems[sku] = item;
        _commerce.Store.StockLevels[sku] = new StockLevel
        {
            Id = _commerce.Store.Id("stock"),
            InventoryItemId = item.Id,
            Available = StateSetDotNetApiSupport.Amount(initialQuantity),
            Reserved = "0",
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        return item;
    }

    public bool Adjust(string sku, decimal quantityDelta, string reason = "manual adjustment")
    {
        if (!_commerce.Store.StockLevels.TryGetValue(sku, out var current)) return false;
        var available = decimal.TryParse(current.Available, NumberStyles.Any, CultureInfo.InvariantCulture, out var parsed)
            ? parsed
            : 0m;
        _commerce.Store.StockLevels[sku] = current with
        {
            Available = StateSetDotNetApiSupport.Amount(available + quantityDelta),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        return true;
    }

    public StockLevel? GetLevel(string sku) =>
        _commerce.Store.StockLevels.TryGetValue(sku, out var level) ? level : null;
}

/// <summary>Carts API.</summary>
public sealed class CartsApi
{
    private readonly StateSetCommerce _commerce;

    internal CartsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Cart Create(string? customerId = null, string currency = "USD")
    {
        var cart = new Cart
        {
            Id = _commerce.Store.Id("cart"),
            CustomerId = customerId,
            Status = "active",
            GrandTotal = "0.00",
            Currency = currency,
            CreatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Carts.Add(cart);
        return cart;
    }

    public Cart AddItem(string cartId, string variantId, int quantity = 1) =>
        Get(cartId) ?? throw new StateSetException($"Cart not found: {cartId}");

    public Cart AddItem(string cartId, string sku, string name, int quantity, decimal unitPrice) =>
        AddItem(cartId, sku, quantity);

    public Cart? Get(string cartId) => _commerce.Store.Carts.FirstOrDefault(cart => cart.Id == cartId);
}

/// <summary>Returns API.</summary>
public sealed class ReturnsApi
{
    private readonly StateSetCommerce _commerce;

    internal ReturnsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Return Create(string orderId, ReturnReason reason, string? notes = null)
    {
        var ret = new Return
        {
            Id = _commerce.Store.Id("ret"),
            OrderId = orderId,
            Reason = StateSetDotNetApiSupport.ReturnReasonName(reason),
            Status = "requested",
            Notes = notes,
            CreatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Returns.Add(ret);
        return ret;
    }

    public List<Return> List() => _commerce.Store.Returns.ToList();

    public Return? Get(string id) => _commerce.Store.Returns.FirstOrDefault(ret => ret.Id == id);

    public Return Approve(string id) => Replace(id, "approved");

    public Return Reject(string id, string reason) => Replace(id, "rejected");

    public Return Complete(string id) => Replace(id, "completed");

    private Return Replace(string id, string status)
    {
        var index = _commerce.Store.Returns.FindIndex(ret => ret.Id == id);
        if (index < 0) throw new StateSetException($"Return not found: {id}");
        var updated = _commerce.Store.Returns[index] with { Status = status };
        _commerce.Store.Returns[index] = updated;
        return updated;
    }
}

/// <summary>Payments API.</summary>
public sealed class PaymentsApi
{
    private readonly StateSetCommerce _commerce;

    internal PaymentsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Payment Create(string orderId, decimal amount, string currency = "USD", PaymentMethod method = PaymentMethod.CreditCard)
    {
        var payment = new Payment
        {
            Id = _commerce.Store.Id("pay"),
            OrderId = orderId,
            Amount = StateSetDotNetApiSupport.Amount(amount),
            Currency = currency,
            Method = StateSetDotNetApiSupport.PaymentMethodName(method),
            Status = "pending",
            CreatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Payments.Add(payment);
        return payment;
    }

    public Payment? Get(string id) => _commerce.Store.Payments.FirstOrDefault(payment => payment.Id == id);

    public List<Payment> List() => _commerce.Store.Payments.ToList();

    public Payment Complete(string id) => Replace(id, "completed");

    public Payment Fail(string id, string reason) => Replace(id, "failed");

    public Refund Refund(string paymentId, decimal amount, string reason)
    {
        var refund = new Refund
        {
            Id = _commerce.Store.Id("refund"),
            RefundNumber = _commerce.Store.Id("rf"),
            PaymentId = paymentId,
            Amount = StateSetDotNetApiSupport.Amount(amount),
            Status = "completed",
            Reason = reason,
            CreatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Refunds.Add(refund);
        return refund;
    }

    private Payment Replace(string id, string status)
    {
        var index = _commerce.Store.Payments.FindIndex(payment => payment.Id == id);
        if (index < 0) throw new StateSetException($"Payment not found: {id}");
        var updated = _commerce.Store.Payments[index] with { Status = status };
        _commerce.Store.Payments[index] = updated;
        return updated;
    }
}

/// <summary>Analytics API.</summary>
public sealed class AnalyticsApi
{
    private readonly StateSetCommerce _commerce;

    internal AnalyticsApi(StateSetCommerce commerce) => _commerce = commerce;

    public SalesSummary GetSalesSummary(TimePeriod period = TimePeriod.Month)
    {
        var revenue = _commerce.Store.Orders.Sum(order =>
            decimal.TryParse(order.TotalAmount, NumberStyles.Any, CultureInfo.InvariantCulture, out var amount) ? amount : 0m);
        var count = _commerce.Store.Orders.Count;
        return new SalesSummary
        {
            TotalRevenue = StateSetDotNetApiSupport.Amount(revenue),
            OrderCount = count,
            AverageOrderValue = StateSetDotNetApiSupport.Amount(count == 0 ? 0 : revenue / count)
        };
    }

    public SalesSummary SalesSummary(TimePeriod period = TimePeriod.Month) => GetSalesSummary(period);

    public List<TopProduct> GetTopProducts(int limit = 10) => new();

    public List<TopProduct> TopProducts(int limit = 10) => GetTopProducts(limit);

    public List<TopCustomer> GetTopCustomers(int limit = 10) => new();

    public List<TopCustomer> TopCustomers(int limit = 10) => GetTopCustomers(limit);
}

/// <summary>Shipments API.</summary>
public sealed class ShipmentsApi
{
    private readonly StateSetCommerce _commerce;

    internal ShipmentsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Shipment Create(string orderId, string recipientName, string shippingAddress, string carrier = "")
    {
        var id = _commerce.Store.Id("ship");
        var shipment = new Shipment
        {
            Id = id,
            ShipmentNumber = id.ToUpperInvariant(),
            OrderId = orderId,
            Status = "pending",
            Carrier = carrier,
            RecipientName = recipientName,
            ShippingAddress = shippingAddress,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Shipments.Add(shipment);
        return shipment;
    }

    public Shipment? Get(string id) => _commerce.Store.Shipments.FirstOrDefault(shipment => shipment.Id == id);

    public List<Shipment> List() => _commerce.Store.Shipments.ToList();

    public Shipment Ship(string id, string trackingNumber) => Replace(id, "shipped", trackingNumber);

    public Shipment Deliver(string id) => Replace(id, "delivered");

    public Shipment Cancel(string id) => Replace(id, "cancelled");

    private Shipment Replace(string id, string status, string? trackingNumber = null)
    {
        var index = _commerce.Store.Shipments.FindIndex(shipment => shipment.Id == id);
        if (index < 0) throw new StateSetException($"Shipment not found: {id}");
        var updated = _commerce.Store.Shipments[index] with
        {
            Status = status,
            TrackingNumber = trackingNumber ?? _commerce.Store.Shipments[index].TrackingNumber,
            ShippedAt = status == "shipped" ? StateSetDotNetApiSupport.Now() : _commerce.Store.Shipments[index].ShippedAt,
            DeliveredAt = status == "delivered" ? StateSetDotNetApiSupport.Now() : _commerce.Store.Shipments[index].DeliveredAt,
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Shipments[index] = updated;
        return updated;
    }
}

/// <summary>Warranties API.</summary>
public sealed class WarrantiesApi
{
    private readonly StateSetCommerce _commerce;

    internal WarrantiesApi(StateSetCommerce commerce) => _commerce = commerce;

    public Warranty Create(string customerId, string productId, WarrantyType warrantyType, int durationMonths)
    {
        var id = _commerce.Store.Id("warranty");
        var warranty = new Warranty
        {
            Id = id,
            WarrantyNumber = id.ToUpperInvariant(),
            CustomerId = customerId,
            ProductId = productId,
            Status = "active",
            WarrantyType = warrantyType.ToString().ToLowerInvariant(),
            DurationMonths = durationMonths,
            StartDate = DateTime.UtcNow.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture),
            EndDate = DateTime.UtcNow.AddMonths(durationMonths).ToString("yyyy-MM-dd", CultureInfo.InvariantCulture),
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Warranties.Add(warranty);
        return warranty;
    }

    public Warranty? Get(string id) => _commerce.Store.Warranties.FirstOrDefault(warranty => warranty.Id == id);

    public List<Warranty> List() => _commerce.Store.Warranties.ToList();

    public WarrantyClaim CreateClaim(string warrantyId, string issueDescription)
    {
        var id = _commerce.Store.Id("claim");
        var claim = new WarrantyClaim
        {
            Id = id,
            ClaimNumber = id.ToUpperInvariant(),
            WarrantyId = warrantyId,
            Status = "pending",
            IssueDescription = issueDescription,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.WarrantyClaims.Add(claim);
        return claim;
    }

    public WarrantyClaim ApproveClaim(string claimId) => ReplaceClaim(claimId, "approved");

    public WarrantyClaim DenyClaim(string claimId, string reason) => ReplaceClaim(claimId, "denied", reason);

    public WarrantyClaim CompleteClaim(string claimId, ClaimResolution resolution) =>
        ReplaceClaim(claimId, "completed");

    private WarrantyClaim ReplaceClaim(string id, string status, string? denialReason = null)
    {
        var index = _commerce.Store.WarrantyClaims.FindIndex(claim => claim.Id == id);
        if (index < 0) throw new StateSetException($"Warranty claim not found: {id}");
        var updated = _commerce.Store.WarrantyClaims[index] with
        {
            Status = status,
            DenialReason = denialReason,
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.WarrantyClaims[index] = updated;
        return updated;
    }
}

/// <summary>Suppliers API.</summary>
public sealed class SuppliersApi
{
    private readonly StateSetCommerce _commerce;

    internal SuppliersApi(StateSetCommerce commerce) => _commerce = commerce;

    public Supplier Create(string name, string email, string phone = "")
    {
        var supplier = new Supplier
        {
            Id = _commerce.Store.Id("sup"),
            Name = name,
            Email = email,
            Phone = phone,
            IsActive = true,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Suppliers.Add(supplier);
        return supplier;
    }

    public Supplier? Get(string id) => _commerce.Store.Suppliers.FirstOrDefault(supplier => supplier.Id == id);

    public List<Supplier> List() => _commerce.Store.Suppliers.ToList();
}

/// <summary>Purchase orders API.</summary>
public sealed class PurchaseOrdersApi
{
    private readonly StateSetCommerce _commerce;

    internal PurchaseOrdersApi(StateSetCommerce commerce) => _commerce = commerce;

    public PurchaseOrder Create(string supplierId, IEnumerable<PurchaseOrderItem> items)
    {
        var total = items.Sum(item => (decimal)item.Quantity * (decimal)item.UnitCost);
        var id = _commerce.Store.Id("po");
        var po = new PurchaseOrder
        {
            Id = id,
            PoNumber = id.ToUpperInvariant(),
            SupplierId = supplierId,
            Status = "draft",
            Subtotal = StateSetDotNetApiSupport.Amount(total),
            Total = StateSetDotNetApiSupport.Amount(total),
            Currency = "USD",
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.PurchaseOrders.Add(po);
        return po;
    }

    public PurchaseOrder? Get(string id) => _commerce.Store.PurchaseOrders.FirstOrDefault(po => po.Id == id);

    public List<PurchaseOrder> List() => _commerce.Store.PurchaseOrders.ToList();

    public PurchaseOrder Submit(string id) => Replace(id, "pending_approval");

    public PurchaseOrder Approve(string id, string approvedBy) => Replace(id, "approved") with { ApprovedBy = approvedBy };

    public PurchaseOrder Send(string id) => Replace(id, "sent");

    public PurchaseOrder Cancel(string id) => Replace(id, "cancelled");

    private PurchaseOrder Replace(string id, string status)
    {
        var index = _commerce.Store.PurchaseOrders.FindIndex(po => po.Id == id);
        if (index < 0) throw new StateSetException($"Purchase order not found: {id}");
        var updated = _commerce.Store.PurchaseOrders[index] with
        {
            Status = status,
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.PurchaseOrders[index] = updated;
        return updated;
    }
}

/// <summary>Invoices API.</summary>
public sealed class InvoicesApi
{
    private readonly StateSetCommerce _commerce;

    internal InvoicesApi(StateSetCommerce commerce) => _commerce = commerce;

    public Invoice Create(string customerId, IEnumerable<InvoiceItem> items, string billingEmail = "")
    {
        var total = items.Sum(item => (decimal)item.Quantity * (decimal)item.UnitPrice);
        var id = _commerce.Store.Id("invn");
        var invoice = new Invoice
        {
            Id = id,
            InvoiceNumber = id.ToUpperInvariant(),
            CustomerId = customerId,
            Status = "draft",
            InvoiceType = "standard",
            Subtotal = StateSetDotNetApiSupport.Amount(total),
            Total = StateSetDotNetApiSupport.Amount(total),
            Currency = "USD",
            BillingEmail = billingEmail,
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Invoices.Add(invoice);
        return invoice;
    }

    public Invoice? Get(string id) => _commerce.Store.Invoices.FirstOrDefault(invoice => invoice.Id == id);

    public List<Invoice> List() => _commerce.Store.Invoices.ToList();

    public Invoice Send(string id) => Replace(id, "sent");

    public Invoice Void(string id) => Replace(id, "voided");

    public Invoice RecordPayment(string id, decimal amount, string paymentMethod) => Replace(id, "paid") with
    {
        AmountPaid = StateSetDotNetApiSupport.Amount(amount)
    };

    public List<Invoice> GetOverdue() => _commerce.Store.Invoices.Where(invoice => invoice.Status == "overdue").ToList();

    private Invoice Replace(string id, string status)
    {
        var index = _commerce.Store.Invoices.FindIndex(invoice => invoice.Id == id);
        if (index < 0) throw new StateSetException($"Invoice not found: {id}");
        var updated = _commerce.Store.Invoices[index] with
        {
            Status = status,
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Invoices[index] = updated;
        return updated;
    }
}

/// <summary>Bill of materials API.</summary>
public sealed class BomApi
{
    private readonly StateSetCommerce _commerce;

    internal BomApi(StateSetCommerce commerce) => _commerce = commerce;

    public BillOfMaterials Create(string productId, string name, string? description = null)
    {
        var id = _commerce.Store.Id("bom");
        var bom = new BillOfMaterials
        {
            Id = id,
            BomNumber = id.ToUpperInvariant(),
            ProductId = productId,
            Name = name,
            Description = description,
            Version = "1",
            Status = "draft",
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.Boms.Add(bom);
        return bom;
    }

    public BillOfMaterials? Get(string id) => _commerce.Store.Boms.FirstOrDefault(bom => bom.Id == id);

    public List<BillOfMaterials> List() => _commerce.Store.Boms.ToList();

    public BomComponent AddComponent(string bomId, string name, string componentSku, decimal quantity)
    {
        var component = new BomComponent
        {
            Id = _commerce.Store.Id("bomc"),
            BomId = bomId,
            Name = name,
            ComponentSku = componentSku,
            Quantity = StateSetDotNetApiSupport.Amount(quantity)
        };
        if (!_commerce.Store.BomComponents.TryGetValue(bomId, out var components))
        {
            components = new List<BomComponent>();
            _commerce.Store.BomComponents[bomId] = components;
        }
        components.Add(component);
        return component;
    }

    public List<BomComponent> GetComponents(string bomId) =>
        _commerce.Store.BomComponents.TryGetValue(bomId, out var components) ? components.ToList() : new List<BomComponent>();

    public BillOfMaterials Activate(string id)
    {
        var index = _commerce.Store.Boms.FindIndex(bom => bom.Id == id);
        if (index < 0) throw new StateSetException($"BOM not found: {id}");
        var updated = _commerce.Store.Boms[index] with { Status = "active" };
        _commerce.Store.Boms[index] = updated;
        return updated;
    }
}

/// <summary>Work orders API.</summary>
public sealed class WorkOrdersApi
{
    private readonly StateSetCommerce _commerce;

    internal WorkOrdersApi(StateSetCommerce commerce) => _commerce = commerce;

    public WorkOrder Create(string productId, decimal quantityToBuild, string? bomId = null)
    {
        var id = _commerce.Store.Id("wo");
        var workOrder = new WorkOrder
        {
            Id = id,
            WorkOrderNumber = id.ToUpperInvariant(),
            ProductId = productId,
            BomId = bomId,
            Status = "planned",
            Priority = "normal",
            QuantityToBuild = StateSetDotNetApiSupport.Amount(quantityToBuild),
            CreatedAt = StateSetDotNetApiSupport.Now(),
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.WorkOrders.Add(workOrder);
        return workOrder;
    }

    public WorkOrder? Get(string id) => _commerce.Store.WorkOrders.FirstOrDefault(workOrder => workOrder.Id == id);

    public List<WorkOrder> List() => _commerce.Store.WorkOrders.ToList();

    public WorkOrder Start(string id) => Replace(id, "in_progress");

    public WorkOrder Complete(string id, decimal quantityCompleted) => Replace(id, "completed") with
    {
        QuantityCompleted = StateSetDotNetApiSupport.Amount(quantityCompleted)
    };

    public WorkOrder Cancel(string id) => Replace(id, "cancelled");

    private WorkOrder Replace(string id, string status)
    {
        var index = _commerce.Store.WorkOrders.FindIndex(workOrder => workOrder.Id == id);
        if (index < 0) throw new StateSetException($"Work order not found: {id}");
        var updated = _commerce.Store.WorkOrders[index] with
        {
            Status = status,
            UpdatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.WorkOrders[index] = updated;
        return updated;
    }
}

/// <summary>Currency API.</summary>
public sealed class CurrencyApi
{
    private readonly StateSetCommerce _commerce;

    internal CurrencyApi(StateSetCommerce commerce) => _commerce = commerce;

    public ExchangeRate SetRate(CurrencyCode fromCurrency, CurrencyCode toCurrency, decimal rate)
    {
        var key = $"{fromCurrency}:{toCurrency}";
        var exchangeRate = new ExchangeRate
        {
            Id = _commerce.Store.Id("rate"),
            BaseCurrency = fromCurrency.ToString(),
            QuoteCurrency = toCurrency.ToString(),
            Rate = rate.ToString(CultureInfo.InvariantCulture),
            ValidFrom = StateSetDotNetApiSupport.Now(),
            CreatedAt = StateSetDotNetApiSupport.Now()
        };
        _commerce.Store.ExchangeRates[key] = exchangeRate;
        return exchangeRate;
    }

    public ExchangeRate? GetRate(CurrencyCode fromCurrency, CurrencyCode toCurrency) =>
        _commerce.Store.ExchangeRates.TryGetValue($"{fromCurrency}:{toCurrency}", out var rate) ? rate : null;

    public ConversionResult Convert(decimal amount, CurrencyCode fromCurrency, CurrencyCode toCurrency)
    {
        var rate = GetRate(fromCurrency, toCurrency);
        var multiplier = rate != null && decimal.TryParse(rate.Rate, NumberStyles.Any, CultureInfo.InvariantCulture, out var parsed)
            ? parsed
            : 1m;
        return new ConversionResult
        {
            FromCurrency = fromCurrency.ToString(),
            ToCurrency = toCurrency.ToString(),
            OriginalAmount = StateSetDotNetApiSupport.Amount(amount),
            ConvertedAmount = StateSetDotNetApiSupport.Amount(amount * multiplier),
            Rate = multiplier.ToString(CultureInfo.InvariantCulture),
            RateAt = StateSetDotNetApiSupport.Now()
        };
    }

    public StoreCurrencySettings GetSettings() => new()
    {
        BaseCurrency = "USD",
        EnabledCurrencies = Enum.GetNames<CurrencyCode>().ToList(),
        AutoConvert = false,
        RoundingMode = "half_up"
    };
}

/// <summary>Subscriptions API.</summary>
public sealed class SubscriptionsApi
{
    private readonly StateSetCommerce _commerce;

    internal SubscriptionsApi(StateSetCommerce commerce) => _commerce = commerce;

    public SubscriptionPlan CreatePlan(string code, string name, string interval, int intervalCount, decimal price, string currency = "USD")
    {
        var plan = new SubscriptionPlan
        {
            Id = _commerce.Store.Id("plan"),
            Code = code,
            Name = name
        };
        _commerce.Store.SubscriptionPlans.Add(plan);
        return plan;
    }

    public SubscriptionPlan? GetPlan(string id) => _commerce.Store.SubscriptionPlans.FirstOrDefault(plan => plan.Id == id);

    public List<SubscriptionPlan> ListPlans() => _commerce.Store.SubscriptionPlans.ToList();

    public SubscriptionPlan ActivatePlan(string id) => GetPlan(id) ?? throw new StateSetException($"Subscription plan not found: {id}");

    public SubscriptionPlan ArchivePlan(string id) => GetPlan(id) ?? throw new StateSetException($"Subscription plan not found: {id}");

    public Subscription Subscribe(string customerId, string planId)
    {
        var subscription = new Subscription
        {
            Id = _commerce.Store.Id("sub"),
            CustomerId = customerId,
            Status = "active"
        };
        _commerce.Store.Subscriptions.Add(subscription);
        return subscription;
    }

    public Subscription? Get(string id) => _commerce.Store.Subscriptions.FirstOrDefault(subscription => subscription.Id == id);

    public List<Subscription> List() => _commerce.Store.Subscriptions.ToList();

    public Subscription Pause(string id) => Replace(id, "paused");

    public Subscription Resume(string id) => Replace(id, "active");

    public Subscription Cancel(string id) => Replace(id, "cancelled");

    private Subscription Replace(string id, string status)
    {
        var index = _commerce.Store.Subscriptions.FindIndex(subscription => subscription.Id == id);
        if (index < 0) throw new StateSetException($"Subscription not found: {id}");
        var updated = _commerce.Store.Subscriptions[index] with { Status = status };
        _commerce.Store.Subscriptions[index] = updated;
        return updated;
    }
}

/// <summary>Promotions API.</summary>
public sealed class PromotionsApi
{
    private readonly StateSetCommerce _commerce;

    internal PromotionsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Promotion Create(string code, string name, string discountType, decimal discountValue)
    {
        var promotion = new Promotion
        {
            Id = _commerce.Store.Id("promo"),
            Code = code,
            Name = name
        };
        _commerce.Store.Promotions.Add(promotion);
        return promotion;
    }

    public Promotion? Get(string id) => _commerce.Store.Promotions.FirstOrDefault(promotion => promotion.Id == id);

    public Promotion? GetByCode(string code) => _commerce.Store.Promotions.FirstOrDefault(promotion => promotion.Code == code);

    public List<Promotion> List() => _commerce.Store.Promotions.ToList();

    public Promotion Activate(string id) => Get(id) ?? throw new StateSetException($"Promotion not found: {id}");

    public Promotion Deactivate(string id) => Get(id) ?? throw new StateSetException($"Promotion not found: {id}");

    public bool Delete(string id) => _commerce.Store.Promotions.RemoveAll(promotion => promotion.Id == id) > 0;

    public List<Promotion> GetActive() => _commerce.Store.Promotions.ToList();

    public Coupon CreateCoupon(string promotionId, string code, int maxUses = -1)
    {
        var coupon = new Coupon
        {
            Id = _commerce.Store.Id("coupon"),
            Code = code
        };
        _commerce.Store.Coupons.Add(coupon);
        return coupon;
    }

    public Coupon? GetCouponByCode(string code) => _commerce.Store.Coupons.FirstOrDefault(coupon => coupon.Code == code);

    public Coupon? ValidateCoupon(string code) => GetCouponByCode(code);
}

/// <summary>Tax API.</summary>
public sealed class TaxApi
{
    private readonly StateSetCommerce _commerce;
    private bool _enabled = true;

    internal TaxApi(StateSetCommerce commerce) => _commerce = commerce;

    public TaxCalculation Calculate(string lineItemsJson, string shippingCountry, string? shippingState = null) => new()
    {
        Subtotal = "0.00",
        TaxAmount = "0.00",
        Total = "0.00"
    };

    public double GetEffectiveRate(string country, string? state = null, string? category = null) => 0;

    public TaxJurisdiction CreateJurisdiction(string name, string code, string countryCode, string? stateCode = null)
    {
        var jurisdiction = new TaxJurisdiction
        {
            Id = _commerce.Store.Id("taxj"),
            Name = name
        };
        _commerce.Store.TaxJurisdictions.Add(jurisdiction);
        return jurisdiction;
    }

    public TaxJurisdiction? GetJurisdiction(string id) =>
        _commerce.Store.TaxJurisdictions.FirstOrDefault(jurisdiction => jurisdiction.Id == id);

    public List<TaxJurisdiction> ListJurisdictions() => _commerce.Store.TaxJurisdictions.ToList();

    public TaxRate CreateRate(string jurisdictionId, string name, decimal rate)
    {
        var taxRate = new TaxRate
        {
            Id = _commerce.Store.Id("taxr"),
            Rate = rate.ToString(CultureInfo.InvariantCulture)
        };
        _commerce.Store.TaxRates.Add(taxRate);
        return taxRate;
    }

    public TaxRate CreateRate(string country, decimal rate)
    {
        var taxRate = new TaxRate
        {
            Id = _commerce.Store.Id("taxr"),
            Country = country,
            Rate = rate.ToString(CultureInfo.InvariantCulture)
        };
        _commerce.Store.TaxRates.Add(taxRate);
        return taxRate;
    }

    public TaxRate? GetRate(string id) => _commerce.Store.TaxRates.FirstOrDefault(rate => rate.Id == id);

    public List<TaxRate> ListRates() => _commerce.Store.TaxRates.ToList();

    public TaxExemption CreateExemption(string customerId, string exemptionType, string effectiveFrom)
    {
        var exemption = new TaxExemption
        {
            Id = _commerce.Store.Id("taxe"),
            CustomerId = customerId
        };
        _commerce.Store.TaxExemptions.Add(exemption);
        return exemption;
    }

    public List<TaxExemption> GetCustomerExemptions(string customerId) =>
        _commerce.Store.TaxExemptions.Where(exemption => exemption.CustomerId == customerId).ToList();

    public bool CustomerIsExempt(string customerId) => GetCustomerExemptions(customerId).Count > 0;

    public TaxSettings GetSettings() => new() { Enabled = _enabled };

    public TaxSettings SetEnabled(bool enabled)
    {
        _enabled = enabled;
        return GetSettings();
    }
}

/// <summary>Quality API.</summary>
public sealed class QualityApi
{
    private readonly StateSetCommerce _commerce;

    internal QualityApi(StateSetCommerce commerce) => _commerce = commerce;

    public Inspection CreateInspection(string inspectionType, string referenceType, string referenceId)
    {
        var inspection = new Inspection
        {
            Id = _commerce.Store.Id("insp"),
            Status = "pending"
        };
        _commerce.Store.Inspections.Add(inspection);
        return inspection;
    }

    public Inspection? GetInspection(string id) => _commerce.Store.Inspections.FirstOrDefault(inspection => inspection.Id == id);

    public List<Inspection> ListInspections() => _commerce.Store.Inspections.ToList();

    public Inspection StartInspection(string id) => ReplaceInspection(id, "in_progress");

    public Inspection CompleteInspection(string id) => ReplaceInspection(id, "completed");

    public Ncr CreateNcr(string source, string severity, string sku, int quantityAffected, string description)
    {
        var ncr = new Ncr
        {
            Id = _commerce.Store.Id("ncr"),
            Status = "open",
            Reason = description
        };
        _commerce.Store.Ncrs.Add(ncr);
        return ncr;
    }

    public Ncr? GetNcr(string id) => _commerce.Store.Ncrs.FirstOrDefault(ncr => ncr.Id == id);

    public List<Ncr> ListNcrs() => _commerce.Store.Ncrs.ToList();

    public Ncr CloseNcr(string id)
    {
        var index = _commerce.Store.Ncrs.FindIndex(ncr => ncr.Id == id);
        if (index < 0) throw new StateSetException($"NCR not found: {id}");
        var updated = _commerce.Store.Ncrs[index] with { Status = "closed" };
        _commerce.Store.Ncrs[index] = updated;
        return updated;
    }

    public QualityHold CreateHold(string sku, int quantityHeld, string reason, string holdType)
    {
        var hold = new QualityHold
        {
            Id = _commerce.Store.Id("hold"),
            Sku = sku,
            Status = "active"
        };
        _commerce.Store.QualityHolds.Add(hold);
        return hold;
    }

    public QualityHold? GetHold(string id) => _commerce.Store.QualityHolds.FirstOrDefault(hold => hold.Id == id);

    public List<QualityHold> ListHolds() => _commerce.Store.QualityHolds.ToList();

    public QualityHold ReleaseHold(string id, string releasedBy)
    {
        var index = _commerce.Store.QualityHolds.FindIndex(hold => hold.Id == id);
        if (index < 0) throw new StateSetException($"Quality hold not found: {id}");
        var updated = _commerce.Store.QualityHolds[index] with { Status = "released" };
        _commerce.Store.QualityHolds[index] = updated;
        return updated;
    }

    public List<QualityHold> GetActiveHolds() =>
        _commerce.Store.QualityHolds.Where(hold => hold.Status == "active").ToList();

    private Inspection ReplaceInspection(string id, string status)
    {
        var index = _commerce.Store.Inspections.FindIndex(inspection => inspection.Id == id);
        if (index < 0) throw new StateSetException($"Inspection not found: {id}");
        var updated = _commerce.Store.Inspections[index] with { Status = status };
        _commerce.Store.Inspections[index] = updated;
        return updated;
    }
}

/// <summary>Lots API.</summary>
public sealed class LotsApi
{
    private readonly StateSetCommerce _commerce;

    internal LotsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Lot Create(string sku, int quantityProduced)
    {
        var id = _commerce.Store.Id("lot");
        var lot = new Lot
        {
            Id = id,
            LotNumber = id.ToUpperInvariant(),
            Sku = sku,
            Status = "active"
        };
        _commerce.Store.Lots.Add(lot);
        return lot;
    }

    public Lot? Get(string id) => _commerce.Store.Lots.FirstOrDefault(lot => lot.Id == id);

    public Lot? GetByNumber(string lotNumber) => _commerce.Store.Lots.FirstOrDefault(lot => lot.LotNumber == lotNumber);

    public List<Lot> List() => _commerce.Store.Lots.ToList();

    public Lot CreateLot(string lotNumber, string sku, int quantity)
    {
        var lot = Create(sku, quantity) with { LotNumber = lotNumber };
        _commerce.Store.Lots[^1] = lot;
        return lot;
    }

    public List<Lot> ListLots() => List();

    public List<Lot> GetActiveLots(string sku) =>
        _commerce.Store.Lots.Where(lot => lot.Sku == sku && lot.Status == "active").ToList();

    public Lot Quarantine(string id, string reason) => Replace(id, "quarantined");

    public Lot ReleaseQuarantine(string id) => Replace(id, "active");

    public List<Lot> GetExpiringLots(int days) => new();

    public List<Lot> GetExpiredLots() => new();

    public List<Lot> GetQuarantined() => _commerce.Store.Lots.Where(lot => lot.Status == "quarantined").ToList();

    private Lot Replace(string id, string status)
    {
        var index = _commerce.Store.Lots.FindIndex(lot => lot.Id == id);
        if (index < 0) throw new StateSetException($"Lot not found: {id}");
        var updated = _commerce.Store.Lots[index] with { Status = status };
        _commerce.Store.Lots[index] = updated;
        return updated;
    }
}

/// <summary>Serials API.</summary>
public sealed class SerialsApi
{
    private readonly StateSetCommerce _commerce;

    internal SerialsApi(StateSetCommerce commerce) => _commerce = commerce;

    public Serial Create(string sku, string? lotNumber = null)
    {
        var id = _commerce.Store.Id("ser");
        var serial = new Serial
        {
            Id = id,
            SerialNumber = id.ToUpperInvariant(),
            Sku = sku,
            Status = "available"
        };
        _commerce.Store.Serials.Add(serial);
        return serial;
    }

    public Serial? Get(string id) => _commerce.Store.Serials.FirstOrDefault(serial => serial.Id == id);

    public Serial? GetBySerial(string serial) => _commerce.Store.Serials.FirstOrDefault(item => item.SerialNumber == serial);

    public List<Serial> List() => _commerce.Store.Serials.ToList();

    public Serial RegisterSerial(string serialNumber, string sku)
    {
        var serial = Create(sku) with { SerialNumber = serialNumber };
        _commerce.Store.Serials[^1] = serial;
        return serial;
    }

    public List<Serial> ListSerials() => List();

    public List<Serial> GetAvailable(string sku, int limit) =>
        _commerce.Store.Serials.Where(serial => serial.Sku == sku && serial.Status == "available").Take(limit).ToList();

    public Serial MarkSold(string id, string customerId, string? orderId = null) => Replace(id, "sold");

    public Serial Quarantine(string id, string reason) => Replace(id, "quarantined");

    public bool IsAvailable(string serial) => GetBySerial(serial)?.Status == "available";

    private Serial Replace(string id, string status)
    {
        var index = _commerce.Store.Serials.FindIndex(serial => serial.Id == id);
        if (index < 0) throw new StateSetException($"Serial not found: {id}");
        var updated = _commerce.Store.Serials[index] with { Status = status };
        _commerce.Store.Serials[index] = updated;
        return updated;
    }
}

/// <summary>Warehouse API.</summary>
public sealed class WarehouseApi
{
    private readonly StateSetCommerce _commerce;

    internal WarehouseApi(StateSetCommerce commerce) => _commerce = commerce;

    public Warehouse CreateWarehouse(string code, string name, string warehouseType = "standard")
    {
        var warehouse = new Warehouse
        {
            Id = _commerce.Store.IntId(),
            Code = code,
            Name = name
        };
        _commerce.Store.Warehouses.Add(warehouse);
        return warehouse;
    }

    public Warehouse? GetWarehouse(int id) => _commerce.Store.Warehouses.FirstOrDefault(warehouse => warehouse.Id == id);

    public Warehouse? GetWarehouseByCode(string code) => _commerce.Store.Warehouses.FirstOrDefault(warehouse => warehouse.Code == code);

    public List<Warehouse> ListWarehouses() => _commerce.Store.Warehouses.ToList();

    public Location CreateLocation(int warehouseId, string locationType, string? zone = null, string? aisle = null)
    {
        var location = new Location
        {
            Id = _commerce.Store.IntId(),
            WarehouseId = warehouseId,
            LocationType = locationType
        };
        _commerce.Store.Locations.Add(location);
        return location;
    }

    public Location? GetLocation(int id) => _commerce.Store.Locations.FirstOrDefault(location => location.Id == id);

    public List<Location> ListLocations(int? warehouseId = null) =>
        warehouseId.HasValue
            ? _commerce.Store.Locations.Where(location => location.WarehouseId == warehouseId.Value).ToList()
            : _commerce.Store.Locations.ToList();

    public List<Location> GetPickableLocations(int warehouseId, string sku) => ListLocations(warehouseId);

    public int GetTotalAvailable(int warehouseId, string sku)
    {
        var level = _commerce.Store.StockLevels.TryGetValue(sku, out var current) ? current : null;
        return level != null && decimal.TryParse(level.Available, NumberStyles.Any, CultureInfo.InvariantCulture, out var available)
            ? (int)available
            : 0;
    }
}

/// <summary>Receiving API.</summary>
public sealed class ReceivingApi
{
    private readonly StateSetCommerce _commerce;

    internal ReceivingApi(StateSetCommerce commerce) => _commerce = commerce;

    public Receipt CreateReceipt(string receiptType, int warehouseId, string? purchaseOrderId = null)
    {
        var id = _commerce.Store.Id("receipt");
        var receipt = new Receipt
        {
            Id = id,
            ReceiptNumber = id.ToUpperInvariant(),
            Status = "draft"
        };
        _commerce.Store.Receipts.Add(receipt);
        return receipt;
    }

    public Receipt? GetReceipt(string id) => _commerce.Store.Receipts.FirstOrDefault(receipt => receipt.Id == id);

    public Receipt? GetReceiptByNumber(string number) => _commerce.Store.Receipts.FirstOrDefault(receipt => receipt.ReceiptNumber == number);

    public List<Receipt> ListReceipts() => _commerce.Store.Receipts.ToList();

    public Receipt StartReceiving(string id) => Replace(id, "receiving");

    public Receipt CompleteReceiving(string id) => Replace(id, "completed");

    public Receipt CancelReceipt(string id) => Replace(id, "cancelled");

    public Receipt CreateReceiptFromPo(string poId, int warehouseId) => CreateReceipt("purchase_order", warehouseId, poId);

    private Receipt Replace(string id, string status)
    {
        var index = _commerce.Store.Receipts.FindIndex(receipt => receipt.Id == id);
        if (index < 0) throw new StateSetException($"Receipt not found: {id}");
        var updated = _commerce.Store.Receipts[index] with { Status = status };
        _commerce.Store.Receipts[index] = updated;
        return updated;
    }
}

/// <summary>Fulfillment API.</summary>
public sealed class FulfillmentApi
{
    private readonly StateSetCommerce _commerce;

    internal FulfillmentApi(StateSetCommerce commerce) => _commerce = commerce;

    public Wave CreateWave(int warehouseId, IEnumerable<string> orderIds, int priority = 0)
    {
        var id = _commerce.Store.Id("wave");
        var wave = new Wave
        {
            Id = id,
            WaveNumber = id.ToUpperInvariant(),
            Status = "draft"
        };
        _commerce.Store.Waves.Add(wave);
        return wave;
    }

    public Wave? GetWave(string id) => _commerce.Store.Waves.FirstOrDefault(wave => wave.Id == id);

    public List<Wave> ListWaves() => _commerce.Store.Waves.ToList();

    public Wave ReleaseWave(string id) => ReplaceWave(id, "released");

    public Wave CompleteWave(string id) => ReplaceWave(id, "completed");

    public Wave CancelWave(string id) => ReplaceWave(id, "cancelled");

    public PickTask? GetPick(string id) => _commerce.Store.Picks.FirstOrDefault(pick => pick.Id == id);

    public List<PickTask> ListPicks() => _commerce.Store.Picks.ToList();

    public List<PickTask> ListPickLists() => ListPicks();

    public PickTask AssignPick(string id, string assignedTo) => ReplacePick(id, "assigned", assignedTo);

    public PickTask StartPick(string id) => ReplacePick(id, "in_progress");

    public PickTask CancelPick(string id) => ReplacePick(id, "cancelled");

    public bool IsOrderReadyToPack(string orderId) => true;

    public bool IsOrderReadyToShip(string orderId) => true;

    private Wave ReplaceWave(string id, string status)
    {
        var index = _commerce.Store.Waves.FindIndex(wave => wave.Id == id);
        if (index < 0) throw new StateSetException($"Wave not found: {id}");
        var updated = _commerce.Store.Waves[index] with { Status = status };
        _commerce.Store.Waves[index] = updated;
        return updated;
    }

    private PickTask ReplacePick(string id, string status, string? assignedTo = null)
    {
        var index = _commerce.Store.Picks.FindIndex(pick => pick.Id == id);
        if (index < 0) throw new StateSetException($"Pick not found: {id}");
        var updated = _commerce.Store.Picks[index] with
        {
            Status = status,
            AssignedTo = assignedTo ?? _commerce.Store.Picks[index].AssignedTo
        };
        _commerce.Store.Picks[index] = updated;
        return updated;
    }
}

/// <summary>Accounts payable API.</summary>
public sealed class AccountsPayableApi
{
    private readonly StateSetCommerce _commerce;

    internal AccountsPayableApi(StateSetCommerce commerce) => _commerce = commerce;

    public Bill CreateBill(string supplierId, string dueDate, string? paymentTerms = null)
    {
        var id = _commerce.Store.Id("bill");
        var bill = new Bill
        {
            Id = id,
            BillNumber = id.ToUpperInvariant(),
            Status = "draft"
        };
        _commerce.Store.Bills.Add(bill);
        return bill;
    }

    public Bill? GetBill(string id) => _commerce.Store.Bills.FirstOrDefault(bill => bill.Id == id);

    public Bill? GetBillByNumber(string number) => _commerce.Store.Bills.FirstOrDefault(bill => bill.BillNumber == number);

    public List<Bill> ListBills() => _commerce.Store.Bills.ToList();

    public Bill ApproveBill(string id) => Replace(id, "approved");

    public Bill CancelBill(string id) => Replace(id, "cancelled");

    public List<Bill> GetOverdueBills() => new();

    public List<Bill> GetBillsDueSoon(int days) => new();

    public ApAgingSummary GetAgingSummary() => new() { TotalOutstanding = "0.00" };

    public double GetTotalOutstanding() => 0;

    private Bill Replace(string id, string status)
    {
        var index = _commerce.Store.Bills.FindIndex(bill => bill.Id == id);
        if (index < 0) throw new StateSetException($"Bill not found: {id}");
        var updated = _commerce.Store.Bills[index] with { Status = status };
        _commerce.Store.Bills[index] = updated;
        return updated;
    }
}

/// <summary>Accounts receivable API.</summary>
public sealed class AccountsReceivableApi
{
    private readonly StateSetCommerce _commerce;

    internal AccountsReceivableApi(StateSetCommerce commerce) => _commerce = commerce;

    public ArAgingSummary GetAgingSummary() => new() { TotalOutstanding = "0.00" };

    public double GetTotalOutstanding() => 0;

    public double GetDso(int days) => 0;

    public CreditMemo CreateCreditMemo(string customerId, decimal amount, string reason)
    {
        var memo = new CreditMemo
        {
            Id = _commerce.Store.Id("cm"),
            CustomerId = customerId,
            Amount = StateSetDotNetApiSupport.Amount(amount)
        };
        _commerce.Store.CreditMemos.Add(memo);
        return memo;
    }

    public CreditMemo? GetCreditMemo(string id) => _commerce.Store.CreditMemos.FirstOrDefault(memo => memo.Id == id);

    public List<CreditMemo> ListCreditMemos() => _commerce.Store.CreditMemos.ToList();

    public List<CreditMemo> ListReceivables() => ListCreditMemos();

    public CreditMemo VoidCreditMemo(string id) => GetCreditMemo(id) ?? throw new StateSetException($"Credit memo not found: {id}");

    public List<CreditMemo> GetUnappliedCredits(string customerId) =>
        _commerce.Store.CreditMemos.Where(memo => memo.CustomerId == customerId).ToList();
}

/// <summary>Cost accounting API.</summary>
public sealed class CostAccountingApi
{
    private readonly StateSetCommerce _commerce;

    internal CostAccountingApi(StateSetCommerce commerce) => _commerce = commerce;

    public ItemCost? GetItemCost(string sku) => _commerce.Store.ItemCosts.FirstOrDefault(cost => cost.Sku == sku);

    public ItemCost SetItemCost(string sku, decimal standardCost, decimal? currentCost = null)
    {
        var existing = _commerce.Store.ItemCosts.FindIndex(cost => cost.Sku == sku);
        var itemCost = new ItemCost
        {
            Sku = sku,
            StandardCost = StateSetDotNetApiSupport.Amount(standardCost),
            CurrentCost = StateSetDotNetApiSupport.Amount(currentCost ?? standardCost)
        };
        if (existing >= 0) _commerce.Store.ItemCosts[existing] = itemCost;
        else _commerce.Store.ItemCosts.Add(itemCost);
        return itemCost;
    }

    public List<ItemCost> ListItemCosts() => _commerce.Store.ItemCosts.ToList();

    public List<ItemCost> ListCostEntries() => ListItemCosts();

    public ItemCost UpdateAverageCost(string sku, int quantity, decimal unitCost) => SetItemCost(sku, unitCost);

    public double GetTotalInventoryValue() => 0;
}

/// <summary>Credit API.</summary>
public sealed class CreditApi
{
    private readonly StateSetCommerce _commerce;

    internal CreditApi(StateSetCommerce commerce) => _commerce = commerce;

    public CreditAccount CreateCreditAccount(string customerId, decimal creditLimit)
    {
        var account = new CreditAccount
        {
            Id = _commerce.Store.Id("credit"),
            CustomerId = customerId,
            CreditLimit = StateSetDotNetApiSupport.Amount(creditLimit)
        };
        _commerce.Store.CreditAccounts.RemoveAll(existing => existing.CustomerId == customerId);
        _commerce.Store.CreditAccounts.Add(account);
        return account;
    }

    public CreditAccount SetCreditLimit(string customerId, decimal limit, string currency = "USD") =>
        CreateCreditAccount(customerId, limit);

    public CreditAccount? GetCreditAccount(string id) =>
        _commerce.Store.CreditAccounts.FirstOrDefault(account => account.Id == id);

    public CreditAccount? GetCreditAccountByCustomer(string customerId) =>
        _commerce.Store.CreditAccounts.FirstOrDefault(account => account.CustomerId == customerId)
        ?? CreateCreditAccount(customerId, 0);

    public CreditAccount? GetCreditLimit(string customerId) => GetCreditAccountByCustomer(customerId);

    public List<CreditAccount> ListCreditAccounts() => _commerce.Store.CreditAccounts.ToList();

    public CreditCheck CheckCredit(string customerId, decimal orderAmount) => new() { Approved = true };

    public CreditAccount AdjustCreditLimit(string customerId, decimal newLimit, string reason) =>
        CreateCreditAccount(customerId, newLimit);

    public CreditAccount SuspendCreditAccount(string customerId, string reason) =>
        GetCreditAccountByCustomer(customerId) ?? throw new StateSetException($"Credit account not found for customer: {customerId}");

    public CreditAccount ReactivateCreditAccount(string customerId) =>
        GetCreditAccountByCustomer(customerId) ?? throw new StateSetException($"Credit account not found for customer: {customerId}");

    public List<CreditAccount> GetOverLimitCustomers() => new();
}

/// <summary>Backorders API.</summary>
public sealed class BackordersApi
{
    private readonly StateSetCommerce _commerce;

    internal BackordersApi(StateSetCommerce commerce) => _commerce = commerce;

    public Backorder CreateBackorder(string orderId, string sku, int quantity, string? expectedDate = null)
    {
        var id = _commerce.Store.Id("bo");
        var backorder = new Backorder
        {
            Id = id,
            BackorderNumber = id.ToUpperInvariant(),
            Status = "pending"
        };
        _commerce.Store.Backorders.Add(backorder);
        return backorder;
    }

    public Backorder? GetBackorder(string id) => _commerce.Store.Backorders.FirstOrDefault(backorder => backorder.Id == id);

    public Backorder? GetBackorderByNumber(string number) =>
        _commerce.Store.Backorders.FirstOrDefault(backorder => backorder.BackorderNumber == number);

    public List<Backorder> ListBackorders() => _commerce.Store.Backorders.ToList();

    public Backorder CancelBackorder(string id)
    {
        var index = _commerce.Store.Backorders.FindIndex(backorder => backorder.Id == id);
        if (index < 0) throw new StateSetException($"Backorder not found: {id}");
        var updated = _commerce.Store.Backorders[index] with { Status = "cancelled" };
        _commerce.Store.Backorders[index] = updated;
        return updated;
    }

    public List<Backorder> GetBackordersForOrder(string orderId) => ListBackorders();

    public List<Backorder> GetBackordersForSku(string sku) => ListBackorders();

    public List<Backorder> GetOverdueBackorders() => new();

    public BackorderSummary GetSummary() => new()
    {
        PendingCount = _commerce.Store.Backorders.Count(backorder => backorder.Status == "pending")
    };

    public int CountPending() => GetSummary().PendingCount;
}

/// <summary>General ledger API.</summary>
public sealed class GeneralLedgerApi
{
    private readonly StateSetCommerce _commerce;

    internal GeneralLedgerApi(StateSetCommerce commerce) => _commerce = commerce;

    public GlAccount CreateAccount(string accountNumber, string name, string accountType)
    {
        var account = new GlAccount
        {
            Id = _commerce.Store.Id("gl"),
            AccountNumber = accountNumber,
            Name = name
        };
        _commerce.Store.GlAccounts.Add(account);
        return account;
    }

    public GlAccount? GetAccount(string id) => _commerce.Store.GlAccounts.FirstOrDefault(account => account.Id == id);

    public GlAccount? GetAccountByNumber(string accountNumber) =>
        _commerce.Store.GlAccounts.FirstOrDefault(account => account.AccountNumber == accountNumber);

    public List<GlAccount> ListAccounts() => _commerce.Store.GlAccounts.ToList();

    public List<GlAccount> InitializeChartOfAccounts()
    {
        if (_commerce.Store.GlAccounts.Count == 0)
        {
            CreateAccount("1000", "Cash", "asset");
            CreateAccount("2000", "Accounts Payable", "liability");
            CreateAccount("4000", "Sales", "revenue");
        }
        return ListAccounts();
    }

    public JournalEntry? GetJournalEntry(string id) => _commerce.Store.JournalEntries.FirstOrDefault(entry => entry.Id == id);

    public List<JournalEntry> ListJournalEntries() => _commerce.Store.JournalEntries.ToList();

    public JournalEntry PostJournalEntry(string id, string postedBy) => ReplaceJournalEntry(id, "posted");

    public JournalEntry VoidJournalEntry(string id) => ReplaceJournalEntry(id, "voided");

    public TrialBalance GetTrialBalance(string asOfDate) => new() { AsOfDate = asOfDate };

    public BalanceSheet GetBalanceSheet(string asOfDate) => new() { AsOfDate = asOfDate };

    public IncomeStatement GetIncomeStatement(string startDate, string endDate) => new()
    {
        StartDate = startDate,
        EndDate = endDate
    };

    public double GetAccountBalance(string accountId, string? asOfDate = null) => 0;

    private JournalEntry ReplaceJournalEntry(string id, string status)
    {
        var index = _commerce.Store.JournalEntries.FindIndex(entry => entry.Id == id);
        if (index < 0)
        {
            var created = new JournalEntry
            {
                Id = id,
                EntryNumber = id,
                Status = status
            };
            _commerce.Store.JournalEntries.Add(created);
            return created;
        }
        var updated = _commerce.Store.JournalEntries[index] with { Status = status };
        _commerce.Store.JournalEntries[index] = updated;
        return updated;
    }
}
