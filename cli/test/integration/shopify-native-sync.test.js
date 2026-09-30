import { beforeEach, afterEach, test } from 'node:test';
import assert from 'node:assert/strict';
import { Commerce } from '@stateset/embedded';
import { createNativeToolkit } from '@stateset/embedded/native-toolkit';
import Database from 'better-sqlite3';
import { IdMapStore } from '../../src/adapters/id-map-store.js';
import { createShopifyWebhookHandlers } from '../../src/adapters/shopify/webhooks.js';
import {
  mapProductToStateSet,
  mapOrderToStateSet,
  mapProductFromStateSet,
} from '../../src/adapters/shopify/mapper.js';

let commerce, db, mappings, handlers;
beforeEach(() => {
  commerce = new Commerce(':memory:');
  db = new Database(':memory:');
  mappings = new IdMapStore(db);
  handlers = createShopifyWebhookHandlers(commerce, mappings);
});
afterEach(() => {
  db.close();
});

async function customer() {
  const result = await handlers['customers/create']({
    id: 1,
    email: 'ada@example.com',
    first_name: 'Ada',
    last_name: 'Lovelace',
  });
  return result.statesetId;
}

async function order() {
  await customer();
  const payload = {
    id: 10,
    customer: { id: 1 },
    currency: 'USD',
    total_price: '12.34',
    line_items: [{ id: 11, name: 'Widget', sku: 'W-1', quantity: 1, price: '12.34' }],
  };
  const result = await handlers['orders/create'](payload);
  return { id: result.statesetId, payload };
}

test('customer partial updates change native fields, preserve omitted fields, and refuse stale snapshots', async () => {
  const id = await customer();
  await handlers['customers/update']({
    id: 1,
    first_name: 'Augusta',
    updated_at: '2026-09-20T00:00:00Z',
  });
  const native = await commerce.customers.get(id);
  assert.equal(native.firstName, 'Augusta');
  assert.equal(native.lastName, 'Lovelace');
  assert.equal(native.email, 'ada@example.com');
  const stale = await handlers['customers/update']({
    id: 1,
    first_name: 'Old',
    updated_at: '2026-09-19T00:00:00Z',
  });
  assert.equal(stale.reason, 'stale_event');
  assert.equal((await commerce.customers.get(id)).firstName, 'Augusta');
  const snapshot = mappings.lookup('shopify', 'customers', '1').externalData;
  await assert.rejects(handlers['customers/update']({ id: 1, email: 'invalid' }));
  assert.equal(mappings.lookup('shopify', 'customers', '1').externalData, snapshot);
});

test('product updates preserve variant identity across SKU changes and retain exact large prices', async () => {
  const original = {
    id: 2,
    title: 'Widget',
    handle: 'widget',
    status: 'active',
    variants: [
      {
        id: 20,
        sku: 'OLD-SKU',
        title: 'Default',
        price: '9007199254740993.25',
        compare_at_price: '9007199254740994.25',
      },
    ],
  };
  const created = await handlers['products/create'](original);
  const before = (await commerce.products.getVariants(created.statesetId))[0];
  assert.equal(before.priceExact, '9007199254740993.25');
  const updated = {
    ...original,
    title: 'Revised Widget',
    variants: [
      {
        ...original.variants[0],
        sku: 'NEW-SKU',
        price: '9007199254740993.26',
        compare_at_price: null,
      },
    ],
  };
  await handlers['products/update'](updated);
  const variants = await commerce.products.getVariants(created.statesetId);
  assert.equal((await commerce.products.get(created.statesetId)).name, 'Revised Widget');
  assert.equal(variants.length, 1);
  assert.equal(variants[0].id, before.id);
  assert.equal(variants[0].sku, 'NEW-SKU');
  assert.equal(variants[0].priceExact, '9007199254740993.26');
  assert.equal(variants[0].compareAtPriceExact, undefined);
  await handlers['products/update'](updated);
  assert.equal((await commerce.products.getVariants(created.statesetId)).length, 1);
  const exported = mapProductFromStateSet({ name: 'Widget', variants });
  assert.equal(exported.variants[0].price, '9007199254740993.26');
});

test('order updates write native statuses and addresses and surface economic edits for reconciliation', async () => {
  const { id, payload } = await order();
  const result = await handlers['orders/updated']({
    id: 10,
    financial_status: 'paid',
    fulfillment_status: 'partial',
    note: 'External payment received',
    shipping_address: {
      address1: '1 Main',
      city: 'Vancouver',
      province_code: 'BC',
      zip: 'V1V1V1',
      country_code: 'CA',
    },
  });
  assert.equal(result.action, 'updated');
  const native = await commerce.orders.get(id);
  assert.equal(native.paymentStatus, 'paid');
  assert.equal(native.fulfillmentStatus, 'partially_fulfilled');
  assert.equal(native.shippingAddress.line1, '1 Main');
  assert.equal(native.totalAmountExact, '12.34');
  const snapshot = mappings.lookup('shopify', 'orders', '10').externalData;
  await assert.rejects(
    handlers['orders/updated']({ ...payload, total_price: '999.00' }),
    /reconciliation/,
  );
  assert.equal(mappings.lookup('shopify', 'orders', '10').externalData, snapshot);
  await assert.rejects(
    commerce.orders.update(id, { paymentStatus: 'invented' }),
    /Invalid payment_status/,
  );
  assert.equal((await commerce.orders.get(id)).paymentStatus, 'paid');
});

