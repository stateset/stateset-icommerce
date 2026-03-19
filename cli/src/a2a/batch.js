/**
 * A2A Batch Operations Service
 *
 * Enables efficient bulk operations instead of one-at-a-time calls.
 * Each item in a batch is independent — one failure never aborts the rest.
 *
 * Features:
 *   - Batch payments with idempotency keys
 *   - Batch quote requests to multiple sellers
 *   - Batch dispute resolution
 *   - Batch escrow creation
 *   - Batch quote fulfillment
 *   - Concurrency control (default 5)
 *   - Batch-level idempotency (same batch ID → cached results)
 *   - Optional progress callbacks
 *
 * @example
 * ```javascript
 * const batch = createBatchService(a2aService, store);
 *
 * const result = await batch.batchPay([
 *   { to: '0xAlice', amount: 10, asset: 'USDC', memo: 'Service A' },
 *   { to: 'bc1qBob', amount: 0.001, asset: 'BTC', network: 'bitcoin', memo: 'Service B' },
 * ]);
 * // { succeeded: 2, failed: 0, results: [...] }
 * ```
 */

import { randomUUID } from 'node:crypto';
import { DEFAULT_NETWORK, getDefaultAssetForNetwork } from './assets.js';

/**
 * Run an array of async tasks with a concurrency limit.
 *
 * @param {Array} items - Items to process
 * @param {number} concurrency - Max parallel executions
 * @param {Function} fn - Async function to call per item: (item, index) => Promise
 * @param {Function} [onProgress] - Optional progress callback: ({ completed, total, item, result }) => void
 * @returns {Promise<Array>} Settled results in order: { status: 'fulfilled'|'rejected', value?, reason? }
 */
async function runWithConcurrency(items, concurrency, fn, onProgress) {
  const results = new Array(items.length);
  let nextIndex = 0;
  let completed = 0;

  async function worker() {
    while (nextIndex < items.length) {
      const idx = nextIndex++;
      try {
        const value = await fn(items[idx], idx);
        results[idx] = { status: 'fulfilled', value };
      } catch (err) {
        results[idx] = { status: 'rejected', reason: err };
      }
      completed++;
      if (onProgress) {
        try {
          onProgress({
            completed,
            total: items.length,
            index: idx,
            result: results[idx],
          });
        } catch (_) {
          // progress callback errors are non-fatal
        }
      }
    }
  }

  const workers = [];
  const workerCount = Math.min(concurrency, items.length);
  for (let i = 0; i < workerCount; i++) {
    workers.push(worker());
  }
  await Promise.all(workers);

  return results;
}

/**
 * Create a batch operations service.
 *
 * @param {Object} a2aService - A2A service instance (from createA2AService)
 * @param {Object} store - A2A store instance
 * @returns {Object} Batch operations API
 */
