# Kotlin API Reference

The Kotlin binding provides `StateSetCommerce` with idiomatic Kotlin APIs for building commerce applications.

## Installation

### Gradle (Kotlin DSL)

```kotlin
implementation("com.stateset:stateset-embedded:0.7.0")
```

### Gradle (Groovy)

```groovy
implementation 'com.stateset:stateset-embedded:0.7.0'
```

## Quick Start

```kotlin
import com.stateset.embedded.StateSetCommerce
import com.stateset.embedded.models.*

fun main() {
    // Initialize with SQLite database
    StateSetCommerce("commerce.db").use { commerce ->

        // Create a customer
        val customer = commerce.customers.create(
            email = "alice@example.com",
            firstName = "Alice",
            lastName = "Smith",
            phone = "+1-555-0123"
        )

        // Create a product
        val product = commerce.products.create(
            name = "Premium Widget",
            sku = "WIDGET-001",
            price = 29.99,
            description = "High-quality widget"
        )

        // Create inventory
        val item = commerce.inventory.createItem(
            sku = "WIDGET-001",
            name = "Premium Widget",
            initialQuantity = 100
        )

        // Create an order
        val order = commerce.orders.create(
            customerId = customer.id,
            items = listOf(
                OrderItem(sku = "WIDGET-001", name = "Widget", quantity = 2, unitPrice = 29.99)
            ),
            currency = "USD"
        )

        // Ship the order
        val shipped = commerce.orders.ship(order.id)
        println("Order ${shipped.orderNumber} shipped!")
    }
}
```

## Common Operations

### Customer Management

```kotlin
// Create customer
val customer = commerce.customers.create(
    email = "test@example.com",
    firstName = "Test",
    lastName = "User"
)

// Get customer by ID
val found = commerce.customers.get(customerId)

// List all customers
val customers = commerce.customers.list()

// Delete customer
val deleted = commerce.customers.delete(customerId)
```

### Inventory Management

```kotlin
// Create inventory item
val item = commerce.inventory.createItem(
    sku = "SKU-001",
    name = "Widget",
    initialQuantity = 100
)

// Adjust inventory
commerce.inventory.adjust("SKU-001", 50, "Received shipment")

// Reserve inventory
val reservation = commerce.inventory.reserve("SKU-001", 10)

// Release reservation
commerce.inventory.release(reservation.id)

// Get stock level
val level = commerce.inventory.getLevel("SKU-001")
println("Available: ${level.available}")
```

### Order Processing

```kotlin
// Create order
val order = commerce.orders.create(
    customerId = customer.id,
    items = listOf(
        OrderItem(sku = "SKU-001", name = "Widget", quantity = 2, unitPrice = 29.99)
    )
)

// Update status
commerce.orders.updateStatus(order.id, "processing")

// Ship order
val shipped = commerce.orders.ship(order.id)

// Cancel order
val cancelled = commerce.orders.cancel(order.id)

// List orders by status
val pending = commerce.orders.listByStatus("pending")
```

### Subscriptions

```kotlin
// Create a subscription plan
val plan = commerce.subscriptions.createPlan(
    code = "PREMIUM",
    name = "Premium Plan",
    interval = "month",
    intervalCount = 1,
    price = 19.99,
    currency = "USD"
)

// Subscribe a customer
val subscription = commerce.subscriptions.subscribe(
    customerId = customer.id,
    planId = plan.id
)

// Pause/Resume/Cancel
commerce.subscriptions.pause(subscription.id)
commerce.subscriptions.resume(subscription.id)
commerce.subscriptions.cancel(subscription.id)
```

### Analytics

```kotlin
// Get sales summary
val summary = commerce.analytics.salesSummary()
println("Total revenue: ${summary.totalRevenue}")

// Get top products
val topProducts = commerce.analytics.topProducts(10)

// Get top customers
val topCustomers = commerce.analytics.topCustomers(10)
```

## Error Handling

```kotlin
try {
    val order = commerce.orders.ship(orderId)
} catch (e: StateSetException) {
    println("StateSet error: ${e.message}")
}
```

## Coroutine Support

```kotlin
import kotlinx.coroutines.*

// Run blocking operations on IO dispatcher
suspend fun processOrder(commerce: StateSetCommerce, orderId: String): Order {
    return withContext(Dispatchers.IO) {
        commerce.orders.ship(orderId)
    }
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
| JVM (Linux) | x64, arm64 | Supported |
| JVM (macOS) | x64, arm64 | Supported |
| JVM (Windows) | x64 | Supported |
| Android | arm64 | Supported |

## Source Files

- Entry point: `StateSetCommerce`
- Sources: `bindings/kotlin/kotlin/src/main/kotlin/com/stateset/embedded/`

## Examples

- `examples/kotlin/BasicUsage.kt`
