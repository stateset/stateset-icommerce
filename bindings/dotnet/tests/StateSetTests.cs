using Xunit;
using StateSet.Embedded;

namespace StateSet.Tests;

public class StateSetTests : IDisposable
{
    private readonly StateSetCommerce _commerce;

    public StateSetTests()
    {
        _commerce = new StateSetCommerce(":memory:");
    }

    public void Dispose()
    {
        _commerce.Dispose();
    }

    #region API Availability Tests

    [Fact]
    public void AllAPIsAvailable()
    {
        Assert.NotNull(_commerce.Customers);
        Assert.NotNull(_commerce.Products);
        Assert.NotNull(_commerce.Orders);
        Assert.NotNull(_commerce.Inventory);
        Assert.NotNull(_commerce.Carts);
        Assert.NotNull(_commerce.Returns);
        Assert.NotNull(_commerce.Payments);
        Assert.NotNull(_commerce.Analytics);
        Assert.NotNull(_commerce.Shipments);
        Assert.NotNull(_commerce.Warranties);
        Assert.NotNull(_commerce.Suppliers);
        Assert.NotNull(_commerce.PurchaseOrders);
        Assert.NotNull(_commerce.Invoices);
        Assert.NotNull(_commerce.Bom);
        Assert.NotNull(_commerce.WorkOrders);
        Assert.NotNull(_commerce.Currency);
        Assert.NotNull(_commerce.Subscriptions);
        Assert.NotNull(_commerce.Promotions);
        Assert.NotNull(_commerce.Tax);
        Assert.NotNull(_commerce.Quality);
        Assert.NotNull(_commerce.Lots);
        Assert.NotNull(_commerce.Serials);
        Assert.NotNull(_commerce.Warehouse);
        Assert.NotNull(_commerce.Receiving);
        Assert.NotNull(_commerce.Fulfillment);
        Assert.NotNull(_commerce.AccountsPayable);
        Assert.NotNull(_commerce.AccountsReceivable);
        Assert.NotNull(_commerce.CostAccounting);
        Assert.NotNull(_commerce.Credit);
        Assert.NotNull(_commerce.Backorders);
        Assert.NotNull(_commerce.GeneralLedger);
    }

    #endregion

    #region Customer Tests

    [Fact]
    public void CreateCustomer()
    {
        var customer = _commerce.Customers.Create(
            email: "test@example.com",
            firstName: "Test",
            lastName: "User"
        );

        Assert.NotEmpty(customer.Id);
        Assert.Equal("test@example.com", customer.Email);
        Assert.Equal("Test", customer.FirstName);
        Assert.Equal("User", customer.LastName);
    }

    [Fact]
    public void GetCustomer()
    {
        var created = _commerce.Customers.Create(
            email: "get@example.com",
            firstName: "Get",
            lastName: "Test"
        );

        var retrieved = _commerce.Customers.Get(created.Id);
        Assert.NotNull(retrieved);
        Assert.Equal(created.Id, retrieved.Id);
        Assert.Equal("get@example.com", retrieved.Email);
    }

    [Fact]
    public void ListCustomers()
    {
        _commerce.Customers.Create(
            email: "list1@example.com",
            firstName: "List",
            lastName: "One"
        );
        _commerce.Customers.Create(
            email: "list2@example.com",
            firstName: "List",
            lastName: "Two"
        );

        var customers = _commerce.Customers.List();
        Assert.True(customers.Count >= 2);
    }

    [Fact]
    public void DeleteCustomer()
    {
        var customer = _commerce.Customers.Create(
            email: "delete@example.com",
            firstName: "Delete",
            lastName: "Me"
        );

        var deleted = _commerce.Customers.Delete(customer.Id);
        Assert.True(deleted);

        var retrieved = _commerce.Customers.Get(customer.Id);
        Assert.Null(retrieved);
    }

    [Fact]
    public void CustomerCount()
    {
        _commerce.Customers.Create(
            email: "count1@example.com",
            firstName: "Count",
            lastName: "One"
        );
        _commerce.Customers.Create(
            email: "count2@example.com",
            firstName: "Count",
            lastName: "Two"
        );

        var count = _commerce.Customers.Count();
        Assert.True(count >= 2);
    }

