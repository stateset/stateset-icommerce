import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

import {
  mapCustomerToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
  mapInventoryToStateSet,
  mapCustomerFromStateSet,
  mapProductFromStateSet,
  mapToStateSet,
  mapFromStateSet,
  mapOrderStatus,
  mapProductStatus,
  derivePaymentStatus,
  stripHtml,
} from '../../src/adapters/woocommerce/mapper.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(__dirname, '..', 'fixtures', 'woocommerce');

function loadFixture(name) {
  return JSON.parse(readFileSync(join(fixturesDir, name), 'utf-8'));
}

// ---------------------------------------------------------------------------
// stripHtml
// ---------------------------------------------------------------------------

describe('woocommerce mapper — stripHtml', () => {
  it('strips HTML tags', () => {
    assert.equal(stripHtml('<p>Hello <b>world</b></p>'), 'Hello world');
  });

  it('decodes HTML entities', () => {
    assert.equal(stripHtml('&amp; &lt; &gt; &quot; &#39;'), '& < > " \'');
  });

  it('does not double-unescape &amp;lt; into <', () => {
    // `&amp;lt;` must decode to the literal `&lt;`, not `<` (decode &amp; last).
    assert.equal(stripHtml('&amp;lt;'), '&lt;');
    assert.equal(stripHtml('a &amp;amp; b'), 'a &amp; b');
  });

  it('returns empty string for null/undefined', () => {
    assert.equal(stripHtml(null), '');
    assert.equal(stripHtml(undefined), '');
  });

  it('returns empty string for non-string', () => {
    assert.equal(stripHtml(42), '');
  });

  it('collapses whitespace', () => {
    assert.equal(stripHtml('<p>  hello   world  </p>'), 'hello world');
  });
});

// ---------------------------------------------------------------------------
// mapOrderStatus
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapOrderStatus', () => {
  it('maps pending to pending', () => {
    assert.equal(mapOrderStatus('pending'), 'pending');
  });

  it('maps processing to processing', () => {
    assert.equal(mapOrderStatus('processing'), 'processing');
  });

  it('maps on-hold to pending', () => {
    assert.equal(mapOrderStatus('on-hold'), 'pending');
  });

  it('maps completed to shipped', () => {
    assert.equal(mapOrderStatus('completed'), 'shipped');
  });

  it('maps cancelled to cancelled', () => {
    assert.equal(mapOrderStatus('cancelled'), 'cancelled');
  });

  it('maps refunded to refunded', () => {
    assert.equal(mapOrderStatus('refunded'), 'refunded');
  });

  it('maps failed to failed', () => {
    assert.equal(mapOrderStatus('failed'), 'failed');
  });

  it('defaults unknown status to pending', () => {
    assert.equal(mapOrderStatus('unknown'), 'pending');
    assert.equal(mapOrderStatus(''), 'pending');
  });
});

// ---------------------------------------------------------------------------
// mapProductStatus
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapProductStatus', () => {
  it('maps publish to active', () => {
    assert.equal(mapProductStatus('publish'), 'active');
  });

  it('maps draft to draft', () => {
    assert.equal(mapProductStatus('draft'), 'draft');
  });

  it('maps pending to pending', () => {
    assert.equal(mapProductStatus('pending'), 'pending');
  });

  it('maps private to active', () => {
    assert.equal(mapProductStatus('private'), 'active');
  });

  it('defaults unknown status to draft', () => {
    assert.equal(mapProductStatus('unknown'), 'draft');
  });
});

// ---------------------------------------------------------------------------
// derivePaymentStatus
// ---------------------------------------------------------------------------

describe('woocommerce mapper — derivePaymentStatus', () => {
  it('returns refunded for refunded orders', () => {
    assert.equal(derivePaymentStatus({ status: 'refunded' }), 'refunded');
  });

  it('returns failed for failed orders', () => {
    assert.equal(derivePaymentStatus({ status: 'failed' }), 'failed');
  });

  it('returns pending for pending orders', () => {
    assert.equal(derivePaymentStatus({ status: 'pending' }), 'pending');
  });

  it('returns paid for processing orders', () => {
    assert.equal(derivePaymentStatus({ status: 'processing' }), 'paid');
  });

  it('returns paid for completed orders', () => {
    assert.equal(derivePaymentStatus({ status: 'completed' }), 'paid');
  });

  it('returns paid when set_paid is true', () => {
    assert.equal(derivePaymentStatus({ status: 'on-hold', set_paid: true }), 'paid');
  });

  it('returns pending for on-hold without set_paid', () => {
    assert.equal(derivePaymentStatus({ status: 'on-hold' }), 'pending');
  });
});

