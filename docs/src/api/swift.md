# Swift API Reference

The Swift package provides `StateSetCommerce` for building commerce applications on Apple platforms.

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/stateset/stateset-embedded-swift", from: "0.6.0")
]
```

Or in Xcode: File > Add Package Dependencies and enter the repository URL.

## Quick Start

```swift
import StateSet

// Initialize with SQLite database
let commerce = try StateSetCommerce(dbPath: "commerce.db")

// Or use in-memory database for testing
let commerce = try StateSetCommerce(dbPath: ":memory:")

// Create a customer
let customer = try commerce.customers.create(
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith",
    phone: "+1-555-0123"
)

// Create a product
let product = try commerce.products.create(
    name: "Premium Widget",
    sku: "WIDGET-001",
    price: 29.99,
    description: "High-quality widget"
)

// Create inventory
let item = try commerce.inventory.createItem(
    sku: "WIDGET-001",
    name: "Premium Widget",
    initialQuantity: 100
)

// Create an order
let order = try commerce.orders.create(
    customerId: customer.id,
    items: [
        OrderItem(sku: "WIDGET-001", name: "Widget", quantity: 2, unitPrice: 29.99)
    ],
    currency: "USD"
)

// Ship the order
let shipped = try commerce.orders.ship(id: order.id)
print("Order \(shipped.orderNumber) shipped!")

// Clean up
commerce.close()
```

## Common Operations

### Customer Management

```swift
// Create customer
let customer = try commerce.customers.create(
    email: "test@example.com",
    firstName: "Test",
    lastName: "User"
)

// Get customer by ID
let found = try commerce.customers.get(id: customerId)

// List all customers
let customers = try commerce.customers.list()

// Delete customer
let deleted = try commerce.customers.delete(id: customerId)
```

### Inventory Management

```swift
// Create inventory item
let item = try commerce.inventory.createItem(
    sku: "SKU-001",
    name: "Widget",
    initialQuantity: 100
)

// Adjust inventory
try commerce.inventory.adjust(
    sku: "SKU-001",
    delta: 50,
    reason: "Received shipment"
)

// Get stock level
let level = try commerce.inventory.getLevel(sku: "SKU-001")
print("Available: \(level.available)")
```

### Order Processing

```swift
// Create order
let order = try commerce.orders.create(
    customerId: customer.id,
    items: [
        OrderItem(sku: "SKU-001", name: "Widget", quantity: 2, unitPrice: 29.99)
    ]
)

// Ship order
let shipped = try commerce.orders.ship(id: order.id)

// Cancel order
let cancelled = try commerce.orders.cancel(id: order.id)
```

### Subscriptions

```swift
// Create a subscription plan
let plan = try commerce.subscriptions.createPlan(
    code: "PREMIUM",
    name: "Premium Plan",
    interval: "month",
    intervalCount: 1,
    price: 19.99,
    currency: "USD"
)

// Subscribe a customer
let subscription = try commerce.subscriptions.subscribe(
    customerId: customer.id,
    planId: plan.id
)

// Pause/Resume/Cancel
let paused = try commerce.subscriptions.pause(id: subscription.id)
let resumed = try commerce.subscriptions.resume(id: subscription.id)
let cancelled = try commerce.subscriptions.cancel(id: subscription.id)
```

### Promotions

```swift
// Create a promotion
let promo = try commerce.promotions.create(
    code: "SUMMER20",
    name: "Summer Sale",
    discountType: "percentage",
    discountValue: 20.0
)

// Activate promotion
try commerce.promotions.activate(id: promo.id)

// Create a coupon
let coupon = try commerce.promotions.createCoupon(
    promotionId: promo.id,
    code: "SAVE20NOW",
    maxUses: 100
)

// Validate coupon
let validation = try commerce.promotions.validateCoupon(code: "SAVE20NOW")
```

### Analytics

```swift
// Get sales summary
let summary = try commerce.analytics.salesSummary()
print("Total revenue: \(summary.totalRevenue)")

// Get top products
let topProducts = try commerce.analytics.topProducts(limit: 10)

// Get top customers
let topCustomers = try commerce.analytics.topCustomers(limit: 10)
```

## Error Handling

```swift
do {
    let order = try commerce.orders.ship(id: orderId)
} catch let error as StateSetError {
    print("StateSet error: \(error.message)")
} catch {
    print("Unknown error: \(error)")
}
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers` | Customer management |
| `products` | Product catalog |
| `orders` | Order lifecycle |
| `inventory` | Stock management |
| `carts` | Shopping carts |
| `returns` | Return processing |
| `payments` | Payment operations |
| `shipments` | Shipping management |
| `warranties` | Warranty tracking |
| `suppliers` | Supplier management |
| `purchaseOrders` | Purchase orders |
| `invoices` | B2B invoicing |
| `bom` | Bills of Materials |
| `workOrders` | Manufacturing |
| `currency` | Multi-currency |
| `subscriptions` | Recurring billing |
| `promotions` | Discounts & coupons |
| `tax` | Tax calculations |
| `quality` | Quality control |
| `lots` | Lot tracking |
| `serials` | Serial numbers |
| `warehouse` | Warehouse ops |
| `receiving` | Receiving |
| `fulfillment` | Picking & packing |
| `accountsPayable` | A/P management |
| `accountsReceivable` | A/R management |
| `costAccounting` | Cost tracking |
| `credit` | Credit management |
| `backorders` | Backorder tracking |
| `generalLedger` | GL accounting |
| `analytics` | Reporting & forecasts |

## Platform Support

| Platform | Architectures | Status |
|----------|---------------|--------|
| macOS | x64, arm64 | Supported |
| iOS | arm64 | Supported |
| iOS Simulator | x64, arm64 | Supported |

## Source Files

- Entry point: `StateSetCommerce`
- Sources: `bindings/swift/Sources/StateSet/`

## Examples

- `examples/swift/BasicUsage.swift`
