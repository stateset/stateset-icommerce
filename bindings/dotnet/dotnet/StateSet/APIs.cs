namespace StateSet.Embedded;

// Each API class is a thin, typed facade over one engine domain. Every method
// is a single native call (`StateSetCommerce.Call`) to the Rust engine; the
// method names in quotes are the stateset-ffi JSON surface
// (crates/stateset-ffi/src/json_api.rs). Nothing here keeps state.

/// <summary>Customers.</summary>
public sealed class CustomersApi
{
    private readonly StateSetCommerce _c;
    internal CustomersApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Create a customer.</summary>
    public Customer Create(string email, string firstName, string lastName, string? phone = null,
        bool? acceptsMarketing = null, IEnumerable<string>? tags = null) =>
        _c.Call<Customer>("customers.create", new
        {
            Email = email,
            FirstName = firstName,
            LastName = lastName,
            Phone = phone,
            AcceptsMarketing = acceptsMarketing,
            Tags = tags?.ToList(),
        });

    /// <summary>Get a customer by id, or null.</summary>
    public Customer? Get(string id) => _c.CallOptional<Customer>("customers.get", new { Id = id });

    /// <summary>Get a customer by email, or null.</summary>
    public Customer? GetByEmail(string email) => _c.CallOptional<Customer>("customers.get_by_email", new { Email = email });

    /// <summary>Update a customer.</summary>
    public Customer Update(string id, UpdateCustomer input) =>
        _c.Call<Customer>("customers.update", new { Id = id, Input = input });

    /// <summary>List customers.</summary>
    public List<Customer> List(int? limit = null, int? offset = null) =>
        _c.Call<List<Customer>>("customers.list", new { Limit = limit, Offset = offset });

    /// <summary>Count customers.</summary>
    public long Count() => _c.Call<long>("customers.count", null);

    /// <summary>Delete a customer. Throws <see cref="StateSetNotFoundException"/> if absent.</summary>
    public void Delete(string id) => _c.Call<bool>("customers.delete", new { Id = id });
}

/// <summary>Products and variants.</summary>
public sealed class ProductsApi
{
    private readonly StateSetCommerce _c;
    internal ProductsApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Create a product with a single default variant carrying <paramref name="sku"/> and <paramref name="price"/>.</summary>
    public Product Create(string name, string sku, decimal price, string? description = null) =>
        Create(name, new[] { new CreateProductVariant { Sku = sku, Price = price, IsDefault = true } }, description);

    /// <summary>Create a product with the given variants.</summary>
    public Product Create(string name, IEnumerable<CreateProductVariant> variants, string? description = null,
        string? slug = null, ProductType? productType = null) =>
        _c.Call<Product>("products.create", new
        {
            Name = name,
            Slug = slug,
            Description = description,
            ProductType = productType,
            Variants = variants.ToList(),
        });

    /// <summary>Get a product by id, or null.</summary>
    public Product? Get(string id) => _c.CallOptional<Product>("products.get", new { Id = id });

    /// <summary>Get a product by slug, or null.</summary>
    public Product? GetBySlug(string slug) => _c.CallOptional<Product>("products.get_by_slug", new { Slug = slug });

    /// <summary>Update a product.</summary>
    public Product Update(string id, UpdateProduct input) =>
        _c.Call<Product>("products.update", new { Id = id, Input = input });

    /// <summary>List products.</summary>
    public List<Product> List(int? limit = null, int? offset = null) =>
        _c.Call<List<Product>>("products.list", new { Limit = limit, Offset = offset });

    /// <summary>Count products.</summary>
    public long Count() => _c.Call<long>("products.count", null);

    /// <summary>Search products by name/description.</summary>
    public List<Product> Search(string query) => _c.Call<List<Product>>("products.search", new { Query = query });

    /// <summary>Activate a product.</summary>
    public Product Activate(string id) => _c.Call<Product>("products.activate", new { Id = id });