    #endregion

    #region Product Tests

    [Fact]
    public void CreateProduct()
    {
        var product = _commerce.Products.Create(
            name: "Test Product",
            sku: "TEST-001",
            price: 29.99m,
            description: "A test product"
        );

        Assert.NotEmpty(product.Id);
        Assert.Equal("Test Product", product.Name);
    }

    [Fact]
    public void ListProducts()
    {
        _commerce.Products.Create(
            name: "Product A",
            sku: "PROD-A",
            price: 10.00m
        );
        _commerce.Products.Create(
            name: "Product B",
            sku: "PROD-B",
            price: 20.00m
        );

        var products = _commerce.Products.List();
        Assert.True(products.Count >= 2);
    }

    #endregion

    #region Inventory Tests

    [Fact]
    public void CreateInventoryItem()
    {
        var item = _commerce.Inventory.CreateItem(
            sku: "INV-001",
            name: "Inventory Item",
            initialQuantity: 100
        );

        Assert.Equal("INV-001", item.Sku);
    }

    [Fact]
    public void AdjustInventory()
    {
        _commerce.Inventory.CreateItem(
            sku: "ADJ-001",
            name: "Adjust Test",
            initialQuantity: 50
        );

        var adjusted = _commerce.Inventory.Adjust(
            sku: "ADJ-001",
            quantityDelta: 10,
            reason: "Received shipment"
        );
        Assert.True(adjusted);

        var level = _commerce.Inventory.GetLevel("ADJ-001");
        Assert.NotNull(level);
    }

    #endregion

    #region Order Tests

    [Fact]
    public void CreateOrder()
    {
        var customer = _commerce.Customers.Create(
            email: "order@example.com",
            firstName: "Order",
            lastName: "Test"
        );

        var order = _commerce.Orders.Create(
            customerId: customer.Id,
            items: new[]
            {
                new OrderItem { Sku = "TEST-SKU", Name = "Test Item", Quantity = 2, UnitPrice = 19.99 }
            },
            currency: "USD"
        );

        Assert.NotEmpty(order.Id);
        Assert.Equal(customer.Id, order.CustomerId);
    }

    [Fact]
    public void OrderLifecycle()
    {
        var customer = _commerce.Customers.Create(
            email: "lifecycle@example.com",
            firstName: "Lifecycle",
            lastName: "Test"
        );

        var order = _commerce.Orders.Create(
            customerId: customer.Id,
            items: new[]
            {
                new OrderItem { Sku = "LIFE-001", Name = "Lifecycle Item", Quantity = 1, UnitPrice = 49.99 }
            }
        );

        // Ship order
        var shipped = _commerce.Orders.Ship(order.Id);
        Assert.Equal("shipped", shipped.Status);
    }

    [Fact]
    public void CancelOrder()
    {
        var customer = _commerce.Customers.Create(
            email: "cancel@example.com",
            firstName: "Cancel",
            lastName: "Test"
        );

        var order = _commerce.Orders.Create(
            customerId: customer.Id,
            items: new[]
            {
                new OrderItem { Sku = "CANCEL-001", Name = "Cancel Item", Quantity = 1, UnitPrice = 25.00 }
            }
        );

        var cancelled = _commerce.Orders.Cancel(order.Id);
        Assert.Equal("cancelled", cancelled.Status);
    }

    #endregion

    #region Cart Tests

    [Fact]
    public void CreateCart()
    {
        var cart = _commerce.Carts.Create(currency: "USD");

        Assert.NotEmpty(cart.Id);
        Assert.Equal("USD", cart.Currency);
    }

    [Fact]
    public void AddItemToCart()
    {
        var cart = _commerce.Carts.Create();

        var updated = _commerce.Carts.AddItem(
            cartId: cart.Id,
            sku: "CART-SKU",
            name: "Cart Item",
            quantity: 2,
            unitPrice: 15.99m
        );

        Assert.NotNull(updated);
    }

    #endregion

    #region Analytics Tests

