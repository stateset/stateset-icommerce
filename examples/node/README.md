# StateSet iCommerce - Node.js Examples

Comprehensive examples demonstrating all features of the StateSet iCommerce engine using the Node.js bindings.

## Prerequisites

- Node.js 18 or higher
- npm or yarn

## Installation

```bash
cd examples/node
npm install
```

## Examples Overview

| # | Example | Description |
|---|---------|-------------|
| 01 | [Getting Started](01_getting_started.js) | Basic setup, customers, products, orders, inventory |
| 02 | [Cart & Checkout](02_cart_and_checkout.js) | Shopping cart, ACP checkout flow, inventory reservation |
| 03 | [Analytics](03_analytics_and_forecasting.js) | Sales summaries, product performance, demand forecasting |
| 04 | [Subscriptions](04_subscriptions.js) | Subscription plans, billing cycles, pause/resume/cancel |
| 05 | [Promotions](05_promotions.js) | Discounts, coupons, BOGO, tiered pricing |
| 06 | [Currency](06_currency.js) | Multi-currency support, exchange rates, conversion |
| 07 | [Tax](07_tax.js) | Tax calculation, jurisdictions, exemptions, US/EU/CA rates |
| 08 | [Manufacturing](08_manufacturing.js) | Bill of Materials, work orders, production tracking |
| 09 | [Full Workflow](09_full_workflow.js) | End-to-end e-commerce workflow demonstration |
| 10 | [Fulfillment](10_payments_and_fulfillment.js) | Payments, refunds, shipments, returns, warranties |
| 11 | [B2B Operations](11_b2b_operations.js) | Suppliers, purchase orders, invoices |

## Running Examples

Run individual examples:

```bash
# Getting started
npm run 01:start

# Shopping cart & checkout
npm run 02:cart

# Analytics & forecasting
npm run 03:analytics

# Subscriptions
npm run 04:subscriptions

# Promotions & discounts
npm run 05:promotions

# Multi-currency
npm run 06:currency

# Tax calculation
npm run 07:tax

# Manufacturing
npm run 08:manufacturing

# Full workflow
npm run 09:workflow

# Payments & fulfillment
npm run 10:fulfillment

# B2B operations
npm run 11:b2b
```

Run all examples:

```bash
npm run all
```

Or run directly:

```bash
node 01_getting_started.js
```

## Example Details

### 01. Getting Started

Core commerce operations:
- Initialize Commerce engine with SQLite or in-memory database
- Create customers with contact info
- Create products with variants and pricing
- Set up inventory tracking with reorder points
- Create and process orders

```javascript
const { Commerce } = require('@stateset/embedded');

const commerce = new Commerce(':memory:'); // or './store.db'

// Create customer
const customer = await commerce.customers.create({
  email: 'alice@example.com',
  firstName: 'Alice',
  lastName: 'Smith'
});

// Create product with variants
const product = await commerce.products.create({
  name: 'T-Shirt',
  variants: [
    { sku: 'TSHIRT-S', name: 'Small', price: 24.99 },
    { sku: 'TSHIRT-M', name: 'Medium', price: 24.99 }
  ]
});

// Create order
const order = await commerce.orders.create({
  customerId: customer.id,
  items: [{ sku: 'TSHIRT-M', name: 'T-Shirt Medium', quantity: 2, unitPrice: 24.99 }]
});
```

### 02. Cart & Checkout (ACP)

Agentic Commerce Protocol implementation:
- Create shopping carts with expiration
- Add/update/remove cart items
- Set shipping and billing addresses
- Reserve inventory for cart items
- Apply discount codes
- Complete checkout (creates order)
- Handle abandoned carts

```javascript
// Create cart
const cart = await commerce.carts.create({
  customerEmail: 'customer@example.com',
  currency: 'USD'
});

// Add items
await commerce.carts.addItem(cart.id, {
  sku: 'LAPTOP-001',
  name: 'Laptop',
  quantity: 1,
  unitPrice: 999.99
});

// Set shipping
await commerce.carts.setShipping(cart.id, {
  shippingAddress: { firstName: 'John', lastName: 'Doe', line1: '123 Main St', city: 'SF', postalCode: '94105', country: 'US' },
  shippingMethod: 'express',
  shippingAmount: 15.99
});

// Reserve inventory and checkout
await commerce.carts.reserveInventory(cart.id);
const result = await commerce.carts.complete(cart.id);
console.log(`Order created: ${result.orderNumber}`);
```

