# @stateset/embedded

[![npm](https://img.shields.io/npm/v/@stateset/embedded.svg)](https://www.npmjs.com/package/@stateset/embedded)
[![node](https://img.shields.io/node/v/@stateset/embedded.svg)](https://nodejs.org)

The SQLite of commerce - an embeddable commerce library powered by Rust.

## Features

- **Zero configuration** - Just point to a file path and start selling
- **Offline-first** - Works without network connectivity
- **Full-featured** - Customers, orders, products, inventory, carts, payments, returns, analytics, and currency
- **Type-safe** - Full TypeScript support with auto-generated types
- **Fast** - Native Rust performance via N-API bindings

## Installation

```bash
npm install @stateset/embedded
```

Requires Node `20.20.0+`. npm resolves your platform's prebuilt native
binary automatically via one optional dependency
(`@stateset/embedded-<platform>`, ~20-26 MB) — the install stays small
instead of bundling every platform.

**Supported platforms:** Linux x64/arm64 (glibc 2.33+ and musl),
macOS x64/arm64 (11+), Windows x64/arm64. On older glibc (e.g. Ubuntu
20.04, past EOL) use the musl build in a container, or the Python package
(`pip install stateset-embedded`), whose manylinux wheels and source
fallback reach further back.

## Development

Repo development uses the workspace-standard Node toolchain: Node `20.20.0+`
and npm `10+`. The binding package now checks that version before running
`build`, `build:debug`, `test`, `test:coverage`, `artifacts`, or `universal`.

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

// Analytics
const summary = await commerce.analytics.salesSummary({ period: 'last30days' });
console.log(`Revenue: $${summary.totalRevenue}`);

// Currency conversion (set a rate, then convert)
await commerce.currency.setRate({
  baseCurrency: 'USD',
  quoteCurrency: 'EUR',
  rate: 0.92,
  source: 'manual'
});
const conversion = await commerce.currency.convert({ from: 'USD', to: 'EUR', amount: 100 });
console.log(`$100 USD = €${conversion.convertedAmount} EUR`);
```

## Agent Framework Embedding

`@stateset/cli` is an **optional peer dependency** — the core engine never
pulls it in. Install it alongside the binding only when you want the full
advanced runtime for server-side agents, then use the dedicated toolkit
entrypoint:

```bash
npm install @stateset/embedded @stateset/cli
```

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/embedded/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const openaiTools = toolkit.getTools({ format: 'openai' });
const mcpTools = toolkit.getTools({ format: 'mcp' });
```

If you want lighter-weight helper entrypoints around an existing `Commerce`
instance, start with `@stateset/embedded/openai`,
`@stateset/embedded/generic`, `@stateset/embedded/langchain`, and
`@stateset/embedded/vercel-ai`.

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';
import { createToolDescriptors } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');
const openaiTools = createOpenAITools(commerce, {
  filter: ['list_customers'],
});
const execution = await executeOpenAIToolCall(commerce, {
  call_id: 'demo_call_1',
  function: {
    name: 'list_customers',
    arguments: '{}',
  },
});
const descriptors = createToolDescriptors(commerce, {
  filter: ['list_customers', 'list_orders', 'get_sales_summary'],
});
```

That gives you both OpenAI-compatible tool definitions and a framework-neutral
`{ name, description, schema, execute }` surface for custom runtimes. The
binding also ships `@stateset/embedded/langchain` and
`@stateset/embedded/vercel-ai` helper subpaths for those JS hosts.

## Money: use the `*Exact` fields

A JavaScript `number` is a double, and a double cannot hold `19.99`. By the time
a price reaches this binding as a `number` it is already `19.989999999999998`;
JavaScript prints it as `19.99` because it prints the shortest string that
round-trips, not because the value is right. So every money value crosses this
boundary twice:

- `total`, `unitPrice`, `subtotal`, … — the `number`, kept for compatibility and
  marked `@deprecated`. Float money will be removed in 2.0.
- `totalExact`, `unitPriceExact`, `subtotalExact`, … — a **string** carrying the
  engine's exact base-10 `Decimal`, with no float anywhere in the path. `0.10`
  plus `0.20` is `"0.30"` here, never `0.30000000000000004`.

Read the `*Exact` field for anything you will store, compare, total or show a
customer. Read the `number` only where an approximation is genuinely fine.

The string carries the engine's scale, not a currency's display format: `2 ×
12.50` renders as `"25.0"`, not `"25.00"`. It is exact either way, so compare two
amounts numerically (parse them, or use a decimal library) rather than by string
equality — `"25.0" !== "25.00"` even though the amounts are the same.

```typescript
const order = await commerce.orders.get(id);

order.totalAmount;                     // prints 59.97 …
order.totalAmount.toPrecision(17);     // … but the value is 59.969999999999999
order.totalAmountExact;                // "59.97" — the actual amount
```

Inputs work the same way, in reverse. Money inputs take an optional
`<field>Exact` string alongside the `number`, and the string wins when both are
sent:

```typescript
await commerce.orders.create({
  customerId,
  items: [
    { sku: 'WIDGET', name: 'Widget', quantity: 3, unitPrice: 0, unitPriceExact: '19.99' },
  ],
});
// -> items[0].totalExact === "59.97"
```

`orders.createExact`, `payments.createExact`, `payments.createRefundExact` and
`carts.addItemExact` are the fully string-typed variants, for callers that would
rather not carry the float half at all.

A money value the engine cannot narrow to a `number` no longer becomes `NaN`: it
throws with `err.code === 'INTERNAL'` and a message naming the field.
`test/fixtures/money-fields.json` is the census of which fields are money, and
`test/money-exactness.js` fails if a new float money field is added without a
twin.

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

### Vector Search (Hybrid Semantic + BM25)

Vector search uses OpenAI embeddings with optional SQLite FTS5 (BM25) for lexical matches.
Set `OPENAI_API_KEY` in your environment.

```bash
export OPENAI_API_KEY=sk-...
```

```typescript
const vector = commerce.vector(process.env.OPENAI_API_KEY!);

// Search products/customers/orders/inventory
const products = await vector.searchProducts('wireless earbuds', 10);
const customers = await vector.searchCustomers('enterprise retail buyers', 10);
const orders = await vector.searchOrders('late shipment', 10);
const inventory = await vector.searchInventory('outdoor gear', 10);

// Index entities (single + bulk)
await vector.indexProduct('<product-id>');
await vector.indexCustomer('<customer-id>');
await vector.indexOrder('<order-id>');
await vector.indexInventoryItem('<inventory-id>');

await vector.indexAllProducts();
await vector.indexAllCustomers();
await vector.indexAllOrders();
await vector.indexAllInventory();

// Stats + maintenance
const stats = await vector.stats();
await vector.clear('products');
await vector.clearAll();
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

### Carts / Checkout

```typescript
// Create a cart
const cart = await commerce.carts.create({
  customerEmail: 'alice@example.com',
  currency: 'USD'
});

// Add items
await commerce.carts.addItem(cart.id, {
  sku: 'SKU-001',
  name: 'Widget',
  quantity: 2,
  unitPrice: 29.99
});

// Set shipping (address + selection)
await commerce.carts.setShipping(cart.id, {
  shippingAddress: {
    firstName: 'Alice',
    lastName: 'Smith',
    line1: '123 Main St',
    city: 'San Francisco',
    postalCode: '94105',
    country: 'US'
  },
  shippingMethod: 'standard',
  shippingCarrier: 'ups',
  shippingAmount: 9.99
});

// Reserve/release inventory for cart items
await commerce.carts.reserveInventory(cart.id);
await commerce.carts.releaseInventory(cart.id);

// Complete checkout (creates an order)
const result = await commerce.carts.complete(cart.id);
console.log(result.orderNumber);

// Expire + query expired carts
await commerce.carts.expire(cart.id);
const expired = await commerce.carts.getExpired();
```

### Analytics

```typescript
// Sales summary
const summary = await commerce.analytics.salesSummary({ period: 'last30days' });

// Top products
const topProducts = await commerce.analytics.topProducts({ period: 'this_month', limit: 10 });

// Product performance + inventory movement
const perf = await commerce.analytics.productPerformance({ period: 'last30days' });
const movement = await commerce.analytics.inventoryMovement({ period: 'last30days' });

// Forecasting
const demand = await commerce.analytics.demandForecast(['SKU-001'], 30);
const revenue = await commerce.analytics.revenueForecast(3, 'month');
```

### Currency

```typescript
// Set an exchange rate
await commerce.currency.setRate({
  baseCurrency: 'USD',
  quoteCurrency: 'EUR',
  rate: 0.92,
  source: 'manual'
});

// Convert currency
const conversion = await commerce.currency.convert({ from: 'USD', to: 'EUR', amount: 100 });

// List rates + store settings
const rates = await commerce.currency.listRates({ baseCurrency: 'USD' });
const settings = await commerce.currency.getSettings();
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
