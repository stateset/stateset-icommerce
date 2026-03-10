# StateSet Embedded Commerce for PHP

[![Packagist Version](https://img.shields.io/packagist/v/stateset/embedded)](https://packagist.org/packages/stateset/embedded)
[![PHP Version](https://img.shields.io/packagist/php-v/stateset/embedded)](https://packagist.org/packages/stateset/embedded)

Local-first commerce engine for PHP. Provides a complete commerce API with embedded SQLite storage.

## Installation

### Via Composer

```bash
composer require stateset/embedded
```

### Installing the Native Extension

The native extension provides significant performance benefits. Install pre-built binaries:

```bash
composer install-extension
```

Or manually download from [GitHub Releases](https://github.com/stateset/stateset-icommerce/releases).

Then add to your `php.ini`:

```ini
extension=stateset_embedded
```

## Quick Start

```php
<?php
require_once 'vendor/autoload.php';

use StateSet\Commerce;

// Create commerce instance (uses SQLite)
$commerce = new Commerce('commerce.db');
// Or use in-memory database
$commerce = new Commerce(':memory:');

// Create a customer
$customer = $commerce->customers()->create(
    email: 'john@example.com',
    firstName: 'John',
    lastName: 'Doe'
);

// Create a product
$product = $commerce->products()->create(
    name: 'Widget Pro',
    description: 'The best widget ever'
);

// Create an order
$order = $commerce->orders()->create(
    customerId: $customer->getId(),
    items: [
        ['sku' => 'WIDGET-001', 'name' => 'Widget Pro', 'quantity' => 2, 'unit_price' => 29.99]
    ]
);

// Process the order
$commerce->orders()->confirm($order->getId());
$commerce->orders()->ship($order->getId(), trackingNumber: '1Z999AA10123456784');
$commerce->orders()->deliver($order->getId());

// Get sales analytics
$summary = $commerce->analytics()->salesSummary(days: 30);
echo "Total Revenue: $" . $summary->getTotalRevenue();
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers()` | Customer management |
| `orders()` | Order processing |
| `products()` | Product catalog |
| `inventory()` | Stock management |
| `returns()` | Return requests |
| `payments()` | Payment recording |
| `carts()` | Shopping carts |
| `analytics()` | Sales reports |
| `shipments()` | Shipment tracking |
| `warranties()` | Warranty management |
| `purchase_orders()` | Supplier orders |
| `invoices()` | Invoice management |
| `bom()` | Bills of materials |
| `work_orders()` | Manufacturing |
| `currency()` | Currency conversion |
| `subscriptions()` | Recurring billing |
| `promotions()` | Discount codes |
| `tax()` | Tax calculations |

## Laravel Integration

```php
// config/services.php
return [
    'stateset' => [
        'database' => storage_path('stateset/commerce.db'),
    ],
];

// app/Providers/StateSetServiceProvider.php
use StateSet\Commerce;

public function register()
{
    $this->app->singleton(Commerce::class, function ($app) {
        return new Commerce(config('services.stateset.database'));
    });
}

// Usage in controllers
public function store(Request $request, Commerce $commerce)
{
    $order = $commerce->orders()->create(
        customerId: $request->customer_id,
        items: $request->items
    );

    return response()->json($order);
}
```

## Building from Source

Requirements:
- PHP 8.1+
- Rust toolchain
- PHP development headers (`php-dev` / `php-devel`)

Default `cargo` builds compile a portable stub so CI can validate the crate
without a local PHP runtime. Build the actual extension with the `runtime`
feature enabled:

```bash
cd bindings/php
cargo build --release --features runtime

# The extension will be at:
# target/release/libstateset_embedded.so (Linux)
# target/release/libstateset_embedded.dylib (macOS)
# target/release/stateset_embedded.dll (Windows)
```

## Running Tests

```bash
composer test
```

## Platform Support

| Platform | PHP Versions | Status |
|----------|--------------|--------|
| Linux x86_64 | 8.1, 8.2, 8.3 | ✅ |
| Linux arm64 | 8.2, 8.3 | ✅ |
| macOS x86_64 | 8.2 | ✅ |
| macOS arm64 | 8.2, 8.3 | ✅ |
| Windows x86_64 | 8.2, 8.3 | ✅ |

## License

MIT