    /// <summary>Archive a product.</summary>
    public Product Archive(string id) => _c.Call<Product>("products.archive", new { Id = id });

    /// <summary>Delete a product.</summary>
    public void Delete(string id) => _c.Call<bool>("products.delete", new { Id = id });

    /// <summary>Add a variant to a product.</summary>
    public ProductVariant AddVariant(string productId, CreateProductVariant variant) =>
        _c.Call<ProductVariant>("products.add_variant", new { ProductId = productId, Variant = variant });

    /// <summary>The variants of a product.</summary>
    public List<ProductVariant> GetVariants(string productId) =>
        _c.Call<List<ProductVariant>>("products.get_variants", new { ProductId = productId });

    /// <summary>Get a variant by SKU, or null.</summary>
    public ProductVariant? GetVariantBySku(string sku) =>
        _c.CallOptional<ProductVariant>("products.get_variant_by_sku", new { Sku = sku });
}

/// <summary>Inventory.</summary>
public sealed class InventoryApi
{
    private readonly StateSetCommerce _c;
    internal InventoryApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Create an inventory item, optionally with opening stock.</summary>
    public InventoryItem CreateItem(string sku, string name, decimal? initialQuantity = null,
        string? description = null, decimal? reorderPoint = null) =>
        _c.Call<InventoryItem>("inventory.create_item", new
        {
            Sku = sku,
            Name = name,
            Description = description,
            InitialQuantity = initialQuantity,
            ReorderPoint = reorderPoint,
        });

    /// <summary>Get an inventory item by SKU, or null.</summary>
    public InventoryItem? GetItem(string sku) => _c.CallOptional<InventoryItem>("inventory.get_item_by_sku", new { Sku = sku });

    /// <summary>List inventory items.</summary>
    public List<InventoryItem> List() => _c.Call<List<InventoryItem>>("inventory.list", null);

    /// <summary>Aggregated stock for a SKU, or null when the SKU is unknown.</summary>
    public StockLevel? GetStock(string sku) => _c.CallOptional<StockLevel>("inventory.get_stock", new { Sku = sku });

    /// <summary>Adjust on-hand stock by <paramref name="quantityDelta"/> (negative to remove).</summary>
    public InventoryTransaction Adjust(string sku, decimal quantityDelta, string reason = "manual adjustment") =>
        _c.Call<InventoryTransaction>("inventory.adjust", new { Sku = sku, Quantity = quantityDelta, Reason = reason });

    /// <summary>Whether at least <paramref name="quantity"/> is available.</summary>
    public bool HasStock(string sku, decimal quantity) =>
        _c.Call<bool>("inventory.has_stock", new { Sku = sku, Quantity = quantity });

    /// <summary>Reserve stock for a reference (e.g. an order).</summary>
    public InventoryReservation Reserve(string sku, decimal quantity, string referenceType, string referenceId,
        long? expiresInSeconds = null) =>
        _c.Call<InventoryReservation>("inventory.reserve", new
        {
            Sku = sku,
            Quantity = quantity,
            ReferenceType = referenceType,
            ReferenceId = referenceId,
            ExpiresInSeconds = expiresInSeconds,
        });

    /// <summary>Release a reservation.</summary>
    public void ReleaseReservation(string reservationId) =>
        _c.Call<bool>("inventory.release_reservation", new { Id = reservationId });

    /// <summary>Confirm a reservation (consumes the reserved stock).</summary>
    public void ConfirmReservation(string reservationId) =>
        _c.Call<bool>("inventory.confirm_reservation", new { Id = reservationId });

    /// <summary>Get a reservation, or null.</summary>
    public InventoryReservation? GetReservation(string reservationId) =>
        _c.CallOptional<InventoryReservation>("inventory.get_reservation", new { Id = reservationId });
}

