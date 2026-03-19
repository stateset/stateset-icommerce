/**
 * A2A Settlement Finality State Machine
 *
 * Tracks settlements through finality stages rather than treating all
 * settlements as immediately complete. Each blockchain has different
 * confirmation requirements before a settlement is considered final.
 *
 * Finality states:
 *   broadcast → unconfirmed (0 conf) → confirming (1..N conf) → final (≥N conf) → settled
 *   Also: failed, reorged (block reorg detected)
 *
 * @example
 * ```javascript
 * const tracker = createFinalityTracker();
 *
 * // Begin tracking a settlement
 * tracker.trackSettlement('intent-1', '0xabc...', 'ethereum', 1000);
 *
 * // Update as confirmations come in
 * tracker.updateConfirmations('intent-1', 6, 1006);
 *
 * // Check if final
 * const { isFinal } = tracker.checkFinality('intent-1');
 *
 * // Detect a block reorg
 * tracker.detectReorg('intent-1', 998); // block went backwards
 * ```
 */

import { EventEmitter } from 'node:events';

// ---------------------------------------------------------------------------
// Per-chain finality requirements
// ---------------------------------------------------------------------------

/**
 * Number of confirmations required before a settlement on a given chain
 * is considered final.
 *
 * @type {Record<string, number>}
 */
const CHAIN_FINALITY_REQUIREMENTS = {
  set_chain: 1, // ~2s
  base: 2, // ~4s
  ethereum: 12, // ~3min
  arbitrum: 1, // instant
  solana: 32, // ~13s (slots)
  bitcoin: 6, // standard Bitcoin settlement threshold
  bitcoin_testnet: 3, // faster operator feedback on testnet
  zcash: 10, // aligned with chain config
  zcash_testnet: 6, // aligned with testnet config
};

/** Conservative default for unknown chains. */
const DEFAULT_FINALITY_BLOCKS = 12;

/**
 * Return the finality requirement (block count) for a chain.
 * Unknown chains default to 12 (conservative).
 *
 * @param {string} chain
 * @returns {number}
 */
export function getFinalityRequirement(chain) {
  return CHAIN_FINALITY_REQUIREMENTS[chain] ?? DEFAULT_FINALITY_BLOCKS;
}

// ---------------------------------------------------------------------------
// Finality states
// ---------------------------------------------------------------------------

