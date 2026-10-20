import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { adaptCommerceApis, getCommerceCtor } from '../commerce.js';
import { PAGE_SIZE, simulationPlans } from './model.js';
import { exploreStore } from './explore-store.js';
import { browseStore } from './browse.js';

async function seedDemo(commerce) {
  const customers = [];
  for (const [firstName, lastName] of [
    ['Ada', 'Lovelace'],
    ['Grace', 'Hopper'],
    ['Alan', 'Turing'],
  ])
    customers.push(
      await commerce.customers.create({
        firstName,
        lastName,
        email: `${firstName.toLowerCase()}@demo.example`,
      }),
    );
  const catalog = [
    ['COFFEE-250G', 'House blend', '12.50', 80],
    ['MATCHA-50G', 'Ceremonial matcha', '24.00', 40],
    ['FILTER-100', 'Paper filters', '8.75', 6],
    ['MUG-STONE', 'Stoneware mug', '18.00', 0],
  ];
  const products = [];
  for (const [sku, name, priceExact, initialQuantity] of catalog) {
    const product = await commerce.products.create({
      name,
      slug: sku.toLowerCase(),
      variants: [{ sku, priceExact, isDefault: true }],
    });
    await commerce.products.activate(product.id);
    await commerce.inventory.createItem({ sku, name, initialQuantity, reorderPoint: 10 });
    products.push(product);
  }
  for (let i = 0; i < 12; i++) {
    const index = i % 2,
      [sku, name, unitPriceExact] = catalog[index];
    const order = await commerce.orders.createExact({
      customerId: customers[i % 3].id,
      currency: 'USD',
      stockPolicy: 'reject_if_insufficient',
      items: [
        {
          productId: products[index].id,
          sku,
          name,
          quantity: (i % 3) + 1,
          unitPrice: unitPriceExact,
        },
      ],
    });
    if (i % 3 !== 0) {
      const payment = await commerce.payments.create({
        orderId: order.id,
        amountExact: order.totalAmountExact,
        currency: 'USD',
        paymentMethod: 'credit_card',
      });
      await commerce.payments.markCompleted(payment.id);
      await commerce.orders.updateStatus(order.id, 'confirmed');
      if (i % 2 === 0) await commerce.orders.updateStatus(order.id, 'processing');
    }
  }
}

export async function readStore(commerce, pages = {}) {
  const tasks = {
    orderCount: () => commerce.orders.count(),
    recentOrders: () => commerce.orders.list({ limit: 3 }),
    customerCount: () => commerce.customers.count(),
    productCount: () => commerce.products.count(),
    paymentCount: () => commerce.payments.count(),
    orders: () =>
      commerce.orders.list({ limit: PAGE_SIZE, offset: (pages.orders || 0) * PAGE_SIZE }),
    customers: () =>
      commerce.customers.list({ limit: PAGE_SIZE, offset: (pages.customers || 0) * PAGE_SIZE }),
    payments: () =>
      commerce.payments.list({ limit: PAGE_SIZE, offset: (pages.payments || 0) * PAGE_SIZE }),
    health: () => commerce.analytics.inventoryHealth(),
    inventory: () => commerce.analytics.lowStockItems(10),
  };
  const result = {
    counts: {},
    errors: [],
    pages: { ...pages },
    checkedAt: new Date().toISOString(),
  };
  await Promise.all(
    Object.entries(tasks).map(async ([key, run]) => {
      try {
        result[key] = await run();
      } catch (error) {
        result.errors.push({ section: key, message: error.message });
      }
    }),
  );
  for (const [key, field] of [
    ['orderCount', 'orders'],
    ['customerCount', 'customers'],
    ['productCount', 'products'],
    ['paymentCount', 'payments'],
  ])
    result.counts[field] = result[key] ?? null;
  result.inventoryTruncated = (result.inventory?.length || 0) > PAGE_SIZE;
  result.inventoryCount = result.inventory?.length ?? null;
  result.inventory = result.inventory?.slice(0, PAGE_SIZE);
  return result;
}

