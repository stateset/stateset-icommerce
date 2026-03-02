import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { StripeAdapter } from '../../src/adapters/stripe/index.js';
import { listAdapters, getAdapter } from '../../src/adapters/index.js';

describe('stripe adapter', () => {
  describe('constructor', () => {
    it('creates with default config', () => {
      const adapter = new StripeAdapter();
      assert.equal(adapter.platformName, 'stripe');
    });

    it('accepts webhook secret', () => {
      const adapter = new StripeAdapter({ webhookSecret: 'whsec_test' });
      assert.equal(adapter.config.webhookSecret, 'whsec_test');
    });
  });

  describe('testConnection()', () => {
    it('returns true when webhook secret is set', async () => {
      const adapter = new StripeAdapter({ webhookSecret: 'whsec_test' });
      assert.equal(await adapter.testConnection(), true);
    });

    it('returns false when no webhook secret', async () => {
      const adapter = new StripeAdapter();
      assert.equal(await adapter.testConnection(), false);
    });
  });

  describe('getSupportedEntities()', () => {
    it('returns customers, payments, subscriptions, invoices', () => {
      const adapter = new StripeAdapter();
      const entities = adapter.getSupportedEntities();
      assert.deepEqual(entities, ['customers', 'payments', 'subscriptions', 'invoices']);
    });
  });

  describe('getSupportedWebhookTopics()', () => {
    it('returns 13 event types', () => {
      const adapter = new StripeAdapter();
      assert.equal(adapter.getSupportedWebhookTopics().length, 13);
    });
  });

  describe('handleWebhook()', () => {
    it('maps payment_intent.succeeded', () => {
      const adapter = new StripeAdapter();
      const result = adapter.handleWebhook('payment_intent.succeeded', {
        data: { object: { id: 'pi_1', amount: 5000, status: 'succeeded', created: 1709251200 } },
      });
      assert.equal(result.externalId, 'pi_1');
      assert.equal(result.data.amount, '50.00');
    });

    it('maps customer.created', () => {
      const adapter = new StripeAdapter();
      const result = adapter.handleWebhook('customer.created', {
        data: { object: { id: 'cus_1', name: 'Alice', email: 'alice@test.com', created: 1709251200 } },
      });
      assert.equal(result.externalId, 'cus_1');
    });

    it('returns null for unsupported event', () => {
      const adapter = new StripeAdapter();
      const result = adapter.handleWebhook('unknown.event', {});
      assert.equal(result, null);
    });
  });

  describe('mapFromStateSet()', () => {
    it('throws — reverse mapping not supported', () => {
      const adapter = new StripeAdapter();
      assert.throws(() => adapter.mapFromStateSet('customers', {}), /not support/);
    });
  });

  describe('fetchBatches()', () => {
    it('throws — webhook-first adapter', async () => {
      const adapter = new StripeAdapter();
      const gen = adapter.fetchBatches('customers');
      await assert.rejects(() => gen.next(), /webhook-first/);
    });
  });

  describe('verifyWebhookSignature()', () => {
    it('returns error when no secret configured', () => {
      const adapter = new StripeAdapter();
      const result = adapter.verifyWebhookSignature('body', 't=1,v1=sig');
      assert.equal(result.valid, false);
      assert.match(result.error, /secret/i);
    });
  });
});

describe('stripe adapter — registry', () => {
  it('is registered in the adapter registry', () => {
    assert.ok(listAdapters().includes('stripe'));
  });

  it('can be instantiated via getAdapter()', async () => {
    const adapter = await getAdapter('stripe', { webhookSecret: 'whsec_test' });
    assert.equal(adapter.platformName, 'stripe');
  });
});
