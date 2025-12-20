# StateSet iCommerce v0.1.6 Release Notes

## Highlights

This release adds **Java bindings** to the StateSet commerce engine, bringing the total to **7 language bindings** across all major platforms. Combined with Ruby and PHP bindings from v0.1.5, StateSet now covers the entire enterprise e-commerce ecosystem.

## New Features

### Java Bindings (JNI)

Full Java support for enterprise e-commerce platforms:

```java
import com.stateset.embedded.*;

try (Commerce commerce = new Commerce("commerce.db")) {
    // Create a customer
    Customer customer = commerce.customers().create(
        "alice@example.com", "Alice", "Smith"
    );

    // Create a product with variant
    Product product = commerce.products().create(
        "WIDGET-001", "Premium Widget", 29.99
    );

    // Shopping cart workflow
    Cart cart = commerce.carts().create(customer.getId(), "USD");
    cart = commerce.carts().addItem(cart.getId(), "WIDGET-001", "Premium Widget", 2, 29.99);
    Order order = commerce.carts().checkout(cart.getId());

    // Get analytics
    SalesSummary summary = commerce.analytics().salesSummary(30);
    System.out.println("Revenue: $" + summary.getTotalRevenue());
}
```

**Installation:**

Maven:
```xml
<dependency>
    <groupId>com.stateset</groupId>
    <artifactId>embedded</artifactId>
    <version>0.1.6</version>
</dependency>
```

Gradle:
```groovy
implementation 'com.stateset:embedded:0.1.6'
```

**Framework Integration:**
- Spring Boot
- Micronaut
- Quarkus
- SAP Hybris / SAP Commerce Cloud
- Broadleaf Commerce

### Ruby Bindings (Magnus)

Native Ruby extension for Rails and other Ruby frameworks:

```ruby
require 'stateset_embedded'

commerce = StateSet::Commerce.new('./store.db')

customer = commerce.customers.create(
  email: 'alice@example.com',
  first_name: 'Alice',
  last_name: 'Smith'
)

order = commerce.orders.create(
  customer_id: customer.id,
  items: [{ sku: 'SKU-001', name: 'Widget', quantity: 2, unit_price: 29.99 }]
)
```

**Installation:**
```bash
gem install stateset_embedded
```

### PHP Bindings (ext-php-rs)

Native PHP extension for Laravel, Magento, and WooCommerce:

```php
<?php
use StateSet\Commerce;

$commerce = new Commerce('./store.db');

$customer = $commerce->customers()->create(
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith'
);

$order = $commerce->orders()->create(
    customerId: $customer->getId(),
    items: [['sku' => 'SKU-001', 'name' => 'Widget', 'quantity' => 2, 'unit_price' => 29.99]]
);
```

**Installation:**
```bash
composer require stateset/embedded
```

## Language Bindings Summary

| Language | Package | Install |
|----------|---------|---------|
| Rust | `stateset-embedded` | `cargo add stateset-embedded` |
| Node.js | `@stateset/embedded` | `npm install @stateset/embedded` |
| Python | `stateset-embedded` | `pip install stateset-embedded` |
| Ruby | `stateset_embedded` | `gem install stateset_embedded` |
| PHP | `stateset/embedded` | `composer require stateset/embedded` |
| Java | `com.stateset:embedded` | Maven/Gradle |
| WASM | `@stateset/embedded-wasm` | `npm install @stateset/embedded-wasm` |

## Platform Support

| Platform | Node.js | Python | Ruby | PHP | Java |
|----------|---------|--------|------|-----|------|
| Linux x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Linux arm64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS arm64 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Windows x86_64 | ✅ | ✅ | ✅ | ✅ | ✅ |

## Stats

| Metric | v0.1.5 | v0.1.6 |
|--------|--------|--------|
| Lines of Code | ~147,000 | ~150,000 |
| Language Bindings | 6 | 7 |
| Domain Models | 254 | 254 |
| Database Tables | 53 | 53 |
| API Methods | 670+ | 670+ |
| MCP Tools | 90 | 90 |
| AI Agents | 8 | 8 |

## APIs Included

All bindings include these core APIs:

- **Customers** - Customer management
- **Products** - Product catalog with variants
- **Orders** - Order lifecycle management
- **Inventory** - Stock tracking and reservations
- **Carts** - Shopping cart and checkout (ACP)
- **Payments** - Payment processing
- **Returns** - RMA and refund processing
- **Analytics** - Sales summaries and metrics
- **Tax** - Multi-jurisdiction tax calculation
- **Promotions** - Discounts and coupon codes
- **Subscriptions** - Recurring billing

## Breaking Changes

None.

## Bug Fixes

- Fixed JNI memory management for thread-safe Commerce handle access
- Updated Product API to properly handle variants (SKU/price on variants, not product)
- Fixed Cart total calculation using `grand_total` field

## Upgrade Guide

Update your dependencies to v0.1.6:

**Rust:**
```toml
stateset-embedded = "0.1.6"
```

**Node.js:**
```bash
npm install @stateset/embedded@0.1.6
```

**Python:**
```bash
pip install stateset-embedded==0.1.6
```

**Ruby:**
```bash
gem install stateset_embedded -v 0.1.6
```

**PHP:**
```bash
composer require stateset/embedded:0.1.6
```

**Java:**
```xml
<version>0.1.6</version>
```

## What's Next

- Go bindings (cgo)
- C#/.NET bindings
- Kotlin-specific DSL
- GraphQL API layer

## Contributors

- StateSet Team

---

Built with Rust for reliability, designed for AI agents.
