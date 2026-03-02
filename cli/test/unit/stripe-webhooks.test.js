import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createStripeWebhookHandlers, getSupportedStripeEvents } from '../../src/adapters/stripe/webhooks.js';

// ---------------------------------------------------------------------------
// Mock commerce and idMapStore
// ---------------------------------------------------------------------------

function createMockCommerce() {
  const records = { payments: [], customers: [], subscriptions: [], invoices: [] };
  return {
    payments: {
      create: async (data) => {
        records.payments.push(data);
        return { id: `pay_${records.payments.length}` };
      },
      refund: async (data) => {
        records.payments.push({ ...data, type: 'refund' });
        return { id: `ref_${records.payments.length}` };
      },
    },
    customers: {
      create: async (data) => {
        records.customers.push(data);
        return { id: `cust_${records.customers.length}` };
      },
    },
    subscriptions: {
      create: async (data) => {
        records.subscriptions.push(data);
        return { id: `sub_${records.subscriptions.length}` };
      },
      cancel: async (id) => {
        records.subscriptions.push({ id, cancelled: true });
      },
    },
    invoices: {
      create: async (data) => {
        records.invoices.push(data);
        return { id: `inv_${records.invoices.length}` };
      },
    },
    _records: records,
  };
}

function createMockIdMapStore() {
  const store = new Map();
  return {
    lookup: (platform, entityType, externalId) => {
      return store.get(`${platform}:${entityType}:${externalId}`) || null;
    },
    store: (platform, entityType, externalId, statesetId, raw) => {
      store.set(`${platform}:${entityType}:${externalId}`, { statesetId, raw });
    },
    _store: store,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('stripe webhooks — handlers', () => {
  let commerce;
  let idMapStore;
  let handlers;

  beforeEach(() => {
    commerce = createMockCommerce();
    idMapStore = createMockIdMapStore();
    handlers = createStripeWebhookHandlers(commerce, idMapStore);
  });

  describe('payment_intent.succeeded', () => {
    it('creates a payment record', async () => {
      const result = await handlers['payment_intent.succeeded']({
        data: { object: { id: 'pi_1', amount: 5000, currency: 'usd', status: 'succeeded', created: 1709251200 } },
      });
      assert.equal(result.action, 'created');
      assert.equal(result.externalId, 'pi_1');
      assert.equal(commerce._records.payments.length, 1);
    });

    it('skips duplicate payment', async () => {
      idMapStore.store('stripe', 'payments', 'pi_1', 'pay_existing', {});
      const result = await handlers['payment_intent.succeeded']({
        data: { object: { id: 'pi_1', amount: 5000, currency: 'usd', status: 'succeeded', created: 1709251200 } },
      });
      assert.equal(result.action, 'skipped');
    });

    it('handles missing payments handler', async () => {
      commerce.payments = null;
      const result = await handlers['payment_intent.succeeded']({
        data: { object: { id: 'pi_1', amount: 5000, status: 'succeeded', created: 1709251200 } },
      });
      assert.equal(result.action, 'skipped');
      assert.match(result.reason, /handler/i);
    });
  });

  describe('payment_intent.payment_failed', () => {
    it('updates existing payment on failure', async () => {
      idMapStore.store('stripe', 'payments', 'pi_1', 'pay_1', {});
      const result = await handlers['payment_intent.payment_failed']({
        data: { object: { id: 'pi_1', amount: 5000, status: 'requires_payment_method', created: 1709251200 } },
      });
      assert.equal(result.action, 'updated');
    });

    it('records unknown failed payment', async () => {
      const result = await handlers['payment_intent.payment_failed']({
        data: { object: { id: 'pi_new', amount: 5000, status: 'requires_payment_method', created: 1709251200 } },
      });
      assert.equal(result.action, 'recorded');
    });
  });

  describe('payment_intent.canceled', () => {
    it('cancels existing payment', async () => {
      idMapStore.store('stripe', 'payments', 'pi_1', 'pay_1', {});
      const result = await handlers['payment_intent.canceled']({
        data: { object: { id: 'pi_1', amount: 5000, status: 'canceled', created: 1709251200 } },
      });
      assert.equal(result.action, 'cancelled');
    });

    it('skips unknown canceled payment', async () => {
      const result = await handlers['payment_intent.canceled']({
        data: { object: { id: 'pi_unknown', amount: 5000, status: 'canceled', created: 1709251200 } },
      });
      assert.equal(result.action, 'skipped');
    });
  });

  describe('charge.succeeded', () => {
    it('creates a charge record', async () => {
      const result = await handlers['charge.succeeded']({
        data: { object: { id: 'ch_1', amount: 3000, currency: 'usd', status: 'succeeded', created: 1709251200 } },
      });
      assert.equal(result.action, 'created');
    });
  });

  describe('charge.refunded', () => {
    it('creates a refund record', async () => {
      const result = await handlers['charge.refunded']({
        data: {
          object: {
            id: 'ch_1',
            refunds: {
              data: [{ id: 're_1', amount: 1500, currency: 'usd', status: 'succeeded', created: 1709251200 }],
            },
          },
        },
      });
      assert.equal(result.action, 'created');
      assert.equal(result.externalId, 're_1');
    });

    it('skips if no refund data', async () => {
      const result = await handlers['charge.refunded']({
        data: { object: { id: 'ch_1', refunds: { data: [] } } },
      });
      assert.equal(result.action, 'skipped');
    });
  });

  describe('charge.dispute.created', () => {
    it('records a dispute', async () => {
      const result = await handlers['charge.dispute.created']({
        data: {
          object: {
            id: 'dp_1',
            amount: 5000,
            currency: 'usd',
            status: 'needs_response',
            reason: 'fraudulent',
            created: 1709251200,
          },
        },
      });
      assert.equal(result.action, 'recorded');
      assert.equal(result.externalId, 'dp_1');
    });
  });

  describe('customer.created', () => {
    it('creates a customer', async () => {
      const result = await handlers['customer.created']({
        data: { object: { id: 'cus_1', name: 'Bob', email: 'bob@example.com', created: 1709251200 } },
      });
      assert.equal(result.action, 'created');
      assert.equal(commerce._records.customers.length, 1);
    });

    it('skips duplicate customer', async () => {
      idMapStore.store('stripe', 'customers', 'cus_1', 'cust_existing', {});
      const result = await handlers['customer.created']({
        data: { object: { id: 'cus_1', name: 'Bob', created: 1709251200 } },
      });
      assert.equal(result.action, 'skipped');
    });
  });

  describe('customer.updated', () => {
    it('updates existing customer', async () => {
      idMapStore.store('stripe', 'customers', 'cus_1', 'cust_1', {});
      const result = await handlers['customer.updated']({
        data: { object: { id: 'cus_1', name: 'Bob Updated', created: 1709251200 } },
      });
      assert.equal(result.action, 'updated');
    });

    it('creates customer if not exists', async () => {
      const result = await handlers['customer.updated']({
        data: { object: { id: 'cus_new', name: 'New Customer', created: 1709251200 } },
      });
      assert.equal(result.action, 'created');
    });
  });

  describe('customer.subscription.created', () => {
    it('creates a subscription', async () => {
      const result = await handlers['customer.subscription.created']({
        data: { object: { id: 'sub_1', status: 'active', customer: 'cus_1', created: 1709251200 } },
      });
      assert.equal(result.action, 'created');
    });
  });

  describe('customer.subscription.updated', () => {
    it('updates existing subscription', async () => {
      idMapStore.store('stripe', 'subscriptions', 'sub_1', 'sub_internal_1', {});
      const result = await handlers['customer.subscription.updated']({
        data: { object: { id: 'sub_1', status: 'past_due', created: 1709251200 } },
      });
      assert.equal(result.action, 'updated');
    });
  });

  describe('customer.subscription.deleted', () => {
    it('cancels existing subscription', async () => {
      idMapStore.store('stripe', 'subscriptions', 'sub_1', 'sub_internal_1', {});
      const result = await handlers['customer.subscription.deleted']({
        data: { object: { id: 'sub_1', status: 'canceled', created: 1709251200 } },
      });
      assert.equal(result.action, 'cancelled');
    });

    it('skips unknown subscription', async () => {
      const result = await handlers['customer.subscription.deleted']({
        data: { object: { id: 'sub_unknown', status: 'canceled', created: 1709251200 } },
      });
      assert.equal(result.action, 'skipped');
    });
  });

  describe('invoice.paid', () => {
    it('creates an invoice', async () => {
      const result = await handlers['invoice.paid']({
        data: {
          object: {
            id: 'in_1',
            amount_due: 2999,
            amount_paid: 2999,
            currency: 'usd',
            status: 'paid',
            created: 1709251200,
          },
        },
      });
      assert.equal(result.action, 'created');
    });

    it('updates existing invoice', async () => {
      idMapStore.store('stripe', 'invoices', 'in_1', 'inv_internal_1', {});
      const result = await handlers['invoice.paid']({
        data: {
          object: { id: 'in_1', amount_due: 2999, status: 'paid', created: 1709251200 },
        },
      });
      assert.equal(result.action, 'updated');
    });
  });

  describe('invoice.payment_failed', () => {
    it('updates existing invoice on failure', async () => {
      idMapStore.store('stripe', 'invoices', 'in_1', 'inv_internal_1', {});
      const result = await handlers['invoice.payment_failed']({
        data: {
          object: { id: 'in_1', amount_due: 2999, status: 'open', created: 1709251200 },
        },
      });
      assert.equal(result.action, 'updated');
    });

    it('records unknown failed invoice', async () => {
      const result = await handlers['invoice.payment_failed']({
        data: {
          object: { id: 'in_new', amount_due: 2999, status: 'open', created: 1709251200 },
        },
      });
      assert.equal(result.action, 'recorded');
    });
  });
});

// ---------------------------------------------------------------------------
// getSupportedStripeEvents
// ---------------------------------------------------------------------------

describe('stripe webhooks — getSupportedStripeEvents', () => {
  it('returns 13 events', () => {
    assert.equal(getSupportedStripeEvents().length, 13);
  });

  it('includes payment_intent.succeeded', () => {
    assert.ok(getSupportedStripeEvents().includes('payment_intent.succeeded'));
  });

  it('includes customer.subscription.deleted', () => {
    assert.ok(getSupportedStripeEvents().includes('customer.subscription.deleted'));
  });

  it('includes invoice.payment_failed', () => {
    assert.ok(getSupportedStripeEvents().includes('invoice.payment_failed'));
  });
});
