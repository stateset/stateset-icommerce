/**
 * Demo Data Seeder
 *
 * Seeds a realistic storefront into a StateSet Commerce database.
 * Uses the Commerce API directly — no AI, no API key needed.
 *
 * Creates:
 *   - 10 customers
 *   - 20 products across 4 categories
 *   - Inventory for all 20 SKUs (3 low-stock)
 *   - 15 orders
 *   - 3 promotions
 *   - 3 subscription plans
 */

// ── Customer data ──────────────────────────────────────────────────

const CUSTOMERS = [
  { email: 'alice@example.com', firstName: 'Alice', lastName: 'Johnson', phone: '+1-555-0101' },
  { email: 'bob@example.com', firstName: 'Bob', lastName: 'Smith', phone: '+1-555-0102' },
  { email: 'carol@example.com', firstName: 'Carol', lastName: 'Williams', phone: '+1-555-0103' },
  { email: 'david@example.com', firstName: 'David', lastName: 'Brown', phone: '+1-555-0104' },
  { email: 'emma@example.com', firstName: 'Emma', lastName: 'Davis', phone: '+1-555-0105' },
  { email: 'frank@example.com', firstName: 'Frank', lastName: 'Miller', phone: '+1-555-0106' },
  { email: 'grace@example.com', firstName: 'Grace', lastName: 'Wilson', phone: '+1-555-0107' },
  { email: 'henry@example.com', firstName: 'Henry', lastName: 'Moore', phone: '+1-555-0108' },
  { email: 'iris@example.com', firstName: 'Iris', lastName: 'Taylor', phone: '+1-555-0109' },
  { email: 'jack@example.com', firstName: 'Jack', lastName: 'Anderson', phone: '+1-555-0110' },
];

// ── Product data ───────────────────────────────────────────────────

const PRODUCTS = [
  // Electronics
  {
    name: 'Wireless Bluetooth Headphones',
    variants: [{ sku: 'WBH-001', name: 'Wireless Bluetooth Headphones', price: 79.99 }],
  },
  {
    name: 'USB-C Charging Cable 6ft',
    variants: [{ sku: 'USB-C-6FT', name: 'USB-C Charging Cable 6ft', price: 12.99 }],
  },
  {
    name: 'Portable Power Bank 10000mAh',
    variants: [{ sku: 'PPB-10K', name: 'Portable Power Bank 10000mAh', price: 29.99 }],
  },
  { name: 'Wireless Mouse', variants: [{ sku: 'WM-001', name: 'Wireless Mouse', price: 24.99 }] },
  {
    name: 'Mechanical Keyboard',
    variants: [{ sku: 'MK-001', name: 'Mechanical Keyboard', price: 89.99 }],
  },
  // Home & Garden
  {
    name: 'Smart LED Bulb 4-Pack',
    variants: [{ sku: 'SLB-4PK', name: 'Smart LED Bulb 4-Pack', price: 34.99 }],
  },
  {
    name: 'Indoor Plant Pot Set',
    variants: [{ sku: 'IPP-SET', name: 'Indoor Plant Pot Set', price: 19.99 }],
  },
  {
    name: 'Bamboo Cutting Board',
    variants: [{ sku: 'BCB-001', name: 'Bamboo Cutting Board', price: 24.99 }],
  },
  {
    name: 'Stainless Steel Water Bottle',
    variants: [{ sku: 'SSWB-32', name: 'Stainless Steel Water Bottle 32oz', price: 18.99 }],
  },
  {
    name: 'Yoga Mat Premium',
    variants: [{ sku: 'YMP-001', name: 'Yoga Mat Premium', price: 39.99 }],
  },
  // Clothing & Accessories
  {
    name: 'Cotton T-Shirt Classic Black M',
    variants: [{ sku: 'CTS-BLK-M', name: 'Cotton T-Shirt Black Medium', price: 19.99 }],
  },
  {
    name: 'Cotton T-Shirt Classic Black L',
    variants: [{ sku: 'CTS-BLK-L', name: 'Cotton T-Shirt Black Large', price: 19.99 }],
  },
  {
    name: 'Running Shoes Pro',
    variants: [{ sku: 'RSP-001', name: 'Running Shoes Pro', price: 119.99 }],
  },
  {
    name: 'Canvas Backpack',
    variants: [{ sku: 'CBP-001', name: 'Canvas Backpack', price: 49.99 }],
  },
  {
    name: 'Sunglasses Aviator',
    variants: [{ sku: 'SGA-001', name: 'Sunglasses Aviator', price: 29.99 }],
  },
  // Office Supplies
  { name: 'Notebook 5-Pack', variants: [{ sku: 'NB-5PK', name: 'Notebook 5-Pack', price: 14.99 }] },
  { name: 'Desk Organizer', variants: [{ sku: 'DO-001', name: 'Desk Organizer', price: 22.99 }] },
  {
    name: 'Ergonomic Chair',
    variants: [{ sku: 'EC-001', name: 'Ergonomic Chair', price: 249.99 }],
  },
  { name: 'Monitor Stand', variants: [{ sku: 'MS-001', name: 'Monitor Stand', price: 34.99 }] },
  { name: 'Desk Lamp LED', variants: [{ sku: 'DL-LED', name: 'Desk Lamp LED', price: 27.99 }] },
];