// ---------------------------------------------------------------------------
// mapCustomerToStateSet
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapCustomerToStateSet', () => {
  it('maps a full customer fixture', () => {
    const fixture = loadFixture('customer.json');
    const result = mapCustomerToStateSet(fixture);

    assert.equal(result.entityType, 'customers');
    assert.equal(result.externalId, '25');
    assert.equal(result.data.email, 'john.doe@example.com');
    assert.equal(result.data.firstName, 'John');
    assert.equal(result.data.lastName, 'Doe');
    assert.equal(result.data.phone, '(555) 555-5555');
    assert.equal(result.data.status, 'active');
    assert.equal(result.raw, fixture);
  });

  it('maps billing address', () => {
    const fixture = loadFixture('customer.json');
    const result = mapCustomerToStateSet(fixture);

    assert.equal(result.data.billingAddress.address1, '969 Market');
    assert.equal(result.data.billingAddress.city, 'San Francisco');
    assert.equal(result.data.billingAddress.province, 'CA');
    assert.equal(result.data.billingAddress.zip, '94103');
    assert.equal(result.data.billingAddress.country, 'US');
  });

  it('maps shipping address', () => {
    const fixture = loadFixture('customer.json');
    const result = mapCustomerToStateSet(fixture);

    assert.equal(result.data.shippingAddress.address1, '969 Market');
    assert.equal(result.data.shippingAddress.country, 'US');
  });

  it('includes metadata', () => {
    const fixture = loadFixture('customer.json');
    const result = mapCustomerToStateSet(fixture);

    assert.equal(result.data.metadata.woocommerceId, '25');
    assert.equal(result.data.metadata.woocommerceUsername, 'john.doe');
  });

  it('handles missing billing/shipping', () => {
    const result = mapCustomerToStateSet({ id: 1, email: 'a@b.com' });
    assert.equal(result.data.billingAddress, null);
    assert.equal(result.data.shippingAddress, null);
    assert.equal(result.data.phone, null);
  });

  it('handles missing email', () => {
    const result = mapCustomerToStateSet({ id: 2 });
    assert.equal(result.data.email, '');
  });
});

// ---------------------------------------------------------------------------
// mapProductToStateSet
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapProductToStateSet', () => {
  it('maps a full product fixture', () => {
    const fixture = loadFixture('product.json');
    const result = mapProductToStateSet(fixture);

    assert.equal(result.entityType, 'products');
    assert.equal(result.externalId, '93');
    assert.equal(result.data.name, 'Premium Widget');
    assert.equal(result.data.sku, 'WIDGET-001');
    assert.equal(result.data.price, 29.99);
    assert.equal(result.data.status, 'active');
  });

  it('strips HTML from description', () => {
    const fixture = loadFixture('product.json');
    const result = mapProductToStateSet(fixture);
    assert.ok(!result.data.description.includes('<p>'));
    assert.ok(result.data.description.includes('high-quality'));
  });

  it('maps images', () => {
    const fixture = loadFixture('product.json');
    const result = mapProductToStateSet(fixture);
    assert.equal(result.data.images.length, 2);
    assert.ok(result.data.images[0].src.includes('widget-front'));
  });

  it('maps categories', () => {
    const fixture = loadFixture('product.json');
    const result = mapProductToStateSet(fixture);
    assert.deepEqual(result.data.categories, ['Widgets', 'Premium']);
  });

  it('maps regular and sale prices', () => {
    const fixture = loadFixture('product.json');
    const result = mapProductToStateSet(fixture);
    assert.equal(result.data.regularPrice, 39.99);
    assert.equal(result.data.salePrice, 29.99);
  });

  it('handles product with no images/categories', () => {
    const result = mapProductToStateSet({ id: 1, name: 'Test', status: 'draft', price: '10' });
    assert.deepEqual(result.data.images, []);
    assert.deepEqual(result.data.categories, []);
    assert.equal(result.data.status, 'draft');
  });

  it('handles missing sku', () => {
    const result = mapProductToStateSet({ id: 5, name: 'No SKU', price: '5' });
    assert.equal(result.data.sku, '');
  });

  it('includes metadata', () => {
    const fixture = loadFixture('product.json');
    const result = mapProductToStateSet(fixture);
    assert.equal(result.data.metadata.woocommerceSlug, 'premium-widget');
    assert.equal(result.data.metadata.woocommerceType, 'simple');
  });
});

