import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

import {
  mapPaymentIntentToStateSet,
  mapChargeToStateSet,
  mapCustomerToStateSet,
  mapSubscriptionToStateSet,
  mapRefundToStateSet,
  mapInvoiceToStateSet,
  mapDisputeToStateSet,
  mapToStateSet,
  centsToDecimal,
  timestampToIso,
  PAYMENT_INTENT_STATUS_MAP,
  CHARGE_STATUS_MAP,
  REFUND_STATUS_MAP,
  SUBSCRIPTION_STATUS_MAP,
  INVOICE_STATUS_MAP,
  DISPUTE_STATUS_MAP,
} from '../../src/adapters/stripe/mapper.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures', 'stripe');

function loadFixture(name) {
  return JSON.parse(fs.readFileSync(path.join(fixturesDir, name), 'utf-8'));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

describe('stripe mapper — helpers', () => {
  describe('centsToDecimal()', () => {
    it('converts 5000 cents to "50.00"', () => {
      assert.equal(centsToDecimal(5000), '50.00');
    });

    it('converts 2999 cents to "29.99"', () => {
      assert.equal(centsToDecimal(2999), '29.99');
    });

    it('converts 0 to "0.00"', () => {
      assert.equal(centsToDecimal(0), '0.00');
    });

    it('handles null', () => {
      assert.equal(centsToDecimal(null), '0.00');
    });

    it('handles undefined', () => {
      assert.equal(centsToDecimal(undefined), '0.00');
    });

    it('handles non-number', () => {
      assert.equal(centsToDecimal('abc'), '0.00');
    });
  });

  describe('timestampToIso()', () => {
    it('converts Unix timestamp to ISO string', () => {
      const result = timestampToIso(1709251200);
      assert.ok(result.startsWith('2024-03-01'));
    });

    it('returns null for null', () => {
      assert.equal(timestampToIso(null), null);
    });

    it('returns null for 0', () => {
      assert.equal(timestampToIso(0), null);
    });

    it('returns null for non-number', () => {
      assert.equal(timestampToIso('abc'), null);
    });
  });
});

// ---------------------------------------------------------------------------
// Payment Intent
// ---------------------------------------------------------------------------

describe('stripe mapper — payment intent', () => {
  it('maps a succeeded payment intent', () => {
    const fixture = loadFixture('payment-intent-succeeded.json');
    const result = mapPaymentIntentToStateSet(fixture);

    assert.equal(result.externalId, 'pi_3Q1abc123def456');
    assert.equal(result.data.amount, '50.00');
    assert.equal(result.data.currency, 'USD');
    assert.equal(result.data.status, 'completed');
    assert.equal(result.data.method, 'card');
    assert.equal(result.data.customerId, 'cus_abc123');
    assert.equal(result.data.orderId, 'ORD-001');
  });

  it('maps all payment intent statuses', () => {
    for (const [stripeStatus, expected] of Object.entries(PAYMENT_INTENT_STATUS_MAP)) {
      const result = mapPaymentIntentToStateSet({
        id: 'pi_test',
        amount: 1000,
        status: stripeStatus,
        created: 1709251200,
      });
      assert.equal(result.data.status, expected, `${stripeStatus} → ${expected}`);
    }
  });

  it('defaults unknown status to pending', () => {
    const result = mapPaymentIntentToStateSet({
      id: 'pi_test',
      amount: 1000,
      status: 'unknown_status',
      created: 1709251200,
    });
    assert.equal(result.data.status, 'pending');
  });

  it('throws on missing id', () => {
    assert.throws(() => mapPaymentIntentToStateSet({}), /missing id/);
  });

  it('throws on null input', () => {
    assert.throws(() => mapPaymentIntentToStateSet(null));
  });

  it('preserves raw data', () => {
    const fixture = loadFixture('payment-intent-succeeded.json');
    const result = mapPaymentIntentToStateSet(fixture);
    assert.equal(result.raw.id, fixture.id);
  });
});

// ---------------------------------------------------------------------------
// Charge
// ---------------------------------------------------------------------------

describe('stripe mapper — charge', () => {
  it('maps a succeeded charge', () => {
    const fixture = loadFixture('charge-refunded.json');
    const result = mapChargeToStateSet(fixture);

    assert.equal(result.externalId, 'ch_abc123');
    assert.equal(result.data.amount, '50.00');
    assert.equal(result.data.status, 'completed');
    assert.equal(result.data.method, 'card');
    assert.equal(result.data.paymentIntentId, 'pi_3Q1abc123def456');
  });

  it('maps all charge statuses', () => {
    for (const [stripeStatus, expected] of Object.entries(CHARGE_STATUS_MAP)) {
      const result = mapChargeToStateSet({
        id: 'ch_test',
        amount: 1000,
        status: stripeStatus,
        created: 1709251200,
      });
      assert.equal(result.data.status, expected);
    }
  });

  it('throws on missing id', () => {
    assert.throws(() => mapChargeToStateSet({}), /missing id/);
  });
});

// ---------------------------------------------------------------------------
// Customer
// ---------------------------------------------------------------------------

describe('stripe mapper — customer', () => {
  it('maps a customer', () => {
    const fixture = loadFixture('customer-created.json');
    const result = mapCustomerToStateSet(fixture);

    assert.equal(result.externalId, 'cus_abc123');
    assert.equal(result.data.email, 'alice@example.com');
    assert.equal(result.data.firstName, 'Alice');
    assert.equal(result.data.lastName, 'Smith');
    assert.equal(result.data.phone, '+1234567890');
  });

  it('handles missing name', () => {
    const result = mapCustomerToStateSet({ id: 'cus_test', created: 1709251200 });
    assert.equal(result.data.firstName, '');
    assert.equal(result.data.lastName, '');
  });

  it('handles single-word name', () => {
    const result = mapCustomerToStateSet({ id: 'cus_test', name: 'Alice', created: 1709251200 });
    assert.equal(result.data.firstName, 'Alice');
    assert.equal(result.data.lastName, '');
  });

  it('handles multi-word last name', () => {
    const result = mapCustomerToStateSet({
      id: 'cus_test',
      name: 'Alice van der Berg',
      created: 1709251200,
    });
    assert.equal(result.data.firstName, 'Alice');
    assert.equal(result.data.lastName, 'van der Berg');
  });

  it('preserves metadata', () => {
    const result = mapCustomerToStateSet({
      id: 'cus_test',
      metadata: { source: 'web' },
      created: 1709251200,
    });
    assert.deepEqual(result.data.metadata, { source: 'web' });
  });

  it('throws on missing id', () => {
    assert.throws(() => mapCustomerToStateSet({}), /missing id/);
  });
});

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

describe('stripe mapper — subscription', () => {
  it('maps an active subscription', () => {
    const fixture = loadFixture('subscription-created.json');
    const result = mapSubscriptionToStateSet(fixture);

    assert.equal(result.externalId, 'sub_xyz789');
    assert.equal(result.data.status, 'active');
    assert.equal(result.data.customerId, 'cus_abc123');
    assert.equal(result.data.planId, 'price_monthly_basic');
    assert.equal(result.data.planName, 'Basic Monthly');
    assert.equal(result.data.amount, '29.99');
    assert.equal(result.data.interval, 'month');
    assert.equal(result.data.intervalCount, 1);
    assert.equal(result.data.cancelAtPeriodEnd, false);
  });

  it('maps all subscription statuses', () => {
    for (const [stripeStatus, expected] of Object.entries(SUBSCRIPTION_STATUS_MAP)) {
      const result = mapSubscriptionToStateSet({
        id: 'sub_test',
        status: stripeStatus,
        created: 1709251200,
      });
      assert.equal(result.data.status, expected, `${stripeStatus} → ${expected}`);
    }
  });

  it('handles missing items', () => {
    const result = mapSubscriptionToStateSet({ id: 'sub_test', created: 1709251200 });
    assert.equal(result.data.planId, null);
    assert.equal(result.data.amount, null);
  });

  it('throws on missing id', () => {
    assert.throws(() => mapSubscriptionToStateSet({}), /missing id/);
  });
});

// ---------------------------------------------------------------------------
// Refund
// ---------------------------------------------------------------------------

describe('stripe mapper — refund', () => {
  it('maps a refund', () => {
    const refund = loadFixture('charge-refunded.json').refunds.data[0];
    const result = mapRefundToStateSet(refund);

    assert.equal(result.externalId, 're_abc123');
    assert.equal(result.data.amount, '25.00');
    assert.equal(result.data.status, 'completed');
    assert.equal(result.data.reason, 'requested_by_customer');
    assert.equal(result.data.chargeId, 'ch_abc123');
  });

  it('maps all refund statuses', () => {
    for (const [stripeStatus, expected] of Object.entries(REFUND_STATUS_MAP)) {
      const result = mapRefundToStateSet({
        id: 're_test',
        amount: 1000,
        status: stripeStatus,
        created: 1709251200,
      });
      assert.equal(result.data.status, expected);
    }
  });

  it('throws on missing id', () => {
    assert.throws(() => mapRefundToStateSet({}), /missing id/);
  });
});

// ---------------------------------------------------------------------------
// Invoice
// ---------------------------------------------------------------------------

describe('stripe mapper — invoice', () => {
  it('maps a paid invoice', () => {
    const fixture = loadFixture('invoice-paid.json');
    const result = mapInvoiceToStateSet(fixture);

    assert.equal(result.externalId, 'in_abc123');
    assert.equal(result.data.amount, '29.99');
    assert.equal(result.data.amountPaid, '29.99');
    assert.equal(result.data.status, 'paid');
    assert.equal(result.data.customerId, 'cus_abc123');
    assert.equal(result.data.subscriptionId, 'sub_xyz789');
    assert.equal(result.data.number, 'INV-0001');
  });

  it('maps all invoice statuses', () => {
    for (const [stripeStatus, expected] of Object.entries(INVOICE_STATUS_MAP)) {
      const result = mapInvoiceToStateSet({
        id: 'in_test',
        amount_due: 1000,
        status: stripeStatus,
        created: 1709251200,
      });
      assert.equal(result.data.status, expected);
    }
  });

  it('throws on missing id', () => {
    assert.throws(() => mapInvoiceToStateSet({}), /missing id/);
  });
});

// ---------------------------------------------------------------------------
// Dispute
// ---------------------------------------------------------------------------

describe('stripe mapper — dispute', () => {
  it('maps a dispute', () => {
    const fixture = loadFixture('dispute-created.json');
    const result = mapDisputeToStateSet(fixture);

    assert.equal(result.externalId, 'dp_abc123');
    assert.equal(result.data.amount, '50.00');
    assert.equal(result.data.status, 'open');
    assert.equal(result.data.reason, 'fraudulent');
    assert.equal(result.data.chargeId, 'ch_abc123');
  });

  it('maps all dispute statuses', () => {
    for (const [stripeStatus, expected] of Object.entries(DISPUTE_STATUS_MAP)) {
      const result = mapDisputeToStateSet({
        id: 'dp_test',
        amount: 1000,
        status: stripeStatus,
        created: 1709251200,
      });
      assert.equal(result.data.status, expected);
    }
  });

  it('throws on missing id', () => {
    assert.throws(() => mapDisputeToStateSet({}), /missing id/);
  });
});

// ---------------------------------------------------------------------------
// Dispatch mapper
// ---------------------------------------------------------------------------

describe('stripe mapper — dispatch', () => {
  it('dispatches payment_intents', () => {
    const fixture = loadFixture('payment-intent-succeeded.json');
    const result = mapToStateSet('payment_intents', fixture);
    assert.equal(result.externalId, fixture.id);
  });

  it('dispatches customers', () => {
    const fixture = loadFixture('customer-created.json');
    const result = mapToStateSet('customers', fixture);
    assert.equal(result.externalId, fixture.id);
  });

  it('dispatches subscriptions', () => {
    const fixture = loadFixture('subscription-created.json');
    const result = mapToStateSet('subscriptions', fixture);
    assert.equal(result.externalId, fixture.id);
  });

  it('dispatches invoices', () => {
    const fixture = loadFixture('invoice-paid.json');
    const result = mapToStateSet('invoices', fixture);
    assert.equal(result.externalId, fixture.id);
  });

  it('throws on unsupported entity type', () => {
    assert.throws(() => mapToStateSet('unknown', {}), /Unsupported/);
  });
});
