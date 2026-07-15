# Node.js API Reference

The Node.js binding exports a `Commerce` class from the `@stateset/embedded` package.

## Installation

```bash
npm install @stateset/embedded
# or
yarn add @stateset/embedded
# or
pnpm add @stateset/embedded
```

## Quick Start

```javascript
import { Commerce } from '@stateset/embedded';

// Initialize with SQLite database
const commerce = new Commerce('commerce.db');

// Or use in-memory database for testing
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

// Create inventory
commerce.inventory.createItem({
    sku: 'WIDGET-001',
    name: 'Premium Widget',
    initialQuantity: 100
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
```

## TypeScript Support

Full TypeScript definitions are included:

```typescript
import { Commerce, Customer, Order, OrderItem } from '@stateset/embedded';

const commerce = new Commerce('commerce.db');

const customer: Customer = commerce.customers.create({
    email: 'typed@example.com',
    firstName: 'Typed',
    lastName: 'User'
});

const items: OrderItem[] = [
    { sku: 'SKU-001', name: 'Item', quantity: 1, unitPrice: 19.99 }
];

const order: Order = commerce.orders.create({
    customerId: customer.id,
    items
});
```

## Common Operations

### Customer Management

```javascript
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

```javascript
// Create inventory item
const item = commerce.inventory.createItem({
    sku: 'SKU-001',
    name: 'Widget',
    initialQuantity: 100
});

// Adjust inventory
commerce.inventory.adjust('SKU-001', 50, 'Received shipment');

// Reserve inventory
const reservation = commerce.inventory.reserve('SKU-001', 10);

// Release reservation
commerce.inventory.release(reservation.id);

// Get stock level
const level = commerce.inventory.getLevel('SKU-001');
console.log(`Available: ${level.available}`);
```

### Order Processing

```javascript
// Create order
const order = commerce.orders.create({
    customerId: customer.id,
    items: [
        { sku: 'SKU-001', name: 'Widget', quantity: 2, unitPrice: 29.99 }
    ]
});

// Update status
commerce.orders.updateStatus(order.id, 'processing');

// Ship order
const shipped = commerce.orders.ship(order.id);

// Cancel order
const cancelled = commerce.orders.cancel(order.id);

// List orders by status
const pending = commerce.orders.listByStatus('pending');
```

### Subscriptions

```javascript
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

// Pause subscription
commerce.subscriptions.pause(subscription.id);

// Resume subscription
commerce.subscriptions.resume(subscription.id);

// Cancel subscription
commerce.subscriptions.cancel(subscription.id);
```

### Promotions

```javascript
// Create a promotion
const promo = commerce.promotions.create({
    code: 'SUMMER20',
    name: 'Summer Sale',
    discountType: 'percentage',
    discountValue: 20
});

// Activate promotion
commerce.promotions.activate(promo.id);

// Create a coupon
const coupon = commerce.promotions.createCoupon(promo.id, 'SAVE20NOW', 100);

// Validate coupon
const valid = commerce.promotions.validateCoupon('SAVE20NOW');
```

### Analytics

```javascript
// Get sales summary
const summary = commerce.analytics.salesSummary();
console.log(`Total revenue: ${summary.totalRevenue}`);
console.log(`Order count: ${summary.orderCount}`);

// Get top products
const topProducts = commerce.analytics.topProducts(10);

// Get top customers
const topCustomers = commerce.analytics.topCustomers(10);
```

## Error Handling

```javascript
import { Commerce, StateSetError } from '@stateset/embedded';

try {
    const order = commerce.orders.ship(orderId);
} catch (error) {
    if (error instanceof StateSetError) {
        console.error(`StateSet error: ${error.message}`);
    } else {
        throw error;
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

## Source Files

- Entry point: `Commerce`
- Types: `bindings/node/index.d.ts`
- Runtime: `bindings/node/index.js`

## Examples

- `examples/node/basic_usage.js`
- `examples/node/04_subscriptions.js`