// ---------------------------------------------------------------------------
// mapOrderToStateSet
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapOrderToStateSet', () => {
  it('maps a full order fixture', () => {
    const fixture = loadFixture('order.json');
    const result = mapOrderToStateSet(fixture);

    assert.equal(result.entityType, 'orders');
    assert.equal(result.externalId, '727');
    assert.equal(result.data.totalAmount, 37.39);
    assert.equal(result.data.currency, 'USD');
    assert.equal(result.data.orderStatus, 'processing');
    assert.equal(result.data.paymentStatus, 'paid');
  });

  it('maps line items', () => {
    const fixture = loadFixture('order.json');
    const result = mapOrderToStateSet(fixture);

    assert.equal(result.data.items.length, 1);
    assert.equal(result.data.items[0].sku, 'WIDGET-001');
    assert.equal(result.data.items[0].name, 'Premium Widget');
    assert.equal(result.data.items[0].quantity, 1);
  });

  it('maps shipping address', () => {
    const fixture = loadFixture('order.json');
    const result = mapOrderToStateSet(fixture);

    assert.equal(result.data.shippingAddress.address1, '969 Market');
    assert.equal(result.data.shippingAddress.city, 'San Francisco');
    assert.equal(result.data.shippingAddress.province, 'CA');
  });

  it('maps billing address', () => {
    const fixture = loadFixture('order.json');
    const result = mapOrderToStateSet(fixture);

    assert.equal(result.data.billingAddress.address1, '969 Market');
    assert.equal(result.data.billingAddress.country, 'US');
  });

  it('resolves customer ID via idMap', () => {
    const fixture = loadFixture('order.json');
    const mockIdMap = {
      lookup: (platform, entityType, externalId) => {
        if (platform === 'woocommerce' && entityType === 'customers' && externalId === '25') {
          return { statesetId: 'ss-cust-1' };
        }
        return null;
      },
    };
    const result = mapOrderToStateSet(fixture, { idMap: mockIdMap, platform: 'woocommerce' });
    assert.equal(result.data.customerId, 'ss-cust-1');
  });

  it('sets customerId to null without idMap', () => {
    const fixture = loadFixture('order.json');
    const result = mapOrderToStateSet(fixture);
    assert.equal(result.data.customerId, null);
  });

  it('includes metadata', () => {
    const fixture = loadFixture('order.json');
    const result = mapOrderToStateSet(fixture);
    assert.equal(result.data.metadata.woocommerceOrderNumber, '727');
    assert.equal(result.data.metadata.woocommercePaymentMethod, 'stripe');
  });

  it('handles order with no line_items', () => {
    const result = mapOrderToStateSet({ id: 1, status: 'pending', total: '0' });
    assert.deepEqual(result.data.items, []);
  });

  it('handles missing shipping/billing', () => {
    const result = mapOrderToStateSet({ id: 2, status: 'pending', total: '10' });
    assert.equal(result.data.shippingAddress, null);
    assert.equal(result.data.billingAddress, null);
  });
});

// ---------------------------------------------------------------------------
// mapInventoryToStateSet
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapInventoryToStateSet', () => {
  it('maps inventory fixture', () => {
    const fixture = loadFixture('inventory-product.json');
    const result = mapInventoryToStateSet(fixture);

    assert.equal(result.entityType, 'inventory');
    assert.equal(result.externalId, '93');
    assert.equal(result.data.sku, 'WIDGET-001');
    assert.equal(result.data.quantity, 75);
    assert.equal(result.data.stockStatus, 'instock');
    assert.equal(result.data.manageStock, true);
  });

  it('generates fallback SKU when missing', () => {
    const result = mapInventoryToStateSet({ id: 42, stock_quantity: 10 });
    assert.equal(result.data.sku, 'WOO-INV-42');
  });

  it('defaults quantity to 0 when stock_quantity is null', () => {
    const result = mapInventoryToStateSet({ id: 1, sku: 'TEST', stock_quantity: null });
    assert.equal(result.data.quantity, 0);
  });

  it('defaults quantity to 0 when stock_quantity is undefined', () => {
    const result = mapInventoryToStateSet({ id: 1, sku: 'TEST' });
    assert.equal(result.data.quantity, 0);
  });

  it('includes metadata', () => {
    const fixture = loadFixture('inventory-product.json');
    const result = mapInventoryToStateSet(fixture);
    assert.equal(result.data.metadata.woocommerceProductId, '93');
  });
});

