/**
 * Tests for src/adapters/shopify/mapper.js
 *
 * Covers: stripHtml, status mappers, entity mappers (customer, product, order,
 * inventory), dispatch (mapToStateSet), and reverse mappers (mapCustomerFromStateSet,
 * mapProductFromStateSet).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  stripHtml,
  mapCustomerStatus,
  mapFinancialStatus,
  mapFulfillmentStatus,
  mapCustomerToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
  mapInventoryToStateSet,
  mapFulfillmentToStateSet,
  mapToStateSet,
  mapCustomerFromStateSet,
  mapProductFromStateSet,
  mapFulfillmentFromStateSet,
} from '../../src/adapters/shopify/mapper.js';

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const customerFixtures = require('../fixtures/shopify/customers.json');
const productFixtures = require('../fixtures/shopify/products.json');
const orderFixtures = require('../fixtures/shopify/orders.json');

// ---------------------------------------------------------------------------
// 1. stripHtml
// ---------------------------------------------------------------------------

describe('stripHtml', () => {
  it('removes simple HTML tags', () => {
    assert.equal(stripHtml('<p>hello</p>'), 'hello');
  });

  it('removes nested HTML tags', () => {
    assert.equal(stripHtml('<div><p>A <strong>bold</strong> move</p></div>'), 'A bold move');
  });

  it('decodes &amp; &lt; &gt; &quot; &#39; &nbsp;', () => {
    const input = '&amp; &lt; &gt; &quot; &#39; &nbsp;';
    // &nbsp; → space, then whitespace normalization + trim collapses trailing space
    assert.equal(stripHtml(input), '& < > " \'');
  });

  it('does not double-unescape &amp;lt; into <', () => {
    // `&amp;lt;` is an escaped `&lt;`; it must decode to the literal text
    // `&lt;`, not be double-unescaped to `<` (regression: decode &amp; last).
    assert.equal(stripHtml('&amp;lt;'), '&lt;');
    assert.equal(stripHtml('a &amp;amp; b'), 'a &amp; b');
  });

  it('normalizes whitespace', () => {
    assert.equal(stripHtml('  hello   world  '), 'hello world');
  });

  it('returns empty string for null input', () => {
    assert.equal(stripHtml(null), '');
  });

  it('returns empty string for empty string input', () => {
    assert.equal(stripHtml(''), '');
  });
});

// ---------------------------------------------------------------------------
// 2. mapCustomerStatus
// ---------------------------------------------------------------------------

describe('mapCustomerStatus', () => {
  it('maps "enabled" to "active"', () => {
    assert.equal(mapCustomerStatus('enabled'), 'active');
  });

  it('maps "disabled" to "inactive"', () => {
    assert.equal(mapCustomerStatus('disabled'), 'inactive');
  });

  it('maps "invited" to "pending"', () => {
    assert.equal(mapCustomerStatus('invited'), 'pending');
  });

  it('maps unknown value to "active" (default)', () => {
    assert.equal(mapCustomerStatus('something_else'), 'active');
  });
});

// ---------------------------------------------------------------------------
// 3. mapFinancialStatus
// ---------------------------------------------------------------------------

describe('mapFinancialStatus', () => {
  it('maps "pending" to "pending"', () => {
    assert.equal(mapFinancialStatus('pending'), 'pending');
  });

  it('maps "authorized" to "pending"', () => {
    assert.equal(mapFinancialStatus('authorized'), 'pending');
  });

  it('maps "paid" to "paid"', () => {
    assert.equal(mapFinancialStatus('paid'), 'paid');
  });

  it('maps "partially_paid" to "pending"', () => {
    assert.equal(mapFinancialStatus('partially_paid'), 'pending');
  });

  it('maps "partially_refunded" to "partially_refunded"', () => {
    assert.equal(mapFinancialStatus('partially_refunded'), 'partially_refunded');
  });

  it('maps "refunded" to "refunded"', () => {
    assert.equal(mapFinancialStatus('refunded'), 'refunded');
  });

  it('maps "voided" to "refunded"', () => {
    assert.equal(mapFinancialStatus('voided'), 'refunded');
  });
});

// ---------------------------------------------------------------------------
// 4. mapFulfillmentStatus
// ---------------------------------------------------------------------------

describe('mapFulfillmentStatus', () => {
  it('maps null to "pending"', () => {
    assert.equal(mapFulfillmentStatus(null), 'pending');
  });

  it('maps "unfulfilled" to "pending"', () => {
    assert.equal(mapFulfillmentStatus('unfulfilled'), 'pending');
  });

  it('maps "partial" to "processing"', () => {
    assert.equal(mapFulfillmentStatus('partial'), 'processing');
  });

  it('maps "fulfilled" to "shipped"', () => {
    assert.equal(mapFulfillmentStatus('fulfilled'), 'shipped');
  });

  it('maps "restocked" to "cancelled"', () => {
    assert.equal(mapFulfillmentStatus('restocked'), 'cancelled');
  });
});

// ---------------------------------------------------------------------------
// 5. mapCustomerToStateSet
// ---------------------------------------------------------------------------

describe('mapCustomerToStateSet', () => {
  const alice = customerFixtures.customers[0]; // enabled, with phone
  const bob = customerFixtures.customers[1]; // enabled, null phone
  const charlie = customerFixtures.customers[2]; // disabled
  const diana = customerFixtures.customers[3]; // invited

  it('maps a full customer with all fields', () => {
    const result = mapCustomerToStateSet(alice);
    assert.equal(result.data.email, 'alice@example.com');
    assert.equal(result.data.firstName, 'Alice');
    assert.equal(result.data.lastName, 'Johnson');
    assert.equal(result.data.phone, '+15551234567');
    assert.equal(result.data.status, 'active');
    assert.equal(result.data.acceptsMarketing, true);
  });

  it('maps customer with null phone', () => {
    const result = mapCustomerToStateSet(bob);
    assert.equal(result.data.phone, null);
  });

  it('maps disabled customer to inactive', () => {
    const result = mapCustomerToStateSet(charlie);
    assert.equal(result.data.status, 'inactive');
  });

  it('maps invited customer to pending', () => {
    const result = mapCustomerToStateSet(diana);
    assert.equal(result.data.status, 'pending');
  });

  it('preserves email, firstName, lastName, acceptsMarketing', () => {
    const result = mapCustomerToStateSet(diana);
    assert.equal(result.data.email, 'diana@example.com');
    assert.equal(result.data.firstName, 'Diana');
    assert.equal(result.data.lastName, 'Prince');
    assert.equal(result.data.acceptsMarketing, true);
  });

  it('returns correct externalId, entityType, and raw', () => {
    const result = mapCustomerToStateSet(alice);
    assert.equal(result.externalId, '1001');
    assert.equal(result.entityType, 'customers');
    assert.deepEqual(result.raw, alice);
  });

  it('metadata includes shopifyId and shopifyTags', () => {
    const result = mapCustomerToStateSet(alice);
    assert.equal(result.data.metadata.shopifyId, '1001');
    assert.equal(result.data.metadata.shopifyTags, 'vip,wholesale');
  });

  it('metadata includes shopifyNote', () => {
    const result = mapCustomerToStateSet(alice);
    assert.equal(result.data.metadata.shopifyNote, 'Preferred customer');
  });
});

// ---------------------------------------------------------------------------
// 6. mapProductToStateSet
// ---------------------------------------------------------------------------

describe('mapProductToStateSet', () => {
  const classicWidget = productFixtures.products[0]; // 2 variants, active, has HTML
  const premiumGadget = productFixtures.products[1]; // 1 variant, no HTML
  const deluxeThing = productFixtures.products[2]; // draft, empty tags
  const simplePart = productFixtures.products[3]; // empty body_html, empty tags
  const giftCard = productFixtures.products[4]; // entity-encoded title, has tags

  it('maps product with multiple variants', () => {
    const result = mapProductToStateSet(classicWidget);
    assert.equal(result.data.variants.length, 2);
    assert.equal(result.data.variants[0].sku, 'WIDGET-SM');
    assert.equal(result.data.variants[1].sku, 'WIDGET-LG');
  });

  it('maps product with single variant', () => {
    const result = mapProductToStateSet(premiumGadget);
    assert.equal(result.data.variants.length, 1);
    assert.equal(result.data.variants[0].sku, 'GADGET-01');
  });

  it('strips HTML from description', () => {
    const result = mapProductToStateSet(classicWidget);
    assert.equal(result.data.description, 'A premium widget for everyday use.');
  });

  it('maps handle to slug', () => {
    const result = mapProductToStateSet(classicWidget);
    assert.equal(result.data.slug, 'classic-widget');
  });

  it('maps active status correctly', () => {
    const result = mapProductToStateSet(classicWidget);
    assert.equal(result.data.status, 'active');
  });

  it('maps draft status correctly', () => {
    const result = mapProductToStateSet(deluxeThing);
    assert.equal(result.data.status, 'draft');
  });

  it('splits tags correctly', () => {
    const result = mapProductToStateSet(classicWidget);
    assert.deepEqual(result.data.tags, ['bestseller', 'widget']);
  });

  it('returns empty array for empty tags', () => {
    const result = mapProductToStateSet(simplePart);
    assert.deepEqual(result.data.tags, []);
  });

  it('maps variant fields: sku, name, price, compareAtPrice, weight, barcode', () => {
    const result = mapProductToStateSet(classicWidget);
    const v = result.data.variants[0];
    assert.equal(v.sku, 'WIDGET-SM');
    assert.equal(v.name, 'Small');
    assert.equal(v.price, 19.99);
    assert.equal(v.compareAtPrice, 24.99);
    assert.equal(v.weight, 100);
    assert.equal(v.barcode, '123456789');
  });

  it('metadata includes shopifyId and shopifyHandle', () => {
    const result = mapProductToStateSet(classicWidget);
    assert.equal(result.data.metadata.shopifyId, '2001');
    assert.equal(result.data.metadata.shopifyHandle, 'classic-widget');
  });
});

// ---------------------------------------------------------------------------
// 7. mapOrderToStateSet
// ---------------------------------------------------------------------------

describe('mapOrderToStateSet', () => {
  const orderPaid = orderFixtures.orders[0]; // paid, null fulfillment, 2 items, shipping addr
  const orderPending = orderFixtures.orders[1]; // pending, partial fulfillment
  const orderRefunded = orderFixtures.orders[2]; // refunded, fulfilled, EUR
  const orderPartialRefund = orderFixtures.orders[3]; // partially_refunded, restocked, null addr
  const orderAuthorized = orderFixtures.orders[4]; // authorized, null fulfillment, GBP

  it('maps order with line items', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.equal(result.data.items.length, 2);
    assert.equal(result.data.items[0].sku, 'WIDGET-SM');
    assert.equal(result.data.items[1].sku, 'PART-001');
  });

  it('resolves customer ID via idMap when provided', () => {
    const mockIdMap = {
      lookup: (platform, entityType, externalId) => {
        if (platform === 'shopify' && entityType === 'customers' && externalId === '1001') {
          return { statesetId: 'ss-cust-001' };
        }
        return null;
      },
    };
    const result = mapOrderToStateSet(orderPaid, {
      idMap: mockIdMap,
      platform: 'shopify',
    });
    assert.equal(result.data.customerId, 'ss-cust-001');
  });

  it('sets customerId to null when idMap is not provided', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.equal(result.data.customerId, null);
  });

  it('maps financial status "paid" to "paid"', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.equal(result.data.paymentStatus, 'paid');
  });

  it('maps financial status "pending" to "pending"', () => {
    const result = mapOrderToStateSet(orderPending);
    assert.equal(result.data.paymentStatus, 'pending');
  });

  it('maps fulfillment status null to "pending"', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.equal(result.data.fulfillmentStatus, 'pending');
  });

  it('maps fulfillment status "partial" to "processing"', () => {
    const result = mapOrderToStateSet(orderPending);
    assert.equal(result.data.fulfillmentStatus, 'processing');
  });

  it('maps line item fields: sku, name, quantity, unitPrice, totalPrice', () => {
    const result = mapOrderToStateSet(orderPaid);
    const li = result.data.items[0];
    assert.equal(li.sku, 'WIDGET-SM');
    assert.equal(li.name, 'Classic Widget - Small');
    assert.equal(li.quantity, 2);
    assert.equal(li.unitPrice, 19.99);
    assert.equal(li.totalPrice, 19.99 * 2);
  });

  it('maps shipping address correctly', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.deepEqual(result.data.shippingAddress, {
      address1: '123 Main St',
      address2: 'Apt 4',
      city: 'Anytown',
      province: 'CA',
      zip: '90210',
      country: 'US',
    });
  });

  it('sets shippingAddress to null when not present', () => {
    const result = mapOrderToStateSet(orderPartialRefund);
    assert.equal(result.data.shippingAddress, null);
  });

  it('preserves currency from Shopify', () => {
    const resultEur = mapOrderToStateSet(orderRefunded);
    assert.equal(resultEur.data.currency, 'EUR');
    const resultGbp = mapOrderToStateSet(orderAuthorized);
    assert.equal(resultGbp.data.currency, 'GBP');
  });

  it('uses total_price from Shopify as totalAmount', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.equal(result.data.totalAmount, 49.98);
  });

  it('metadata includes shopifyId and shopifyOrderNumber', () => {
    const result = mapOrderToStateSet(orderPaid);
    assert.equal(result.data.metadata.shopifyId, '5001');
    assert.equal(result.data.metadata.shopifyOrderNumber, '1001');
  });

  it('line item metadata includes shopifyLineItemId and shopifyVariantId', () => {
    const result = mapOrderToStateSet(orderPaid);
    const li = result.data.items[0];
    assert.equal(li.metadata.shopifyLineItemId, '6001');
    assert.equal(li.metadata.shopifyVariantId, '3001');
  });
});

// ---------------------------------------------------------------------------
// 8. mapInventoryToStateSet
// ---------------------------------------------------------------------------

describe('mapInventoryToStateSet', () => {
  it('maps basic inventory level', () => {
    const inv = { inventory_item_id: 4001, sku: 'WIDGET-SM', available: 50, location_id: 100 };
    const result = mapInventoryToStateSet(inv);
    assert.equal(result.entityType, 'inventory');
    assert.equal(result.externalId, '4001');
    assert.equal(result.data.sku, 'WIDGET-SM');
    assert.equal(result.data.quantity, 50);
  });

  it('maps zero available quantity', () => {
    const inv = { inventory_item_id: 4002, sku: 'WIDGET-LG', available: 0 };
    const result = mapInventoryToStateSet(inv);
    assert.equal(result.data.quantity, 0);
  });

  it('maps null available to 0', () => {
    const inv = { inventory_item_id: 4003, sku: 'GADGET-01', available: null };
    const result = mapInventoryToStateSet(inv);
    assert.equal(result.data.quantity, 0);
  });

  it('uses deterministic fallback SKU when sku is missing', () => {
    const inv = { inventory_item_id: 9999, available: 3 };
    const result = mapInventoryToStateSet(inv);
    assert.equal(result.data.sku, 'SHOPIFY-INV-9999');
  });

  it('metadata includes shopifyInventoryItemId', () => {
    const inv = { inventory_item_id: 4004, sku: 'THING-RED', available: 10, location_id: 200 };
    const result = mapInventoryToStateSet(inv);
    assert.equal(result.data.metadata.shopifyInventoryItemId, '4004');
    assert.equal(result.data.metadata.shopifyLocationId, '200');
  });
});

// ---------------------------------------------------------------------------
// 9. mapFulfillmentToStateSet
// ---------------------------------------------------------------------------

describe('mapFulfillmentToStateSet', () => {
  it('maps fulfillment payload into shipment-compatible format', () => {
    const fulfillment = {
      id: 7001,
      order_id: 5001,
      status: 'success',
      tracking_number: 'TRACK123',
      tracking_company: 'FedEx',
    };

    const result = mapFulfillmentToStateSet(fulfillment);
    assert.equal(result.entityType, 'fulfillments');
    assert.equal(result.externalId, '7001');
    assert.equal(result.data.status, 'shipped');
    assert.equal(result.data.trackingNumber, 'TRACK123');
    assert.equal(result.data.carrier, 'FedEx');
    assert.equal(result.data.metadata.shopifyOrderId, '5001');
  });

  it('resolves orderId via idMap context when available', () => {
    const fulfillment = { id: 7002, order_id: 5002, status: 'open' };
    const idMap = {
      lookup: (platform, entityType, externalId) => {
        if (platform === 'shopify' && entityType === 'orders' && externalId === '5002') {
          return { statesetId: 'ord-ss-1' };
        }
        return null;
      },
    };
    const result = mapFulfillmentToStateSet(fulfillment, { idMap, platform: 'shopify' });
    assert.equal(result.data.orderId, 'ord-ss-1');
  });

  it('maps fulfillment status cancelled to cancelled', () => {
    const fulfillment = { id: 7003, status: 'cancelled' };
    const result = mapFulfillmentToStateSet(fulfillment);
    assert.equal(result.data.status, 'cancelled');
  });
});

// ---------------------------------------------------------------------------
// 10. mapToStateSet dispatch
// ---------------------------------------------------------------------------

describe('mapToStateSet dispatch', () => {
  it('dispatches "customers" to mapCustomerToStateSet', () => {
    const result = mapToStateSet('customers', customerFixtures.customers[0]);
    assert.equal(result.entityType, 'customers');
    assert.equal(result.data.email, 'alice@example.com');
  });

  it('dispatches "products" to mapProductToStateSet', () => {
    const result = mapToStateSet('products', productFixtures.products[0]);
    assert.equal(result.entityType, 'products');
    assert.equal(result.data.name, 'Classic Widget');
  });

  it('dispatches "orders" to mapOrderToStateSet', () => {
    const result = mapToStateSet('orders', orderFixtures.orders[0]);
    assert.equal(result.entityType, 'orders');
    assert.equal(result.data.paymentStatus, 'paid');
  });

  it('dispatches "inventory" to mapInventoryToStateSet', () => {
    const inv = { inventory_item_id: 4001, sku: 'WIDGET-SM', available: 50 };
    const result = mapToStateSet('inventory', inv);
    assert.equal(result.entityType, 'inventory');
    assert.equal(result.data.sku, 'WIDGET-SM');
  });

  it('dispatches "fulfillments" to mapFulfillmentToStateSet', () => {
    const fulfillment = { id: 7001, status: 'success' };
    const result = mapToStateSet('fulfillments', fulfillment);
    assert.equal(result.entityType, 'fulfillments');
    assert.equal(result.data.status, 'shipped');
  });

  it('throws for unknown entity type', () => {
    assert.throws(() => mapToStateSet('widgets', {}), { message: 'Unknown entity type: widgets' });
  });
});

// ---------------------------------------------------------------------------
// 11. mapCustomerFromStateSet (reverse mapper)
// ---------------------------------------------------------------------------

describe('mapCustomerFromStateSet', () => {
  it('round-trips email, first_name, last_name', () => {
    const ss = {
      email: 'alice@example.com',
      firstName: 'Alice',
      lastName: 'Johnson',
      phone: '+15551234567',
      status: 'active',
      acceptsMarketing: true,
    };
    const shopify = mapCustomerFromStateSet(ss);
    assert.equal(shopify.email, 'alice@example.com');
    assert.equal(shopify.first_name, 'Alice');
    assert.equal(shopify.last_name, 'Johnson');
  });

  it('maps status back to Shopify state', () => {
    const active = mapCustomerFromStateSet({ email: 'a@b.com', status: 'active' });
    assert.equal(active.state, 'enabled');
    const inactive = mapCustomerFromStateSet({ email: 'a@b.com', status: 'inactive' });
    assert.equal(inactive.state, 'disabled');
  });

  it('preserves accepts_marketing', () => {
    const ss = { email: 'a@b.com', acceptsMarketing: true, status: 'active' };
    const shopify = mapCustomerFromStateSet(ss);
    assert.equal(shopify.accepts_marketing, true);
  });
});

// ---------------------------------------------------------------------------
// 12. mapProductFromStateSet (reverse mapper)
// ---------------------------------------------------------------------------

describe('mapProductFromStateSet', () => {
  it('maps name to title and description to body_html', () => {
    const ss = {
      name: 'Classic Widget',
      description: 'A premium widget.',
      slug: 'classic-widget',
      status: 'active',
      variants: [],
    };
    const shopify = mapProductFromStateSet(ss);
    assert.equal(shopify.title, 'Classic Widget');
    assert.equal(shopify.body_html, 'A premium widget.');
  });

  it('maps status correctly', () => {
    const active = mapProductFromStateSet({ name: 'A', status: 'active', variants: [] });
    assert.equal(active.status, 'active');
    const draft = mapProductFromStateSet({ name: 'B', status: 'draft', variants: [] });
    assert.equal(draft.status, 'draft');
  });

  it('maps variants back to Shopify format', () => {
    const ss = {
      name: 'Widget',
      description: '',
      slug: 'widget',
      status: 'active',
      variants: [
        { sku: 'W-SM', name: 'Small', price: 19.99, compareAtPrice: 24.99 },
        { sku: 'W-LG', name: 'Large', price: 29.99, compareAtPrice: null },
      ],
    };
    const shopify = mapProductFromStateSet(ss);
    assert.equal(shopify.variants.length, 2);
    assert.equal(shopify.variants[0].sku, 'W-SM');
    assert.equal(shopify.variants[0].title, 'Small');
    assert.equal(shopify.variants[0].price, '19.99');
    assert.equal(shopify.variants[0].compare_at_price, '24.99');
    assert.equal(shopify.variants[1].compare_at_price, null);
  });
});

// ---------------------------------------------------------------------------
// 13. mapFulfillmentFromStateSet (reverse mapper)
// ---------------------------------------------------------------------------

describe('mapFulfillmentFromStateSet', () => {
  it('maps shipment fields back to Shopify-like fulfillment payload', () => {
    const shipment = {
      trackingNumber: 'TRACK-500',
      carrier: 'UPS',
      status: 'shipped',
    };
    const result = mapFulfillmentFromStateSet(shipment);
    assert.equal(result.tracking_number, 'TRACK-500');
    assert.equal(result.tracking_company, 'UPS');
    assert.equal(result.status, 'shipped');
  });
});