    [Fact]
    public void SalesSummary()
    {
        var summary = _commerce.Analytics.SalesSummary();
        Assert.NotNull(summary);
    }

    [Fact]
    public void TopProducts()
    {
        var products = _commerce.Analytics.TopProducts();
        Assert.NotNull(products);
    }

    [Fact]
    public void TopCustomers()
    {
        var customers = _commerce.Analytics.TopCustomers();
        Assert.NotNull(customers);
    }

    #endregion

    #region Supplier Tests

    [Fact]
    public void CreateSupplier()
    {
        var supplier = _commerce.Suppliers.Create(
            name: "Test Supplier",
            email: "supplier@example.com"
        );

        Assert.NotEmpty(supplier.Id);
        Assert.Equal("Test Supplier", supplier.Name);
    }

    [Fact]
    public void ListSuppliers()
    {
        _commerce.Suppliers.Create(
            name: "Supplier A",
            email: "suppliera@example.com"
        );

        var suppliers = _commerce.Suppliers.List();
        Assert.True(suppliers.Count >= 1);
    }

    #endregion

    #region Invoice Tests

    [Fact]
    public void CreateInvoice()
    {
        var customer = _commerce.Customers.Create(
            email: "invoice@example.com",
            firstName: "Invoice",
            lastName: "Test"
        );

        var invoice = _commerce.Invoices.Create(
            customerId: customer.Id,
            items: new[]
            {
                new InvoiceItem { Description = "Service", Quantity = 1, UnitPrice = 100.00 }
            }
        );

        Assert.NotEmpty(invoice.Id);
    }

    [Fact]
    public void ListInvoices()
    {
        var invoices = _commerce.Invoices.List();
        Assert.NotNull(invoices);
    }

    #endregion

    #region Subscription Tests

    [Fact]
    public void CreateSubscriptionPlan()
    {
        var plan = _commerce.Subscriptions.CreatePlan(
            code: "BASIC",
            name: "Basic Plan",
            interval: "month",
            intervalCount: 1,
            price: 9.99m,
            currency: "USD"
        );

        Assert.NotEmpty(plan.Id);
        Assert.Equal("BASIC", plan.Code);
    }

    [Fact]
    public void ListSubscriptionPlans()
    {
        _commerce.Subscriptions.CreatePlan(
            code: "LIST-PLAN",
            name: "List Plan",
            interval: "month",
            intervalCount: 1,
            price: 19.99m
        );

        var plans = _commerce.Subscriptions.ListPlans();
        Assert.True(plans.Count >= 1);
    }

    [Fact]
    public void SubscribeCustomer()
    {
        var customer = _commerce.Customers.Create(
            email: "subscribe@example.com",
            firstName: "Subscribe",
            lastName: "Test"
        );

        var plan = _commerce.Subscriptions.CreatePlan(
            code: "SUB-PLAN",
            name: "Subscribe Plan",
            interval: "month",
            intervalCount: 1,
            price: 29.99m
        );

        var subscription = _commerce.Subscriptions.Subscribe(
            customerId: customer.Id,
            planId: plan.Id
        );

        Assert.NotEmpty(subscription.Id);
        Assert.Equal(customer.Id, subscription.CustomerId);
    }

    #endregion

    #region Promotion Tests

    [Fact]
    public void CreatePromotion()
    {
        var promo = _commerce.Promotions.Create(
            code: "TEST20",
            name: "Test Discount",
            discountType: "percentage",
            discountValue: 20.0m
        );

        Assert.NotEmpty(promo.Id);
        Assert.Equal("TEST20", promo.Code);
    }

    [Fact]
    public void GetActivePromotions()
    {
        var promo = _commerce.Promotions.Create(
            code: "ACTIVE10",
            name: "Active Promo",
            discountType: "percentage",
            discountValue: 10.0m
        );
        _commerce.Promotions.Activate(promo.Id);

        var active = _commerce.Promotions.GetActive();
        Assert.True(active.Count >= 1);
    }

