# C# / .NET API Reference

The .NET binding exposes `StateSetCommerce` and domain APIs for building commerce applications.

## Installation

```bash
dotnet add package StateSet.Embedded
```

## Quick Start

```csharp
using StateSet.Embedded;

// Initialize with SQLite database
using var commerce = new StateSetCommerce("commerce.db");

// Or use in-memory database for testing
using var commerce = new StateSetCommerce(":memory:");

// Create a customer
var customer = commerce.Customers.Create(
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith",
    phone: "+1-555-0123"
);

// Create a product
var product = commerce.Products.Create(
    name: "Premium Widget",
    sku: "WIDGET-001",
    price: 29.99m,
    description: "High-quality widget"
);

// Create inventory
var item = commerce.Inventory.CreateItem(
    sku: "WIDGET-001",
    name: "Premium Widget",
    initialQuantity: 100
);

// Create an order
var order = commerce.Orders.Create(
    customerId: customer.Id,
    items: new[] {
        new OrderItem { Sku = "WIDGET-001", Name = "Widget", Quantity = 2, UnitPrice = 29.99 }
    },
    currency: "USD"
);

// Ship the order
var shipped = commerce.Orders.Ship(order.Id);
Console.WriteLine($"Order {shipped.OrderNumber} shipped!");
```

## Common Operations

### Customer Management

```csharp
// Create customer
var customer = commerce.Customers.Create(
    email: "test@example.com",
    firstName: "Test",
    lastName: "User"
);

// Get customer by ID
var found = commerce.Customers.Get(customerId);

// List all customers
var customers = commerce.Customers.List();

// Delete customer
var deleted = commerce.Customers.Delete(customerId);

// Get customer count
var count = commerce.Customers.Count();
```

### Inventory Management

```csharp
// Create inventory item
var item = commerce.Inventory.CreateItem(
    sku: "SKU-001",
    name: "Widget",
    initialQuantity: 100
);

// Adjust inventory
commerce.Inventory.Adjust(
    sku: "SKU-001",
    quantityDelta: 50,
    reason: "Received shipment"
);

// Get stock level
var level = commerce.Inventory.GetLevel("SKU-001");
Console.WriteLine($"Available: {level.Available}");
```

### Order Processing

```csharp
// Create order
var order = commerce.Orders.Create(
    customerId: customer.Id,
    items: new[] {
        new OrderItem { Sku = "SKU-001", Name = "Widget", Quantity = 2, UnitPrice = 29.99 }
    }
);

// Update status
commerce.Orders.UpdateStatus(order.Id, OrderStatus.Processing);

// Ship order
var shipped = commerce.Orders.Ship(order.Id);

// Cancel order
var cancelled = commerce.Orders.Cancel(order.Id);
```

### Subscriptions

```csharp
// Create a subscription plan
var plan = commerce.Subscriptions.CreatePlan(
    code: "PREMIUM",
    name: "Premium Plan",
    interval: "month",
    intervalCount: 1,
    price: 19.99m,
    currency: "USD"
);

// Subscribe a customer
var subscription = commerce.Subscriptions.Subscribe(
    customerId: customer.Id,
    planId: plan.Id
);

// Pause/Resume/Cancel
var paused = commerce.Subscriptions.Pause(subscription.Id);
var resumed = commerce.Subscriptions.Resume(subscription.Id);
var cancelled = commerce.Subscriptions.Cancel(subscription.Id);
```

### Promotions

```csharp
// Create a promotion
var promo = commerce.Promotions.Create(
    code: "SUMMER20",
    name: "Summer Sale",
    discountType: "percentage",
    discountValue: 20.0m
);

// Create a coupon
var coupon = commerce.Promotions.CreateCoupon(
    promotionId: promo.Id,
    code: "SAVE20NOW",
    maxUses: 100
);

// Validate coupon
var valid = commerce.Promotions.ValidateCoupon("SAVE20NOW");
```

### Tax

```csharp
// Get effective tax rate
var rate = commerce.Tax.GetEffectiveRate(
    country: "US",
    state: "CA",
    category: "general"
);

// Create tax exemption
var exemption = commerce.Tax.CreateExemption(
    customerId: customer.Id,
    exemptionType: "resale",
    effectiveFrom: "2024-01-01"
);
```

### Analytics

```csharp
// Get sales summary
var summary = commerce.Analytics.SalesSummary();
Console.WriteLine($"Total revenue: {summary.TotalRevenue}");

// Get top products
var topProducts = commerce.Analytics.TopProducts(10);

// Get top customers
var topCustomers = commerce.Analytics.TopCustomers(10);
```

## Error Handling

```csharp
try
{
    var customer = commerce.Customers.Create(
        email: "test@example.com",
        firstName: "Test",
        lastName: "User"
    );
}
catch (StateSetException ex)
{
    Console.WriteLine($"Error: {ex.Message}");
}
```

## Available APIs

| API | Description |
|-----|-------------|
| `Customers` | Customer management |
| `Products` | Product catalog |
| `Orders` | Order lifecycle |
| `Inventory` | Stock management |
| `Carts` | Shopping carts |
| `Returns` | Return processing |
| `Payments` | Payment operations |
| `Shipments` | Shipping management |
| `Warranties` | Warranty tracking |
| `Suppliers` | Supplier management |
| `PurchaseOrders` | Purchase orders |
| `Invoices` | B2B invoicing |
| `Bom` | Bills of Materials |
| `WorkOrders` | Manufacturing |
| `Currency` | Multi-currency |
| `Subscriptions` | Recurring billing |
| `Promotions` | Discounts & coupons |
| `Tax` | Tax calculations |
| `Quality` | Quality control |
| `Lots` | Lot tracking |
| `Serials` | Serial numbers |
| `Warehouse` | Warehouse ops |
| `Receiving` | Receiving |
| `Fulfillment` | Picking & packing |
| `AccountsPayable` | A/P management |
| `AccountsReceivable` | A/R management |
| `CostAccounting` | Cost tracking |
| `Credit` | Credit management |
| `Backorders` | Backorder tracking |
| `GeneralLedger` | GL accounting |
| `Analytics` | Reporting & forecasts |

## Platform Support

| Platform | Architectures | Status |
|----------|---------------|--------|
| Windows | x64, arm64 | Supported |
| Linux | x64, arm64 | Supported |
| macOS | x64, arm64 | Supported |

## Source Files

- Entry point: `StateSetCommerce`
- Sources: `bindings/dotnet/dotnet/StateSet/`

## Examples

- `examples/dotnet/BasicUsage.cs`
