# @stateset/embedded-wasm

StateSet Embedded Commerce for browsers using WebAssembly.

## Installation

```bash
npm install @stateset/embedded-wasm
```

## Usage

### Browser (ES Modules)

```html
<script type="module">
import init, { Commerce } from '@stateset/embedded-wasm';

async function main() {
  // Initialize WASM module
  await init();

  // Create commerce instance (in-memory storage)
  const commerce = new Commerce();

  // Create a customer
  const customer = commerce.customers.create({
    email: "alice@example.com",
    firstName: "Alice",
    lastName: "Smith"
  });
  console.log("Created customer:", customer.id);

  // Create inventory
  const item = commerce.inventory.createItem({
    sku: "WIDGET-001",
    name: "Premium Widget",
    initialQuantity: 100
  });

  // Check stock
  const stock = commerce.inventory.getStock("WIDGET-001");
  console.log("Available:", stock.totalAvailable);

  // Create an order
  const order = commerce.orders.create({
    customerId: customer.id,
    items: [
      { sku: "WIDGET-001", name: "Premium Widget", quantity: 2, unitPrice: 29.99 }
    ]
  });
  console.log("Order:", order.orderNumber, "Total:", order.totalAmount);
}

main();
</script>
```

### With Bundler (Webpack, Vite, etc.)

```javascript
import init, { Commerce } from '@stateset/embedded-wasm';

async function initCommerce() {
  await init();
  return new Commerce();
}

const commerce = await initCommerce();

// Use commerce API...
const customer = commerce.customers.create({
  email: "alice@example.com",
  firstName: "Alice",
  lastName: "Smith"
});
```

### React Example

```jsx
import { useState, useEffect } from 'react';
import init, { Commerce } from '@stateset/embedded-wasm';

function useCommerce() {
  const [commerce, setCommerce] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    init().then(() => {
      setCommerce(new Commerce());
      setLoading(false);
    });
  }, []);

  return { commerce, loading };
}

function App() {
  const { commerce, loading } = useCommerce();
  const [customers, setCustomers] = useState([]);

  if (loading) return <div>Loading...</div>;

  const addCustomer = () => {
    const customer = commerce.customers.create({
      email: `user${Date.now()}@example.com`,
      firstName: "New",
      lastName: "User"
    });
    setCustomers([...customers, customer]);
  };

  return (
    <div>
      <button onClick={addCustomer}>Add Customer</button>
      <ul>
        {customers.map(c => (
          <li key={c.id}>{c.fullName} ({c.email})</li>
        ))}
      </ul>
    </div>
  );
}
```

## API Reference

### Commerce

Main entry point for all operations.

```javascript
const commerce = new Commerce();
```

**Note:** Data is stored in-memory and will be lost on page refresh. For persistence, consider using IndexedDB to save/restore state.

### Customers

```javascript
// Create
const customer = commerce.customers.create({
  email: "alice@example.com",
  firstName: "Alice",
  lastName: "Smith",
  phone: "+1234567890",        // optional
  acceptsMarketing: true       // optional
});

// Get by ID
const customer = commerce.customers.get(customerId);

// Get by email
const customer = commerce.customers.getByEmail("alice@example.com");

// List all
const customers = commerce.customers.list();

// Count
const count = commerce.customers.count();

// Properties
customer.id           // UUID string
customer.email        // string
customer.firstName    // string
customer.lastName     // string
customer.fullName     // "firstName lastName"
customer.phone        // string or null
customer.status       // "active"
customer.acceptsMarketing // boolean
customer.createdAt    // ISO 8601 string
customer.updatedAt    // ISO 8601 string
```

### Orders

