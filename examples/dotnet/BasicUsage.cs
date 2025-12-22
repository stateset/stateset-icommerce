/**
 * StateSet iCommerce - C# / .NET Example
 *
 * Demonstrates the full commerce workflow:
 * - Customer creation
 * - Product catalog management
 * - Inventory tracking
 * - Order processing
 * - Analytics
 *
 * Run with: dotnet run
 */

using StateSet;

Console.WriteLine("=== StateSet iCommerce - C# Example ===");
Console.WriteLine();

// Initialize commerce with in-memory database
using var commerce = new StateSetCommerce(":memory:");
Console.WriteLine("✓ Commerce initialized");
Console.WriteLine();

// 1. Create a customer
Console.WriteLine("1. Creating customer...");
var customer = commerce.Customers.Create(
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith",
    phone: "+1-555-0123"
);
Console.WriteLine($"   Created customer: {customer.FirstName} {customer.LastName} ({customer.Email})");

// 2. Create products
Console.WriteLine();
Console.WriteLine("2. Creating products...");
var widget = commerce.Products.Create(
    name: "Premium Widget",
    sku: "WIDGET-001",
    price: 29.99m,
    description: "A high-quality widget for all your needs"
);
Console.WriteLine($"   Created product: {widget.Name} ({widget.Id})");

var gadget = commerce.Products.Create(
    name: "Super Gadget",
    sku: "GADGET-001",
    price: 49.99m,
    description: "An amazing gadget"
);
Console.WriteLine($"   Created product: {gadget.Name} ({gadget.Id})");

// 3. Create inventory
Console.WriteLine();
Console.WriteLine("3. Setting up inventory...");
commerce.Inventory.CreateItem(
    sku: "WIDGET-001",
    name: "Premium Widget",
    initialQuantity: 100
);
Console.WriteLine("   Created inventory for WIDGET-001 (100 units)");

commerce.Inventory.CreateItem(
    sku: "GADGET-001",
    name: "Super Gadget",
    initialQuantity: 50
);
Console.WriteLine("   Created inventory for GADGET-001 (50 units)");

// Check stock
var widgetStock = commerce.Inventory.GetStock("WIDGET-001");
Console.WriteLine($"   Stock check WIDGET-001: {widgetStock?.TotalAvailable} available");

// 4. Create an order
Console.WriteLine();
Console.WriteLine("4. Creating order...");
var order = commerce.Orders.Create(
    customerId: customer.Id,
    items: new[]
    {
        new OrderItem
        {
            ProductId = widget.Id,
            Sku = "WIDGET-001",
            Name = "Premium Widget",
            Quantity = 2,
            UnitPrice = "29.99"
        },
        new OrderItem
        {
            ProductId = gadget.Id,
            Sku = "GADGET-001",
            Name = "Super Gadget",
            Quantity = 1,
            UnitPrice = "49.99"
        }
    },
    currency: "USD"
);
Console.WriteLine($"   Created order {order.OrderNumber} (total: ${order.TotalAmount})");

// 5. Process the order
Console.WriteLine();
Console.WriteLine("5. Processing order...");

// Update order status
order = commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);
Console.WriteLine($"   Order status: {order.Status}");

// Adjust inventory (fulfill)
commerce.Inventory.Adjust("WIDGET-001", -2, "Order fulfillment");
commerce.Inventory.Adjust("GADGET-001", -1, "Order fulfillment");
Console.WriteLine("   Inventory adjusted");

// Ship the order
order = commerce.Orders.UpdateStatus(order.Id, OrderStatus.Shipped);
Console.WriteLine($"   Order shipped (status: {order.Status})");

// 6. Check final inventory
Console.WriteLine();
Console.WriteLine("6. Final inventory check...");
var finalWidgetStock = commerce.Inventory.GetStock("WIDGET-001");
Console.WriteLine($"   WIDGET-001: {finalWidgetStock?.TotalAvailable} available (was 100)");

var finalGadgetStock = commerce.Inventory.GetStock("GADGET-001");
Console.WriteLine($"   GADGET-001: {finalGadgetStock?.TotalAvailable} available (was 50)");

// 7. Analytics
Console.WriteLine();
Console.WriteLine("7. Analytics...");
var salesSummary = commerce.Analytics.GetSalesSummary(TimePeriod.Today);
Console.WriteLine($"   Revenue: ${salesSummary.TotalRevenue}");
Console.WriteLine($"   Orders: {salesSummary.OrderCount}");
Console.WriteLine($"   AOV: ${salesSummary.AverageOrderValue}");

// Get top products
var topProducts = commerce.Analytics.GetTopProducts(TimePeriod.Month, 5);
Console.WriteLine($"   Top products: {topProducts.Count}");

// 8. Summary
Console.WriteLine();
Console.WriteLine("=== Summary ===");
var customers = commerce.Customers.List();
var products = commerce.Products.List();
var orders = commerce.Orders.List();
Console.WriteLine($"Customers: {customers.Count}");
Console.WriteLine($"Products: {products.Count}");
Console.WriteLine($"Orders: {orders.Count}");

Console.WriteLine();
Console.WriteLine("✓ Example completed successfully!");