/// <summary>Carts and checkout.</summary>
public sealed class CartsApi
{
    private readonly StateSetCommerce _c;
    internal CartsApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Create a cart. <paramref name="currency"/> null means the store default.</summary>
    public Cart Create(string? customerId = null, string? currency = null, string? customerEmail = null,
        string? customerName = null, IEnumerable<AddCartItem>? items = null) =>
        _c.Call<Cart>("carts.create", new
        {
            CustomerId = customerId,
            CustomerEmail = customerEmail,
            CustomerName = customerName,
            Currency = currency,
            Items = items?.ToList(),
        });

    /// <summary>Get a cart, or null.</summary>
    public Cart? Get(string cartId) => _c.CallOptional<Cart>("carts.get", new { Id = cartId });

    /// <summary>List carts.</summary>
    public List<Cart> List() => _c.Call<List<Cart>>("carts.list", null);

    /// <summary>Add a line to a cart.</summary>
    public CartItem AddItem(string cartId, AddCartItem item) =>
        _c.Call<CartItem>("carts.add_item", new { CartId = cartId, Item = item });

    /// <summary>Add a line to a cart.</summary>
    public CartItem AddItem(string cartId, string sku, string name, int quantity, decimal unitPrice,
        string? productId = null, string? variantId = null) =>
        AddItem(cartId, new AddCartItem
        {
            Sku = sku,
            Name = name,
            Quantity = quantity,
            UnitPrice = unitPrice,
            ProductId = productId,
            VariantId = variantId,
        });

    /// <summary>Change a line's quantity.</summary>
    public CartItem UpdateItemQuantity(string itemId, int quantity) =>
        _c.Call<CartItem>("carts.update_item", new { ItemId = itemId, Input = new { Quantity = quantity } });

    /// <summary>Remove a line.</summary>
    public void RemoveItem(string itemId) => _c.Call<bool>("carts.remove_item", new { ItemId = itemId });

    /// <summary>The lines in a cart.</summary>
    public List<CartItem> GetItems(string cartId) => _c.Call<List<CartItem>>("carts.get_items", new { Id = cartId });

    /// <summary>Remove every line.</summary>
    public void ClearItems(string cartId) => _c.Call<bool>("carts.clear_items", new { Id = cartId });

    /// <summary>Set the shipping address.</summary>
    public Cart SetShippingAddress(string cartId, CartAddress address) =>
        _c.Call<Cart>("carts.set_shipping_address", new { Id = cartId, Address = address });

    /// <summary>Set the billing address.</summary>
    public Cart SetBillingAddress(string cartId, CartAddress address) =>
        _c.Call<Cart>("carts.set_billing_address", new { Id = cartId, Address = address });

    /// <summary>Set shipping address, method and amount.</summary>
    public Cart SetShipping(string cartId, CartAddress address, string? method = null, string? carrier = null,
        decimal? amount = null) =>
        _c.Call<Cart>("carts.set_shipping", new
        {
            Id = cartId,
            Shipping = new { ShippingAddress = address, ShippingMethod = method, ShippingCarrier = carrier, ShippingAmount = amount },
        });

    /// <summary>Choose the payment method.</summary>
    public Cart SetPayment(string cartId, string paymentMethod, string? paymentToken = null) =>
        _c.Call<Cart>("carts.set_payment", new { Id = cartId, Payment = new { PaymentMethod = paymentMethod, PaymentToken = paymentToken } });

    /// <summary>Apply a coupon code.</summary>
    public Cart ApplyDiscount(string cartId, string couponCode) =>
        _c.Call<Cart>("carts.apply_discount", new { Id = cartId, CouponCode = couponCode });

    /// <summary>Remove the applied coupon.</summary>
    public Cart RemoveDiscount(string cartId) => _c.Call<Cart>("carts.remove_discount", new { Id = cartId });

    /// <summary>Recalculate totals.</summary>
    public Cart Recalculate(string cartId) => _c.Call<Cart>("carts.recalculate", new { Id = cartId });

