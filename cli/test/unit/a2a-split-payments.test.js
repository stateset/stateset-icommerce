import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createSplitPaymentService } from '../../src/a2a/splits.js';

/**
 * Helper: build a mock store with all methods required by the split payment service.
 * Each method is a mock.fn returning a sensible default.
 * Callers can override individual methods after creation.
 */
function makeStore() {
  /** @type {Map<string, object>} */
  const payments = new Map();
  /** @type {Map<string, object>} */
  const recipients = new Map();

  return {
    createSplitPayment: mock.fn(async (record) => {
      payments.set(record.id, { ...record, recipients: [] });
    }),
    getSplitPayment: mock.fn(async (id) => {
      const p = payments.get(id);
      if (!p) return null;
      // Attach recipients that belong to this payment
      const recs = [];
      for (const r of recipients.values()) {
        if (r.split_payment_id === id) recs.push({ ...r });
      }
      return { ...p, recipients: recs };
    }),
    updateSplitPayment: mock.fn(async (id, updates) => {
      const p = payments.get(id);
      if (p) {
        Object.assign(p, updates);
      }
    }),
    listSplitPayments: mock.fn(async () => []),
    createSplitRecipient: mock.fn(async (record) => {
      recipients.set(record.id, { ...record });
    }),
    getSplitRecipient: mock.fn(async (id) => {
      return recipients.get(id) || null;
    }),
    updateSplitRecipient: mock.fn(async (id, updates) => {
      const r = recipients.get(id);
      if (r) {
        Object.assign(r, updates);
      }
    }),
    listSplitRecipients: mock.fn(async () => []),
    // expose internals for assertions
    _payments: payments,
    _recipients: recipients,
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// createSplitPayment
// ─────────────────────────────────────────────────────────────────────────────

describe('createSplitPayment', () => {
  let store;
  let service;

  beforeEach(() => {
    store = makeStore();
    service = createSplitPaymentService(store);
  });

  // ── Happy paths ──

  it('should create a percentage split with two recipients', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      recipients: [
        { address: '0xAlice', percent: 60 },
        { address: '0xBob', percent: 40 },
      ],
    });

    assert.equal(result.success, true);
    assert.ok(result.splitPayment);
    assert.equal(result.splitPayment.senderAddress, '0xSender');
    assert.equal(result.splitPayment.totalAmountDecimal, 100);
    assert.equal(result.splitPayment.totalAmount, 100_000_000);
    assert.equal(result.splitPayment.splitType, 'percentage');
    assert.equal(result.splitPayment.asset, 'USDC');
    assert.equal(result.splitPayment.network, 'set_chain');
    assert.equal(result.splitPayment.status, 'pending');
    assert.equal(result.splitPayment.recipients.length, 2);

    // Verify amounts (60% and 40% of 100 USDC)
    const alice = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xAlice',
    );
    const bob = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xBob',
    );
    assert.equal(alice.shareAmount, 60_000_000);
    assert.equal(alice.shareAmountDecimal, 60);
    assert.equal(alice.sharePercent, 60);
    assert.equal(bob.shareAmount, 40_000_000);
    assert.equal(bob.shareAmountDecimal, 40);
    assert.equal(bob.sharePercent, 40);
  });

  it('should create a percentage split with three recipients and handle remainder correctly', async () => {
    // 33.33 + 33.33 + 33.34 = 100
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      recipients: [
        { address: '0xAlice', percent: 33.33 },
        { address: '0xBob', percent: 33.33 },
        { address: '0xCharlie', percent: 33.34 },
      ],
    });

    assert.equal(result.success, true);
    const recs = result.splitPayment.recipients;
    assert.equal(recs.length, 3);

    // The last recipient (Charlie) gets the remainder to avoid rounding drift
    const totalShares = recs.reduce((s, r) => s + r.shareAmount, 0);
    assert.equal(totalShares, 100_000_000);
  });

  it('should create a fixed split with two recipients', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      splitType: 'fixed',
      recipients: [
        { address: '0xAlice', amount: 60 },
        { address: '0xBob', amount: 40 },
      ],
    });

    assert.equal(result.success, true);
    assert.equal(result.splitPayment.splitType, 'fixed');
    assert.equal(result.splitPayment.recipients.length, 2);

    const alice = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xAlice',
    );
    const bob = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xBob',
    );
    assert.equal(alice.shareAmount, 60_000_000);
    assert.equal(bob.shareAmount, 40_000_000);
    // Fixed splits should not have percent
    assert.equal(alice.sharePercent, null);
    assert.equal(bob.sharePercent, null);
  });

  it('should deduct platform fee before splitting (percentage)', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      platformFeePercent: 10,
      platformFeeAddress: '0xPlatform',
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    assert.equal(result.success, true);
    assert.equal(result.splitPayment.platformFeePercent, 10);
    // Platform fee: 10% of 100 = 10 USDC
    assert.equal(result.splitPayment.platformFeeAmount, 10_000_000);
    assert.equal(result.splitPayment.platformFeeAddress, '0xPlatform');

    // Remaining: 90 USDC split 50/50 = 45 each
    // There should be 3 recipients (Alice, Bob, Platform)
    assert.equal(result.splitPayment.recipients.length, 3);

    const alice = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xAlice',
    );
    const bob = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xBob',
    );
    assert.equal(alice.shareAmount, 45_000_000);
    assert.equal(bob.shareAmount, 45_000_000);
  });

  it('should deduct platform fee before splitting (fixed)', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      splitType: 'fixed',
      platformFeePercent: 5,
      platformFeeAddress: '0xPlatform',
      recipients: [
        { address: '0xAlice', amount: 55 },
        { address: '0xBob', amount: 40 },
      ],
    });

    assert.equal(result.success, true);
    // Platform fee: 5% of 100 = 5 USDC; remaining = 95
    // 55 + 40 = 95 -- matches
    const alice = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xAlice',
    );
    const bob = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xBob',
    );
    assert.equal(alice.shareAmount, 55_000_000);
    assert.equal(bob.shareAmount, 40_000_000);
  });

  it('should include optional metadata, memo, reference fields', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 50,
      memo: 'Split for order 42',
      referenceType: 'order',
      referenceId: 'ORD-42',
      metadata: { source: 'test' },
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    assert.equal(result.splitPayment.memo, 'Split for order 42');
    assert.equal(result.splitPayment.referenceType, 'order');
    assert.equal(result.splitPayment.referenceId, 'ORD-42');
  });

  it('should normalize asset to uppercase', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 20,
      asset: 'usdc',
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    assert.equal(result.splitPayment.asset, 'USDC');
  });

  it('should derive ZEC as the default asset for zcash splits', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 2,
      network: 'zcash',
      recipients: [
        { address: 'u1alice', percent: 50 },
        { address: 'u1bob', percent: 50 },
      ],
    });

    assert.equal(result.splitPayment.asset, 'ZEC');
    assert.equal(result.splitPayment.network, 'zcash');
    assert.equal(result.splitPayment.totalAmount, 200_000_000);
    assert.equal(result.splitPayment.recipients[0].shareAmountDecimal, 1);
  });

  it('should call store.createSplitPayment and store.createSplitRecipient correctly', async () => {
    await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 10,
      recipients: [
        { address: '0xAlice', percent: 70 },
        { address: '0xBob', percent: 30 },
      ],
    });

    // 1 parent record
    assert.equal(store.createSplitPayment.mock.calls.length, 1);
    const parentArg = store.createSplitPayment.mock.calls[0].arguments[0];
    assert.equal(parentArg.sender_address, '0xSender');
    assert.equal(parentArg.status, 'pending');

    // 2 recipient records
    assert.equal(store.createSplitRecipient.mock.calls.length, 2);
  });

  it('should not create platform fee recipient when platformFeePercent is 0', async () => {
    await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      platformFeePercent: 0,
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    // Only 2 recipients, no platform fee recipient
    assert.equal(store.createSplitRecipient.mock.calls.length, 2);
  });

  it('should not create platform fee recipient when platformFeeAddress is missing', async () => {
    await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      platformFeePercent: 5,
      // platformFeeAddress omitted
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    // Only 2 recipients, no platform fee recipient
    assert.equal(store.createSplitRecipient.mock.calls.length, 2);
  });

  // ── Validation errors ──

  it('should reject when senderAddress is missing', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          totalAmount: 100,
          recipients: [
            { address: '0xAlice', percent: 50 },
            { address: '0xBob', percent: 50 },
          ],
        }),
      { message: 'senderAddress is required' },
    );
  });

  it('should reject when totalAmount is missing', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          recipients: [
            { address: '0xAlice', percent: 50 },
            { address: '0xBob', percent: 50 },
          ],
        }),
      { message: 'totalAmount is required' },
    );
  });

  it('should reject when totalAmount is zero', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 0,
          recipients: [
            { address: '0xAlice', percent: 50 },
            { address: '0xBob', percent: 50 },
          ],
        }),
      { message: 'totalAmount must be greater than 0' },
    );
  });

  it('should reject when totalAmount is negative', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: -10,
          recipients: [
            { address: '0xAlice', percent: 50 },
            { address: '0xBob', percent: 50 },
          ],
        }),
      { message: 'totalAmount must be greater than 0' },
    );
  });

  it('should reject when recipients has fewer than 2 entries', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          recipients: [{ address: '0xAlice', percent: 100 }],
        }),
      { message: 'recipients must be an array with at least 2 entries' },
    );
  });

  it('should reject when recipients is empty', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          recipients: [],
        }),
      { message: 'recipients must be an array with at least 2 entries' },
    );
  });

  it('should reject when recipients is not an array', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          recipients: 'not-an-array',
        }),
      { message: 'recipients must be an array with at least 2 entries' },
    );
  });

  it('should reject when splitType is invalid', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'proportional',
          recipients: [
            { address: '0xAlice', percent: 50 },
            { address: '0xBob', percent: 50 },
          ],
        }),
      { message: 'splitType must be one of: percentage, fixed' },
    );
  });

  it('should reject when percentage recipients do not sum to 100', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'percentage',
          recipients: [
            { address: '0xAlice', percent: 40 },
            { address: '0xBob', percent: 40 },
          ],
        }),
      (err) => {
        assert.ok(err.message.includes('must sum to 100'));
        return true;
      },
    );
  });

  it('should reject when percentage recipients sum to more than 100', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'percentage',
          recipients: [
            { address: '0xAlice', percent: 60 },
            { address: '0xBob', percent: 50 },
          ],
        }),
      (err) => {
        assert.ok(err.message.includes('must sum to 100'));
        return true;
      },
    );
  });

  it('should reject when fixed amounts do not sum to total minus platform fee', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'fixed',
          recipients: [
            { address: '0xAlice', amount: 30 },
            { address: '0xBob', amount: 30 },
          ],
        }),
      (err) => {
        assert.ok(err.message.includes('must sum to'));
        return true;
      },
    );
  });

  it('should reject when fixed amounts exceed total minus platform fee', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'fixed',
          platformFeePercent: 10,
          platformFeeAddress: '0xPlatform',
          recipients: [
            { address: '0xAlice', amount: 60 },
            { address: '0xBob', amount: 40 },
          ],
        }),
      (err) => {
        // Remaining after 10% fee is 90, but 60+40=100
        assert.ok(err.message.includes('must sum to'));
        return true;
      },
    );
  });

  it('should reject when a recipient address is missing', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          recipients: [
            { address: '0xAlice', percent: 50 },
            { percent: 50 },
          ],
        }),
      { message: 'recipients[1].address is required' },
    );
  });

  it('should reject when a percentage recipient is missing percent', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'percentage',
          recipients: [
            { address: '0xAlice', percent: 50 },
            { address: '0xBob' },
          ],
        }),
      { message: 'recipients[1].percent is required for percentage splits' },
    );
  });

  it('should reject when a fixed recipient is missing amount', async () => {
    await assert.rejects(
      () =>
        service.createSplitPayment({
          senderAddress: '0xSender',
          totalAmount: 100,
          splitType: 'fixed',
          recipients: [
            { address: '0xAlice', amount: 50 },
            { address: '0xBob' },
          ],
        }),
      { message: 'recipients[1].amount is required for fixed splits' },
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// executeSplitPayment
// ─────────────────────────────────────────────────────────────────────────────

describe('executeSplitPayment', () => {
  let store;
  let service;

  beforeEach(() => {
    store = makeStore();
    service = createSplitPaymentService(store);
  });

  /**
   * Helper: create a split payment and return its ID for execution tests.
   */
  async function createTestSplit(overrides = {}) {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 100,
      recipients: [
        { address: '0xAlice', percent: 60 },
        { address: '0xBob', percent: 40 },
      ],
      memo: 'test memo',
      ...overrides,
    });
    return result.splitPayment.id;
  }

  it('should execute all payments successfully and set status to completed', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => ({ id: 'pay-123' }));

    const result = await service.executeSplitPayment(splitId, payFn);

    assert.equal(result.success, true);
    assert.equal(result.splitPayment.status, 'completed');
    assert.ok(result.splitPayment.completedAt);

    // payFn called once per recipient (2 recipients, no platform fee)
    assert.equal(payFn.mock.calls.length, 2);

    // Verify payFn called with correct arguments
    const firstCall = payFn.mock.calls[0].arguments;
    assert.equal(typeof firstCall[0], 'string'); // recipient address
    assert.equal(typeof firstCall[1], 'number'); // decimal amount
    assert.equal(firstCall[2], 'USDC'); // asset
    assert.equal(firstCall[3], 'set_chain'); // network
    assert.equal(firstCall[4], 'test memo'); // memo
  });

  it('passes the split network through to payFn', async () => {
    const splitId = await createTestSplit({
      asset: 'BTC',
      network: 'bitcoin',
      recipients: [
        { address: 'bc1qalice', percent: 60 },
        { address: 'bc1qbob', percent: 40 },
      ],
    });

    const payFn = mock.fn(async () => ({ id: 'pay-btc' }));
    await service.executeSplitPayment(splitId, payFn);

    const firstCall = payFn.mock.calls[0].arguments;
    assert.equal(firstCall[2], 'BTC');
    assert.equal(firstCall[3], 'bitcoin');
    assert.equal(firstCall[4], 'test memo');
  });

  it('should set status to failed when all payments fail', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => {
      throw new Error('Payment network down');
    });

    const result = await service.executeSplitPayment(splitId, payFn);

    assert.equal(result.success, false);
    assert.equal(result.splitPayment.status, 'failed');
    // completedAt is not set for failed splits (undefined maps to undefined via formatSplitPayment)
    assert.ok(!result.splitPayment.completedAt);
    assert.equal(payFn.mock.calls.length, 2);
  });

  it('should set status to partial when some payments fail', async () => {
    const splitId = await createTestSplit();

    let callCount = 0;
    const payFn = mock.fn(async () => {
      callCount++;
      if (callCount === 1) {
        return { id: 'pay-ok' };
      }
      throw new Error('Second payment failed');
    });

    const result = await service.executeSplitPayment(splitId, payFn);

    assert.equal(result.success, false);
    assert.equal(result.splitPayment.status, 'partial');
    // completedAt is not set for partial splits
    assert.ok(!result.splitPayment.completedAt);
    assert.equal(payFn.mock.calls.length, 2);
  });

  it('should throw when split payment is not found', async () => {
    const payFn = mock.fn(async () => ({ id: 'pay-123' }));

    await assert.rejects(
      () => service.executeSplitPayment('nonexistent-id', payFn),
      { message: 'Split payment not found' },
    );

    assert.equal(payFn.mock.calls.length, 0);
  });

  it('should throw when split payment is not in pending status', async () => {
    const splitId = await createTestSplit();

    // Execute once to move to completed
    const payFn = mock.fn(async () => ({ id: 'pay-123' }));
    await service.executeSplitPayment(splitId, payFn);

    // Try executing again — should fail since status is now completed
    await assert.rejects(
      () => service.executeSplitPayment(splitId, payFn),
      (err) => {
        assert.ok(err.message.includes('Cannot execute split payment in status'));
        assert.ok(err.message.includes('Expected: pending'));
        return true;
      },
    );
  });

  it('should update status to processing before executing payments', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => ({ id: 'pay-123' }));
    await service.executeSplitPayment(splitId, payFn);

    // Find the updateSplitPayment call that set status to 'processing'
    const processingCall = store.updateSplitPayment.mock.calls.find(
      (c) => c.arguments[1].status === 'processing',
    );
    assert.ok(processingCall, 'should have set status to processing');
    assert.equal(processingCall.arguments[0], splitId);
  });

  it('should update each recipient status to completed on success', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => ({ id: 'pay-xyz' }));
    await service.executeSplitPayment(splitId, payFn);

    // Check updateSplitRecipient calls
    const recipientUpdates = store.updateSplitRecipient.mock.calls;
    assert.equal(recipientUpdates.length, 2);

    for (const call of recipientUpdates) {
      assert.equal(call.arguments[1].status, 'completed');
      assert.equal(call.arguments[1].payment_id, 'pay-xyz');
    }
  });

  it('should update each recipient status to failed on error', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => {
      throw new Error('boom');
    });
    await service.executeSplitPayment(splitId, payFn);

    const recipientUpdates = store.updateSplitRecipient.mock.calls;
    assert.equal(recipientUpdates.length, 2);

    for (const call of recipientUpdates) {
      assert.equal(call.arguments[1].status, 'failed');
    }
  });

  it('should extract paymentId from result when result.id is missing', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => ({ paymentId: 'alt-pay-id' }));
    await service.executeSplitPayment(splitId, payFn);

    const recipientUpdates = store.updateSplitRecipient.mock.calls;
    for (const call of recipientUpdates) {
      assert.equal(call.arguments[1].payment_id, 'alt-pay-id');
    }
  });

  it('should handle null payment result gracefully', async () => {
    const splitId = await createTestSplit();

    const payFn = mock.fn(async () => null);
    const result = await service.executeSplitPayment(splitId, payFn);

    assert.equal(result.success, true);
    assert.equal(result.splitPayment.status, 'completed');

    // payment_id should be null
    const recipientUpdates = store.updateSplitRecipient.mock.calls;
    for (const call of recipientUpdates) {
      assert.equal(call.arguments[1].payment_id, null);
    }
  });

  it('should execute payments for split with platform fee recipient', async () => {
    const splitId = await createTestSplit({
      platformFeePercent: 10,
      platformFeeAddress: '0xPlatform',
    });

    const payFn = mock.fn(async () => ({ id: 'pay-ok' }));
    const result = await service.executeSplitPayment(splitId, payFn);

    assert.equal(result.success, true);
    // 2 recipients + 1 platform fee recipient = 3
    assert.equal(payFn.mock.calls.length, 3);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// getSplitPayment
// ─────────────────────────────────────────────────────────────────────────────

describe('getSplitPayment', () => {
  let store;
  let service;

  beforeEach(() => {
    store = makeStore();
    service = createSplitPaymentService(store);
  });

  it('should return formatted split payment when found', async () => {
    const createResult = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 50,
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    const id = createResult.splitPayment.id;
    const fetched = await service.getSplitPayment(id);

    assert.ok(fetched);
    assert.equal(fetched.id, id);
    assert.equal(fetched.senderAddress, '0xSender');
    assert.equal(fetched.totalAmountDecimal, 50);
    assert.equal(fetched.recipients.length, 2);
    // Verify camelCase formatting
    assert.ok('splitType' in fetched);
    assert.ok('createdAt' in fetched);
  });

  it('should return null when split payment is not found', async () => {
    const result = await service.getSplitPayment('nonexistent-id');
    assert.equal(result, null);
  });

  it('should return recipients with camelCase keys', async () => {
    const createResult = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 80,
      recipients: [
        { address: '0xAlice', percent: 50 },
        { address: '0xBob', percent: 50 },
      ],
    });

    const fetched = await service.getSplitPayment(createResult.splitPayment.id);
    const recipient = fetched.recipients[0];

    assert.ok('recipientAddress' in recipient);
    assert.ok('sharePercent' in recipient);
    assert.ok('shareAmount' in recipient);
    assert.ok('shareAmountDecimal' in recipient);
    assert.ok('splitPaymentId' in recipient);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// listSplitPayments
// ─────────────────────────────────────────────────────────────────────────────

describe('listSplitPayments', () => {
  let store;
  let service;

  beforeEach(() => {
    store = makeStore();
    service = createSplitPaymentService(store);
  });

  it('should return an empty array when no split payments exist', async () => {
    const result = await service.listSplitPayments();
    assert.deepEqual(result, []);
  });

  it('should pass snake_case filter to the store', async () => {
    await service.listSplitPayments({
      senderAddress: '0xSender',
      status: 'pending',
      limit: 10,
      offset: 5,
    });

    assert.equal(store.listSplitPayments.mock.calls.length, 1);
    const filterArg = store.listSplitPayments.mock.calls[0].arguments[0];
    assert.equal(filterArg.sender_address, '0xSender');
    assert.equal(filterArg.status, 'pending');
    assert.equal(filterArg.limit, 10);
    assert.equal(filterArg.offset, 5);
  });

  it('should format results from the store', async () => {
    // Override listSplitPayments to return raw rows
    store.listSplitPayments = mock.fn(async () => [
      {
        id: 'sp-1',
        status: 'completed',
        sender_address: '0xSender1',
        total_amount: 50_000_000,
        total_amount_decimal: 50,
        asset: 'USDC',
        network: 'set_chain',
        split_type: 'percentage',
        platform_fee_percent: null,
        platform_fee_amount: null,
        platform_fee_address: null,
        memo: null,
        reference_type: null,
        reference_id: null,
        metadata: null,
        completed_at: '2026-01-01T00:00:00.000Z',
        created_at: '2026-01-01T00:00:00.000Z',
        updated_at: '2026-01-01T00:00:00.000Z',
        recipients: [],
      },
      {
        id: 'sp-2',
        status: 'pending',
        sender_address: '0xSender2',
        total_amount: 100_000_000,
        total_amount_decimal: 100,
        asset: 'USDC',
        network: 'set_chain',
        split_type: 'fixed',
        platform_fee_percent: 5,
        platform_fee_amount: 5_000_000,
        platform_fee_address: '0xPlatform',
        memo: 'order split',
        reference_type: 'order',
        reference_id: 'ORD-1',
        metadata: '{"key":"value"}',
        completed_at: null,
        created_at: '2026-01-02T00:00:00.000Z',
        updated_at: '2026-01-02T00:00:00.000Z',
        recipients: [],
      },
    ]);

    const result = await service.listSplitPayments({});

    assert.equal(result.length, 2);
    assert.equal(result[0].id, 'sp-1');
    assert.equal(result[0].senderAddress, '0xSender1');
    assert.equal(result[0].status, 'completed');
    assert.equal(result[1].id, 'sp-2');
    assert.equal(result[1].senderAddress, '0xSender2');
    assert.equal(result[1].splitType, 'fixed');
    assert.equal(result[1].memo, 'order split');
  });

  it('should omit undefined filter keys from store filter', async () => {
    await service.listSplitPayments({ status: 'completed' });

    const filterArg = store.listSplitPayments.mock.calls[0].arguments[0];
    assert.equal(filterArg.status, 'completed');
    assert.equal(filterArg.sender_address, undefined);
    assert.equal(filterArg.limit, undefined);
    assert.equal(filterArg.offset, undefined);
  });

  it('should call listSplitPayments with empty filter when no args', async () => {
    await service.listSplitPayments();

    assert.equal(store.listSplitPayments.mock.calls.length, 1);
    const filterArg = store.listSplitPayments.mock.calls[0].arguments[0];
    assert.deepEqual(filterArg, {});
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// USDC decimal precision
// ─────────────────────────────────────────────────────────────────────────────

describe('USDC decimal precision', () => {
  let store;
  let service;

  beforeEach(() => {
    store = makeStore();
    service = createSplitPaymentService(store);
  });

  it('should handle fractional USDC amounts correctly (6 decimals)', async () => {
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 99.99,
      splitType: 'fixed',
      recipients: [
        { address: '0xAlice', amount: 49.995 },
        { address: '0xBob', amount: 49.995 },
      ],
    });

    assert.equal(result.success, true);
    assert.equal(result.splitPayment.totalAmount, Math.round(99.99 * 1_000_000));

    const alice = result.splitPayment.recipients.find(
      (r) => r.recipientAddress === '0xAlice',
    );
    assert.equal(alice.shareAmount, Math.round(49.995 * 1_000_000));
  });

  it('should distribute remainder to last recipient in percentage split', async () => {
    // 100 / 3 = 33.333... — impossible to divide evenly
    const result = await service.createSplitPayment({
      senderAddress: '0xSender',
      totalAmount: 1, // 1 USDC = 1,000,000 smallest units
      recipients: [
        { address: '0xAlice', percent: 33.33 },
        { address: '0xBob', percent: 33.33 },
        { address: '0xCharlie', percent: 33.34 },
      ],
    });

    const recs = result.splitPayment.recipients;
    const total = recs.reduce((s, r) => s + r.shareAmount, 0);
    assert.equal(total, 1_000_000, 'total shares must equal total amount in smallest units');

    // Alice and Bob get Math.round(1_000_000 * 33.33 / 100) = 333_300
    // Charlie gets remainder = 1_000_000 - 333_300 - 333_300 = 333_400
    const charlie = recs.find((r) => r.recipientAddress === '0xCharlie');
    const alice = recs.find((r) => r.recipientAddress === '0xAlice');
    const bob = recs.find((r) => r.recipientAddress === '0xBob');
    assert.equal(alice.shareAmount, 333300);
    assert.equal(bob.shareAmount, 333300);
    assert.equal(charlie.shareAmount, 333400);
  });
});