    [Fact]
    public void CreateCoupon()
    {
        var promo = _commerce.Promotions.Create(
            code: "COUPON-PROMO",
            name: "Coupon Promotion",
            discountType: "percentage",
            discountValue: 15.0m
        );

        var coupon = _commerce.Promotions.CreateCoupon(
            promotionId: promo.Id,
            code: "SAVE15NOW",
            maxUses: 100
        );

        Assert.NotEmpty(coupon.Id);
        Assert.Equal("SAVE15NOW", coupon.Code);
    }

    #endregion

    #region Tax Tests

    [Fact]
    public void GetTaxSettings()
    {
        var settings = _commerce.Tax.GetSettings();
        Assert.NotNull(settings);
    }

    [Fact]
    public void CreateTaxRate()
    {
        var rate = _commerce.Tax.CreateRate(
            country: "US",
            rate: 8.25m
        );

        Assert.NotEmpty(rate.Id);
        Assert.Equal("US", rate.Country);
    }

    #endregion

    #region Warehouse Tests

    [Fact]
    public void CreateWarehouse()
    {
        var warehouse = _commerce.Warehouse.CreateWarehouse(
            code: "WH-TEST",
            name: "Test Warehouse",
            warehouseType: "distribution"
        );

        Assert.Equal("WH-TEST", warehouse.Code);
        Assert.Equal("Test Warehouse", warehouse.Name);
    }

    [Fact]
    public void ListWarehouses()
    {
        _commerce.Warehouse.CreateWarehouse(
            code: "WH-LIST",
            name: "List Warehouse"
        );

        var warehouses = _commerce.Warehouse.ListWarehouses();
        Assert.True(warehouses.Count >= 1);
    }

    [Fact]
    public void CreateZone()
    {
        var warehouse = _commerce.Warehouse.CreateWarehouse(
            code: "WH-ZONE",
            name: "Zone Warehouse"
        );

        var zone = _commerce.Warehouse.CreateZone(
            warehouseId: warehouse.Id,
            code: "ZONE-A",
            name: "Zone A",
            zoneType: "storage"
        );

        Assert.NotEmpty(zone.Id);
        Assert.Equal("ZONE-A", zone.Code);
    }

    #endregion

    #region Quality Tests

    [Fact]
    public void CreateInspection()
    {
        var inspection = _commerce.Quality.CreateInspection(
            inspectionType: "incoming",
            referenceType: "purchase_order",
            referenceId: "test-po-id"
        );

        Assert.NotEmpty(inspection.Id);
    }

    [Fact]
    public void ListInspections()
    {
        var inspections = _commerce.Quality.ListInspections();
        Assert.NotNull(inspections);
    }

    #endregion

    #region Lots Tests

    [Fact]
    public void CreateLot()
    {
        _commerce.Inventory.CreateItem(
            sku: "LOT-SKU",
            name: "Lot Item",
            initialQuantity: 0
        );

        var lot = _commerce.Lots.CreateLot(
            lotNumber: "LOT-001",
            sku: "LOT-SKU",
            quantity: 100
        );

        Assert.NotEmpty(lot.Id);
        Assert.Equal("LOT-001", lot.LotNumber);
    }

    [Fact]
    public void ListLots()
    {
        var lots = _commerce.Lots.ListLots();
        Assert.NotNull(lots);
    }

    #endregion

    #region Serials Tests

    [Fact]
    public void RegisterSerial()
    {
        _commerce.Inventory.CreateItem(
            sku: "SERIAL-SKU",
            name: "Serial Item",
            initialQuantity: 0
        );

        var serial = _commerce.Serials.RegisterSerial(
            serialNumber: "SN-001",
            sku: "SERIAL-SKU"
        );

        Assert.NotEmpty(serial.Id);
        Assert.Equal("SN-001", serial.SerialNumber);
    }

    [Fact]
    public void ListSerials()
    {
        var serials = _commerce.Serials.ListSerials();
        Assert.NotNull(serials);
    }

    #endregion

    #region General Ledger Tests

    [Fact]
    public void CreateGlAccount()
    {
        var account = _commerce.GeneralLedger.CreateAccount(
            accountNumber: "1000",
            name: "Cash",
            accountType: "asset"
        );

        Assert.NotEmpty(account.Id);
        Assert.Equal("1000", account.AccountNumber);
        Assert.Equal("Cash", account.Name);
    }