```javascript
// Create
const order = commerce.orders.create({
  customerId: customer.id,
  items: [
    { sku: "SKU-001", name: "Product", quantity: 2, unitPrice: 29.99 }
  ],
  currency: "USD",    // optional, default "USD"
  notes: "Gift wrap"  // optional
});

// Get
const order = commerce.orders.get(orderId);

// Get items
const items = commerce.orders.getItems(orderId);

// List all
const orders = commerce.orders.list();

// Update status
const order = commerce.orders.updateStatus(orderId, "processing");

// Ship
const order = commerce.orders.ship(orderId, "1Z123ABC"); // tracking optional

// Cancel
const order = commerce.orders.cancel(orderId);

// Count
const count = commerce.orders.count();

// Properties
order.id               // UUID string
order.orderNumber      // "ORD-123"
order.customerId       // UUID string
order.status           // "pending", "shipped", "cancelled", etc.
order.totalAmount      // number
order.currency         // "USD"
order.paymentStatus    // "pending", "paid", etc.
order.fulfillmentStatus // "unfulfilled", "shipped", etc.
order.trackingNumber   // string or null
order.items            // array of order items
order.createdAt        // ISO 8601 string
order.updatedAt        // ISO 8601 string
```

### Products

```javascript
// Create
const product = commerce.products.create({
  name: "Premium Widget",
  description: "A high-quality widget",
  variants: [
    { sku: "WIDGET-SM", price: 19.99, name: "Small" },
    { sku: "WIDGET-LG", price: 29.99, name: "Large" }
  ]
});

// Get
const product = commerce.products.get(productId);

// Get variant by SKU
const variant = commerce.products.getVariantBySku("WIDGET-SM");

// List all
const products = commerce.products.list();

// Count
const count = commerce.products.count();
```

### Inventory

```javascript
// Create item
const item = commerce.inventory.createItem({
  sku: "WIDGET-001",
  name: "Premium Widget",
  description: "High quality",     // optional
  initialQuantity: 100,            // optional, default 0
  reorderPoint: 10                 // optional
});

// Get stock
const stock = commerce.inventory.getStock("WIDGET-001");
stock.sku            // "WIDGET-001"
stock.name           // "Premium Widget"
stock.totalOnHand    // 100
stock.totalAllocated // 0
stock.totalAvailable // 100

// Adjust stock
commerce.inventory.adjust("WIDGET-001", -5, "Sold 5 units");
commerce.inventory.adjust("WIDGET-001", 50, "Received shipment");

// Reserve for order
const reservation = commerce.inventory.reserve(
  "WIDGET-001",   // sku
  10,             // quantity
  "order",        // reference type
  "ord-123"       // reference id
);

// Confirm reservation
commerce.inventory.confirmReservation(reservation.id);

// Release reservation
commerce.inventory.releaseReservation(reservation.id);
```

### Returns

```javascript
// Create return request
const ret = commerce.returns.create({
  orderId: order.id,
  reason: "defective",
  items: [
    { orderItemId: order.items[0].id, quantity: 1 }
  ],
  reasonDetails: "Product stopped working"  // optional
});

// Get
const ret = commerce.returns.get(returnId);

// Approve
const ret = commerce.returns.approve(returnId);

// Reject
const ret = commerce.returns.reject(returnId, "Item was used");

// List all
const returns = commerce.returns.list();

// Count
const count = commerce.returns.count();
```

## Building from Source

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build for web
npm run build

# Build for Node.js
npm run build:node

# Build for bundlers
npm run build:bundler
```

## Storage Considerations

This library uses in-memory storage, meaning all data is lost when the page refreshes. For persistence, you have several options:

1. **IndexedDB**: Save/restore the state to IndexedDB
2. **LocalStorage**: For small datasets, serialize to JSON
3. **Server Sync**: Sync with a backend server

Example with localStorage:

```javascript
// Save state (simplified - you'd need to implement full serialization)
function saveState(commerce) {
  const state = {
    customers: commerce.customers.list(),
    orders: commerce.orders.list(),
    products: commerce.products.list()
  };
  localStorage.setItem('commerce-state', JSON.stringify(state));
}

// Restore state
function restoreState(commerce, state) {
  state.customers.forEach(c => {
    commerce.customers.create(c);
  });
  // ... etc
}
```

## Browser Support

- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 79+

Requires WebAssembly support.

## Package Size

- WASM binary: ~200KB (gzipped: ~70KB)
- JavaScript glue code: ~35KB

## License

MIT OR Apache-2.0
