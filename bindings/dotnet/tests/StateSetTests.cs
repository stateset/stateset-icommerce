using StateSet.Embedded;
using Xunit;

namespace StateSet.Tests;

/// <summary>
/// End-to-end tests against the REAL Rust engine through P/Invoke. There is
/// no managed fake: if the native library is missing, every test fails.
/// </summary>
public sealed class StateSetTests : IDisposable
{
    private readonly StateSetCommerce _commerce = new(":memory:");

    public void Dispose() => _commerce.Dispose();

    private Customer NewCustomer(string email = "alice@example.com") =>
        _commerce.Customers.Create(email, "Alice", "Smith");

    private static CreateOrderItem Line(Customer c, string sku, int qty, decimal price) =>
        new() { ProductId = c.Id, Sku = sku, Name = sku, Quantity = qty, UnitPrice = price };

    // ------------------------------------------------------------------
    // Proof that this is not an in-memory fake
    // ------------------------------------------------------------------

    [Fact]
    public void DataPersistsAcrossInstancesOnTheSameFile()
    {
        var dir = Path.Combine(Path.GetTempPath(), "stateset-dotnet-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        var path = Path.Combine(dir, "store.db");
        try
        {
            string customerId, orderId;
            using (var first = new StateSetCommerce(path))
            {
                var customer = first.Customers.Create("persist@example.com", "Per", "Sistence");
                var order = first.Orders.Create(customer.Id, new[]
                {
                    new CreateOrderItem { ProductId = customer.Id, Sku = "P-1", Name = "Persisted", Quantity = 3, UnitPrice = 0.10m },
                });
                customerId = customer.Id;
                orderId = order.Id;
            }

            Assert.True(new FileInfo(path).Length > 0, "the SQLite file must exist and be non-empty");

            using var second = new StateSetCommerce(path);
            var reread = second.Customers.Get(customerId);
            Assert.NotNull(reread);
            Assert.Equal("persist@example.com", reread!.Email);
            var order2 = second.Orders.Get(orderId);
            Assert.NotNull(order2);
            Assert.Equal(0.30m, order2!.TotalAmount);
            Assert.Equal("0.30", order2.TotalAmount.ToString(System.Globalization.CultureInfo.InvariantCulture));
            Assert.Single(second.Customers.List());
        }
        finally
        {
            try { Directory.Delete(dir, recursive: true); } catch (IOException) { }
        }
    }

    [Fact]
    public void SeparateInMemoryStoresDoNotShareData()
    {
        NewCustomer();
        using var other = new StateSetCommerce(":memory:");
        Assert.Equal(0, other.Customers.Count());
        Assert.Equal(1, _commerce.Customers.Count());
    }

    [Fact]
    public void NativeSurfaceReportsVersionsAndMethods()
    {
        var v = _commerce.Version;
        Assert.Equal(1, v.GetProperty("json_api_version").GetInt32());
        Assert.Equal(1, v.GetProperty("abi_version").GetInt32());
        Assert.Contains("orders.create", _commerce.NativeMethodNames);
        Assert.Contains("payments.create_refund", _commerce.NativeMethodNames);
    }

    // ------------------------------------------------------------------
    // Customers
    // ------------------------------------------------------------------

    [Fact]
    public void CustomerCrud()
    {
        var c = _commerce.Customers.Create("bob@example.com", "Bob", "Jones", phone: "+15550001");
        Assert.False(string.IsNullOrEmpty(c.Id));
        Assert.Equal(CustomerStatus.Active, c.Status);
        Assert.Equal("Bob Jones", c.FullName);

        Assert.Equal(c.Id, _commerce.Customers.Get(c.Id)!.Id);
        Assert.Equal(c.Id, _commerce.Customers.GetByEmail("bob@example.com")!.Id);

        var updated = _commerce.Customers.Update(c.Id, new UpdateCustomer { FirstName = "Robert" });
        Assert.Equal("Robert", updated.FirstName);

        Assert.Equal(1, _commerce.Customers.Count());
        _commerce.Customers.Delete(c.Id);
        var afterDelete = _commerce.Customers.Get(c.Id);
        Assert.True(afterDelete is null || afterDelete.Status == CustomerStatus.Deleted);
    }

    [Fact]
    public void DuplicateEmailIsRefused()
    {
        NewCustomer("dup@example.com");
        var ex = Assert.ThrowsAny<StateSetException>(() => NewCustomer("dup@example.com"));
        Assert.Equal(StateSetErrorCode.InvalidArgument, ex.Code);
    }

    [Fact]
    public void LookupsOfMissingEntitiesReturnNull()
    {
        var id = Guid.NewGuid().ToString();
        Assert.Null(_commerce.Customers.Get(id));
        Assert.Null(_commerce.Orders.Get(id));
        Assert.Null(_commerce.Products.Get(id));
        Assert.Null(_commerce.Carts.Get(id));
        Assert.Null(_commerce.Payments.Get(id));
        Assert.Null(_commerce.Returns.Get(id));
        Assert.Null(_commerce.Shipments.Get(id));
        Assert.Null(_commerce.Inventory.GetStock("NO-SUCH-SKU"));
    }

    [Fact]
    public void MalformedIdsAreRefusedNotCoerced()
    {
        var ex = Assert.Throws<StateSetValidationException>(() => _commerce.Orders.Get("not-a-uuid"));
        Assert.Equal("invalid_argument", ex.Kind);
    }

    [Fact]
    public void MutatingAMissingEntityThrowsNotFound()
    {
        Assert.Throws<StateSetNotFoundException>(() => _commerce.Orders.Cancel(Guid.NewGuid().ToString()));
    }

    [Fact]
    public void DisposedInstanceRefusesCalls()
    {
        var c = new StateSetCommerce(":memory:");
        c.Dispose();
        c.Dispose(); // idempotent
        Assert.Throws<ObjectDisposedException>(() => c.Customers.List());
    }

    // ------------------------------------------------------------------
    // Products
    // ------------------------------------------------------------------

    [Fact]
    public void ProductWithExactDecimalPrice()
    {
        var p = _commerce.Products.Create("Premium Widget", "WIDGET-001", 29.99m, "A widget");
        Assert.Equal("Premium Widget", p.Name);
        var variant = _commerce.Products.GetVariantBySku("WIDGET-001");
        Assert.NotNull(variant);
        Assert.Equal(29.99m, variant!.Price);
        Assert.Equal(p.Id, variant.ProductId);

        _commerce.Products.AddVariant(p.Id, new CreateProductVariant { Sku = "WIDGET-002", Price = 31.50m });
        Assert.Equal(2, _commerce.Products.GetVariants(p.Id).Count);
        Assert.Empty(_commerce.Products.Search("Premium")); // search covers active products only
        Assert.Equal(ProductStatus.Active, _commerce.Products.Activate(p.Id).Status);
        Assert.Single(_commerce.Products.Search("Premium"));
        Assert.Equal(1, _commerce.Products.Count());
    }

    [Fact]
    public void NegativePriceIsRefused()
    {
        Assert.ThrowsAny<StateSetException>(() => _commerce.Products.Create("Bad", "BAD-1", -1m));
    }

    // ------------------------------------------------------------------
    // Inventory
    // ------------------------------------------------------------------

    [Fact]
    public void InventoryAdjustAndReserve()
    {
        _commerce.Inventory.CreateItem("INV-1", "Bolt", initialQuantity: 100m);
        Assert.Equal(100m, _commerce.Inventory.GetStock("INV-1")!.TotalOnHand);

        var tx = _commerce.Inventory.Adjust("INV-1", -5.5m, "damaged");
        Assert.Equal(-5.5m, tx.Quantity);
        Assert.Equal(94.5m, _commerce.Inventory.GetStock("INV-1")!.TotalOnHand);

        var res = _commerce.Inventory.Reserve("INV-1", 10m, "order", "ORD-1");
        Assert.Equal(10m, res.Quantity);
        Assert.Equal(84.5m, _commerce.Inventory.GetStock("INV-1")!.TotalAvailable);
        Assert.True(_commerce.Inventory.HasStock("INV-1", 84.5m));
        Assert.False(_commerce.Inventory.HasStock("INV-1", 84.6m));

        _commerce.Inventory.ReleaseReservation(res.Id);
        Assert.Equal(94.5m, _commerce.Inventory.GetStock("INV-1")!.TotalAvailable);
        Assert.NotNull(_commerce.Inventory.GetItem("INV-1"));
    }

    [Fact]
    public void OverReservationIsRefused()
    {
        _commerce.Inventory.CreateItem("INV-2", "Nut", initialQuantity: 1m);
        Assert.ThrowsAny<StateSetException>(() => _commerce.Inventory.Reserve("INV-2", 2m, "order", "X"));
    }

    // ------------------------------------------------------------------
    // Orders
    // ------------------------------------------------------------------

    [Fact]
    public void OrderTotalsAreExactDecimals()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 3, 0.10m), Line(c, "B", 1, 19.99m) });
        Assert.Equal(20.29m, order.TotalAmount);
        Assert.Equal(2, order.Items.Count);
        Assert.Equal("USD", order.Currency);
        Assert.Equal(OrderStatus.Pending, order.Status);
        Assert.Equal(order.Id, _commerce.Orders.GetByNumber(order.OrderNumber)!.Id);
        Assert.Single(_commerce.Orders.ListForCustomer(c.Id));
    }

    [Fact]
    public void OrderLifecycleFollowsTheEngineStateMachine()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 1, 10m) });
        order = _commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);
        Assert.Equal(OrderStatus.Confirmed, order.Status);
        order = _commerce.Orders.Ship(order.Id, "1Z999");
        Assert.Equal(OrderStatus.Shipped, order.Status);
        Assert.Equal("1Z999", order.TrackingNumber);
        order = _commerce.Orders.Deliver(order.Id);
        Assert.Equal(OrderStatus.Delivered, order.Status);
        // A delivered order cannot be cancelled -- the engine says so.
        Assert.ThrowsAny<StateSetException>(() => _commerce.Orders.Cancel(order.Id));
    }

    [Fact]
    public void CancelPendingOrder()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 1, 10m) });
        Assert.Equal(OrderStatus.Cancelled, _commerce.Orders.Cancel(order.Id).Status);
    }

    [Fact]
    public void ExplicitCurrencyIsValidated()
    {
        var c = NewCustomer();
        Assert.Equal("EUR", _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 1, 1m) }, currency: "eur").Currency);
        Assert.Throws<StateSetValidationException>(() =>
            _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 1, 1m) }, currency: "EURO"));
    }

    // ------------------------------------------------------------------
    // Carts / checkout
    // ------------------------------------------------------------------

    private static readonly CartAddress Addr = new()
    {
        FirstName = "Alice",
        LastName = "Smith",
        Line1 = "1 Main St",
        City = "Austin",
        State = "TX",
        PostalCode = "78701",
        Country = "US",
    };

    [Fact]
    public void CartCheckoutCreatesARealOrder()
    {
        var c = NewCustomer();
        _commerce.Inventory.CreateItem("CART-1", "Mug", initialQuantity: 10m);
        var cart = _commerce.Carts.Create(customerId: c.Id, customerEmail: c.Email);
        Assert.Equal(CartStatus.Active, cart.Status);

        var item = _commerce.Carts.AddItem(cart.Id, "CART-1", "Mug", 2, 12.50m);
        Assert.Equal(25.00m, item.Total);
        Assert.Single(_commerce.Carts.GetItems(cart.Id));

        _commerce.Carts.SetShipping(cart.Id, Addr, method: "standard", amount: 5.00m);
        _commerce.Carts.SetPayment(cart.Id, "credit_card", "tok_test");
        var result = _commerce.Carts.Complete(cart.Id);

        var order = _commerce.Orders.Get(result.OrderId);
        Assert.NotNull(order);
        Assert.Equal(result.TotalCharged, order!.TotalAmount);
        Assert.Equal(CartStatus.Completed, _commerce.Carts.Get(cart.Id)!.Status);
    }

    [Fact]
    public void CartItemQuantityUpdateAndRemoval()
    {
        var cart = _commerce.Carts.Create();
        var item = _commerce.Carts.AddItem(cart.Id, "CART-2", "Pen", 1, 1.25m);
        Assert.Equal(3.75m, _commerce.Carts.UpdateItemQuantity(item.Id, 3).Total);
        _commerce.Carts.RemoveItem(item.Id);
        Assert.Empty(_commerce.Carts.GetItems(cart.Id));
        Assert.Equal(CartStatus.Cancelled, _commerce.Carts.Cancel(cart.Id).Status);
    }

    // ------------------------------------------------------------------
    // Payments / refunds
    // ------------------------------------------------------------------

    [Fact]
    public void PaymentAndPartialRefund()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 1, 100m) });
        var payment = _commerce.Payments.Create(order.Id, 100.00m, customerId: c.Id);
        Assert.Equal(PaymentStatus.Pending, payment.Status);
        Assert.Equal(100.00m, payment.Amount);

        payment = _commerce.Payments.Complete(payment.Id);
        Assert.Equal(PaymentStatus.Completed, payment.Status);

        var refund = _commerce.Payments.Refund(payment.Id, 30.01m, "partial");
        Assert.Equal(30.01m, refund.Amount);
        _commerce.Payments.CompleteRefund(refund.Id);
        Assert.Single(_commerce.Payments.GetRefunds(payment.Id));
        Assert.Equal(30.01m, _commerce.Payments.Get(payment.Id)!.AmountRefunded);

        // Over-refund is refused by the engine.
        Assert.ThrowsAny<StateSetException>(() => _commerce.Payments.Refund(payment.Id, 70.00m, "too much"));
        Assert.Single(_commerce.Payments.ForOrder(order.Id));
    }

    [Fact]
    public void FailedPayment()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "A", 1, 5m) });
        var payment = _commerce.Payments.Create(order.Id, 5m);
        var failed = _commerce.Payments.Fail(payment.Id, "card declined", "card_declined");
        Assert.Equal(PaymentStatus.Failed, failed.Status);
        Assert.Equal("card declined", failed.FailureReason);
    }

    // ------------------------------------------------------------------
    // Returns
    // ------------------------------------------------------------------

    [Fact]
    public void ReturnLifecycle()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "R-1", 2, 15m) });
        _commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);
        _commerce.Orders.Ship(order.Id);
        _commerce.Orders.Deliver(order.Id);

        var ret = _commerce.Returns.Create(order.Id, ReturnReason.Defective,
            new[] { new CreateReturnItem { OrderItemId = order.Items[0].Id, Quantity = 1, Condition = ItemCondition.Damaged } },
            reasonDetails: "cracked");
        Assert.Equal(ReturnStatus.Requested, ret.Status);
        Assert.Single(ret.Items);

        Assert.Equal(ReturnStatus.Approved, _commerce.Returns.Approve(ret.Id).Status);
        Assert.Equal("RT-1", _commerce.Returns.AddTracking(ret.Id, "RT-1").TrackingNumber);
        Assert.Equal(ReturnStatus.Received, _commerce.Returns.MarkReceived(ret.Id).Status);
        // The engine refuses to complete a return with undecided items.
        Assert.ThrowsAny<StateSetException>(() => _commerce.Returns.Complete(ret.Id));
        var item = _commerce.Returns.SetItemDisposition(ret.Id, ret.Items[0].Id, ReturnDisposition.Scrap);
        Assert.Equal(ReturnDisposition.Scrap, item.Disposition);
        Assert.Equal(ReturnStatus.Completed, _commerce.Returns.Complete(ret.Id).Status);
        Assert.Single(_commerce.Returns.ListForOrder(order.Id));
    }

    [Fact]
    public void ReturningMoreThanWasOrderedIsRefused()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "R-2", 1, 15m) });
        _commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);
        _commerce.Orders.Ship(order.Id);
        _commerce.Orders.Deliver(order.Id);
        Assert.ThrowsAny<StateSetException>(() => _commerce.Returns.Create(order.Id, ReturnReason.ChangedMind,
            new[] { new CreateReturnItem { OrderItemId = order.Items[0].Id, Quantity = 5 } }));
    }

    // ------------------------------------------------------------------
    // Shipments
    // ------------------------------------------------------------------

    [Fact]
    public void ShipmentLifecycle()
    {
        var c = NewCustomer();
        var order = _commerce.Orders.Create(c.Id, new[] { Line(c, "S-1", 1, 15m) });
        _commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);

        var s = _commerce.Shipments.Create(order.Id, "Alice Smith", "1 Main St, Austin TX",
            carrier: ShippingCarrier.Ups, method: ShippingMethod.Ground, shippingCost: 7.25m);
        Assert.Equal(ShipmentStatus.Pending, s.Status);
        Assert.Equal(ShippingCarrier.Ups, s.Carrier);
        Assert.Equal(7.25m, s.ShippingCost);

        // pending -> shipped is not a legal transition; the engine says so.
        Assert.Throws<StateSetValidationException>(() => _commerce.Shipments.Ship(s.Id, "1ZTRACK"));
        Assert.Equal(ShipmentStatus.Processing, _commerce.Shipments.MarkProcessing(s.Id).Status);
        Assert.Equal(ShipmentStatus.ReadyToShip, _commerce.Shipments.MarkReady(s.Id).Status);
        s = _commerce.Shipments.Ship(s.Id, "1ZTRACK");
        Assert.Equal(ShipmentStatus.Shipped, s.Status);
        Assert.Equal(s.Id, _commerce.Shipments.GetByTracking("1ZTRACK")!.Id);
        Assert.Equal(ShipmentStatus.InTransit, _commerce.Shipments.MarkInTransit(s.Id).Status);
        Assert.Equal(ShipmentStatus.OutForDelivery, _commerce.Shipments.MarkOutForDelivery(s.Id).Status);
        Assert.Equal(ShipmentStatus.Delivered, _commerce.Shipments.Deliver(s.Id).Status);
        Assert.Single(_commerce.Shipments.ForOrder(order.Id));
        Assert.Single(_commerce.Shipments.List());
    }

    // ------------------------------------------------------------------
    // Concurrency: one handle, many threads
    // ------------------------------------------------------------------

    [Fact]
    public void ConcurrentCallsOnOneHandle()
    {
        Parallel.For(0, 16, i => _commerce.Customers.Create($"p{i}@example.com", "P", i.ToString()));
        Assert.Equal(16, _commerce.Customers.Count());
    }
}