test('shipment update changes native tracking, carrier, and delivery status', async () => {
  const { id } = await order();
  const shipment = await commerce.shipments.create({
    orderId: id,
    recipientName: 'Ada',
    shippingAddress: '1 Main, Vancouver',
  });
  mappings.store('shopify', 'fulfillments', '30', shipment.id, { id: 30, status: 'pending' });
  const pendingSnapshot = mappings.lookup('shopify', 'fulfillments', '30').externalData;
  await assert.rejects(
    handlers['fulfillments/update']({ id: 30, shipment_status: 'delivered' }),
    /Invalid shipment status transition/,
  );
  assert.equal(mappings.lookup('shopify', 'fulfillments', '30').externalData, pendingSnapshot);
  await commerce.shipments.update(shipment.id, { status: 'processing', expectedVersion: 1 });
  await assert.rejects(
    commerce.shipments.update(shipment.id, { notes: 'stale', expectedVersion: 1 }),
    /[Vv]ersion/,
  );
  await commerce.shipments.update(shipment.id, { status: 'ready_to_ship', expectedVersion: 2 });
  await handlers['fulfillments/update']({
    id: 30,
    status: 'success',
    tracking_number: 'TRACK-123',
    tracking_company: 'UPS',
  });
  const shipped = await commerce.shipments.get(shipment.id);
  assert.equal(shipped.status, 'shipped');
  assert.equal(shipped.trackingNumber, 'TRACK-123');
  assert.equal(shipped.carrier, 'ups');
  await commerce.shipments.update(shipment.id, { status: 'in_transit' });
  await commerce.shipments.update(shipment.id, { status: 'out_for_delivery' });
  await handlers['fulfillments/update']({ id: 30, shipment_status: 'delivered' });
  assert.equal((await commerce.shipments.get(shipment.id)).status, 'delivered');
  await assert.rejects(
    commerce.shipments.update(shipment.id, { status: 'invented' }),
    /Invalid status/,
  );
});

test('missing native write support is an error and never advances a mapping', async () => {
  const id = await customer();
  const unsupported = createShopifyWebhookHandlers({ customers: {} }, mappings);
  const snapshot = mappings.lookup('shopify', 'customers', '1').externalData;
  await assert.rejects(unsupported['customers/update']({ id: 1, first_name: 'Changed' }), /update/);
  assert.equal(mappings.lookup('shopify', 'customers', '1').externalData, snapshot);
  assert.equal((await commerce.customers.get(id)).firstName, 'Ada');
});

test('Shopify mapping rejects malformed money and preserves explicit zero totals', () => {
  assert.throws(() => mapProductToStateSet({ variants: [{ price: '12garbage' }] }), /Invalid/);
  const mapped = mapOrderToStateSet({
    id: 10,
    total_price: '0.00',
    line_items: [{ id: 1, quantity: 2, price: '9007199254740993.25' }],
  });
  assert.equal(mapped.data.totalAmount, '0');
  assert.equal(mapped.data.items[0].unitPriceExact, '9007199254740993.25');
  assert.equal(mapped.data.items[0].totalPrice, '18014398509481986.5');
});

test('new native update tools preview by default and execute only with apply enabled', async () => {
  const { id } = await order();
  const shipment = await commerce.shipments.create({
    orderId: id,
    recipientName: 'Ada',
    shippingAddress: '1 Main',
  });
  const preview = createNativeToolkit(commerce);
  const applied = createNativeToolkit(commerce, { allowApply: true });
  const orderPatch = { id, input: { notes: 'Reviewed' } };
  const shipmentPatch = { id: shipment.id, input: { trackingNumber: 'TRACK-NEW' } };
  assert.equal((await preview.executeTool('orders.update', orderPatch)).preview, true);
  assert.equal((await preview.executeTool('shipments.update', shipmentPatch)).preview, true);
  assert.notEqual((await commerce.orders.get(id)).notes, 'Reviewed');
  assert.notEqual((await commerce.shipments.get(shipment.id)).trackingNumber, 'TRACK-NEW');
  await applied.executeTool('orders.update', orderPatch);
  await applied.executeTool('shipments.update', shipmentPatch);
  assert.equal((await commerce.orders.get(id)).notes, 'Reviewed');
  assert.equal((await commerce.shipments.get(shipment.id)).trackingNumber, 'TRACK-NEW');
});

test('general order update retains the native guard against orphaning payment obligations', async () => {
  const { id } = await order();
  await commerce.payments.createExact({ orderId: id, amount: '12.34', currency: 'USD' });
  await assert.rejects(commerce.orders.update(id, { status: 'cancelled' }), /payment|captur|fund/i);
  assert.notEqual((await commerce.orders.get(id)).status, 'cancelled');
});