// ── Inventory levels ───────────────────────────────────────────────

const INVENTORY = [
  { sku: 'WBH-001', name: 'Wireless Bluetooth Headphones', qty: 150 },
  { sku: 'USB-C-6FT', name: 'USB-C Charging Cable 6ft', qty: 500 },
  { sku: 'PPB-10K', name: 'Portable Power Bank 10000mAh', qty: 200 },
  { sku: 'WM-001', name: 'Wireless Mouse', qty: 300 },
  { sku: 'MK-001', name: 'Mechanical Keyboard', qty: 75 },
  { sku: 'SLB-4PK', name: 'Smart LED Bulb 4-Pack', qty: 400 },
  { sku: 'IPP-SET', name: 'Indoor Plant Pot Set', qty: 250 },
  { sku: 'BCB-001', name: 'Bamboo Cutting Board', qty: 180 },
  { sku: 'SSWB-32', name: 'Stainless Steel Water Bottle 32oz', qty: 350 },
  { sku: 'YMP-001', name: 'Yoga Mat Premium', qty: 120 },
  { sku: 'CTS-BLK-M', name: 'Cotton T-Shirt Black Medium', qty: 500 },
  { sku: 'CTS-BLK-L', name: 'Cotton T-Shirt Black Large', qty: 500 },
  { sku: 'RSP-001', name: 'Running Shoes Pro', qty: 100 },
  { sku: 'CBP-001', name: 'Canvas Backpack', qty: 200 },
  { sku: 'SGA-001', name: 'Sunglasses Aviator', qty: 300 },
  { sku: 'NB-5PK', name: 'Notebook 5-Pack', qty: 600 },
  { sku: 'DO-001', name: 'Desk Organizer', qty: 150 },
  { sku: 'EC-001', name: 'Ergonomic Chair', qty: 50 },
  { sku: 'MS-001', name: 'Monitor Stand', qty: 175 },
  { sku: 'DL-LED', name: 'Desk Lamp LED', qty: 225 },
];

// Items to mark as low-stock (sold most of inventory)
const LOW_STOCK_ADJUSTMENTS = [
  { sku: 'EC-001', adjustment: -45, reason: 'Sales' },
  { sku: 'MK-001', adjustment: -70, reason: 'Sales' },
  { sku: 'RSP-001', adjustment: -92, reason: 'Sales' },
];

// ── Order templates (indexed by customer email) ────────────────────