    [Fact]
    public void ListGlAccounts()
    {
        _commerce.GeneralLedger.CreateAccount(
            accountNumber: "2000",
            name: "Accounts Payable",
            accountType: "liability"
        );

        var accounts = _commerce.GeneralLedger.ListAccounts();
        Assert.True(accounts.Count >= 1);
    }

    [Fact]
    public void InitializeChartOfAccounts()
    {
        var accounts = _commerce.GeneralLedger.InitializeChartOfAccounts();
        Assert.NotNull(accounts);
        Assert.True(accounts.Count > 0);
    }

    #endregion

    #region Accounts Payable Tests

    [Fact]
    public void ListBills()
    {
        var bills = _commerce.AccountsPayable.ListBills();
        Assert.NotNull(bills);
    }

    [Fact]
    public void GetApAgingSummary()
    {
        var summary = _commerce.AccountsPayable.GetAgingSummary();
        Assert.NotNull(summary);
    }

    #endregion

    #region Accounts Receivable Tests

    [Fact]
    public void ListReceivables()
    {
        var receivables = _commerce.AccountsReceivable.ListReceivables();
        Assert.NotNull(receivables);
    }

    [Fact]
    public void GetArAgingSummary()
    {
        var summary = _commerce.AccountsReceivable.GetAgingSummary();
        Assert.NotNull(summary);
    }

    #endregion

    #region Credit Tests

    [Fact]
    public void GetCreditLimit()
    {
        var customer = _commerce.Customers.Create(
            email: "credit@example.com",
            firstName: "Credit",
            lastName: "Test"
        );

        var limit = _commerce.Credit.GetCreditLimit(customer.Id);
        Assert.NotNull(limit);
    }

    [Fact]
    public void SetCreditLimit()
    {
        var customer = _commerce.Customers.Create(
            email: "setcredit@example.com",
            firstName: "Set",
            lastName: "Credit"
        );

        var limit = _commerce.Credit.SetCreditLimit(
            customerId: customer.Id,
            limit: 10000.00m,
            currency: "USD"
        );

        Assert.NotNull(limit);
    }

    #endregion

    #region Backorders Tests

    [Fact]
    public void ListBackorders()
    {
        var backorders = _commerce.Backorders.ListBackorders();
        Assert.NotNull(backorders);
    }

    [Fact]
    public void GetBackorderSummary()
    {
        var summary = _commerce.Backorders.GetSummary();
        Assert.NotNull(summary);
    }

    #endregion

    #region Fulfillment Tests

    [Fact]
    public void ListWaves()
    {
        var waves = _commerce.Fulfillment.ListWaves();
        Assert.NotNull(waves);
    }

    [Fact]
    public void ListPickLists()
    {
        var pickLists = _commerce.Fulfillment.ListPickLists();
        Assert.NotNull(pickLists);
    }

    #endregion

    #region Receiving Tests

    [Fact]
    public void ListReceipts()
    {
        var receipts = _commerce.Receiving.ListReceipts();
        Assert.NotNull(receipts);
    }

    #endregion

    #region Cost Accounting Tests

    [Fact]
    public void ListCostEntries()
    {
        var entries = _commerce.CostAccounting.ListCostEntries();
        Assert.NotNull(entries);
    }

    #endregion

    #region BOM Tests

    [Fact]
    public void CreateBom()
    {
        var product = _commerce.Products.Create(
            name: "BOM Product",
            sku: "BOM-PROD",
            price: 99.99m
        );

        var bom = _commerce.Bom.Create(
            productId: product.Id,
            name: "Test BOM",
            version: "1.0"
        );

        Assert.NotEmpty(bom.Id);
        Assert.Equal("Test BOM", bom.Name);
    }

    [Fact]
    public void ListBoms()
    {
        var boms = _commerce.Bom.List();
        Assert.NotNull(boms);
    }

    #endregion

    #region Work Orders Tests