### 03. Analytics & Forecasting

Business intelligence capabilities:
- Sales summaries by time period
- Revenue breakdown by period
- Top products and product performance
- Customer metrics and top customers
- Inventory health and low stock alerts
- Demand forecasting
- Revenue forecasting
- Order status breakdown
- Fulfillment and return metrics

```javascript
// Sales summary
const sales = await commerce.analytics.salesSummary({ period: 'last30days' });
console.log(`Revenue: $${sales.totalRevenue}, Orders: ${sales.orderCount}`);

// Demand forecast
const forecast = await commerce.analytics.demandForecast(['SKU-001'], 30);
console.log(`30-day forecast: ${forecast[0].forecastedDemand} units`);

// Revenue forecast
const revenue = await commerce.analytics.revenueForecast(3, 'month');
```

### 04. Subscriptions

Recurring billing management:
- Create subscription plans (monthly, yearly, custom intervals)
- Trial periods and setup fees
- Subscribe customers to plans
- Pause, resume, skip, cancel subscriptions
- Billing cycles and payment tracking
- Subscription events history

```javascript
// Create plan
const plan = await commerce.subscriptions.createPlan({
  name: 'Pro Monthly',
  billingInterval: 'monthly',
  price: 29.99,
  trialDays: 14
});

// Subscribe customer
const subscription = await commerce.subscriptions.subscribe({
  customerId: customer.id,
  planId: plan.id
});

// Manage subscription
await commerce.subscriptions.pause(subscription.id, { reason: 'Vacation' });
await commerce.subscriptions.resume(subscription.id);
```

### 05. Promotions & Discounts

Flexible discount system:
- Percentage and fixed amount discounts
- Buy X Get Y (BOGO)
- Free shipping promotions
- Tiered discounts
- Bundle discounts
- Coupon code management
- Apply promotions to carts
- Usage tracking and limits

```javascript
// Create promotion
const promo = await commerce.promotions.create({
  name: '20% Off',
  promotionType: 'percentage_off',
  percentageOff: 0.20,
  trigger: 'coupon_code'
});

// Create coupon
await commerce.promotions.createCoupon({
  promotionId: promo.id,
  code: 'SAVE20'
});

// Apply to cart
const result = await commerce.promotions.apply({
  couponCodes: ['SAVE20'],
  lineItems: [{ id: '1', quantity: 1, unitPrice: 100, lineTotal: 100 }],
  subtotal: 100
});
console.log(`Discount: $${result.totalDiscount}`);
```

### 06. Multi-Currency

International commerce support:
- Configure store currency settings
- Set exchange rates (manual or API)
- Currency conversion
- Format amounts with currency symbols
- Enable/disable currencies

```javascript
// Set up currencies
await commerce.currency.updateSettings({
  baseCurrency: 'USD',
  enabledCurrencies: ['USD', 'EUR', 'GBP']
});

// Set rates
await commerce.currency.setRate({ baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92 });

// Convert
const result = await commerce.currency.convert({ from: 'USD', to: 'EUR', amount: 100 });
console.log(`$100 = €${result.convertedAmount}`);
```

### 07. Tax Calculation

Comprehensive tax system:
- Configure tax settings
- Create jurisdictions (country/state/city)
- Define tax rates by product category
- Customer tax exemptions
- Calculate tax for transactions
- US state tax lookup
- EU VAT rates
- Canadian GST/HST/PST

```javascript
// Create jurisdiction
const ca = await commerce.tax.createJurisdiction({
  name: 'California', code: 'US-CA', level: 'state', countryCode: 'US', stateCode: 'CA'
});

// Create rate
await commerce.tax.createRate({
  jurisdictionId: ca.id, rate: 0.0875, name: 'CA Sales Tax', effectiveFrom: '2024-01-01'
});

// Calculate tax
const calc = await commerce.tax.calculate({
  lineItems: [{ id: '1', quantity: 1, unitPrice: 100 }],
  shippingAddress: { state: 'CA', country: 'US' }
});
console.log(`Tax: $${calc.totalTax}`);
```

### 08. Manufacturing

