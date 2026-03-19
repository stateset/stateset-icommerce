/**
 * Tests for cli/src/a2a/settlement-finality.js
 *
 * Covers: createFinalityTracker — trackSettlement, updateConfirmations,
 * checkFinality, detectReorg, listPending, getMetrics, event emissions,
 * per-chain finality requirements, markFailed, markSettled.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createFinalityTracker } from '../../src/a2a/settlement-finality.js';

// ===========================================================================
// Tests
// ===========================================================================

describe('createFinalityTracker', () => {
  /** @type {ReturnType<typeof createFinalityTracker>} */
  let tracker;

  beforeEach(() => {
    tracker = createFinalityTracker();
  });

  // -----------------------------------------------------------------------
  // trackSettlement
  // -----------------------------------------------------------------------

  describe('trackSettlement', () => {
    it('creates a pending entry with broadcast state', () => {
      const record = tracker.trackSettlement('intent-1', '0xabc', 'ethereum', 1000);

      assert.equal(record.intentId, 'intent-1');
      assert.equal(record.txHash, '0xabc');
      assert.equal(record.chain, 'ethereum');
      assert.equal(record.blockNumber, 1000);
      assert.equal(record.confirmations, 0);
      assert.equal(record.state, 'broadcast');
      assert.equal(record.requiredConfirmations, 12);
      assert.equal(record.finalAt, null);
    });

    it('throws if intent is already tracked', () => {
      tracker.trackSettlement('intent-1', '0xabc', 'ethereum', 1000);
      assert.throws(
        () => tracker.trackSettlement('intent-1', '0xdef', 'ethereum', 1001),
        /already being tracked/,
      );
    });

    it('sets correct finality requirement per chain', () => {
      const rec1 = tracker.trackSettlement('i1', '0x1', 'set_chain', 100);
      assert.equal(rec1.requiredConfirmations, 1);

      const rec2 = tracker.trackSettlement('i2', '0x2', 'base', 200);
      assert.equal(rec2.requiredConfirmations, 2);

      const rec3 = tracker.trackSettlement('i3', '0x3', 'ethereum', 300);
      assert.equal(rec3.requiredConfirmations, 12);

      const rec4 = tracker.trackSettlement('i4', '0x4', 'arbitrum', 400);
      assert.equal(rec4.requiredConfirmations, 1);

      const rec5 = tracker.trackSettlement('i5', '0x5', 'solana', 500);
      assert.equal(rec5.requiredConfirmations, 32);

      const rec6 = tracker.trackSettlement('i6', '0x6', 'bitcoin', 600);
      assert.equal(rec6.requiredConfirmations, 6);

      const rec7 = tracker.trackSettlement('i7', '0x7', 'zcash', 700);
      assert.equal(rec7.requiredConfirmations, 10);
    });

    it('unknown chain defaults to conservative finality (12 blocks)', () => {
      const record = tracker.trackSettlement('i-unknown', '0xz', 'polygon', 100);
      assert.equal(record.requiredConfirmations, 12);
    });
  });

  // -----------------------------------------------------------------------
  // updateConfirmations
  // -----------------------------------------------------------------------

  describe('updateConfirmations', () => {
    it('moves from broadcast to unconfirmed at 0 confirmations', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      const updated = tracker.updateConfirmations('i1', 0, 1000);
      assert.equal(updated.state, 'unconfirmed');
    });

    it('moves from broadcast to confirming at 1+ confirmations', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      const updated = tracker.updateConfirmations('i1', 3, 1003);
      assert.equal(updated.state, 'confirming');
      assert.equal(updated.confirmations, 3);
    });

    it('moves to final when confirmations >= required', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      const updated = tracker.updateConfirmations('i1', 12, 1012);
      assert.equal(updated.state, 'final');
      assert.equal(updated.confirmations, 12);
    });

    it('remains final once finality is reached', () => {
      tracker.trackSettlement('i1', '0x1', 'base', 100);
      tracker.updateConfirmations('i1', 2, 102);
      const updated = tracker.updateConfirmations('i1', 5, 105);
      assert.equal(updated.state, 'final');
    });

    it('throws for unknown intentId', () => {
      assert.throws(
        () => tracker.updateConfirmations('nonexistent', 1, 100),
        /not found/,
      );
    });

    it('does not change failed settlements', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.markFailed('i1', 'tx reverted');
      const updated = tracker.updateConfirmations('i1', 12, 1012);
      assert.equal(updated.state, 'failed');
    });

    it('progresses through all states correctly for set_chain (1 block)', () => {
      tracker.trackSettlement('i1', '0x1', 'set_chain', 500);

      const s0 = tracker.updateConfirmations('i1', 0, 500);
      assert.equal(s0.state, 'unconfirmed');

      const s1 = tracker.updateConfirmations('i1', 1, 501);
      assert.equal(s1.state, 'final');
    });

    it('progresses through all states correctly for solana (32 slots)', () => {
      tracker.trackSettlement('i1', 'sig123', 'solana', 10000);

      const s0 = tracker.updateConfirmations('i1', 0, 10000);
      assert.equal(s0.state, 'unconfirmed');

      const s10 = tracker.updateConfirmations('i1', 10, 10010);
      assert.equal(s10.state, 'confirming');

      const s31 = tracker.updateConfirmations('i1', 31, 10031);
      assert.equal(s31.state, 'confirming');

      const s32 = tracker.updateConfirmations('i1', 32, 10032);
      assert.equal(s32.state, 'final');
    });
  });

  // -----------------------------------------------------------------------
  // checkFinality
  // -----------------------------------------------------------------------

  describe('checkFinality', () => {
    it('returns isFinal=false for broadcast state', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      const result = tracker.checkFinality('i1');
      assert.equal(result.isFinal, false);
      assert.equal(result.state, 'broadcast');
      assert.equal(result.confirmations, 0);
      assert.equal(result.required, 12);
    });

    it('returns isFinal=true when confirmations >= required', () => {
      tracker.trackSettlement('i1', '0x1', 'base', 100);
      tracker.updateConfirmations('i1', 2, 102);
      const result = tracker.checkFinality('i1');
      assert.equal(result.isFinal, true);
      assert.equal(result.state, 'final');
    });

    it('returns isFinal=true for settled state', () => {
      tracker.trackSettlement('i1', '0x1', 'arbitrum', 100);
      tracker.updateConfirmations('i1', 1, 101);
      tracker.markSettled('i1');
      const result = tracker.checkFinality('i1');
      assert.equal(result.isFinal, true);
      assert.equal(result.state, 'settled');
    });

    it('returns isFinal=false for confirming state', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.updateConfirmations('i1', 5, 1005);
      const result = tracker.checkFinality('i1');
      assert.equal(result.isFinal, false);
      assert.equal(result.state, 'confirming');
    });

    it('throws for unknown intentId', () => {
      assert.throws(
        () => tracker.checkFinality('nonexistent'),
        /not found/,
      );
    });
  });

  // -----------------------------------------------------------------------
  // Per-chain finality requirements
  // -----------------------------------------------------------------------

  describe('per-chain finality requirements', () => {
    it('ethereum needs 12 confirmations', () => {
      tracker.trackSettlement('eth1', '0x1', 'ethereum', 1000);
      tracker.updateConfirmations('eth1', 11, 1011);
      assert.equal(tracker.checkFinality('eth1').isFinal, false);
      tracker.updateConfirmations('eth1', 12, 1012);
      assert.equal(tracker.checkFinality('eth1').isFinal, true);
    });

    it('base needs 2 confirmations', () => {
      tracker.trackSettlement('base1', '0x1', 'base', 100);
      tracker.updateConfirmations('base1', 1, 101);
      assert.equal(tracker.checkFinality('base1').isFinal, false);
      tracker.updateConfirmations('base1', 2, 102);
      assert.equal(tracker.checkFinality('base1').isFinal, true);
    });

    it('set_chain needs 1 confirmation', () => {
      tracker.trackSettlement('sc1', '0x1', 'set_chain', 100);
      tracker.updateConfirmations('sc1', 1, 101);
      assert.equal(tracker.checkFinality('sc1').isFinal, true);
    });

    it('arbitrum needs 1 confirmation', () => {
      tracker.trackSettlement('arb1', '0x1', 'arbitrum', 100);
      tracker.updateConfirmations('arb1', 1, 101);
      assert.equal(tracker.checkFinality('arb1').isFinal, true);
    });

    it('solana needs 32 confirmations (slots)', () => {
      tracker.trackSettlement('sol1', 'sig', 'solana', 5000);
      tracker.updateConfirmations('sol1', 31, 5031);
      assert.equal(tracker.checkFinality('sol1').isFinal, false);
      tracker.updateConfirmations('sol1', 32, 5032);
      assert.equal(tracker.checkFinality('sol1').isFinal, true);
    });

    it('bitcoin needs 6 confirmations', () => {
      tracker.trackSettlement('btc1', 'tx', 'bitcoin', 1000);
      tracker.updateConfirmations('btc1', 5, 1005);
      assert.equal(tracker.checkFinality('btc1').isFinal, false);
      tracker.updateConfirmations('btc1', 6, 1006);
      assert.equal(tracker.checkFinality('btc1').isFinal, true);
    });

    it('zcash needs 10 confirmations', () => {
      tracker.trackSettlement('zec1', 'tx', 'zcash', 2000);
      tracker.updateConfirmations('zec1', 9, 2009);
      assert.equal(tracker.checkFinality('zec1').isFinal, false);
      tracker.updateConfirmations('zec1', 10, 2010);
      assert.equal(tracker.checkFinality('zec1').isFinal, true);
    });

    it('unknown chain requires 12 blocks (conservative default)', () => {
      tracker.trackSettlement('unk1', '0x1', 'avalanche', 100);
      tracker.updateConfirmations('unk1', 11, 111);
      assert.equal(tracker.checkFinality('unk1').isFinal, false);
      tracker.updateConfirmations('unk1', 12, 112);
      assert.equal(tracker.checkFinality('unk1').isFinal, true);
    });
  });

  // -----------------------------------------------------------------------
  // detectReorg
  // -----------------------------------------------------------------------

  describe('detectReorg', () => {
    it('marks as reorged when new block < original block', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.updateConfirmations('i1', 5, 1005);
      const result = tracker.detectReorg('i1', 998);
      assert.equal(result.state, 'reorged');
      assert.equal(result.confirmations, 0);
    });

    it('does not reorg if new block >= original block', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.updateConfirmations('i1', 5, 1005);
      const result = tracker.detectReorg('i1', 1000);
      assert.notEqual(result.state, 'reorged');
    });

    it('does not reorg if new block > original block', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      const result = tracker.detectReorg('i1', 1002);
      assert.notEqual(result.state, 'reorged');
    });

    it('throws for unknown intentId', () => {
      assert.throws(
        () => tracker.detectReorg('nonexistent', 100),
        /not found/,
      );
    });
  });

  // -----------------------------------------------------------------------
  // listPending
  // -----------------------------------------------------------------------

  describe('listPending', () => {
    it('returns only non-final, non-failed, non-settled settlements', () => {
      // broadcast
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      // confirming
      tracker.trackSettlement('i2', '0x2', 'ethereum', 2000);
      tracker.updateConfirmations('i2', 3, 2003);
      // final
      tracker.trackSettlement('i3', '0x3', 'set_chain', 100);
      tracker.updateConfirmations('i3', 1, 101);
      // failed
      tracker.trackSettlement('i4', '0x4', 'base', 200);
      tracker.markFailed('i4', 'reverted');

      const pending = tracker.listPending();
      const ids = pending.map((p) => p.intentId);
      assert.deepEqual(ids.sort(), ['i1', 'i2']);
    });

    it('returns empty array when all are final', () => {
      tracker.trackSettlement('i1', '0x1', 'arbitrum', 100);
      tracker.updateConfirmations('i1', 1, 101);
      assert.equal(tracker.listPending().length, 0);
    });

    it('includes reorged settlements', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.detectReorg('i1', 998);
      const pending = tracker.listPending();
      assert.equal(pending.length, 1);
      assert.equal(pending[0].state, 'reorged');
    });
  });

  // -----------------------------------------------------------------------
  // getMetrics
  // -----------------------------------------------------------------------

  describe('getMetrics', () => {
    it('returns zeroes with no settlements', () => {
      const metrics = tracker.getMetrics();
      assert.equal(metrics.totalTracked, 0);
      assert.equal(metrics.totalFinal, 0);
      assert.equal(metrics.totalReorgs, 0);
      assert.equal(metrics.totalFailed, 0);
      assert.equal(metrics.avgConfirmationTimeMs, 0);
      assert.equal(metrics.finalityRate, 0);
      assert.equal(metrics.pendingCount, 0);
    });

    it('computes totalTracked correctly', () => {
      tracker.trackSettlement('i1', '0x1', 'set_chain', 100);
      tracker.trackSettlement('i2', '0x2', 'base', 200);
      assert.equal(tracker.getMetrics().totalTracked, 2);
    });

    it('computes totalFinal after settlements reach finality', () => {
      tracker.trackSettlement('i1', '0x1', 'set_chain', 100);
      tracker.updateConfirmations('i1', 1, 101);
      assert.equal(tracker.getMetrics().totalFinal, 1);
    });

    it('computes totalReorgs after reorg detection', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.detectReorg('i1', 998);
      assert.equal(tracker.getMetrics().totalReorgs, 1);
    });

    it('computes totalFailed', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.markFailed('i1', 'reverted');
      assert.equal(tracker.getMetrics().totalFailed, 1);
    });

    it('computes avgConfirmationTimeMs for finalized settlements', () => {
      tracker.trackSettlement('i1', '0x1', 'arbitrum', 100);
      // Small delay to ensure non-zero duration
      tracker.updateConfirmations('i1', 1, 101);
      const metrics = tracker.getMetrics();
      assert.equal(metrics.totalFinal, 1);
      assert.equal(typeof metrics.avgConfirmationTimeMs, 'number');
      // Should be >= 0 (typically very small in tests)
      assert.ok(metrics.avgConfirmationTimeMs >= 0);
    });

    it('computes finalityRate correctly', () => {
      tracker.trackSettlement('i1', '0x1', 'arbitrum', 100);
      tracker.trackSettlement('i2', '0x2', 'arbitrum', 200);
      tracker.updateConfirmations('i1', 1, 101);
      const metrics = tracker.getMetrics();
      assert.equal(metrics.finalityRate, 0.5);
    });

    it('computes pendingCount correctly', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.trackSettlement('i2', '0x2', 'set_chain', 100);
      tracker.updateConfirmations('i2', 1, 101);
      assert.equal(tracker.getMetrics().pendingCount, 1);
    });
  });

  // -----------------------------------------------------------------------
  // Events
  // -----------------------------------------------------------------------

  describe('event emissions', () => {
    it('emits settlement_confirmed when confirmations increase', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);

      const events = [];
      tracker.events.on('settlement_confirmed', (e) => events.push(e));

      tracker.updateConfirmations('i1', 3, 1003);

      assert.equal(events.length, 1);
      assert.equal(events[0].intentId, 'i1');
      assert.equal(events[0].confirmations, 3);
      assert.equal(events[0].required, 12);
      assert.equal(events[0].chain, 'ethereum');
    });

    it('emits settlement_final when finality is reached', () => {
      tracker.trackSettlement('i1', '0x1', 'base', 100);

      const events = [];
      tracker.events.on('settlement_final', (e) => events.push(e));

      tracker.updateConfirmations('i1', 2, 102);

      assert.equal(events.length, 1);
      assert.equal(events[0].intentId, 'i1');
      assert.equal(events[0].txHash, '0x1');
      assert.equal(events[0].chain, 'base');
      assert.equal(events[0].confirmations, 2);
      assert.equal(typeof events[0].durationMs, 'number');
    });

    it('does not emit settlement_final more than once', () => {
      tracker.trackSettlement('i1', '0x1', 'set_chain', 100);

      const events = [];
      tracker.events.on('settlement_final', (e) => events.push(e));

      tracker.updateConfirmations('i1', 1, 101);
      tracker.updateConfirmations('i1', 2, 102);
      tracker.updateConfirmations('i1', 3, 103);

      assert.equal(events.length, 1);
    });

    it('emits settlement_reorged on reorg', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.updateConfirmations('i1', 5, 1005);

      const events = [];
      tracker.events.on('settlement_reorged', (e) => events.push(e));

      tracker.detectReorg('i1', 998);

      assert.equal(events.length, 1);
      assert.equal(events[0].intentId, 'i1');
      assert.equal(events[0].previousBlock, 1000);
      assert.equal(events[0].newBlock, 998);
      assert.equal(events[0].previousState, 'confirming');
      assert.equal(events[0].chain, 'ethereum');
    });

    it('does not emit settlement_reorged when block is not backwards', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);

      const events = [];
      tracker.events.on('settlement_reorged', (e) => events.push(e));

      tracker.detectReorg('i1', 1001);

      assert.equal(events.length, 0);
    });

    it('emits settlement_failed on markFailed', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);

      const events = [];
      tracker.events.on('settlement_failed', (e) => events.push(e));

      tracker.markFailed('i1', 'tx reverted');

      assert.equal(events.length, 1);
      assert.equal(events[0].intentId, 'i1');
      assert.equal(events[0].reason, 'tx reverted');
      assert.equal(events[0].chain, 'ethereum');
    });

    it('emits both confirmed and final in single update when 1-block chain goes to final', () => {
      tracker.trackSettlement('i1', '0x1', 'arbitrum', 100);

      const confirmed = [];
      const final = [];
      tracker.events.on('settlement_confirmed', (e) => confirmed.push(e));
      tracker.events.on('settlement_final', (e) => final.push(e));

      tracker.updateConfirmations('i1', 1, 101);

      assert.equal(confirmed.length, 1);
      assert.equal(final.length, 1);
    });
  });

  // -----------------------------------------------------------------------
  // getSettlementStatus
  // -----------------------------------------------------------------------

  describe('getSettlementStatus', () => {
    it('returns full status with progress', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      tracker.updateConfirmations('i1', 6, 1006);

      const status = tracker.getSettlementStatus('i1');
      assert.equal(status.intentId, 'i1');
      assert.equal(status.txHash, '0x1');
      assert.equal(status.chain, 'ethereum');
      assert.equal(status.confirmations, 6);
      assert.equal(status.requiredConfirmations, 12);
      assert.equal(status.state, 'confirming');
      assert.equal(status.isFinal, false);
      assert.equal(status.progress, 0.5);
    });

    it('progress is 1 when finalized', () => {
      tracker.trackSettlement('i1', '0x1', 'set_chain', 100);
      tracker.updateConfirmations('i1', 1, 101);

      const status = tracker.getSettlementStatus('i1');
      assert.equal(status.progress, 1);
      assert.equal(status.isFinal, true);
    });

    it('throws for unknown intentId', () => {
      assert.throws(
        () => tracker.getSettlementStatus('nonexistent'),
        /not found/,
      );
    });
  });

  // -----------------------------------------------------------------------
  // markSettled
  // -----------------------------------------------------------------------

  describe('markSettled', () => {
    it('transitions final → settled', () => {
      tracker.trackSettlement('i1', '0x1', 'set_chain', 100);
      tracker.updateConfirmations('i1', 1, 101);
      const result = tracker.markSettled('i1');
      assert.equal(result.state, 'settled');
    });

    it('throws if not in final state', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      assert.throws(
        () => tracker.markSettled('i1'),
        /current state is broadcast, expected final/,
      );
    });

    it('throws for unknown intentId', () => {
      assert.throws(
        () => tracker.markSettled('nonexistent'),
        /not found/,
      );
    });
  });

  // -----------------------------------------------------------------------
  // markFailed
  // -----------------------------------------------------------------------

  describe('markFailed', () => {
    it('transitions to failed state', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);
      const result = tracker.markFailed('i1', 'tx reverted');
      assert.equal(result.state, 'failed');
    });

    it('uses "unknown" reason when none provided', () => {
      tracker.trackSettlement('i1', '0x1', 'ethereum', 1000);

      const events = [];
      tracker.events.on('settlement_failed', (e) => events.push(e));

      tracker.markFailed('i1');
      assert.equal(events[0].reason, 'unknown');
    });

    it('throws for unknown intentId', () => {
      assert.throws(
        () => tracker.markFailed('nonexistent'),
        /not found/,
      );
    });
  });

  // -----------------------------------------------------------------------
  // Exposed constants
  // -----------------------------------------------------------------------

  describe('exposed constants', () => {
    it('exposes CHAIN_FINALITY_REQUIREMENTS', () => {
      assert.equal(tracker.CHAIN_FINALITY_REQUIREMENTS.set_chain, 1);
      assert.equal(tracker.CHAIN_FINALITY_REQUIREMENTS.base, 2);
      assert.equal(tracker.CHAIN_FINALITY_REQUIREMENTS.ethereum, 12);
      assert.equal(tracker.CHAIN_FINALITY_REQUIREMENTS.arbitrum, 1);
      assert.equal(tracker.CHAIN_FINALITY_REQUIREMENTS.solana, 32);
    });

    it('exposes FinalityState enum', () => {
      assert.equal(tracker.FinalityState.BROADCAST, 'broadcast');
      assert.equal(tracker.FinalityState.UNCONFIRMED, 'unconfirmed');
      assert.equal(tracker.FinalityState.CONFIRMING, 'confirming');
      assert.equal(tracker.FinalityState.FINAL, 'final');
      assert.equal(tracker.FinalityState.SETTLED, 'settled');
      assert.equal(tracker.FinalityState.FAILED, 'failed');
      assert.equal(tracker.FinalityState.REORGED, 'reorged');
    });
  });
});
