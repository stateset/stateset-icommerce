# @stateset/embedded

The SQLite of commerce - an embeddable commerce library powered by Rust.

## Features

- **Zero configuration** - Just point to a file path and start selling
- **Offline-first** - Works without network connectivity
- **Full-featured** - Customers, orders, products, inventory, and returns
- **Type-safe** - Full TypeScript support with auto-generated types
- **Fast** - Native Rust performance via N-API bindings

## Installation

```bash
npm install @stateset/embedded
```

## Quick Start

```javascript
const { Commerce } = require('@stateset/embedded');

// Create a commerce instance with SQLite backend
const commerce = new Commerce('./store.db');

// Or use in-memory database for testing
// const commerce = new Commerce(':memory:');

// Create a customer
const customer = await commerce.customers.create({
  email: 'alice@example.com',
  firstName: 'Alice',
  lastName: 'Smith',
  phone: '+1-555-0123',
  acceptsMarketing: true
});

// Create a product
const product = await commerce.products.create({
  name: 'Premium Widget',
  description: 'A high-quality widget',
  variants: [
    { sku: 'WIDGET-001', name: 'Small', price: 19.99 },
    { sku: 'WIDGET-002', name: 'Large', price: 29.99 }
  ]
});

// Set up inventory
await commerce.inventory.createItem({
  sku: 'WIDGET-001',
  name: 'Small Widget',
  initialQuantity: 100,
  reorderPoint: 10
});

// Create an order
const order = await commerce.orders.create({
  customerId: customer.id,
  items: [
    { sku: 'WIDGET-001', name: 'Small Widget', quantity: 2, unitPrice: 19.99 }
  ],
  currency: 'USD'
});

// Ship the order
await commerce.orders.ship(order.id, 'TRACK123456');
```

## API Reference

### Commerce

Main entry point for all commerce operations.

```typescript
const commerce = new Commerce(dbPath: string);
```

### Customers

```typescript
// Create a customer
const customer = await commerce.customers.create({
  email: string,
  firstName: string,
  lastName: string,
  phone?: string,
  acceptsMarketing?: boolean
});

// Get customer by ID
const customer = await commerce.customers.get(id: string);

// Get customer by email
const customer = await commerce.customers.getByEmail(email: string);

// List all customers
const customers = await commerce.customers.list();

// Count customers
const count = await commerce.customers.count();
```

### Orders

```typescript
// Create an order
const order = await commerce.orders.create({
  customerId: string,
  items: [{ sku: string, name: string, quantity: number, unitPrice: number }],
  currency?: string,
  notes?: string
});

// Get order by ID
const order = await commerce.orders.get(id: string);

// List all orders
const orders = await commerce.orders.list();

// Update order status
const order = await commerce.orders.updateStatus(id: string, status: string);

// Ship order
const order = await commerce.orders.ship(id: string, trackingNumber?: string);

// Cancel order
const order = await commerce.orders.cancel(id: string);

// Count orders
const count = await commerce.orders.count();
```

### Products

```typescript
// Create a product
const product = await commerce.products.create({
  name: string,
  description?: string,
  variants?: [{ sku: string, name?: string, price: number, compareAtPrice?: number }]
});

// Get product by ID
const product = await commerce.products.get(id: string);

// Get variant by SKU
const variant = await commerce.products.getVariantBySku(sku: string);

// List all products
const products = await commerce.products.list();

// Count products
const count = await commerce.products.count();
```

### Inventory

```typescript
// Create inventory item
const item = await commerce.inventory.createItem({
  sku: string,
  name: string,
  description?: string,
  initialQuantity?: number,
  reorderPoint?: number
});

// Get stock level
const stock = await commerce.inventory.getStock(sku: string);

// Adjust inventory
await commerce.inventory.adjust(sku: string, quantity: number, reason: string);

// Reserve inventory
const reservation = await commerce.inventory.reserve(
  sku: string,
  quantity: number,
  referenceType: string,
  referenceId: string,
  expiresInSeconds?: number
);

// Confirm reservation
await commerce.inventory.confirmReservation(reservationId: string);

// Release reservation
await commerce.inventory.releaseReservation(reservationId: string);
```

### Returns

```typescript
// Create a return
const ret = await commerce.returns.create({
  orderId: string,
  reason: string, // 'defective', 'wrong_item', 'not_as_described', etc.
  reasonDetails?: string,
  items: [{ orderItemId: string, quantity: number }]
});

// Get return by ID
const ret = await commerce.returns.get(id: string);

// Approve return
const ret = await commerce.returns.approve(id: string);

// Reject return
const ret = await commerce.returns.reject(id: string, reason: string);

// List all returns
const returns = await commerce.returns.list();

// Count returns
const count = await commerce.returns.count();
```

## TypeScript

This package includes TypeScript definitions out of the box. All types are automatically generated from the Rust source code.

```typescript
import { Commerce, CustomerOutput, OrderOutput } from '@stateset/embedded';

const commerce = new Commerce('./store.db');
const customer: CustomerOutput = await commerce.customers.create({...});
```

## Database

The library uses SQLite under the hood with automatic schema migrations. Your data is stored in a single file that you specify when creating the Commerce instance.

- Use a file path like `./store.db` for persistent storage
- Use `:memory:` for in-memory databases (great for testing)

## License

MIT OR Apache-2.0
