# WASM API Reference

The WebAssembly binding provides `Commerce` for building commerce applications in browsers and Node.js.

## Installation

```bash
npm install @stateset/embedded-wasm
# or
yarn add @stateset/embedded-wasm
```

## Quick Start (Browser)

```typescript
import init, { Commerce } from '@stateset/embedded-wasm';

async function main() {
    // Initialize WASM module
    await init();

    // Create commerce instance (uses IndexedDB for persistence)
    const commerce = new Commerce(':memory:');

    // Create a customer
    const customer = commerce.customers.create({
        email: 'alice@example.com',
        firstName: 'Alice',
        lastName: 'Smith',
        phone: '+1-555-0123'
    });

    // Create a product
    const product = commerce.products.create({
        name: 'Premium Widget',
        sku: 'WIDGET-001',
        price: 29.99,
        description: 'High-quality widget'
    });

    // Create an order
    const order = commerce.orders.create({
        customerId: customer.id,
        items: [
            { sku: 'WIDGET-001', name: 'Widget', quantity: 2, unitPrice: 29.99 }
        ],
        currency: 'USD'
    });

    // Ship the order
    const shipped = commerce.orders.ship(order.id);
    console.log(`Order ${shipped.orderNumber} shipped!`);
}

main();
```

## Quick Start (Node.js)

```javascript
const { Commerce } = require('@stateset/embedded-wasm');

async function main() {
    // Create commerce instance with file-based SQLite
    const commerce = new Commerce('commerce.db');

    // Create a customer
    const customer = commerce.customers.create({
        email: 'alice@example.com',
        firstName: 'Alice',
        lastName: 'Smith'
    });

    console.log(`Created customer: ${customer.id}`);
}

main();
```

## Common Operations

### Customer Management

```typescript
// Create customer
const customer = commerce.customers.create({
    email: 'test@example.com',
    firstName: 'Test',
    lastName: 'User'
});

// Get customer by ID
const found = commerce.customers.get(customerId);

// List all customers
const customers = commerce.customers.list();

// Delete customer
const deleted = commerce.customers.delete(customerId);
```

### Inventory Management

```typescript
// Create inventory item
const item = commerce.inventory.createItem({
    sku: 'SKU-001',
    name: 'Widget',
    initialQuantity: 100
});

// Adjust inventory
commerce.inventory.adjust('SKU-001', 50, 'Received shipment');

// Get stock level
const level = commerce.inventory.getLevel('SKU-001');
console.log(`Available: ${level.available}`);
```

### Order Processing

```typescript
// Create order
const order = commerce.orders.create({
    customerId: customer.id,
    items: [
        { sku: 'SKU-001', name: 'Widget', quantity: 2, unitPrice: 29.99 }
    ]
});

// Ship order
const shipped = commerce.orders.ship(order.id);

// Cancel order
const cancelled = commerce.orders.cancel(order.id);
```

### Subscriptions

```typescript
// Create a subscription plan
const plan = commerce.subscriptions.createPlan({
    code: 'PREMIUM',
    name: 'Premium Plan',
    interval: 'month',
    intervalCount: 1,
    price: 19.99,
    currency: 'USD'
});

// Subscribe a customer
const subscription = commerce.subscriptions.subscribe(customer.id, plan.id);

// Pause/Resume/Cancel
commerce.subscriptions.pause(subscription.id);
commerce.subscriptions.resume(subscription.id);
commerce.subscriptions.cancel(subscription.id);
```

### Analytics

```typescript
// Get sales summary
const summary = commerce.analytics.salesSummary();
console.log(`Total revenue: ${summary.totalRevenue}`);

// Get top products
const topProducts = commerce.analytics.topProducts(10);

// Get top customers
const topCustomers = commerce.analytics.topCustomers(10);
```

## React Integration

```tsx
import { useEffect, useState } from 'react';
import init, { Commerce } from '@stateset/embedded-wasm';

function useCommerce() {
    const [commerce, setCommerce] = useState<Commerce | null>(null);

    useEffect(() => {
        async function initCommerce() {
            await init();
            setCommerce(new Commerce(':memory:'));
        }
        initCommerce();
    }, []);

    return commerce;
}

function App() {
    const commerce = useCommerce();
    const [customers, setCustomers] = useState([]);

    useEffect(() => {
        if (commerce) {
            setCustomers(commerce.customers.list());
        }
    }, [commerce]);

    return (
        <ul>
            {customers.map(c => (
                <li key={c.id}>{c.email}</li>
            ))}
        </ul>
    );
}
```

## Error Handling

```typescript
try {
    const order = commerce.orders.ship(orderId);
} catch (error) {
    if (error instanceof Error) {
        console.error(`Error: ${error.message}`);
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

## Bundle Size

The WASM bundle is approximately 2MB gzipped and includes:
- Full SQLite implementation
- All 31 commerce APIs
- In-memory and file-based storage

## Platform Support

| Platform | Status |
|----------|--------|
| Modern browsers | Supported |
| Node.js 16+ | Supported |
| Deno | Supported |
| Cloudflare Workers | Supported |

## Source Files

- Package: `@stateset/embedded-wasm`
- Types: `bindings/wasm/pkg/stateset_embedded.d.ts`
- Node types: `bindings/wasm/pkg-node/*.d.ts`

## Examples

- `bindings/wasm/README.md`
- `examples/wasm/browser/`
- `examples/wasm/node/`