async function loadStore({ dbPath, directory, payload, operation, kernel }) {
  let target = path.join(directory, 'demo.db');
  const marker = path.join(directory, 'demo.ready');
  const fresh = !fs.existsSync(marker);
  if (!dbPath && fresh) {
    for (const suffix of ['', '-wal', '-shm']) fs.rmSync(target + suffix, { force: true });
  }
  if (dbPath) {
    // Never initialize or migrate the operator's source database. SQLite's
    // online backup includes committed WAL data, unlike copying the .db file.
    const require = createRequire(import.meta.url);
    let Database;
    try {
      Database = require('better-sqlite3');
    } catch {
      throw new Error(
        'Browsing --db needs the optional better-sqlite3 package. Reinstall @stateset/cli with optional dependencies, or use --demo.',
      );
    }
    target = path.join(directory, 'snapshot.db');
    for (const suffix of ['', '-wal', '-shm']) fs.rmSync(target + suffix, { force: true });
    const source = new Database(dbPath, { readonly: true, fileMustExist: true, timeout: 5000 });
    try {
      await source.backup(target);
    } finally {
      source.close();
    }
  }
  // The workbench owns this private copy and needs only one connection. Use
  // the native async opener rather than starting a general-purpose pool for
  // every refresh; no operator/global engine setting is changed.
  const Commerce = getCommerceCtor();
  if (typeof Commerce.open !== 'function')
    throw new Error(
      'The native engine is too old for the terminal workbench. Update @stateset/embedded.',
    );
  const commerce = adaptCommerceApis(await Commerce.open(target, { maxConnections: 1 }));
  try {
    if (!dbPath && fresh) {
      await seedDemo(commerce);
      fs.writeFileSync(marker, 'ready\n', { mode: 0o600 });
    }
    if (operation === 'shipment-preview') {
      const { previewShipment } = await import('./shipment-engine.js');
      return await previewShipment(commerce, kernel, payload);
    }
    if (operation === 'refund-preview') {
      const { previewRefund } = await import('./refund-engine.js');
      return await previewRefund(commerce, kernel, payload);
    }
    if (operation === 'order-preview') {
      const { previewOrder } = await import('./order-engine.js');
      return await previewOrder(commerce, kernel, payload);
    }
    const snapshot = await readStore(commerce, payload.pages);
    snapshot.alerts = snapshot.inventory;
    if (payload.focus) {
      const { domain, id } = payload.focus;
      if (!['orders', 'customers', 'payments'].includes(domain) || typeof id !== 'string')
        throw new Error('Unsupported record link');
      snapshot.focusRecord = await commerce[domain].get(id);
      if (!snapshot.focusRecord) throw new Error('Linked record no longer exists');
    }
    if (payload.explorer) snapshot.explorer = exploreStore(target, payload.explorer);
    return await browseStore(commerce, target, payload, snapshot);
  } finally {
    await commerce.close();
  }
}

if (process.send)
  process.once('message', async (request) => {
    try {
      process.send({
        type: 'progress',
        message:
          request.operation === 'snapshot'
            ? 'Opening private store snapshot'
            : 'Validating simulation inputs',
      });
      let data;
      if (request.operation === 'snapshot') data = await loadStore(request);
      else if (['order-preview', 'refund-preview', 'shipment-preview'].includes(request.operation))
        data = await loadStore(request);
      else if (request.operation === 'shipment-apply') {
        const { applyShipment } = await import('./shipment-engine.js');
        data = await applyShipment(request.dbPath, request.kernel, request.payload);
      } else if (request.operation === 'shipment-reconcile') {
        const { reconcileShipment } = await import('./shipment-engine.js');
        data = reconcileShipment(request.dbPath, request.kernel, request.payload);
      } else if (request.operation === 'order-apply') {
        const { applyOrder } = await import('./order-engine.js');
        data = await applyOrder(request.dbPath, request.kernel, request.payload);
      } else if (request.operation === 'order-reconcile') {
        const { reconcileOrder } = await import('./order-engine.js');
        data = reconcileOrder(request.dbPath, request.kernel, request.payload);
      } else if (request.operation === 'refund-apply') {
        const { applyRefund } = await import('./refund-engine.js');
        data = await applyRefund(request.dbPath, request.kernel, request.payload);
      } else if (request.operation === 'refund-reconcile') {
        const { reconcileRefund } = await import('./refund-engine.js');
        data = reconcileRefund(request.dbPath, request.kernel, request.payload);
      } else if (request.operation === 'simulate') {
        // This process is already bounded and killable by the terminal host.
        // Run directly here so quitting cannot orphan a second worker process.
        const { executeInventoryPlan } = await import('../simulation/planning.js');
        const directory = fs.mkdtempSync(path.join(request.directory, 'lab-'));
        try {
          const results = [];
          const inputs = simulationPlans(
            request.payload.preset,
            request.payload.seed,
            request.payload.controls,
            request.payload.products || [],
          );
          for (const [index, plan] of inputs.entries()) {
            process.send({
              type: 'progress',
              message: `Plan ${index + 1}/${inputs.length}: ${plan.name}, ${plan.products.length} SKU(s), ${plan.days} days`,
            });
            results.push(
              await executeInventoryPlan(plan, path.join(directory, `plan-${index}.db`)),
            );
          }
          data = {
            success: true,
            results,
            inputs,
            simulation: true,
            liveStoreRead: false,
            liveStoreModified: false,
            externalServicesCalled: false,
          };
        } finally {
          fs.rmSync(directory, { recursive: true, force: true });
        }
      } else throw new Error('Unknown terminal operation');
      process.send({ success: true, data }, () => process.exit(0));
    } catch (error) {
      process.send(
        {
          success: false,
          error: String(error.message).split(request.directory).join('<private-store>'),
        },
        () => process.exit(1),
      );
    }
  });