export function createBatchService(a2aService, store) {
  /** @type {Map<string, Object>} Batch result cache keyed by batch ID */
  const _batchCache = new Map();

  /**
   * Execute multiple payments atomically (error-isolated).
   *
   * @param {Array<Object>} payments - Payments to execute
   * @param {string} payments[].to - Recipient address
   * @param {number} payments[].amount - Amount to pay
   * @param {string} [payments[].asset] - Asset (default: selected network payment asset)
   * @param {string} [payments[].network] - Network
   * @param {string} [payments[].memo] - Memo
   * @param {Object} [options]
   * @param {number} [options.concurrency=5] - Max parallel payments
   * @param {string} [options.batchId] - Idempotency key for the entire batch
   * @param {Function} [options.onProgress] - Progress callback
   * @returns {Promise<Object>} { succeeded, failed, results[], batchId }
   */
  async function batchPay(payments, options = {}) {
    const { concurrency = 5, batchId = randomUUID(), onProgress } = options;

    if (!Array.isArray(payments) || payments.length === 0) {
      throw new Error('payments must be a non-empty array');
    }

    // Batch idempotency: return cached results if this batch was already processed
    if (_batchCache.has(batchId)) {
      return _batchCache.get(batchId);
    }

    const settled = await runWithConcurrency(
      payments,
      concurrency,
      async (payment) => {
        const idempotencyKey = payment.idempotencyKey || `batch-${batchId}-${randomUUID()}`;
        return a2aService.pay({
          to: payment.to,
          amount: payment.amount,
          asset: payment.asset,
          network: payment.network,
          memo: payment.memo,
          idempotencyKey,
        });
      },
      onProgress,
    );

    const results = settled.map((s, i) => ({
      index: i,
      to: payments[i].to,
      amount: payments[i].amount,
      success: s.status === 'fulfilled',
      payment: s.status === 'fulfilled' ? s.value : null,
      error: s.status === 'rejected' ? s.reason.message : null,
    }));

    const succeeded = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;

    const result = { batchId, succeeded, failed, results };
    _batchCache.set(batchId, result);
    return result;
  }

  /**
   * Request quotes from multiple sellers simultaneously.
   *
   * @param {Array<Object>} requests - Quote requests
   * @param {string} requests[].seller - Seller agent address or ID
   * @param {Array} requests[].items - Items to quote
   * @param {string} [requests[].asset] - Preferred quote asset
   * @param {string} [requests[].network] - Preferred settlement network
   * @param {string} [requests[].message] - Quote request message
   * @param {number} [requests[].maxRounds] - Max negotiation rounds
   * @param {Object} [options]
   * @param {number} [options.concurrency=5] - Max parallel requests
   * @param {string} [options.batchId] - Idempotency key
   * @param {Function} [options.onProgress] - Progress callback
   * @returns {Promise<Object>} { sent, failed, quoteIds[], batchId }
   */
  async function batchRequestQuotes(requests, options = {}) {
    const { concurrency = 5, batchId = randomUUID(), onProgress } = options;

    if (!Array.isArray(requests) || requests.length === 0) {
      throw new Error('requests must be a non-empty array');
    }

    if (_batchCache.has(batchId)) {
      return _batchCache.get(batchId);
    }

    const settled = await runWithConcurrency(
      requests,
      concurrency,
      async (req) => {
        return a2aService.requestQuote({
          seller: req.seller,
          items: req.items,
          asset: req.asset,
          network: req.network,
          message: req.message,
          maxRounds: req.maxRounds,
        });
      },
      onProgress,
    );

    const quoteIds = [];
    let sent = 0;
    let failed = 0;

    const results = settled.map((s, i) => {
      if (s.status === 'fulfilled') {
        sent++;
        const quoteId = s.value?.quote?.id || null;
        quoteIds.push(quoteId);
        return {
          index: i,
          seller: requests[i].seller,
          success: true,
          quoteId,
          error: null,
        };
      }
      failed++;
      return {
        index: i,
        seller: requests[i].seller,
        success: false,
        quoteId: null,
        error: s.reason.message,
      };
    });

    const result = { batchId, sent, failed, quoteIds, results };
    _batchCache.set(batchId, result);
    return result;
  }

  /**
   * Resolve multiple disputes at once.
   *
   * @param {Array<Object>} disputes - Disputes to resolve
   * @param {string} disputes[].disputeId - Dispute ID
   * @param {string} disputes[].resolutionType - Resolution type (full_refund, partial_refund, etc.)
   * @param {number} [disputes[].amount] - Resolution amount (for partial)
   * @param {string} [disputes[].note] - Resolution note
   * @param {string} [disputes[].resolvedBy] - Who resolved
   * @param {Object} [options]
   * @param {number} [options.concurrency=5] - Max parallel resolutions
   * @param {string} [options.batchId] - Idempotency key
   * @param {Function} [options.onProgress] - Progress callback
   * @returns {Promise<Object>} { resolved, failed, results[], batchId }
   */
  async function batchResolveDisputes(disputes, options = {}) {
    const { concurrency = 5, batchId = randomUUID(), onProgress } = options;

    if (!Array.isArray(disputes) || disputes.length === 0) {
      throw new Error('disputes must be a non-empty array');
    }

    if (_batchCache.has(batchId)) {
      return _batchCache.get(batchId);
    }

    const settled = await runWithConcurrency(
      disputes,
      concurrency,
      async (d) => {
        return store.updateDispute(d.disputeId, {
          status: 'resolved',
          resolution_type: d.resolutionType,
          resolution_amount: d.amount,
          resolution_note: d.note || null,
          resolved_by: d.resolvedBy || 'batch',
          resolved_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        });
      },
      onProgress,
    );

    const results = settled.map((s, i) => ({
      index: i,
      disputeId: disputes[i].disputeId,
      success: s.status === 'fulfilled',
      result: s.status === 'fulfilled' ? s.value : null,
      error: s.status === 'rejected' ? s.reason.message : null,
    }));

    const resolved = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;

    const result = { batchId, resolved, failed, results };
    _batchCache.set(batchId, result);
    return result;
  }

  /**
   * Create multiple escrows.
   *
   * @param {Array<Object>} escrows - Escrows to create
   * @param {string} escrows[].buyerAddress - Buyer wallet address
   * @param {string} escrows[].sellerAddress - Seller wallet address
   * @param {number} escrows[].amount - Escrow amount
   * @param {string} [escrows[].asset] - Escrow asset
   * @param {string} [escrows[].network] - Settlement network
   * @param {Array} [escrows[].conditions] - Release conditions
   * @param {Object} [options]
   * @param {number} [options.concurrency=5] - Max parallel creations
   * @param {string} [options.batchId] - Idempotency key
   * @param {Function} [options.onProgress] - Progress callback
   * @returns {Promise<Object>} { created, failed, escrowIds[], results[], batchId }
   */
  async function batchCreateEscrows(escrows, options = {}) {
    const { concurrency = 5, batchId = randomUUID(), onProgress } = options;

    if (!Array.isArray(escrows) || escrows.length === 0) {
      throw new Error('escrows must be a non-empty array');
    }

    if (_batchCache.has(batchId)) {
      return _batchCache.get(batchId);
    }

    const settled = await runWithConcurrency(
      escrows,
      concurrency,
      async (e) => {
        const now = new Date().toISOString();
        const escrowId = randomUUID();
        const network = e.network || DEFAULT_NETWORK;
        const asset = e.asset || getDefaultAssetForNetwork(network);
        const escrowRecord = {
          id: escrowId,
          status: 'created',
          buyer_address: e.buyerAddress,
          seller_address: e.sellerAddress,
          amount: e.amount,
          amount_decimal: e.amountDecimal ?? e.amount,
          asset,
          network,
          release_conditions: e.conditions || [],
          created_at: now,
          updated_at: now,
        };
        await store.createEscrow(escrowRecord);
        return { id: escrowId, ...escrowRecord };
      },
      onProgress,
    );

    const escrowIds = [];
    const results = settled.map((s, i) => {
      if (s.status === 'fulfilled') {
        escrowIds.push(s.value.id);
        return {
          index: i,
          success: true,
          escrowId: s.value.id,
          error: null,
        };
      }
      return {
        index: i,
        success: false,
        escrowId: null,
        error: s.reason.message,
      };
    });

    const created = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;

    const result = { batchId, created, failed, escrowIds, results };
    _batchCache.set(batchId, result);
    return result;
  }

  /**
   * Mark multiple quotes as fulfilled.
   *
   * @param {Array<string>} quoteIds - Quote IDs to fulfill
   * @param {Object} [options]
   * @param {number} [options.concurrency=5] - Max parallel fulfillments
   * @param {string} [options.batchId] - Idempotency key
   * @param {Function} [options.onProgress] - Progress callback
   * @returns {Promise<Object>} { fulfilled, failed, results[], batchId }
   */
  async function batchFulfillQuotes(quoteIds, options = {}) {
    const { concurrency = 5, batchId = randomUUID(), onProgress } = options;

    if (!Array.isArray(quoteIds) || quoteIds.length === 0) {
      throw new Error('quoteIds must be a non-empty array');
    }

    if (_batchCache.has(batchId)) {
      return _batchCache.get(batchId);
    }

    const settled = await runWithConcurrency(
      quoteIds,
      concurrency,
      async (quoteId) => {
        return a2aService.fulfillQuote(quoteId);
      },
      onProgress,
    );

    const results = settled.map((s, i) => ({
      index: i,
      quoteId: quoteIds[i],
      success: s.status === 'fulfilled',
      result: s.status === 'fulfilled' ? s.value : null,
      error: s.status === 'rejected' ? s.reason.message : null,
    }));

    const fulfilled = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;

    const result = { batchId, fulfilled, failed, results };
    _batchCache.set(batchId, result);
    return result;
  }

  /**
   * Clear the batch cache (useful for testing).
   */
  function clearCache() {
    _batchCache.clear();
  }

  return {
    batchPay,
    batchRequestQuotes,
    batchResolveDisputes,
    batchCreateEscrows,
    batchFulfillQuotes,
    clearCache,
  };
}

export default { createBatchService };