// ---------------------------------------------------------------------------
// Reverse mappers
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapCustomerFromStateSet', () => {
  it('maps StateSet customer back to WooCommerce format', () => {
    const statesetCustomer = {
      email: 'jane@example.com',
      firstName: 'Jane',
      lastName: 'Smith',
      phone: '555-1234',
      billingAddress: {
        address1: '123 Main St',
        city: 'Portland',
        province: 'OR',
        zip: '97201',
        country: 'US',
      },
    };
    const result = mapCustomerFromStateSet(statesetCustomer);
    assert.equal(result.email, 'jane@example.com');
    assert.equal(result.first_name, 'Jane');
    assert.equal(result.last_name, 'Smith');
    assert.equal(result.billing.address_1, '123 Main St');
    assert.equal(result.billing.city, 'Portland');
    assert.equal(result.billing.state, 'OR');
  });

  it('handles missing addresses', () => {
    const result = mapCustomerFromStateSet({ email: 'a@b.com' });
    assert.equal(result.billing.address_1, '');
    assert.equal(result.shipping.address_1, '');
  });
});

describe('woocommerce mapper — mapProductFromStateSet', () => {
  it('maps StateSet product back to WooCommerce format', () => {
    const statesetProduct = {
      name: 'Widget',
      description: 'A nice widget',
      sku: 'W-001',
      price: 19.99,
      regularPrice: 24.99,
      salePrice: 19.99,
      status: 'active',
      images: [{ src: 'https://example.com/img.jpg', alt: 'Widget' }],
    };
    const result = mapProductFromStateSet(statesetProduct);
    assert.equal(result.name, 'Widget');
    assert.equal(result.sku, 'W-001');
    assert.equal(result.regular_price, '24.99');
    assert.equal(result.sale_price, '19.99');
    assert.equal(result.status, 'publish');
    assert.equal(result.images.length, 1);
  });

  it('maps draft status', () => {
    const result = mapProductFromStateSet({ name: 'Test', status: 'draft' });
    assert.equal(result.status, 'draft');
  });

  it('uses price as regular_price when regularPrice is null', () => {
    const result = mapProductFromStateSet({ name: 'Test', price: 10 });
    assert.equal(result.regular_price, '10');
  });
});

// ---------------------------------------------------------------------------
// Dispatch functions
// ---------------------------------------------------------------------------

describe('woocommerce mapper — mapToStateSet dispatch', () => {
  it('dispatches customers', () => {
    const result = mapToStateSet('customers', { id: 1, email: 'a@b.com' });
    assert.equal(result.entityType, 'customers');
  });

  it('dispatches products', () => {
    const result = mapToStateSet('products', { id: 1, name: 'P', price: '10' });
    assert.equal(result.entityType, 'products');
  });

  it('dispatches orders', () => {
    const result = mapToStateSet('orders', { id: 1, status: 'pending', total: '10' });
    assert.equal(result.entityType, 'orders');
  });

  it('dispatches inventory', () => {
    const result = mapToStateSet('inventory', { id: 1, sku: 'X', stock_quantity: 5 });
    assert.equal(result.entityType, 'inventory');
  });

  it('throws on unknown entity type', () => {
    assert.throws(() => mapToStateSet('unknown', {}), /Unknown entity type/);
  });
});

describe('woocommerce mapper — mapFromStateSet dispatch', () => {
  it('dispatches customers', () => {
    const result = mapFromStateSet('customers', { email: 'a@b.com' });
    assert.ok(result.email);
  });

  it('dispatches products', () => {
    const result = mapFromStateSet('products', { name: 'Widget' });
    assert.ok(result.name);
  });

  it('returns record as-is for unsupported types', () => {
    const record = { foo: 'bar' };
    const result = mapFromStateSet('orders', record);
    assert.equal(result, record);
  });
});
