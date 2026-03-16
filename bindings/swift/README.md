# StateSet Swift Binding

**The SQLite of Commerce** - Embedded commerce engine for Swift and iOS applications.

[![Swift](https://img.shields.io/badge/Swift-5.5+-orange.svg)](https://swift.org/)
[![Platform](https://img.shields.io/badge/Platform-iOS%20%7C%20macOS%20%7C%20Linux-blue.svg)](https://developer.apple.com/)

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/stateset/stateset-icommerce", from: "0.8.0")
]
```

Or in Xcode: File > Add Packages > Enter the repository URL.

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
```

## API Reference

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

## Common Operations

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

// Pause subscription
let paused = try commerce.subscriptions.pause(id: subscription.id)

// Resume subscription
let resumed = try commerce.subscriptions.resume(id: subscription.id)
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

// Create a coupon code
let coupon = try commerce.promotions.createCoupon(
    promotionId: promo.id,
    code: "SAVE20NOW",
    maxUses: 100
)

// Validate a coupon
if let valid = try commerce.promotions.validateCoupon(code: "SAVE20NOW") {
    print("Coupon is valid: \(valid.code)")
}
```

### Tax Calculations

```swift
// Get effective tax rate
let rate = try commerce.tax.getEffectiveRate(
    country: "US",
    state: "CA",
    category: "general"
)

// Create tax exemption
let exemption = try commerce.tax.createExemption(
    customerId: customer.id,
    exemptionType: "resale",
    effectiveFrom: "2024-01-01"
)
```

### Warehouse & Fulfillment

```swift
// Create warehouse
let warehouse = try commerce.warehouse.createWarehouse(
    code: "WH-001",
    name: "Main Warehouse",
    warehouseType: "distribution"
)

// Create fulfillment wave
let wave = try commerce.fulfillment.createWave(
    warehouseId: warehouse.id,
    orderIds: [order.id],
    priority: 1
)

// Release wave for picking
let released = try commerce.fulfillment.releaseWave(id: wave.id)
```

## Error Handling

```swift
do {
    let customer = try commerce.customers.create(
        email: "test@example.com",
        firstName: "Test",
        lastName: "User"
    )
} catch StateSetError.initializationFailed(let msg) {
    print("Failed to initialize: \(msg)")
} catch StateSetError.invalidJSON(let msg) {
    print("Invalid response: \(msg)")
} catch {
    print("Error: \(error)")
}
```

## Platform Support

| Platform | Architectures | Status |
|----------|---------------|--------|
| macOS 12+ | x86_64, arm64 | Supported |
| iOS 15+ | arm64 | Supported |
| Linux | x86_64, arm64 | Supported |
| Windows | x86_64 | Planned |

## Thread Safety

`StateSetCommerce` is thread-safe. SQLite operations are serialized internally via connection pooling.

## License

MIT OR Apache-2.0
