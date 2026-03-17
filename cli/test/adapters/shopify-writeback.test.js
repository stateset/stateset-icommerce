import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ShopifyClient } from '../../src/adapters/shopify/client.js';

describe('ShopifyClient write-back capabilities', () => {
  let client;
  let fetchCalls;
  let originalFetch;

  beforeEach(() => {
    fetchCalls = [];
    originalFetch = globalThis.fetch;

    globalThis.fetch = async (url, options) => {
      fetchCalls.push({ url: url.toString(), options });

      return {
        ok: true,
        headers: new Map([['link', '']]),
        json: async () => ({
          order: { id: 12345, status: 'open' },
          fulfillment: { id: 67890, status: 'success' },
          inventory_level: { available: 50 },
          refund: { id: 99999 },
        }),
      };
    };

    client = new ShopifyClient({
      shopDomain: 'testshop.myshopify.com',
      accessToken: 'shpat_test_token',
      requestsPerSecond: 100,
    });
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('post() sends authenticated POST request', async () => {
    const { data } = await client.post('/orders.json', { order: { email: 'a@b.com' } });

    assert.ok(data.order);
    assert.equal(fetchCalls.length, 1);
    assert.equal(fetchCalls[0].options.method, 'POST');
    assert.ok(fetchCalls[0].url.includes('/orders.json'));

    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.order.email, 'a@b.com');
  });

  it('put() sends authenticated PUT request', async () => {
    const { data } = await client.put('/orders/12345.json', { order: { note: 'Updated' } });

    assert.equal(fetchCalls[0].options.method, 'PUT');
    assert.ok(fetchCalls[0].url.includes('/orders/12345.json'));
  });

  it('createOrder creates a Shopify order', async () => {
    const order = await client.createOrder({
      email: 'buyer@example.com',
      line_items: [{ variant_id: 123, quantity: 1 }],
    });

    assert.equal(order.id, 12345);
    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.order.email, 'buyer@example.com');
  });

  it('updateOrder updates order fields', async () => {
    const order = await client.updateOrder(12345, { note: 'VIP customer' });

    assert.equal(order.id, 12345);
    assert.ok(fetchCalls[0].url.includes('/orders/12345.json'));
    assert.equal(fetchCalls[0].options.method, 'PUT');
  });

  it('createFulfillment creates fulfillment with tracking', async () => {
    const fulfillment = await client.createFulfillment(12345, {
      tracking_number: 'TRACK123',
      tracking_company: 'FedEx',
    });

    assert.equal(fulfillment.id, 67890);
    assert.ok(fetchCalls[0].url.includes('/orders/12345/fulfillments.json'));

    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.fulfillment.tracking_number, 'TRACK123');
    assert.equal(body.fulfillment.notify_customer, true);
  });

  it('adjustInventory sends inventory adjustment', async () => {
    const level = await client.adjustInventory('inv_item_1', 'loc_1', -5);

    assert.equal(level.available, 50);
    assert.ok(fetchCalls[0].url.includes('/inventory_levels/adjust.json'));

    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.inventory_item_id, 'inv_item_1');
    assert.equal(body.location_id, 'loc_1');
    assert.equal(body.available_adjustment, -5);
  });

  it('createRefund creates a refund', async () => {
    const refund = await client.createRefund(12345, {
      shipping: { full_refund: true },
    });

    assert.equal(refund.id, 99999);
    assert.ok(fetchCalls[0].url.includes('/orders/12345/refunds.json'));
  });

  it('includes Shopify access token header', async () => {
    await client.post('/orders.json', {});

    const headers = fetchCalls[0].options.headers;
    assert.equal(headers['X-Shopify-Access-Token'], 'shpat_test_token');
  });

  it('handles Shopify API errors', async () => {
    globalThis.fetch = async () => ({
      ok: false,
      status: 422,
      statusText: 'Unprocessable Entity',
      text: async () => '{"errors": {"order": "is invalid"}}',
    });

    await assert.rejects(
      () => client.createOrder({ email: 'bad' }),
      /422 Unprocessable Entity/,
    );
  });
});