    /// <summary>Mark the cart ready for payment.</summary>
    public Cart MarkReadyForPayment(string cartId) => _c.Call<Cart>("carts.mark_ready_for_payment", new { Id = cartId });

    /// <summary>Begin checkout.</summary>
    public Cart BeginCheckout(string cartId) => _c.Call<Cart>("carts.begin_checkout", new { Id = cartId });

    /// <summary>Complete checkout: creates the order (and payment record).</summary>
    public CheckoutResult Complete(string cartId) => _c.Call<CheckoutResult>("carts.complete", new { Id = cartId });

    /// <summary>Cancel the cart.</summary>
    public Cart Cancel(string cartId) => _c.Call<Cart>("carts.cancel", new { Id = cartId });

    /// <summary>Mark the cart abandoned.</summary>
    public Cart Abandon(string cartId) => _c.Call<Cart>("carts.abandon", new { Id = cartId });
}

/// <summary>Orders.</summary>
public sealed class OrdersApi
{
    private readonly StateSetCommerce _c;
    internal OrdersApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>
    /// Create an order. <paramref name="currency"/> null means the store
    /// default; an explicit value must be a valid ISO 4217 code.
    /// </summary>
    public Order Create(string customerId, IEnumerable<CreateOrderItem> items, string? currency = null,
        Address? shippingAddress = null, string? notes = null, string? paymentMethod = null) =>
        _c.Call<Order>("orders.create", new
        {
            CustomerId = customerId,
            Items = items.ToList(),
            Currency = currency,
            ShippingAddress = shippingAddress,
            Notes = notes,
            PaymentMethod = paymentMethod,
        });

    /// <summary>Get an order, or null.</summary>
    public Order? Get(string id) => _c.CallOptional<Order>("orders.get", new { Id = id });

    /// <summary>Get an order by its number, or null.</summary>
    public Order? GetByNumber(string orderNumber) =>
        _c.CallOptional<Order>("orders.get_by_number", new { OrderNumber = orderNumber });

    /// <summary>List orders.</summary>
    public List<Order> List(int? limit = null, int? offset = null) =>
        _c.Call<List<Order>>("orders.list", new { Limit = limit, Offset = offset });

    /// <summary>Orders for a customer.</summary>
    public List<Order> ListForCustomer(string customerId) =>
        _c.Call<List<Order>>("orders.list_for_customer", new { CustomerId = customerId });

    /// <summary>Count orders.</summary>
    public long Count() => _c.Call<long>("orders.count", null);

    /// <summary>Transition an order's status (the engine enforces the state machine).</summary>
    public Order UpdateStatus(string id, OrderStatus status) =>
        _c.Call<Order>("orders.update_status", new { Id = id, Status = status });

    /// <summary>Ship an order.</summary>
    public Order Ship(string id, string? trackingNumber = null) =>
        _c.Call<Order>("orders.ship", new { Id = id, TrackingNumber = trackingNumber });

    /// <summary>Mark an order delivered.</summary>
    public Order Deliver(string id) => _c.Call<Order>("orders.deliver", new { Id = id });

    /// <summary>Cancel an order.</summary>
    public Order Cancel(string id) => _c.Call<Order>("orders.cancel", new { Id = id });
}

/// <summary>Payments and refunds.</summary>
public sealed class PaymentsApi
{
    private readonly StateSetCommerce _c;
    internal PaymentsApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Record a payment.</summary>
    public Payment Create(string orderId, decimal amount, string? currency = null,
        PaymentMethod method = PaymentMethod.CreditCard, string? customerId = null, string? externalId = null,
        string? idempotencyKey = null) =>
        _c.Call<Payment>("payments.create", new
        {
            OrderId = orderId,
            CustomerId = customerId,
            PaymentMethod = method,
            Amount = amount,
            Currency = currency,
            ExternalId = externalId,
            IdempotencyKey = idempotencyKey,
        });

    /// <summary>Get a payment, or null.</summary>
    public Payment? Get(string id) => _c.CallOptional<Payment>("payments.get", new { Id = id });

