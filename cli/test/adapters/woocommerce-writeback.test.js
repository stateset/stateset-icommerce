import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { WooCommerceClient } from '../../src/adapters/woocommerce/client.js';

describe('WooCommerceClient write-back capabilities', () => {
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
        headers: new Map([['x-wp-totalpages', '1']]),
        json: async () => ({
          id: 42,
          status: 'processing',
          note: 'Test note',
          stock_quantity: 100,
        }),
        text: async () => '{}',
      };
    };

    client = new WooCommerceClient({
      siteUrl: 'https://mystore.example.com',
      consumerKey: 'ck_test',
      consumerSecret: 'cs_test',
      requestsPerSecond: 100, // High limit for tests
    });
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('post() sends authenticated POST request', async () => {
    const { data } = await client.post('/orders', { status: 'processing' });

    assert.equal(data.id, 42);
    assert.equal(fetchCalls.length, 1);
    assert.equal(fetchCalls[0].options.method, 'POST');
    assert.ok(fetchCalls[0].url.includes('/orders'));

    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.status, 'processing');
  });

  it('put() sends authenticated PUT request', async () => {
    const { data } = await client.put('/orders/42', { status: 'completed' });

    assert.equal(data.id, 42);
    assert.equal(fetchCalls[0].options.method, 'PUT');
    assert.ok(fetchCalls[0].url.includes('/orders/42'));
  });

  it('delete() sends authenticated DELETE request', async () => {
    const { data } = await client.delete('/orders/42', { force: true });

    assert.equal(fetchCalls[0].options.method, 'DELETE');
    assert.ok(fetchCalls[0].url.includes('/orders/42'));
    assert.ok(fetchCalls[0].url.includes('force=true'));
  });

  it('createOrder sends order data', async () => {
    const order = await client.createOrder({
      payment_method: 'bacs',
      billing: { first_name: 'Alice' },
      line_items: [{ product_id: 1, quantity: 2 }],
    });

    assert.equal(order.id, 42);
    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.payment_method, 'bacs');
  });

  it('updateOrder updates order fields', async () => {
    const order = await client.updateOrder(42, { status: 'completed' });

    assert.equal(order.id, 42);
    assert.ok(fetchCalls[0].url.includes('/orders/42'));
    assert.equal(fetchCalls[0].options.method, 'PUT');
  });

  it('addOrderNote creates a note', async () => {
    const note = await client.addOrderNote(42, 'Shipped via FedEx', true);

    assert.ok(fetchCalls[0].url.includes('/orders/42/notes'));
    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.note, 'Shipped via FedEx');
    assert.equal(body.customer_note, true);
  });

  it('updateProductStock updates stock quantity', async () => {
    const product = await client.updateProductStock(15, 100);

    assert.ok(fetchCalls[0].url.includes('/products/15'));
    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.stock_quantity, 100);
    assert.equal(body.manage_stock, true);
  });

  it('createRefund creates a refund', async () => {
    const refund = await client.createRefund(42, {
      amount: '25.00',
      reason: 'Customer request',
    });

    assert.ok(fetchCalls[0].url.includes('/orders/42/refunds'));
    const body = JSON.parse(fetchCalls[0].options.body);
    assert.equal(body.amount, '25.00');
  });

  it('handles API errors', async () => {
    globalThis.fetch = async () => ({
      ok: false,
      status: 404,
      statusText: 'Not Found',
      text: async () => 'Order not found',
    });

    await assert.rejects(() => client.updateOrder(999, { status: 'completed' }), /404 Not Found/);
  });

  it('includes Basic Auth header', async () => {
    await client.post('/orders', {});

    const authHeader = fetchCalls[0].options.headers?.Authorization;
    assert.ok(authHeader);
    assert.ok(authHeader.startsWith('Basic '));
  });
});
