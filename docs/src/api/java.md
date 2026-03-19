# Java API Reference

The Java binding provides `com.stateset.embedded.Commerce` for building commerce applications.

## Installation

### Maven

```xml
<dependency>
    <groupId>com.stateset</groupId>
    <artifactId>stateset-embedded</artifactId>
    <version>0.8.1</version>
</dependency>
```

### Gradle

```groovy
implementation 'com.stateset:stateset-embedded:0.8.1'
```

## Quick Start

```java
import com.stateset.embedded.Commerce;
import com.stateset.embedded.models.*;

public class Example {
    public static void main(String[] args) {
        // Initialize with SQLite database
        try (Commerce commerce = new Commerce("commerce.db")) {

            // Create a customer
            Customer customer = commerce.customers().create(
                "alice@example.com",
                "Alice",
                "Smith",
                "+1-555-0123"
            );

            // Create a product
            Product product = commerce.products().create(
                "Premium Widget",
                "WIDGET-001",
                29.99,
                "High-quality widget"
            );

            // Create inventory
            InventoryItem item = commerce.inventory().createItem(
                "WIDGET-001",
                "Premium Widget",
                100
            );

            // Create an order
            Order order = commerce.orders().create(
                customer.getId(),
                List.of(new OrderItem("WIDGET-001", "Widget", 2, 29.99)),
                "USD"
            );

            // Ship the order
            Order shipped = commerce.orders().ship(order.getId());
            System.out.println("Order " + shipped.getOrderNumber() + " shipped!");
        }
    }
}
```

## Common Operations

### Customer Management

```java
// Create customer
Customer customer = commerce.customers().create(
    "test@example.com",
    "Test",
    "User"
);

// Get customer by ID
Customer found = commerce.customers().get(customerId);

// List all customers
List<Customer> customers = commerce.customers().list();

// Delete customer
boolean deleted = commerce.customers().delete(customerId);
```

### Inventory Management

```java
// Create inventory item
InventoryItem item = commerce.inventory().createItem(
    "SKU-001",
    "Widget",
    100
);

// Adjust inventory
commerce.inventory().adjust("SKU-001", 50, "Received shipment");

// Reserve inventory
Reservation reservation = commerce.inventory().reserve("SKU-001", 10, null);

// Release reservation
commerce.inventory().release(reservation.getId());

// Get stock level
StockLevel level = commerce.inventory().getLevel("SKU-001");
System.out.println("Available: " + level.getAvailable());
```

### Order Processing

```java
// Create order
Order order = commerce.orders().create(
    customer.getId(),
    List.of(new OrderItem("SKU-001", "Widget", 2, 29.99)),
    "USD"
);

// Update status
commerce.orders().updateStatus(order.getId(), "processing");

// Ship order
Order shipped = commerce.orders().ship(order.getId());

// Cancel order
Order cancelled = commerce.orders().cancel(order.getId());

// List orders by status
List<Order> pending = commerce.orders().listByStatus("pending");
```

### Subscriptions

```java
// Create a subscription plan
SubscriptionPlan plan = commerce.subscriptions().createPlan(
    "PREMIUM",
    "Premium Plan",
    "month",
    1,
    19.99,
    "USD"
);

// Subscribe a customer
Subscription subscription = commerce.subscriptions().subscribe(
    customer.getId(),
    plan.getId()
);

// Pause/Resume/Cancel
commerce.subscriptions().pause(subscription.getId());
commerce.subscriptions().resume(subscription.getId());
commerce.subscriptions().cancel(subscription.getId());
```

### Analytics

```java
// Get sales summary
SalesSummary summary = commerce.analytics().salesSummary();
System.out.println("Total revenue: " + summary.getTotalRevenue());

// Get top products
List<TopProduct> topProducts = commerce.analytics().topProducts(10);

// Get top customers
List<TopCustomer> topCustomers = commerce.analytics().topCustomers(10);
```

## Error Handling

```java
try {
    Order order = commerce.orders().ship(orderId);
} catch (StateSetException e) {
    System.err.println("StateSet error: " + e.getMessage());
} catch (Exception e) {
    throw e;
}
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers()` | Customer management |
| `products()` | Product catalog |
| `orders()` | Order lifecycle |
| `inventory()` | Stock management |
| `carts()` | Shopping carts |
| `returns()` | Return processing |
| `payments()` | Payment operations |
| `shipments()` | Shipping management |
| `warranties()` | Warranty tracking |
| `suppliers()` | Supplier management |
| `purchaseOrders()` | Purchase orders |
| `invoices()` | B2B invoicing |
| `bom()` | Bills of Materials |
| `workOrders()` | Manufacturing |
| `currency()` | Multi-currency |
| `subscriptions()` | Recurring billing |
| `promotions()` | Discounts & coupons |
| `tax()` | Tax calculations |
| `quality()` | Quality control |
| `lots()` | Lot tracking |
| `serials()` | Serial numbers |
| `warehouse()` | Warehouse ops |
| `receiving()` | Receiving |
| `fulfillment()` | Picking & packing |
| `accountsPayable()` | A/P management |
| `accountsReceivable()` | A/R management |
| `costAccounting()` | Cost tracking |
| `credit()` | Credit management |
| `backorders()` | Backorder tracking |
| `generalLedger()` | GL accounting |
| `analytics()` | Reporting & forecasts |

## Platform Support

| Platform | Architectures | Status |
|----------|---------------|--------|
| Linux | x64, arm64 | Supported |
| macOS | x64, arm64 | Supported |
| Windows | x64 | Supported |

## Source Files

- Entry point: `com.stateset.embedded.Commerce`
- Sources: `bindings/java/java/src/main/java/com/stateset/embedded/`

## Examples

- `examples/java/BasicUsage.java`