/** @enum {string} */
const FinalityState = {
  BROADCAST: 'broadcast',
  UNCONFIRMED: 'unconfirmed',
  CONFIRMING: 'confirming',
  FINAL: 'final',
  SETTLED: 'settled',
  FAILED: 'failed',
  REORGED: 'reorged',
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Create a finality tracker instance.
 *
 * Emits the following events on its `.events` EventEmitter:
 *   - `settlement_confirmed` — confirmations increased
 *   - `settlement_final` — chain-specific finality reached
 *   - `settlement_reorged` — block reorg detected
 *   - `settlement_failed` — settlement marked as failed
 *
 * @returns {Object} Finality tracker API
 */
export function createFinalityTracker() {
  const emitter = new EventEmitter();

  /**
   * In-memory map of intentId → settlement record.
   * @type {Map<string, Object>}
   */
  const _settlements = new Map();

  // Metrics accumulators
  const _metrics = {
    totalTracked: 0,
    totalFinal: 0,
    totalReorgs: 0,
    totalFailed: 0,
    /** @type {number[]} - confirmation durations in ms for completed settlements */
    confirmationDurations: [],
  };

  // -----------------------------------------------------------------------
  // Helpers
  // -----------------------------------------------------------------------

  /**
   * Derive the correct FinalityState from current confirmations.
   *
   * @param {number} confirmations
   * @param {number} required
   * @returns {string}
   */
  function deriveState(confirmations, required) {
    if (confirmations <= 0) {
      return FinalityState.UNCONFIRMED;
    }
    if (confirmations >= required) {
      return FinalityState.FINAL;
    }
    return FinalityState.CONFIRMING;
  }

  // -----------------------------------------------------------------------
  // Core methods
  // -----------------------------------------------------------------------

  /**
   * Begin tracking a settlement through finality stages.
   *
   * @param {string} intentId - Unique intent / payment ID
   * @param {string} txHash - On-chain transaction hash
   * @param {string} chain - Blockchain identifier
   * @param {number} blockNumber - Block in which the tx was included
   * @returns {Object} The created settlement record
   */
  function trackSettlement(intentId, txHash, chain, blockNumber) {
    if (_settlements.has(intentId)) {
      throw new Error(`Settlement ${intentId} is already being tracked`);
    }

    const required = getFinalityRequirement(chain);
    const now = Date.now();

    const record = {
      intentId,
      txHash,
      chain,
      blockNumber,
      confirmations: 0,
      requiredConfirmations: required,
      state: FinalityState.BROADCAST,
      createdAt: now,
      updatedAt: now,
      finalAt: null,
      latestBlock: blockNumber,
    };

    _settlements.set(intentId, record);
    _metrics.totalTracked++;

    return { ...record };
  }

  /**
   * Update the confirmation count for a tracked settlement.
   * Automatically transitions the state as confirmations accumulate.
   *
   * @param {string} intentId
   * @param {number} confirmations - Current number of confirmations
   * @param {number} latestBlock - Latest observed block number on the chain
   * @returns {Object} Updated settlement record
   */
  function updateConfirmations(intentId, confirmations, latestBlock) {
    const record = _settlements.get(intentId);
    if (!record) {
      throw new Error(`Settlement ${intentId} not found`);
    }

    if (record.state === FinalityState.FAILED || record.state === FinalityState.SETTLED) {
      return { ...record };
    }

    const previousState = record.state;
    const previousConfirmations = record.confirmations;
    const now = Date.now();

    record.confirmations = confirmations;
    record.latestBlock = latestBlock;
    record.updatedAt = now;

    const newState = deriveState(confirmations, record.requiredConfirmations);

    // Only move forward (not backward unless reorged separately)
    if (
      newState !== previousState &&
      previousState !== FinalityState.FINAL &&
      previousState !== FinalityState.SETTLED
    ) {
      record.state = newState;
    }

    // Emit confirmed event when confirmations increase
    if (confirmations > previousConfirmations) {
      emitter.emit('settlement_confirmed', {
        intentId,
        confirmations,
        required: record.requiredConfirmations,
        chain: record.chain,
      });
    }

    // Emit final event on transition to final
    if (record.state === FinalityState.FINAL && previousState !== FinalityState.FINAL) {
      record.finalAt = now;
      _metrics.totalFinal++;
      _metrics.confirmationDurations.push(now - record.createdAt);

      emitter.emit('settlement_final', {
        intentId,
        txHash: record.txHash,
        chain: record.chain,
        confirmations,
        durationMs: now - record.createdAt,
      });
    }

    return { ...record };
  }

  /**
   * Check whether a settlement has reached finality.
   *
   * @param {string} intentId
   * @returns {{ state: string, isFinal: boolean, confirmations: number, required: number }}
   */
  function checkFinality(intentId) {
    const record = _settlements.get(intentId);
    if (!record) {
      throw new Error(`Settlement ${intentId} not found`);
    }

    return {
      state: record.state,
      isFinal: record.state === FinalityState.FINAL || record.state === FinalityState.SETTLED,
      confirmations: record.confirmations,
      required: record.requiredConfirmations,
    };
  }

  /**
   * Detect a block reorganisation. If the new block number is less than the
   * previously recorded block number, the settlement is marked as reorged.
   *
   * @param {string} intentId
   * @param {number} newBlockNumber - Newly observed block number
   * @returns {Object} Updated settlement record
   */
  function detectReorg(intentId, newBlockNumber) {
    const record = _settlements.get(intentId);
    if (!record) {
      throw new Error(`Settlement ${intentId} not found`);
    }

    if (newBlockNumber < record.blockNumber) {
      const previousState = record.state;
      record.state = FinalityState.REORGED;
      record.confirmations = 0;
      record.updatedAt = Date.now();
      _metrics.totalReorgs++;

      emitter.emit('settlement_reorged', {
        intentId,
        previousBlock: record.blockNumber,
        newBlock: newBlockNumber,
        previousState,
        chain: record.chain,
      });
    }

    return { ...record };
  }

  /**
   * Mark a settlement as failed.
   *
   * @param {string} intentId
   * @param {string} [reason]
   * @returns {Object} Updated settlement record
   */
  function markFailed(intentId, reason) {
    const record = _settlements.get(intentId);
    if (!record) {
      throw new Error(`Settlement ${intentId} not found`);
    }

    record.state = FinalityState.FAILED;
    record.updatedAt = Date.now();
    _metrics.totalFailed++;

    emitter.emit('settlement_failed', {
      intentId,
      reason: reason || 'unknown',
      chain: record.chain,
    });

    return { ...record };
  }

  /**
   * Mark a final settlement as fully settled (application-level).
   *
   * @param {string} intentId
   * @returns {Object} Updated settlement record
   */
  function markSettled(intentId) {
    const record = _settlements.get(intentId);
    if (!record) {
      throw new Error(`Settlement ${intentId} not found`);
    }
    if (record.state !== FinalityState.FINAL) {
      throw new Error(
        `Cannot settle ${intentId}: current state is ${record.state}, expected final`,
      );
    }

    record.state = FinalityState.SETTLED;
    record.updatedAt = Date.now();

    return { ...record };
  }

  /**
   * Return the full status object for a tracked settlement.
   *
   * @param {string} intentId
   * @returns {Object} Settlement status
   */
  function getSettlementStatus(intentId) {
    const record = _settlements.get(intentId);
    if (!record) {
      throw new Error(`Settlement ${intentId} not found`);
    }

    return {
      ...record,
      isFinal: record.state === FinalityState.FINAL || record.state === FinalityState.SETTLED,
      progress:
        record.requiredConfirmations > 0
          ? Math.min(record.confirmations / record.requiredConfirmations, 1)
          : 1,
    };
  }

  /**
   * List all settlements that have not yet reached finality.
   *
   * @returns {Object[]} Array of pending settlement records
   */
  function listPending() {
    const pending = [];
    for (const record of _settlements.values()) {
      if (
        record.state !== FinalityState.FINAL &&
        record.state !== FinalityState.SETTLED &&
        record.state !== FinalityState.FAILED
      ) {
        pending.push({ ...record });
      }
    }
    return pending;
  }

  /**
   * Compute aggregate metrics for the finality tracker.
   *
   * @returns {Object} Metrics object
   */
  function getMetrics() {
    const durations = _metrics.confirmationDurations;
    const avgConfirmationTimeMs =
      durations.length > 0 ? durations.reduce((sum, d) => sum + d, 0) / durations.length : 0;

    const finalityRate =
      _metrics.totalTracked > 0 ? _metrics.totalFinal / _metrics.totalTracked : 0;

    return {
      totalTracked: _metrics.totalTracked,
      totalFinal: _metrics.totalFinal,
      totalReorgs: _metrics.totalReorgs,
      totalFailed: _metrics.totalFailed,
      avgConfirmationTimeMs: Math.round(avgConfirmationTimeMs),
      finalityRate,
      pendingCount: listPending().length,
    };
  }

  // -----------------------------------------------------------------------
  // Public surface
  // -----------------------------------------------------------------------

  return {
    trackSettlement,
    updateConfirmations,
    checkFinality,
    detectReorg,
    markFailed,
    markSettled,
    getSettlementStatus,
    listPending,
    getMetrics,
    events: emitter,
    /** Expose for testing / documentation. */
    CHAIN_FINALITY_REQUIREMENTS,
    FinalityState,
  };
}
