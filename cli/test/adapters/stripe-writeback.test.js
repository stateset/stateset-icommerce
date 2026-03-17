import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { StripeAdapter } from '../../src/adapters/stripe/index.js';

describe('StripeAdapter write-back capabilities', () => {
  let adapter;
  let fetchCalls;
  let originalFetch;

  beforeEach(() => {
    fetchCalls = [];
    originalFetch = globalThis.fetch;

    // Mock fetch
    globalThis.fetch = async (url, options) => {
      fetchCalls.push({ url, options });

      // Mock successful Stripe responses
      const path = typeof url === 'string' ? url : url.toString();

      if (path.includes('/v1/balance')) {
        return { ok: true, json: async () => ({ available: [{ amount: 1000 }] }) };
      }
      if (path.includes('/v1/payment_intents') && options?.method === 'POST') {
        return {
          ok: true,
          json: async () => ({
            id: 'pi_test_123',
            status: 'requires_confirmation',
            amount: 5000,
            currency: 'usd',
          }),
        };
      }
      if (path.includes('/v1/refunds') && options?.method === 'POST') {
        return {
          ok: true,
          json: async () => ({
            id: 're_test_456',
            status: 'succeeded',
            amount: 2000,
          }),
        };
      }
      if (path.includes('/v1/customers') && options?.method === 'POST') {
        return {
          ok: true,
          json: async () => ({
            id: 'cus_test_789',
            email: 'test@example.com',
          }),
        };
      }

      return { ok: true, json: async () => ({}) };
    };

    adapter = new StripeAdapter({
      webhookSecret: 'whsec_test',
      apiKey: 'sk_test_key',
    });
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('createPaymentIntent sends correct params', async () => {
    const result = await adapter.createPaymentIntent({
      amount: 5000,
      currency: 'usd',
      description: 'Test payment',
    });

    assert.equal(result.id, 'pi_test_123');
    assert.equal(fetchCalls.length, 1);
    assert.equal(fetchCalls[0].options.method, 'POST');
    assert.ok(fetchCalls[0].url.includes('/v1/payment_intents'));

    // Verify URL-encoded body
    const body = fetchCalls[0].options.body;
    assert.ok(body.includes('amount=5000'));
    assert.ok(body.includes('currency=usd'));
  });

  it('createRefund sends correct params', async () => {
    const result = await adapter.createRefund({
      payment_intent: 'pi_test_123',
      amount: 2000,
      reason: 'requested_by_customer',
    });

    assert.equal(result.id, 're_test_456');
    assert.equal(fetchCalls.length, 1);
    assert.ok(fetchCalls[0].url.includes('/v1/refunds'));
  });

  it('createCustomer sends correct params', async () => {
    const result = await adapter.createCustomer({
      email: 'test@example.com',
      name: 'Test User',
    });

    assert.equal(result.id, 'cus_test_789');
    const body = fetchCalls[0].options.body;
    assert.ok(body.includes('email=test%40example.com'));
  });

  it('updateFulfillment updates payment intent metadata', async () => {
    await adapter.updateFulfillment('pi_test_123', {
      fulfillment_status: 'shipped',
      tracking_number: 'TRACK123',
    });

    assert.equal(fetchCalls.length, 1);
    assert.ok(fetchCalls[0].url.includes('/v1/payment_intents/pi_test_123'));
    const body = fetchCalls[0].options.body;
    assert.ok(body.includes('metadata'));
  });

  it('throws if no API key configured for write operations', async () => {
    const readOnlyAdapter = new StripeAdapter({ webhookSecret: 'whsec_test' });
    await assert.rejects(
      () => readOnlyAdapter.createPaymentIntent({ amount: 100 }),
      /API key is required/,
    );
  });

  it('mapFromStateSet converts payment_intent correctly', () => {
    const result = adapter.mapFromStateSet('payment_intent', {
      amount: 50.0,
      currency: 'USD',
      orderId: 'ord-123',
    });

    assert.equal(result.amount, 5000); // $50 → 5000 cents
    assert.equal(result.currency, 'usd');
    assert.equal(result.metadata.stateset_order_id, 'ord-123');
  });

  it('mapFromStateSet converts refund correctly', () => {
    const result = adapter.mapFromStateSet('refund', {
      stripePaymentIntentId: 'pi_123',
      amount: 25.0,
      returnId: 'ret-456',
    });

    assert.equal(result.payment_intent, 'pi_123');
    assert.equal(result.amount, 2500);
    assert.equal(result.metadata.stateset_return_id, 'ret-456');
  });

  it('mapFromStateSet converts customer correctly', () => {
    const result = adapter.mapFromStateSet('customer', {
      email: 'alice@example.com',
      firstName: 'Alice',
      lastName: 'Smith',
      id: 'cust-789',
    });

    assert.equal(result.email, 'alice@example.com');
    assert.equal(result.name, 'Alice Smith');
    assert.equal(result.metadata.stateset_customer_id, 'cust-789');
  });

  it('mapFromStateSet throws for unsupported entity type', () => {
    assert.throws(
      () => adapter.mapFromStateSet('unknown', {}),
      /Unsupported reverse mapping entity/,
    );
  });

  it('testConnection uses API key when available', async () => {
    const result = await adapter.testConnection();
    assert.equal(result, true);
    assert.ok(fetchCalls[0].url.includes('/v1/balance'));
  });

  it('handles Stripe API errors', async () => {
    globalThis.fetch = async () => ({
      ok: false,
      json: async () => ({ error: { message: 'Invalid API key' } }),
    });

    await assert.rejects(() => adapter.createPaymentIntent({ amount: 100 }), /Invalid API key/);
  });
});