const ORDERS = [
  {
    customerEmail: 'alice@example.com',
    items: [
      { sku: 'WBH-001', name: 'Wireless Bluetooth Headphones', quantity: 2, unitPrice: 79.99 },
      { sku: 'USB-C-6FT', name: 'USB-C Charging Cable 6ft', quantity: 1, unitPrice: 12.99 },
    ],
  },
  {
    customerEmail: 'bob@example.com',
    items: [
      { sku: 'MK-001', name: 'Mechanical Keyboard', quantity: 1, unitPrice: 89.99 },
      { sku: 'WM-001', name: 'Wireless Mouse', quantity: 1, unitPrice: 24.99 },
    ],
  },
  {
    customerEmail: 'carol@example.com',
    items: [
      { sku: 'SLB-4PK', name: 'Smart LED Bulb 4-Pack', quantity: 3, unitPrice: 34.99 },
      { sku: 'IPP-SET', name: 'Indoor Plant Pot Set', quantity: 2, unitPrice: 19.99 },
    ],
  },
  {
    customerEmail: 'david@example.com',
    items: [{ sku: 'EC-001', name: 'Ergonomic Chair', quantity: 1, unitPrice: 249.99 }],
  },
  {
    customerEmail: 'emma@example.com',
    items: [
      { sku: 'CTS-BLK-M', name: 'Cotton T-Shirt Black Medium', quantity: 2, unitPrice: 19.99 },
      { sku: 'CTS-BLK-L', name: 'Cotton T-Shirt Black Large', quantity: 2, unitPrice: 19.99 },
      { sku: 'CBP-001', name: 'Canvas Backpack', quantity: 1, unitPrice: 49.99 },
    ],
  },
  {
    customerEmail: 'frank@example.com',
    items: [
      { sku: 'RSP-001', name: 'Running Shoes Pro', quantity: 1, unitPrice: 119.99 },
      { sku: 'YMP-001', name: 'Yoga Mat Premium', quantity: 1, unitPrice: 39.99 },
    ],
  },
  {
    customerEmail: 'grace@example.com',
    items: [
      { sku: 'USB-C-6FT', name: 'USB-C Charging Cable 6ft', quantity: 5, unitPrice: 12.99 },
      { sku: 'PPB-10K', name: 'Portable Power Bank 10000mAh', quantity: 2, unitPrice: 29.99 },
    ],
  },
  {
    customerEmail: 'henry@example.com',
    items: [
      { sku: 'DL-LED', name: 'Desk Lamp LED', quantity: 1, unitPrice: 27.99 },
      { sku: 'MS-001', name: 'Monitor Stand', quantity: 1, unitPrice: 34.99 },
      { sku: 'DO-001', name: 'Desk Organizer', quantity: 1, unitPrice: 22.99 },
    ],
  },
  {
    customerEmail: 'iris@example.com',
    items: [
      { sku: 'NB-5PK', name: 'Notebook 5-Pack', quantity: 3, unitPrice: 14.99 },
      { sku: 'BCB-001', name: 'Bamboo Cutting Board', quantity: 1, unitPrice: 24.99 },
    ],
  },
  {
    customerEmail: 'jack@example.com',
    items: [
      { sku: 'WBH-001', name: 'Wireless Bluetooth Headphones', quantity: 1, unitPrice: 79.99 },
      { sku: 'SGA-001', name: 'Sunglasses Aviator', quantity: 1, unitPrice: 29.99 },
    ],
  },
  // Repeat customers
  {
    customerEmail: 'alice@example.com',
    items: [
      { sku: 'SSWB-32', name: 'Stainless Steel Water Bottle 32oz', quantity: 1, unitPrice: 18.99 },
      { sku: 'NB-5PK', name: 'Notebook 5-Pack', quantity: 2, unitPrice: 14.99 },
    ],
  },
  {
    customerEmail: 'bob@example.com',
    items: [{ sku: 'CBP-001', name: 'Canvas Backpack', quantity: 1, unitPrice: 49.99 }],
  },
  {
    customerEmail: 'carol@example.com',
    items: [
      { sku: 'PPB-10K', name: 'Portable Power Bank 10000mAh', quantity: 1, unitPrice: 29.99 },
      { sku: 'USB-C-6FT', name: 'USB-C Charging Cable 6ft', quantity: 3, unitPrice: 12.99 },
    ],
  },
  {
    customerEmail: 'david@example.com',
    items: [
      { sku: 'SGA-001', name: 'Sunglasses Aviator', quantity: 2, unitPrice: 29.99 },
      { sku: 'CTS-BLK-L', name: 'Cotton T-Shirt Black Large', quantity: 1, unitPrice: 19.99 },
    ],
  },
  {
    customerEmail: 'emma@example.com',
    items: [
      { sku: 'WM-001', name: 'Wireless Mouse', quantity: 1, unitPrice: 24.99 },
      { sku: 'DL-LED', name: 'Desk Lamp LED', quantity: 1, unitPrice: 27.99 },
    ],
  },
];

// ── Seed runner ────────────────────────────────────────────────────

/**
 * Seed demo data into a Commerce instance.
 *
 * @param {object} commerce  Commerce instance from @stateset/embedded
 * @param {object} [options]
 * @param {boolean} [options.quiet]  Suppress progress output
 */
