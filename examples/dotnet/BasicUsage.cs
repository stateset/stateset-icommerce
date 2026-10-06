// StateSet Embedded Commerce - .NET example.
//
// Build the native engine first, then run:
//   cargo build -p stateset-dotnet --release
//   cd examples/dotnet && dotnet run
//
// Everything below runs inside the Rust engine and is written to
// example-store.db; run it twice and the second run sees the first run's data.

using StateSet.Embedded;

using var commerce = new StateSetCommerce("example-store.db");

Console.WriteLine("=== StateSet Embedded Commerce (.NET) ===\n");

var email = $"alice+{Guid.NewGuid():N}@example.com";
var customer = commerce.Customers.Create(email, "Alice", "Smith", phone: "+1-555-0123");
Console.WriteLine($"Customer: {customer.FullName} <{customer.Email}>");

var widgetSku = $"WIDGET-{Guid.NewGuid():N}"[..15];
var widget = commerce.Products.Create($"Premium Widget {widgetSku}", widgetSku, 29.99m, "A high-quality widget");
commerce.Products.Activate(widget.Id);
commerce.Inventory.CreateItem(widgetSku, "Premium Widget", initialQuantity: 100m);
Console.WriteLine($"Product: {widget.Name} ({widgetSku}), stock {commerce.Inventory.GetStock(widgetSku)!.TotalOnHand}");

var order = commerce.Orders.Create(customer.Id, new[]
{
    new CreateOrderItem { ProductId = widget.Id, Sku = widgetSku, Name = "Premium Widget", Quantity = 3, UnitPrice = 29.99m },
});
Console.WriteLine($"Order {order.OrderNumber}: {order.TotalAmount} {order.Currency} ({order.Status})");

order = commerce.Orders.UpdateStatus(order.Id, OrderStatus.Confirmed);
var payment = commerce.Payments.Create(order.Id, order.TotalAmount, customerId: customer.Id);
payment = commerce.Payments.Complete(payment.Id);
Console.WriteLine($"Payment {payment.PaymentNumber}: {payment.Amount} {payment.Status}");

commerce.Inventory.Adjust(widgetSku, -3m, "order fulfillment");
order = commerce.Orders.Ship(order.Id, "1Z999AA10123456784");
Console.WriteLine($"Shipped with tracking {order.TrackingNumber}; stock now {commerce.Inventory.GetStock(widgetSku)!.TotalOnHand}");

var refund = commerce.Payments.Refund(payment.Id, 10.00m, "goodwill credit");
Console.WriteLine($"Refund {refund.RefundNumber}: {refund.Amount}");

Console.WriteLine($"\nStore now holds {commerce.Customers.Count()} customers and {commerce.Orders.Count()} orders.");