Production management:
- Bill of Materials (BOM) creation
- Component management
- Work order creation and tracking
- Production status updates
- Quantity tracking

```javascript
// Create BOM
const bom = await commerce.bom.create({
  name: 'Widget BOM',
  productId: product.id,
  revision: '1.0'
});

// Add components
await commerce.bom.addComponent(bom.id, { componentSku: 'PART-A', name: 'Part A', quantity: 2 });

// Create work order
const wo = await commerce.workOrders.create({
  productId: product.id,
  bomId: bom.id,
  quantityToBuild: 10
});

// Track production
await commerce.workOrders.start(wo.id);
await commerce.workOrders.complete(wo.id, 10);
```

### 09. Full Workflow

Complete e-commerce demonstration:
- Store setup (products, inventory, tax, currency)
- Customer registration
- Shopping cart and checkout
- Payment processing
- Order fulfillment and shipping
- Returns and refunds
- Warranty registration
- Analytics reporting

### 10. Payments & Fulfillment

Transaction management:
- Payment creation and processing
- Payment completion and failure handling
- Refunds (full and partial)
- Shipment creation and tracking
- Shipment lifecycle (ship, deliver, cancel)
- Returns management (create, approve, reject)
- Warranty registration and claims

```javascript
// Process payment
const payment = await commerce.payments.create({
  orderId: order.id,
  amount: 99.99,
  paymentMethod: 'credit_card'
});
await commerce.payments.markCompleted(payment.id);

// Create shipment
const shipment = await commerce.shipments.create({
  orderId: order.id,
  recipientName: 'John Doe',
  shippingAddress: '123 Main St, SF, CA 94105',
  carrier: 'ups',
  trackingNumber: '1Z999...'
});
await commerce.shipments.ship(shipment.id);
await commerce.shipments.deliver(shipment.id);
```

### 11. B2B Operations

Business-to-business features:
- Supplier management
- Purchase order creation and lifecycle
- Invoice generation
- Payment recording
- Overdue invoice tracking

```javascript
// Create supplier
const supplier = await commerce.purchaseOrders.createSupplier({
  name: 'Acme Supplies',
  supplierCode: 'ACM-001',
  email: 'orders@acme.com'
});

// Create purchase order
const po = await commerce.purchaseOrders.create({
  supplierId: supplier.id,
  items: [{ sku: 'PART-001', name: 'Widget Part', quantity: 100, unitCost: 5.00 }]
});
await commerce.purchaseOrders.submit(po.id);
await commerce.purchaseOrders.approve(po.id, 'Manager');

// Create invoice
const invoice = await commerce.invoices.create({
  customerId: customer.id,
  items: [{ description: 'Consulting', quantity: 10, unitPrice: 100 }]
});
await commerce.invoices.send(invoice.id);
await commerce.invoices.recordPayment(invoice.id, { amount: 1000, paymentMethod: 'wire' });
```

## API Reference

The `Commerce` class provides access to all APIs through property getters:

| API | Description |
|-----|-------------|
| `commerce.customers` | Customer management |
| `commerce.products` | Product catalog |
| `commerce.orders` | Order processing |
| `commerce.inventory` | Stock management |
| `commerce.carts` | Shopping cart & checkout |
| `commerce.returns` | Return processing |
| `commerce.payments` | Payment handling |
| `commerce.shipments` | Fulfillment |
| `commerce.warranties` | Warranty management |
| `commerce.purchaseOrders` | Procurement & suppliers |
| `commerce.invoices` | B2B invoicing |
| `commerce.bom` | Bill of Materials |
| `commerce.workOrders` | Manufacturing |
| `commerce.analytics` | Business intelligence |
| `commerce.currency` | Multi-currency |
| `commerce.subscriptions` | Recurring billing |
| `commerce.promotions` | Discounts & coupons |
| `commerce.tax` | Tax calculation |

## TypeScript Support

Full TypeScript definitions are included. Import types directly:

```typescript
import {
  Commerce,
  CustomerOutput,
  OrderOutput,
  CreateOrderInput
} from '@stateset/embedded';
```

## Database

All examples use an in-memory SQLite database (`:memory:`). For persistent storage, use a file path:

```javascript
const commerce = new Commerce('./my-store.db');
```

## License

MIT