export async function seedDemoData(commerce, options = {}) {
  const log = options.quiet ? () => {} : (msg) => console.log(msg);

  log('');
  log('  StateSet Commerce - Demo Data Seeder');
  log('  ─────────────────────────────────────');
  log('');

  // 1. Customers
  log('  [1/6] Creating customers...');
  const customerMap = new Map();
  for (const data of CUSTOMERS) {
    try {
      const customer = await commerce.customers.create(data);
      customerMap.set(data.email, customer.id);
      log(`    + ${data.firstName} ${data.lastName} <${data.email}>`);
    } catch (err) {
      log(`    ! Skipped ${data.email}: ${err.message}`);
    }
  }
  log(`  Done: ${customerMap.size} customers\n`);

  // 2. Products
  log('  [2/6] Creating products...');
  let productCount = 0;
  for (const data of PRODUCTS) {
    try {
      await commerce.products.create(data);
      productCount++;
    } catch (err) {
      log(`    ! Skipped ${data.name}: ${err.message}`);
    }
  }
  log(`  Done: ${productCount} products\n`);

  // 3. Inventory
  log('  [3/6] Setting up inventory...');
  let inventoryCount = 0;
  for (const { sku, name, qty } of INVENTORY) {
    try {
      await commerce.inventory.createItem({ sku, name, initialQuantity: qty });
      inventoryCount++;
    } catch (err) {
      log(`    ! Skipped ${sku}: ${err.message}`);
    }
  }

  // Low-stock adjustments
  for (const { sku, adjustment, reason } of LOW_STOCK_ADJUSTMENTS) {
    try {
      await commerce.inventory.adjust(sku, adjustment, reason);
    } catch {
      // best-effort
    }
  }
  log(`  Done: ${inventoryCount} SKUs stocked (3 low-stock)\n`);

  // 4. Orders
  log('  [4/6] Creating orders...');
  let orderCount = 0;
  for (const orderData of ORDERS) {
    try {
      const customerId = customerMap.get(orderData.customerEmail);
      if (!customerId) continue;

      await commerce.orders.create({
        customerId,
        items: orderData.items,
        currency: 'USD',
      });
      orderCount++;
    } catch (err) {
      log(`    ! Skipped order for ${orderData.customerEmail}: ${err.message}`);
    }
  }
  log(`  Done: ${orderCount} orders\n`);

  // 5. Promotions
  log('  [5/6] Creating promotions...');
  const promos = [
    {
      name: 'Welcome 10% Off',
      promotionType: 'percentage_off',
      percentageOff: 0.1,
      code: 'WELCOME10',
    },
    {
      name: 'Summer Sale 20%',
      promotionType: 'percentage_off',
      percentageOff: 0.2,
      code: 'SUMMER20',
    },
    { name: 'Free Shipping', promotionType: 'free_shipping', code: 'FREESHIP' },
  ];
  let promoCount = 0;
  for (const promo of promos) {
    try {
      await commerce.promotions.create(promo);
      promoCount++;
    } catch {
      // best-effort — promotions may not be available in all builds
    }
  }
  log(`  Done: ${promoCount} promotions\n`);

  // 6. Subscription plans
  log('  [6/6] Creating subscription plans...');
  const plans = [
    { name: 'Basic Monthly', price: 9.99, interval: 'monthly' },
    { name: 'Pro Monthly', price: 29.99, interval: 'monthly' },
    { name: 'Enterprise Annual', price: 299.99, interval: 'yearly' },
  ];
  let planCount = 0;
  for (const plan of plans) {
    try {
      await commerce.subscriptions.createPlan(plan);
      planCount++;
    } catch {
      // best-effort
    }
  }
  log(`  Done: ${planCount} subscription plans\n`);

  // Summary
  log('  ─────────────────────────────────────');
  log('  Demo data created successfully!');
  log('');
  log('  Summary:');
  log(`    ${customerMap.size} customers`);
  log(`    ${productCount} products across 4 categories`);
  log(`    ${inventoryCount} SKUs stocked (3 low-stock)`);
  log(`    ${orderCount} orders`);
  log(`    ${promoCount} promotions`);
  log(`    ${planCount} subscription plans`);
  log('');
  log('  Try these commands:');
  log('');
  log('    stateset "show me all customers"');
  log('    stateset "what products are low on stock?"');
  log('    stateset "show me pending orders"');
  log('    stateset "what is my revenue this month?"');
  log('    stateset "who are my top customers?"');
  log('');

  return {
    customers: customerMap.size,
    products: productCount,
    inventory: inventoryCount,
    orders: orderCount,
    promotions: promoCount,
    subscriptionPlans: planCount,
  };
}