    [Fact]
    public void CreateWorkOrder()
    {
        var product = _commerce.Products.Create(
            name: "WO Product",
            sku: "WO-PROD",
            price: 49.99m
        );

        var workOrder = _commerce.WorkOrders.Create(
            productId: product.Id,
            quantity: 10
        );

        Assert.NotEmpty(workOrder.Id);
    }

    [Fact]
    public void ListWorkOrders()
    {
        var workOrders = _commerce.WorkOrders.List();
        Assert.NotNull(workOrders);
    }

    #endregion

    #region Purchase Orders Tests

    [Fact]
    public void CreatePurchaseOrder()
    {
        var supplier = _commerce.Suppliers.Create(
            name: "PO Supplier",
            email: "posupplier@example.com"
        );

        var po = _commerce.PurchaseOrders.Create(
            supplierId: supplier.Id,
            items: new[]
            {
                new PurchaseOrderItem { Sku = "PO-SKU", Name = "PO Item", Quantity = 10, UnitCost = 5.00 }
            }
        );

        Assert.NotEmpty(po.Id);
    }

    [Fact]
    public void ListPurchaseOrders()
    {
        var pos = _commerce.PurchaseOrders.List();
        Assert.NotNull(pos);
    }

    #endregion

    #region Shipments Tests

    [Fact]
    public void CreateShipment()
    {
        var customer = _commerce.Customers.Create(
            email: "shipment@example.com",
            firstName: "Shipment",
            lastName: "Test"
        );

        var order = _commerce.Orders.Create(
            customerId: customer.Id,
            items: new[]
            {
                new OrderItem { Sku = "SHIP-SKU", Name = "Ship Item", Quantity = 1, UnitPrice = 30.00 }
            }
        );

        var shipment = _commerce.Shipments.Create(
            orderId: order.Id,
            recipientName: "Test Recipient",
            shippingAddress: "123 Test St"
        );

        Assert.NotEmpty(shipment.Id);
    }

    [Fact]
    public void ListShipments()
    {
        var shipments = _commerce.Shipments.List();
        Assert.NotNull(shipments);
    }

    #endregion

    #region Warranties Tests

    [Fact]
    public void CreateWarranty()
    {
        var customer = _commerce.Customers.Create(
            email: "warranty@example.com",
            firstName: "Warranty",
            lastName: "Test"
        );

        var warranty = _commerce.Warranties.Create(
            customerId: customer.Id,
            warrantyType: "standard",
            durationMonths: 12
        );

        Assert.NotEmpty(warranty.Id);
    }

    [Fact]
    public void ListWarranties()
    {
        var warranties = _commerce.Warranties.List();
        Assert.NotNull(warranties);
    }

    #endregion

    #region Returns Tests

    [Fact]
    public void CreateReturn()
    {
        var customer = _commerce.Customers.Create(
            email: "return@example.com",
            firstName: "Return",
            lastName: "Test"
        );

        var order = _commerce.Orders.Create(
            customerId: customer.Id,
            items: new[]
            {
                new OrderItem { Sku = "RET-SKU", Name = "Return Item", Quantity = 1, UnitPrice = 50.00 }
            }
        );

        var returnRequest = _commerce.Returns.Create(
            orderId: order.Id,
            reason: "defective"
        );

        Assert.NotEmpty(returnRequest.Id);
    }

    [Fact]
    public void ListReturns()
    {
        var returns = _commerce.Returns.List();
        Assert.NotNull(returns);
    }

    #endregion

    #region Payments Tests

    [Fact]
    public void CreatePayment()
    {
        var customer = _commerce.Customers.Create(
            email: "payment@example.com",
            firstName: "Payment",
            lastName: "Test"
        );

        var order = _commerce.Orders.Create(
            customerId: customer.Id,
            items: new[]
            {
                new OrderItem { Sku = "PAY-SKU", Name = "Payment Item", Quantity = 1, UnitPrice = 100.00 }
            }
        );

        var payment = _commerce.Payments.Create(
            orderId: order.Id,
            amount: 100.00m,
            method: "credit_card"
        );

        Assert.NotEmpty(payment.Id);
    }

    [Fact]
    public void ListPayments()
    {
        var payments = _commerce.Payments.List();
        Assert.NotNull(payments);
    }

    #endregion
}