    /// <summary>List payments.</summary>
    public List<Payment> List() => _c.Call<List<Payment>>("payments.list", null);

    /// <summary>Payments for an order.</summary>
    public List<Payment> ForOrder(string orderId) => _c.Call<List<Payment>>("payments.for_order", new { OrderId = orderId });

    /// <summary>Mark a payment processing.</summary>
    public Payment MarkProcessing(string id) => _c.Call<Payment>("payments.mark_processing", new { Id = id });

    /// <summary>Mark a payment completed (captured).</summary>
    public Payment Complete(string id) => _c.Call<Payment>("payments.mark_completed", new { Id = id });

    /// <summary>Mark a payment failed.</summary>
    public Payment Fail(string id, string reason, string? code = null) =>
        _c.Call<Payment>("payments.mark_failed", new { Id = id, Reason = reason, Code = code });

    /// <summary>Cancel a payment.</summary>
    public Payment Cancel(string id) => _c.Call<Payment>("payments.cancel", new { Id = id });

    /// <summary>Create a refund. <paramref name="amount"/> null refunds the remaining balance.</summary>
    public Refund Refund(string paymentId, decimal? amount, string? reason = null, string? idempotencyKey = null) =>
        _c.Call<Refund>("payments.create_refund", new
        {
            PaymentId = paymentId,
            Amount = amount,
            Reason = reason,
            IdempotencyKey = idempotencyKey,
        });

    /// <summary>Get a refund, or null.</summary>
    public Refund? GetRefund(string refundId) => _c.CallOptional<Refund>("payments.get_refund", new { Id = refundId });

    /// <summary>Refunds for a payment.</summary>
    public List<Refund> GetRefunds(string paymentId) =>
        _c.Call<List<Refund>>("payments.get_refunds", new { PaymentId = paymentId });

    /// <summary>Mark a refund completed.</summary>
    public Refund CompleteRefund(string refundId) => _c.Call<Refund>("payments.complete_refund", new { Id = refundId });

    /// <summary>Mark a refund failed.</summary>
    public Refund FailRefund(string refundId, string reason) =>
        _c.Call<Refund>("payments.fail_refund", new { Id = refundId, Reason = reason });
}

/// <summary>Returns (RMAs).</summary>
public sealed class ReturnsApi
{
    private readonly StateSetCommerce _c;
    internal ReturnsApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Request a return for lines of an order.</summary>
    public Return Create(string orderId, ReturnReason reason, IEnumerable<CreateReturnItem> items,
        string? reasonDetails = null, string? notes = null, string? idempotencyKey = null) =>
        _c.Call<Return>("returns.create", new
        {
            OrderId = orderId,
            Reason = reason,
            ReasonDetails = reasonDetails,
            Items = items.ToList(),
            Notes = notes,
            IdempotencyKey = idempotencyKey,
        });

    /// <summary>Get a return, or null.</summary>
    public Return? Get(string id) => _c.CallOptional<Return>("returns.get", new { Id = id });

    /// <summary>List returns.</summary>
    public List<Return> List() => _c.Call<List<Return>>("returns.list", null);

    /// <summary>Returns for an order.</summary>
    public List<Return> ListForOrder(string orderId) => _c.Call<List<Return>>("returns.list_for_order", new { OrderId = orderId });

    /// <summary>Approve a return.</summary>
    public Return Approve(string id) => _c.Call<Return>("returns.approve", new { Id = id });

    /// <summary>Reject a return.</summary>
    public Return Reject(string id, string reason) => _c.Call<Return>("returns.reject", new { Id = id, Reason = reason });

    /// <summary>Record that the returned goods arrived.</summary>
    public Return MarkReceived(string id) => _c.Call<Return>("returns.mark_received", new { Id = id });

