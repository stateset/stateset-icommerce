/**
 * Unit tests for a2a/batch.js — Batch Operations Service
 *
 * Covers: batchPay, batchRequestQuotes, batchResolveDisputes,
 * batchCreateEscrows, batchFulfillQuotes, error isolation,
 * concurrency control, batch idempotency, progress tracking.
 */

import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createBatchService } from '../../src/a2a/batch.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a mock A2A service */
function createMockA2AService(overrides = {}) {
  return {
    pay: mock.fn(async ({ to, amount }) => ({
      success: true,
      payment: { id: `pay-${to}-${amount}`, status: 'submitted' },
    })),
    requestQuote: mock.fn(async ({ seller }) => ({
      success: true,
      quote: { id: `quote-${seller}`, status: 'requested' },
    })),
    fulfillQuote: mock.fn(async (quoteId) => ({
      success: true,
      quote: { id: quoteId, status: 'fulfilled' },
    })),
    ...overrides,
  };
}

/** Build a mock store */
function createMockStore(overrides = {}) {
  return {
    updateDispute: mock.fn(async (id, updates) => ({ id, ...updates })),
    createEscrow: mock.fn(async (record) => record),
    ...overrides,
  };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('createBatchService', () => {
  /** @type {ReturnType<typeof createBatchService>} */
  let batch;
  let a2aService;
  let store;

  beforeEach(() => {
    a2aService = createMockA2AService();
    store = createMockStore();
    batch = createBatchService(a2aService, store);
  });

  // -----------------------------------------------------------------------
  // batchPay
  // -----------------------------------------------------------------------

  describe('batchPay', () => {
    it('executes multiple payments and returns succeeded/failed counts', async () => {
      const result = await batch.batchPay([
        { to: '0xAlice', amount: 10, asset: 'USDC', memo: 'Svc A' },
        { to: '0xBob', amount: 20, asset: 'USDC', memo: 'Svc B' },
        { to: '0xCharlie', amount: 30, memo: 'Svc C' },
      ]);

      assert.equal(result.succeeded, 3);
      assert.equal(result.failed, 0);
      assert.equal(result.results.length, 3);
      assert.ok(result.batchId);
      assert.equal(a2aService.pay.mock.calls.length, 3);

      // Each result includes metadata
      assert.equal(result.results[0].to, '0xAlice');
      assert.equal(result.results[0].amount, 10);
      assert.equal(result.results[0].success, true);
      assert.ok(result.results[0].payment);
    });

    it('isolates failures — one bad payment does not block others', async () => {
      let callCount = 0;
      a2aService.pay = mock.fn(async ({ to }) => {
        callCount++;
        if (to === '0xBad') throw new Error('insufficient funds');
        return { success: true, payment: { id: `pay-${callCount}` } };
      });

      const result = await batch.batchPay([
        { to: '0xAlice', amount: 10 },
        { to: '0xBad', amount: 999 },
        { to: '0xCharlie', amount: 30 },
      ]);

      assert.equal(result.succeeded, 2);
      assert.equal(result.failed, 1);
      assert.equal(result.results[1].success, false);
      assert.equal(result.results[1].error, 'insufficient funds');
      // Other payments still succeeded
      assert.equal(result.results[0].success, true);
      assert.equal(result.results[2].success, true);
    });

    it('throws on empty payments array', async () => {
      await assert.rejects(
        () => batch.batchPay([]),
        { message: 'payments must be a non-empty array' },
      );
    });

    it('throws on non-array payments', async () => {
      await assert.rejects(
        () => batch.batchPay(null),
        { message: 'payments must be a non-empty array' },
      );
    });

    it('passes idempotencyKey per payment', async () => {
      await batch.batchPay([{ to: '0xAlice', amount: 10 }]);

      const call = a2aService.pay.mock.calls[0];
      assert.ok(call.arguments[0].idempotencyKey);
      assert.ok(call.arguments[0].idempotencyKey.startsWith('batch-'));
    });

    it('invokes progress callback during execution', async () => {
      const progressEvents = [];
      const onProgress = ({ completed, total }) => {
        progressEvents.push({ completed, total });
      };

      await batch.batchPay(
        [
          { to: '0xAlice', amount: 10 },
          { to: '0xBob', amount: 20 },
        ],
        { onProgress },
      );

      assert.equal(progressEvents.length, 2);
      assert.equal(progressEvents[0].total, 2);
      // completed increments
      const completedValues = progressEvents.map((e) => e.completed).sort();
      assert.deepEqual(completedValues, [1, 2]);
    });
  });

  // -----------------------------------------------------------------------
  // batchRequestQuotes
  // -----------------------------------------------------------------------

  describe('batchRequestQuotes', () => {
    it('sends quote requests to multiple sellers', async () => {
      const result = await batch.batchRequestQuotes([
        { seller: '0xSeller1', items: [{ description: 'Widget', quantity: 1 }] },
        { seller: '0xSeller2', items: [{ description: 'Gadget', quantity: 2 }] },
        { seller: '0xSeller3', items: [{ description: 'Gizmo', quantity: 3 }] },
      ]);

      assert.equal(result.sent, 3);
      assert.equal(result.failed, 0);
      assert.equal(result.quoteIds.length, 3);
      assert.equal(a2aService.requestQuote.mock.calls.length, 3);
    });

    it('isolates failures in quote requests', async () => {
      a2aService.requestQuote = mock.fn(async ({ seller }) => {
        if (seller === '0xOffline') throw new Error('agent unreachable');
        return { success: true, quote: { id: `q-${seller}` } };
      });

      const result = await batch.batchRequestQuotes([
        { seller: '0xSeller1', items: [{ description: 'A' }] },
        { seller: '0xOffline', items: [{ description: 'B' }] },
      ]);

      assert.equal(result.sent, 1);
      assert.equal(result.failed, 1);
      assert.equal(result.quoteIds.length, 1);
      assert.equal(result.results[1].error, 'agent unreachable');
    });

    it('throws on empty requests', async () => {
      await assert.rejects(
        () => batch.batchRequestQuotes([]),
        { message: 'requests must be a non-empty array' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // batchResolveDisputes
  // -----------------------------------------------------------------------

  describe('batchResolveDisputes', () => {
    it('resolves multiple disputes at once', async () => {
      const result = await batch.batchResolveDisputes([
        { disputeId: 'd-1', resolutionType: 'full_refund', note: 'Refund', resolvedBy: 'admin' },
        { disputeId: 'd-2', resolutionType: 'partial_refund', amount: 50, resolvedBy: 'admin' },
        { disputeId: 'd-3', resolutionType: 'release_to_seller', resolvedBy: 'admin' },
      ]);

      assert.equal(result.resolved, 3);
      assert.equal(result.failed, 0);
      assert.equal(store.updateDispute.mock.calls.length, 3);

      // Check that dispute updates include correct resolution types
      const firstCall = store.updateDispute.mock.calls[0].arguments;
      assert.equal(firstCall[0], 'd-1');
      assert.equal(firstCall[1].resolution_type, 'full_refund');
      assert.equal(firstCall[1].status, 'resolved');
    });

    it('isolates failures in dispute resolutions', async () => {
      store.updateDispute = mock.fn(async (id) => {
        if (id === 'd-bad') throw new Error('dispute not found');
        return { id, status: 'resolved' };
      });

      const result = await batch.batchResolveDisputes([
        { disputeId: 'd-1', resolutionType: 'full_refund' },
        { disputeId: 'd-bad', resolutionType: 'full_refund' },
      ]);

      assert.equal(result.resolved, 1);
      assert.equal(result.failed, 1);
      assert.equal(result.results[1].error, 'dispute not found');
    });

    it('throws on empty disputes', async () => {
      await assert.rejects(
        () => batch.batchResolveDisputes([]),
        { message: 'disputes must be a non-empty array' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // batchCreateEscrows
  // -----------------------------------------------------------------------

  describe('batchCreateEscrows', () => {
    it('creates multiple escrows and returns escrowIds', async () => {
      const result = await batch.batchCreateEscrows([
        { buyerAddress: '0xBuyer1', sellerAddress: '0xSeller1', amount: 100 },
        { buyerAddress: '0xBuyer2', sellerAddress: '0xSeller2', amount: 200 },
      ]);

      assert.equal(result.created, 2);
      assert.equal(result.failed, 0);
      assert.equal(result.escrowIds.length, 2);
      assert.equal(store.createEscrow.mock.calls.length, 2);

      // Verify each escrow got a UUID
      for (const id of result.escrowIds) {
        assert.ok(id);
        assert.equal(typeof id, 'string');
        assert.ok(id.includes('-')); // UUID format
      }
    });

    it('isolates failures in escrow creation', async () => {
      let callIndex = 0;
      store.createEscrow = mock.fn(async (record) => {
        callIndex++;
        if (callIndex === 2) throw new Error('db error');
        return record;
      });

      const result = await batch.batchCreateEscrows([
        { buyerAddress: '0xB1', sellerAddress: '0xS1', amount: 100 },
        { buyerAddress: '0xB2', sellerAddress: '0xS2', amount: 200 },
        { buyerAddress: '0xB3', sellerAddress: '0xS3', amount: 300 },
      ]);

      assert.equal(result.created, 2);
      assert.equal(result.failed, 1);
      assert.equal(result.escrowIds.length, 2);
    });

    it('throws on empty escrows', async () => {
      await assert.rejects(
        () => batch.batchCreateEscrows([]),
        { message: 'escrows must be a non-empty array' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // batchFulfillQuotes
  // -----------------------------------------------------------------------

  describe('batchFulfillQuotes', () => {
    it('fulfills multiple quotes', async () => {
      const result = await batch.batchFulfillQuotes(['q-1', 'q-2', 'q-3']);

      assert.equal(result.fulfilled, 3);
      assert.equal(result.failed, 0);
      assert.equal(a2aService.fulfillQuote.mock.calls.length, 3);
    });

    it('isolates failures in fulfillment', async () => {
      a2aService.fulfillQuote = mock.fn(async (id) => {
        if (id === 'q-missing') throw new Error('Quote not found');
        return { success: true, quote: { id, status: 'fulfilled' } };
      });

      const result = await batch.batchFulfillQuotes(['q-1', 'q-missing', 'q-3']);

      assert.equal(result.fulfilled, 2);
      assert.equal(result.failed, 1);
      assert.equal(result.results[1].error, 'Quote not found');
    });

    it('throws on empty quoteIds', async () => {
      await assert.rejects(
        () => batch.batchFulfillQuotes([]),
        { message: 'quoteIds must be a non-empty array' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // Concurrency control
  // -----------------------------------------------------------------------

  describe('concurrency control', () => {
    it('limits parallel executions to concurrency param', async () => {
      let maxConcurrent = 0;
      let currentConcurrent = 0;

      a2aService.pay = mock.fn(async ({ to }) => {
        currentConcurrent++;
        if (currentConcurrent > maxConcurrent) {
          maxConcurrent = currentConcurrent;
        }
        // Simulate async work
        await new Promise((r) => setTimeout(r, 10));
        currentConcurrent--;
        return { success: true, payment: { id: `pay-${to}` } };
      });

      // 10 payments with concurrency 3
      const payments = Array.from({ length: 10 }, (_, i) => ({
        to: `0xAddr${i}`,
        amount: i + 1,
      }));

      await batch.batchPay(payments, { concurrency: 3 });

      assert.ok(maxConcurrent <= 3, `max concurrent was ${maxConcurrent}, expected <= 3`);
      assert.equal(a2aService.pay.mock.calls.length, 10);
    });

    it('defaults to concurrency of 5', async () => {
      let maxConcurrent = 0;
      let currentConcurrent = 0;

      a2aService.pay = mock.fn(async () => {
        currentConcurrent++;
        if (currentConcurrent > maxConcurrent) {
          maxConcurrent = currentConcurrent;
        }
        await new Promise((r) => setTimeout(r, 10));
        currentConcurrent--;
        return { success: true, payment: { id: 'p' } };
      });

      const payments = Array.from({ length: 20 }, (_, i) => ({
        to: `0xAddr${i}`,
        amount: 1,
      }));

      await batch.batchPay(payments);

      assert.ok(maxConcurrent <= 5, `max concurrent was ${maxConcurrent}, expected <= 5`);
    });
  });

  // -----------------------------------------------------------------------
  // Batch idempotency
  // -----------------------------------------------------------------------

  describe('batch idempotency', () => {
    it('returns cached results when same batchId is resubmitted', async () => {
      const batchId = 'fixed-batch-id';

      const firstResult = await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { batchId },
      );

      // Reset mock call count
      a2aService.pay.mock.resetCalls();

      const secondResult = await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { batchId },
      );

      // Same object should be returned
      assert.deepEqual(firstResult, secondResult);

      // pay should NOT have been called again
      assert.equal(a2aService.pay.mock.calls.length, 0);
    });

    it('different batchIds produce independent results', async () => {
      const r1 = await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { batchId: 'batch-1' },
      );

      const r2 = await batch.batchPay(
        [{ to: '0xBob', amount: 20 }],
        { batchId: 'batch-2' },
      );

      assert.notEqual(r1.batchId, r2.batchId);
      assert.equal(a2aService.pay.mock.calls.length, 2);
    });

    it('idempotency works across batch operation types', async () => {
      const batchId = 'shared-id';

      // First call with batchPay
      const r1 = await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { batchId },
      );

      // Second call with batchRequestQuotes using same batchId —
      // should NOT return cached because internal cache stores the full result
      // but the cache is keyed by batchId regardless of operation type
      a2aService.requestQuote.mock.resetCalls();

      const r2 = await batch.batchRequestQuotes(
        [{ seller: '0xSeller', items: [{ description: 'Widget' }] }],
        { batchId },
      );

      // Returns the cached batchPay result (same batchId)
      assert.deepEqual(r1, r2);
      assert.equal(a2aService.requestQuote.mock.calls.length, 0);
    });

    it('clearCache resets idempotency', async () => {
      const batchId = 'clear-test';

      await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { batchId },
      );

      a2aService.pay.mock.resetCalls();
      batch.clearCache();

      await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { batchId },
      );

      // Should have been called again after cache clear
      assert.equal(a2aService.pay.mock.calls.length, 1);
    });
  });

  // -----------------------------------------------------------------------
  // Edge cases
  // -----------------------------------------------------------------------

  describe('edge cases', () => {
    it('handles a single-item batch', async () => {
      const result = await batch.batchPay([{ to: '0xAlice', amount: 1 }]);

      assert.equal(result.succeeded, 1);
      assert.equal(result.failed, 0);
    });

    it('handles all items failing', async () => {
      a2aService.pay = mock.fn(async () => {
        throw new Error('network down');
      });

      const result = await batch.batchPay([
        { to: '0xA', amount: 1 },
        { to: '0xB', amount: 2 },
      ]);

      assert.equal(result.succeeded, 0);
      assert.equal(result.failed, 2);
      assert.equal(result.results[0].error, 'network down');
      assert.equal(result.results[1].error, 'network down');
    });

    it('progress callback errors do not break batch execution', async () => {
      const onProgress = () => {
        throw new Error('callback bug');
      };

      const result = await batch.batchPay(
        [{ to: '0xAlice', amount: 10 }],
        { onProgress },
      );

      assert.equal(result.succeeded, 1);
    });
  });
});
