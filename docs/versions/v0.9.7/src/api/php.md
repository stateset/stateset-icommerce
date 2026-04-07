# PHP API Reference

The PHP extension provides `StateSet\Commerce` and related classes for building commerce applications.

## Installation

### PECL (Coming Soon)

```bash
pecl install stateset-embedded
```

### Manual Installation

Build from source:

```bash
cd bindings/php
cargo build --release
# Copy libstateset_php.so to your PHP extensions directory
```

Add to `php.ini`:

```ini
extension=stateset_php.so
```

## Quick Start

```php
<?php

use StateSet\Commerce;

// Initialize with SQLite database
$commerce = new Commerce("commerce.db");

// Or use in-memory database for testing
$commerce = new Commerce(":memory:");

// Create a customer
$customer = $commerce->customers()->create(
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith",
    phone: "+1-555-0123"
);

// Create a product
$product = $commerce->products()->create(
    name: "Premium Widget",
    sku: "WIDGET-001",
    price: 29.99,
    description: "High-quality widget"
);

// Create inventory
$item = $commerce->inventory()->createItem(
    sku: "WIDGET-001",
    name: "Premium Widget",
    initialQuantity: 100
);

// Create an order
$order = $commerce->orders()->create(
    customerId: $customer->id,
    items: [
        ["sku" => "WIDGET-001", "name" => "Widget", "quantity" => 2, "unit_price" => 29.99]
    ],
    currency: "USD"
);

// Ship the order
$shipped = $commerce->orders()->ship($order->id);
echo "Order {$shipped->orderNumber} shipped!\n";
```

## Common Operations

### Customer Management

```php
// Create customer
$customer = $commerce->customers()->create(
    email: "test@example.com",
    firstName: "Test",
    lastName: "User"
);

// Get customer by ID
$found = $commerce->customers()->get($customerId);

// List all customers
$customers = $commerce->customers()->list();

// Delete customer
$deleted = $commerce->customers()->delete($customerId);
```

### Inventory Management

```php
// Create inventory item
$item = $commerce->inventory()->createItem(
    sku: "SKU-001",
    name: "Widget",
    initialQuantity: 100
);

// Adjust inventory
$commerce->inventory()->adjust("SKU-001", 50, "Received shipment");

// Reserve inventory
$reservation = $commerce->inventory()->reserve("SKU-001", 10);

// Release reservation
$commerce->inventory()->release($reservation->id);

// Get stock level
$level = $commerce->inventory()->getLevel("SKU-001");
echo "Available: {$level->available}\n";
```

### Order Processing

```php
// Create order
$order = $commerce->orders()->create(
    customerId: $customer->id,
    items: [
        ["sku" => "SKU-001", "name" => "Widget", "quantity" => 2, "unit_price" => 29.99]
    ]
);

// Update status
$commerce->orders()->updateStatus($order->id, "processing");

// Ship order
$shipped = $commerce->orders()->ship($order->id);

// Cancel order
$cancelled = $commerce->orders()->cancel($order->id);

// List orders by status
$pending = $commerce->orders()->listByStatus("pending");
```

### Subscriptions

```php
// Create a subscription plan
$plan = $commerce->subscriptions()->createPlan(
    code: "PREMIUM",
    name: "Premium Plan",
    interval: "month",
    intervalCount: 1,
    price: 19.99,
    currency: "USD"
);

// Subscribe a customer
$subscription = $commerce->subscriptions()->subscribe($customer->id, $plan->id);

// Pause/Resume/Cancel
$paused = $commerce->subscriptions()->pause($subscription->id);
$resumed = $commerce->subscriptions()->resume($subscription->id);
$cancelled = $commerce->subscriptions()->cancel($subscription->id);
```

### Promotions

```php
// Create a promotion
$promo = $commerce->promotions()->create(
    code: "SUMMER20",
    name: "Summer Sale",
    discountType: "percentage",
    discountValue: 20.0
);

// Activate promotion
$commerce->promotions()->activate($promo->id);

// Create a coupon
$coupon = $commerce->promotions()->createCoupon($promo->id, "SAVE20NOW", 100);

// Validate coupon
$valid = $commerce->promotions()->validateCoupon("SAVE20NOW");
```

### Tax

```php
// Get effective tax rate
$rate = $commerce->tax()->getEffectiveRate("US", "CA", "general");

// Create tax exemption
$exemption = $commerce->tax()->createExemption(
    $customer->id,
    "resale",
    "2024-01-01"
);
```

### Analytics

```php
// Get sales summary
$summary = $commerce->analytics()->salesSummary();
echo "Total revenue: {$summary->totalRevenue}\n";

// Get top products
$topProducts = $commerce->analytics()->topProducts(10);

// Get top customers
$topCustomers = $commerce->analytics()->topCustomers(10);
```

## Error Handling

```php
try {
    $order = $commerce->orders()->ship($orderId);
} catch (StateSetException $e) {
    echo "StateSet error: {$e->getMessage()}\n";
} catch (Exception $e) {
    throw $e;
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

| Platform | PHP Versions | Status |
|----------|--------------|--------|
| Linux | 8.1+ | Supported |
| macOS | 8.1+ | Supported |
| Windows | 8.1+ | Supported |

## Source Files

- Entry point: `StateSet\Commerce`
- PHP stubs: `bindings/php/stubs/StateSet.php`
- Rust source: `bindings/php/src/lib.rs`

## Examples

- `bindings/php/README.md`
- `examples/php/basic_usage.php`