    /// <summary>
    /// Decide what happens to a received item (restock, refurbish, scrap,
    /// return_to_vendor, quarantine). Every item needs one before
    /// <see cref="Complete"/>.
    /// </summary>
    public ReturnItem SetItemDisposition(string id, string itemId, ReturnDisposition disposition,
        int? warehouseId = null, string? dispositionBy = null) =>
        _c.Call<ReturnItem>("returns.set_item_disposition", new
        {
            Id = id,
            ItemId = itemId,
            Disposition = disposition,
            WarehouseId = warehouseId,
            DispositionBy = dispositionBy,
        });

    /// <summary>Complete a return.</summary>
    public Return Complete(string id) => _c.Call<Return>("returns.complete", new { Id = id });

    /// <summary>Cancel a return.</summary>
    public Return Cancel(string id) => _c.Call<Return>("returns.cancel", new { Id = id });

    /// <summary>Attach the inbound tracking number.</summary>
    public Return AddTracking(string id, string trackingNumber) =>
        _c.Call<Return>("returns.add_tracking", new { Id = id, TrackingNumber = trackingNumber });
}

/// <summary>Shipments.</summary>
public sealed class ShipmentsApi
{
    private readonly StateSetCommerce _c;
    internal ShipmentsApi(StateSetCommerce commerce) => _c = commerce;

    /// <summary>Create a shipment for an order.</summary>
    public Shipment Create(string orderId, string recipientName, string shippingAddress,
        ShippingCarrier? carrier = null, ShippingMethod? method = null, string? trackingNumber = null,
        IEnumerable<CreateShipmentItem>? items = null, decimal? shippingCost = null) =>
        _c.Call<Shipment>("shipments.create", new
        {
            OrderId = orderId,
            RecipientName = recipientName,
            ShippingAddress = shippingAddress,
            Carrier = carrier,
            ShippingMethod = method,
            TrackingNumber = trackingNumber,
            Items = items?.ToList(),
            ShippingCost = shippingCost,
        });

    /// <summary>Get a shipment, or null.</summary>
    public Shipment? Get(string id) => _c.CallOptional<Shipment>("shipments.get", new { Id = id });

    /// <summary>Find a shipment by tracking number, or null.</summary>
    public Shipment? GetByTracking(string trackingNumber) =>
        _c.CallOptional<Shipment>("shipments.get_by_tracking", new { TrackingNumber = trackingNumber });

    /// <summary>List shipments.</summary>
    public List<Shipment> List() => _c.Call<List<Shipment>>("shipments.list", null);

    /// <summary>Shipments for an order.</summary>
    public List<Shipment> ForOrder(string orderId) => _c.Call<List<Shipment>>("shipments.for_order", new { OrderId = orderId });

    /// <summary>Mark processing.</summary>
    public Shipment MarkProcessing(string id) => _c.Call<Shipment>("shipments.mark_processing", new { Id = id });

    /// <summary>Mark ready to ship.</summary>
    public Shipment MarkReady(string id) => _c.Call<Shipment>("shipments.mark_ready", new { Id = id });

    /// <summary>Ship with an optional tracking number.</summary>
    public Shipment Ship(string id, string? trackingNumber = null) =>
        _c.Call<Shipment>("shipments.ship", new { Id = id, TrackingNumber = trackingNumber });

    /// <summary>Mark in transit.</summary>
    public Shipment MarkInTransit(string id) => _c.Call<Shipment>("shipments.mark_in_transit", new { Id = id });

    /// <summary>Mark out for delivery.</summary>
    public Shipment MarkOutForDelivery(string id) => _c.Call<Shipment>("shipments.mark_out_for_delivery", new { Id = id });

    /// <summary>Mark delivered (from out-for-delivery).</summary>
    public Shipment Deliver(string id) => _c.Call<Shipment>("shipments.mark_delivered", new { Id = id });

    /// <summary>Cancel a shipment.</summary>
    public Shipment Cancel(string id) => _c.Call<Shipment>("shipments.cancel", new { Id = id });
}
