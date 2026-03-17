/**
 * Tests for cli/src/a2a/dispute-resolver.js
 *
 * Covers: createDisputeResolver — tick(), start(), stop(), getMetrics(),
 * auto-transitions (filed -> evidence_period -> under_review),
 * rule-based arbitration (non_delivery, poor_quality, overcharged,
 * unauthorized, evidence-based), escalation, notifications, escrow actions.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import { createDisputeResolver } from '../../src/a2a/dispute-resolver.js';

// ---------------------------------------------------------------------------
// Constants (mirror the source for timestamp manipulation)
// ---------------------------------------------------------------------------

const EVIDENCE_DEADLINE_MS = 72 * 60 * 60 * 1000; // 72 hours
const AUTO_EVIDENCE_PERIOD_MS = 24 * 60 * 60 * 1000; // 24 hours
const REVIEW_DEADLINE_MS = 7 * 24 * 60 * 60 * 1000; // 7 days

// ---------------------------------------------------------------------------
// Helpers: timestamps in the past
// ---------------------------------------------------------------------------

function hoursAgo(hours) {
  return new Date(Date.now() - hours * 60 * 60 * 1000).toISOString();
}

function daysAgo(days) {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
}

// ---------------------------------------------------------------------------
// Mock store factory
// ---------------------------------------------------------------------------

function createMockStore() {
  const disputes = new Map();
  const evidence = new Map();
  const reputations = new Map();

  return {
    listDisputes: async (filter) => {
      let results = [...disputes.values()];
      if (filter?.status) {
        results = results.filter((d) => d.status === filter.status);
      }
      return results;
    },
    listEvidenceByDispute: async (disputeId) =>
      [...evidence.values()].filter((e) => e.dispute_id === disputeId),
    getReputationScore: (address) => {
      const rep = reputations.get(address);
      return rep || null;
    },

    // internal seeding helpers
    _disputes: disputes,
    _evidence: evidence,
    _reputations: reputations,
  };
}

// ---------------------------------------------------------------------------
// Mock dispute service factory
// ---------------------------------------------------------------------------

function createMockDisputeService() {
  const calls = {
    moveToEvidencePeriod: [],
    moveToReview: [],
    resolveDispute: [],
    escalateDispute: [],
  };

  return {
    moveToEvidencePeriod: async (id) => {
      calls.moveToEvidencePeriod.push(id);
    },
    moveToReview: async (id) => {
      calls.moveToReview.push(id);
    },
    resolveDispute: async (id, params) => {
      calls.resolveDispute.push({ id, params });
      // Build a mock escrow action based on resolution type
      const escrowAction = _buildMockEscrowAction(params.resolutionType, id);
      return { success: true, escrowAction };
    },
    escalateDispute: async (id) => {
      calls.escalateDispute.push(id);
    },
    _calls: calls,
  };
}

function _buildMockEscrowAction(resolutionType, disputeId) {
  switch (resolutionType) {
    case 'full_refund':
      return { action: 'refund', escrowId: `escrow-${disputeId}` };
    case 'partial_refund':
      return { action: 'partial_refund', escrowId: `escrow-${disputeId}` };
    case 'release_to_seller':
      return { action: 'release', escrowId: `escrow-${disputeId}` };
    case 'split':
      return { action: 'split', escrowId: `escrow-${disputeId}` };
    default:
      return null;
  }
}

// ---------------------------------------------------------------------------
// Mock escrow service factory
// ---------------------------------------------------------------------------

function createMockEscrowService() {
  const calls = {
    refundEscrow: [],
    releaseEscrow: [],
  };

  return {
    refundEscrow: async (escrowId) => {
      calls.refundEscrow.push(escrowId);
    },
    releaseEscrow: async (escrowId) => {
      calls.releaseEscrow.push(escrowId);
    },
    _calls: calls,
  };
}

// ---------------------------------------------------------------------------
// Mock notification service factory
// ---------------------------------------------------------------------------

function createMockNotificationService() {
  const calls = [];

  return {
    sendNotification: async (params) => {
      calls.push(params);
    },
    _calls: calls,
  };
}

// ---------------------------------------------------------------------------
// Dispute seeding helpers
// ---------------------------------------------------------------------------

let _disputeCounter = 0;

function seedDispute(store, overrides = {}) {
  _disputeCounter++;
  const id = overrides.id || `dispute-${_disputeCounter}`;
  const dispute = {
    id,
    status: 'filed',
    category: 'non_delivery',
    filed_by: '0xBuyer',
    filed_against: '0xSeller',
    escrow_id: `escrow-${id}`,
    amount_decimal: 100,
    asset: 'USDC',
    created_at: hoursAgo(25), // default: filed > 24h ago
    evidence_deadline: null,
    review_deadline: null,
    ...overrides,
  };
  store._disputes.set(id, dispute);
  return dispute;
}

function seedEvidence(store, overrides = {}) {
  const id = overrides.id || `evidence-${Date.now()}-${Math.random()}`;
  const record = {
    id,
    dispute_id: 'dispute-1',
    submitted_by: '0xBuyer',
    evidence_type: 'text',
    content: 'Some evidence',
    ...overrides,
  };
  store._evidence.set(id, record);
  return record;
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe('createDisputeResolver', () => {
  let store;
  let disputeService;
  let escrowService;
  let notificationService;
  let resolver;

  beforeEach(() => {
    _disputeCounter = 0;
    store = createMockStore();
    disputeService = createMockDisputeService();
    escrowService = createMockEscrowService();
    notificationService = createMockNotificationService();
    resolver = createDisputeResolver(
      store,
      disputeService,
      escrowService,
      notificationService,
      { autoResolveThreshold: 500, intervalMs: 60_000 },
    );
  });

  afterEach(() => {
    if (resolver) resolver.stop();
  });

  // =========================================================================
  // 1. Auto-transition: filed -> evidence_period after 24h
  // =========================================================================

  describe('filed -> evidence_period transition', () => {
    it('transitions disputes filed more than 24h ago', async () => {
      seedDispute(store, {
        id: 'd-old',
        status: 'filed',
        created_at: hoursAgo(25),
      });

      const result = await resolver.tick();

      assert.equal(result.transitions, 1);
      assert.deepStrictEqual(disputeService._calls.moveToEvidencePeriod, ['d-old']);
    });

    it('does NOT transition disputes filed less than 24h ago', async () => {
      seedDispute(store, {
        id: 'd-new',
        status: 'filed',
        created_at: hoursAgo(10),
      });

      const result = await resolver.tick();

      assert.equal(result.transitions, 0);
      assert.equal(disputeService._calls.moveToEvidencePeriod.length, 0);
    });

    it('transitions multiple filed disputes in one tick', async () => {
      seedDispute(store, { id: 'd-1', status: 'filed', created_at: hoursAgo(30) });
      seedDispute(store, { id: 'd-2', status: 'filed', created_at: hoursAgo(48) });
      seedDispute(store, { id: 'd-3', status: 'filed', created_at: hoursAgo(12) }); // too recent

      const result = await resolver.tick();

      assert.equal(result.transitions, 2);
      assert.ok(disputeService._calls.moveToEvidencePeriod.includes('d-1'));
      assert.ok(disputeService._calls.moveToEvidencePeriod.includes('d-2'));
      assert.ok(!disputeService._calls.moveToEvidencePeriod.includes('d-3'));
    });

    it('continues processing when one transition fails', async () => {
      seedDispute(store, { id: 'd-fail', status: 'filed', created_at: hoursAgo(30) });
      seedDispute(store, { id: 'd-ok', status: 'filed', created_at: hoursAgo(30) });

      let callCount = 0;
      disputeService.moveToEvidencePeriod = async (id) => {
        callCount++;
        if (id === 'd-fail') throw new Error('transition error');
        disputeService._calls.moveToEvidencePeriod.push(id);
      };

      const result = await resolver.tick();

      // d-fail threw but d-ok should still have been processed
      assert.equal(callCount, 2);
      assert.equal(result.transitions, 1);
    });
  });

  // =========================================================================
  // 2. Auto-transition: evidence_period -> under_review after evidence deadline
  // =========================================================================

  describe('evidence_period -> under_review transition', () => {
    it('transitions disputes past the evidence deadline', async () => {
      seedDispute(store, {
        id: 'd-ev',
        status: 'evidence_period',
        evidence_deadline: hoursAgo(1), // deadline already passed
      });

      const result = await resolver.tick();

      assert.equal(result.transitions, 1);
      assert.deepStrictEqual(disputeService._calls.moveToReview, ['d-ev']);
    });

    it('does NOT transition disputes before the evidence deadline', async () => {
      const future = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
      seedDispute(store, {
        id: 'd-ev-future',
        status: 'evidence_period',
        evidence_deadline: future,
      });

      const result = await resolver.tick();

      assert.equal(disputeService._calls.moveToReview.length, 0);
    });

    it('does NOT transition when evidence_deadline is null', async () => {
      seedDispute(store, {
        id: 'd-ev-null',
        status: 'evidence_period',
        evidence_deadline: null,
      });

      const result = await resolver.tick();

      assert.equal(disputeService._calls.moveToReview.length, 0);
    });

    it('handles moveToReview failure gracefully', async () => {
      seedDispute(store, {
        id: 'd-ev-fail',
        status: 'evidence_period',
        evidence_deadline: hoursAgo(1),
      });

      disputeService.moveToReview = async () => {
        throw new Error('review transition failed');
      };

      // Should not throw
      const result = await resolver.tick();
      assert.equal(result.transitions, 0);
    });
  });

  // =========================================================================
  // 3. Auto-resolve: non_delivery without delivery proof -> full_refund
  // =========================================================================

  describe('non_delivery without delivery proof', () => {
    it('auto-resolves to full_refund', async () => {
      seedDispute(store, {
        id: 'd-nd',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 200,
      });

      const result = await resolver.tick();

      assert.equal(result.resolutions, 1);
      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.id, 'd-nd');
      assert.equal(call.params.resolutionType, 'full_refund');
      assert.equal(call.params.resolvedBy, 'auto-resolver');
      assert.ok(call.params.note.includes('Non-delivery'));
      assert.ok(call.params.note.includes('no delivery proof'));
    });
  });

  // =========================================================================
  // 4. Auto-resolve: non_delivery with delivery proof -> split
  // =========================================================================

  describe('non_delivery with delivery proof', () => {
    it('auto-resolves to split 50/50', async () => {
      const disputeId = 'd-nd-proof';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 200,
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        id: 'ev-proof',
        dispute_id: disputeId,
        submitted_by: '0xSeller',
        evidence_type: 'delivery_proof',
      });

      const result = await resolver.tick();

      assert.equal(result.resolutions, 1);
      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'split');
      assert.equal(call.params.amount, 100); // 200 / 2
      assert.ok(call.params.note.includes('Split 50/50'));
    });

    it('rounds split amount to 2 decimal places', async () => {
      const disputeId = 'd-nd-odd';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 99.99,
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        dispute_id: disputeId,
        submitted_by: '0xSeller',
        evidence_type: 'delivery_proof',
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.amount, 50); // Math.round(99.99/2 * 100) / 100 = 50.00
    });
  });

  // =========================================================================
  // 5. Auto-resolve: poor_quality with low seller reputation -> full_refund
  // =========================================================================

  describe('poor_quality with low seller reputation', () => {
    it('auto-resolves to full_refund when seller score < 2.5', async () => {
      seedDispute(store, {
        id: 'd-pq',
        status: 'under_review',
        category: 'poor_quality',
        review_deadline: hoursAgo(1),
        amount_decimal: 150,
        filed_against: '0xBadSeller',
      });
      store._reputations.set('0xBadSeller', { average_score: 1.8 });

      const result = await resolver.tick();

      assert.equal(result.resolutions, 1);
      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'full_refund');
      assert.ok(call.params.note.includes('Poor quality'));
      assert.ok(call.params.note.includes('1.8'));
    });

    it('does NOT auto-resolve to full_refund when seller score >= 2.5', async () => {
      seedDispute(store, {
        id: 'd-pq-good',
        status: 'under_review',
        category: 'poor_quality',
        review_deadline: hoursAgo(1),
        amount_decimal: 150,
        filed_against: '0xGoodSeller',
      });
      store._reputations.set('0xGoodSeller', { average_score: 3.5 });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      // Should fall through to evidence-based rules, not poor_quality rule
      assert.notEqual(call.params.resolutionType, undefined);
      // With no evidence from either party, falls to default split
      assert.equal(call.params.resolutionType, 'split');
    });

    it('defaults seller score to 3 when no reputation data', async () => {
      seedDispute(store, {
        id: 'd-pq-norepo',
        status: 'under_review',
        category: 'poor_quality',
        review_deadline: hoursAgo(1),
        amount_decimal: 150,
        filed_against: '0xUnknownSeller',
      });
      // No reputation set => getReputationScore returns null => sellerScore defaults to 3

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      // Score defaults to 3, which is >= 2.5, so poor_quality rule doesn't fire
      assert.notEqual(call.params.resolutionType, 'full_refund');
    });

    it('handles getReputationScore throwing', async () => {
      seedDispute(store, {
        id: 'd-pq-err',
        status: 'under_review',
        category: 'poor_quality',
        review_deadline: hoursAgo(1),
        amount_decimal: 150,
        filed_against: '0xErrorSeller',
      });
      store.getReputationScore = () => {
        throw new Error('reputation service unavailable');
      };

      await resolver.tick();

      // Falls back to sellerScore = 3 (default), poor_quality rule won't fire
      const call = disputeService._calls.resolveDispute[0];
      assert.ok(call); // resolved without throwing
    });
  });

  // =========================================================================
  // 6. Auto-resolve: overcharged -> partial_refund (20%)
  // =========================================================================

  describe('overcharged -> partial_refund', () => {
    it('auto-resolves to 20% partial refund', async () => {
      seedDispute(store, {
        id: 'd-oc',
        status: 'under_review',
        category: 'overcharged',
        review_deadline: hoursAgo(1),
        amount_decimal: 500,
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'partial_refund');
      assert.equal(call.params.amount, 100); // 500 * 0.2
      assert.ok(call.params.note.includes('Overcharge'));
      assert.ok(call.params.note.includes('20%'));
    });

    it('rounds refund amount to 2 decimal places', async () => {
      seedDispute(store, {
        id: 'd-oc-odd',
        status: 'under_review',
        category: 'overcharged',
        review_deadline: hoursAgo(1),
        amount_decimal: 33.33,
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      // Math.round(33.33 * 0.2 * 100) / 100 = Math.round(666.6) / 100 = 6.67
      assert.equal(call.params.amount, 6.67);
    });
  });

  // =========================================================================
  // 7. Auto-resolve: unauthorized -> full_refund
  // =========================================================================

  describe('unauthorized -> full_refund', () => {
    it('always auto-resolves to full_refund', async () => {
      seedDispute(store, {
        id: 'd-unauth',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 300,
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'full_refund');
      assert.ok(call.params.note.includes('Unauthorized'));
    });

    it('refunds even when seller has evidence', async () => {
      const disputeId = 'd-unauth-ev';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 300,
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        dispute_id: disputeId,
        submitted_by: '0xSeller',
        evidence_type: 'receipt',
      });

      await resolver.tick();

      // Unauthorized always wins regardless of evidence
      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'full_refund');
    });
  });

  // =========================================================================
  // 8. Escalates disputes above autoResolveThreshold
  // =========================================================================

  describe('escalation for high-value disputes', () => {
    it('escalates disputes above the threshold', async () => {
      seedDispute(store, {
        id: 'd-big',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 1000, // threshold is 500
      });

      const result = await resolver.tick();

      assert.equal(result.escalations, 1);
      assert.equal(result.resolutions, 0);
      assert.deepStrictEqual(disputeService._calls.escalateDispute, ['d-big']);
      assert.equal(disputeService._calls.resolveDispute.length, 0);
    });

    it('does NOT escalate disputes at or below the threshold', async () => {
      seedDispute(store, {
        id: 'd-exact',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 500, // exactly at threshold
      });

      const result = await resolver.tick();

      assert.equal(result.escalations, 0);
      assert.equal(result.resolutions, 1);
    });

    it('emits escalated event with reason', async () => {
      seedDispute(store, {
        id: 'd-esc',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 750,
      });

      const events = [];
      resolver.on('escalated', (evt) => events.push(evt));

      await resolver.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].disputeId, 'd-esc');
      assert.ok(events[0].reason.includes('750'));
      assert.ok(events[0].reason.includes('500'));
    });

    it('handles escalation failure gracefully', async () => {
      seedDispute(store, {
        id: 'd-esc-fail',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 1000,
      });

      disputeService.escalateDispute = async () => {
        throw new Error('escalation failed');
      };

      // Should not throw
      const result = await resolver.tick();
      assert.equal(result.escalations, 0);
    });

    it('uses default threshold of 1000 when not specified', async () => {
      const defaultResolver = createDisputeResolver(
        store,
        disputeService,
        escrowService,
        notificationService,
      );

      seedDispute(store, {
        id: 'd-def',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 999,
      });

      const result = await defaultResolver.tick();

      // 999 <= 1000 default threshold, should resolve, not escalate
      assert.equal(result.resolutions, 1);
      assert.equal(result.escalations, 0);

      defaultResolver.stop();
    });
  });

  // =========================================================================
  // 9. Both parties have evidence -> split 50/50
  // =========================================================================

  describe('evidence-based: both parties', () => {
    it('splits 50/50 when both parties submit evidence', async () => {
      const disputeId = 'd-both';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
        amount_decimal: 400,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        id: 'ev-buyer',
        dispute_id: disputeId,
        submitted_by: '0xBuyer',
      });
      seedEvidence(store, {
        id: 'ev-seller',
        dispute_id: disputeId,
        submitted_by: '0xSeller',
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'split');
      assert.equal(call.params.amount, 200); // 400 / 2
      assert.ok(call.params.note.includes('Both parties'));
    });
  });

  // =========================================================================
  // 10. Only filer has evidence -> full_refund
  // =========================================================================

  describe('evidence-based: only filer', () => {
    it('full refund when only filing party has evidence', async () => {
      const disputeId = 'd-filer-only';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
        amount_decimal: 250,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        id: 'ev-filer',
        dispute_id: disputeId,
        submitted_by: '0xBuyer',
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'full_refund');
      assert.ok(call.params.note.includes('Only filing party'));
    });
  });

  // =========================================================================
  // 11. Only respondent has evidence -> release_to_seller
  // =========================================================================

  describe('evidence-based: only respondent', () => {
    it('releases to seller when only respondent has evidence', async () => {
      const disputeId = 'd-resp-only';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
        amount_decimal: 250,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        id: 'ev-resp',
        dispute_id: disputeId,
        submitted_by: '0xSeller',
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'release_to_seller');
      assert.ok(call.params.note.includes('Only respondent'));
    });
  });

  // =========================================================================
  // Default: no evidence from either party -> split 50/50
  // =========================================================================

  describe('default: no evidence', () => {
    it('splits 50/50 when neither party has evidence', async () => {
      seedDispute(store, {
        id: 'd-none',
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
        amount_decimal: 180,
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'split');
      assert.equal(call.params.amount, 90);
      assert.ok(call.params.note.includes('No clear evidence'));
    });
  });

  // =========================================================================
  // 12. Sends notifications on transitions
  // =========================================================================

  describe('notifications', () => {
    it('sends notification to both parties on evidence_period transition', async () => {
      seedDispute(store, {
        id: 'd-notif-ep',
        status: 'filed',
        created_at: hoursAgo(30),
        filed_by: '0xAlice',
        filed_against: '0xBob',
        evidence_deadline: hoursAgo(-48), // future
      });

      await resolver.tick();

      const notifs = notificationService._calls;
      assert.equal(notifs.length, 2);

      const recipients = notifs.map((n) => n.recipientAddress);
      assert.ok(recipients.includes('0xAlice'));
      assert.ok(recipients.includes('0xBob'));

      for (const n of notifs) {
        assert.equal(n.eventType, 'dispute.evidence_period');
        assert.equal(n.payload.disputeId, 'd-notif-ep');
      }
    });

    it('sends notification to both parties on resolution', async () => {
      seedDispute(store, {
        id: 'd-notif-res',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
        filed_by: '0xAlice',
        filed_against: '0xBob',
      });

      await resolver.tick();

      const notifs = notificationService._calls;
      assert.equal(notifs.length, 2);

      const recipients = notifs.map((n) => n.recipientAddress);
      assert.ok(recipients.includes('0xAlice'));
      assert.ok(recipients.includes('0xBob'));

      for (const n of notifs) {
        assert.equal(n.eventType, 'dispute.resolved');
        assert.equal(n.payload.disputeId, 'd-notif-res');
        assert.equal(n.payload.resolutionType, 'full_refund');
      }
    });

    it('works without notification service (null)', async () => {
      const resolverNoNotif = createDisputeResolver(
        store,
        disputeService,
        escrowService,
        null,
        { autoResolveThreshold: 500 },
      );

      seedDispute(store, {
        id: 'd-no-notif',
        status: 'filed',
        created_at: hoursAgo(30),
      });

      // Should not throw
      const result = await resolverNoNotif.tick();
      assert.equal(result.transitions, 1);

      resolverNoNotif.stop();
    });

    it('handles notification failure gracefully (best effort)', async () => {
      notificationService.sendNotification = async () => {
        throw new Error('notification service down');
      };

      seedDispute(store, {
        id: 'd-notif-fail',
        status: 'filed',
        created_at: hoursAgo(30),
      });

      // Should not throw despite notification failure
      const result = await resolver.tick();
      assert.equal(result.transitions, 1);
    });
  });

  // =========================================================================
  // 13. Executes escrow actions on resolution
  // =========================================================================

  describe('escrow action execution', () => {
    it('calls refundEscrow for full_refund resolution', async () => {
      seedDispute(store, {
        id: 'd-escrow-refund',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      await resolver.tick();

      assert.equal(escrowService._calls.refundEscrow.length, 1);
      assert.equal(escrowService._calls.refundEscrow[0], 'escrow-d-escrow-refund');
    });

    it('calls releaseEscrow for release_to_seller resolution', async () => {
      const disputeId = 'd-escrow-release';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        dispute_id: disputeId,
        submitted_by: '0xSeller',
      });

      await resolver.tick();

      assert.equal(escrowService._calls.releaseEscrow.length, 1);
      assert.equal(escrowService._calls.releaseEscrow[0], `escrow-${disputeId}`);
    });

    it('calls refundEscrow for partial_refund resolution', async () => {
      seedDispute(store, {
        id: 'd-escrow-partial',
        status: 'under_review',
        category: 'overcharged',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      await resolver.tick();

      assert.equal(escrowService._calls.refundEscrow.length, 1);
      assert.equal(escrowService._calls.refundEscrow[0], 'escrow-d-escrow-partial');
    });

    it('calls refundEscrow for split resolution', async () => {
      const disputeId = 'd-escrow-split';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
        amount_decimal: 200,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      seedEvidence(store, {
        id: 'ev-b',
        dispute_id: disputeId,
        submitted_by: '0xBuyer',
      });
      seedEvidence(store, {
        id: 'ev-s',
        dispute_id: disputeId,
        submitted_by: '0xSeller',
      });

      await resolver.tick();

      assert.equal(escrowService._calls.refundEscrow.length, 1);
      assert.equal(escrowService._calls.refundEscrow[0], `escrow-${disputeId}`);
    });

    it('handles escrow action failure gracefully', async () => {
      seedDispute(store, {
        id: 'd-escrow-fail',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      escrowService.refundEscrow = async () => {
        throw new Error('escrow service unavailable');
      };

      // Should not throw; resolution still counts
      const result = await resolver.tick();
      assert.equal(result.resolutions, 1);
    });

    it('skips escrow action when escrowService is null', async () => {
      const resolverNoEscrow = createDisputeResolver(
        store,
        disputeService,
        null,
        notificationService,
        { autoResolveThreshold: 500 },
      );

      seedDispute(store, {
        id: 'd-no-escrow',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      // Should not throw
      const result = await resolverNoEscrow.tick();
      assert.equal(result.resolutions, 1);

      resolverNoEscrow.stop();
    });

    it('skips escrow action when resolveDispute returns no escrowAction', async () => {
      disputeService.resolveDispute = async (id, params) => {
        disputeService._calls.resolveDispute.push({ id, params });
        return { success: true, escrowAction: null };
      };

      seedDispute(store, {
        id: 'd-no-action',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      const result = await resolver.tick();
      assert.equal(result.resolutions, 1);
      assert.equal(escrowService._calls.refundEscrow.length, 0);
      assert.equal(escrowService._calls.releaseEscrow.length, 0);
    });
  });

  // =========================================================================
  // 14. getMetrics() tracking
  // =========================================================================

  describe('getMetrics()', () => {
    it('returns initial metrics', () => {
      const metrics = resolver.getMetrics();
      assert.equal(metrics.totalTicks, 0);
      assert.equal(metrics.autoTransitions, 0);
      assert.equal(metrics.autoResolutions, 0);
      assert.equal(metrics.autoEscalations, 0);
      assert.equal(metrics.lastTickAt, null);
      assert.equal(metrics.running, false);
    });

    it('updates after a tick with transitions', async () => {
      seedDispute(store, { id: 'd-m1', status: 'filed', created_at: hoursAgo(30) });
      seedDispute(store, { id: 'd-m2', status: 'filed', created_at: hoursAgo(30) });

      await resolver.tick();

      const metrics = resolver.getMetrics();
      assert.equal(metrics.totalTicks, 1);
      assert.equal(metrics.autoTransitions, 2);
      assert.equal(metrics.autoResolutions, 0);
      assert.equal(metrics.autoEscalations, 0);
      assert.ok(metrics.lastTickAt);
    });

    it('updates after a tick with resolutions', async () => {
      seedDispute(store, {
        id: 'd-m3',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 50,
      });

      await resolver.tick();

      const metrics = resolver.getMetrics();
      assert.equal(metrics.totalTicks, 1);
      assert.equal(metrics.autoResolutions, 1);
    });

    it('updates after a tick with escalations', async () => {
      seedDispute(store, {
        id: 'd-m4',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 1000,
      });

      await resolver.tick();

      const metrics = resolver.getMetrics();
      assert.equal(metrics.totalTicks, 1);
      assert.equal(metrics.autoEscalations, 1);
    });

    it('accumulates across multiple ticks', async () => {
      seedDispute(store, { id: 'd-acc1', status: 'filed', created_at: hoursAgo(30) });

      await resolver.tick();

      // Simulate the real store behavior: the dispute moved out of 'filed'
      store._disputes.get('d-acc1').status = 'evidence_period';

      // Now seed a resolution for next tick
      seedDispute(store, {
        id: 'd-acc2',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 50,
      });

      await resolver.tick();

      const metrics = resolver.getMetrics();
      assert.equal(metrics.totalTicks, 2);
      assert.equal(metrics.autoTransitions, 1);
      assert.equal(metrics.autoResolutions, 1);
    });

    it('reflects running state after start()', () => {
      resolver.start();
      const metrics = resolver.getMetrics();
      assert.equal(metrics.running, true);
    });

    it('reflects stopped state after stop()', () => {
      resolver.start();
      resolver.stop();
      const metrics = resolver.getMetrics();
      assert.equal(metrics.running, false);
    });

    it('returns a copy — mutating it does not affect internal state', () => {
      const metrics = resolver.getMetrics();
      metrics.totalTicks = 999;
      assert.equal(resolver.getMetrics().totalTicks, 0);
    });
  });

  // =========================================================================
  // start() / stop() lifecycle
  // =========================================================================

  describe('start() / stop()', () => {
    it('start() is idempotent — calling twice does not create duplicate timers', () => {
      resolver.start();
      resolver.start();
      assert.equal(resolver.getMetrics().running, true);
      resolver.stop();
      assert.equal(resolver.getMetrics().running, false);
    });

    it('stop() is idempotent — calling twice does not throw', () => {
      resolver.start();
      resolver.stop();
      resolver.stop();
      assert.equal(resolver.getMetrics().running, false);
    });

    it('stop() before start() does not throw', () => {
      resolver.stop();
      assert.equal(resolver.getMetrics().running, false);
    });
  });

  // =========================================================================
  // Event emitter
  // =========================================================================

  describe('event emitter', () => {
    it('emits transition events for filed -> evidence_period', async () => {
      seedDispute(store, {
        id: 'd-evt-trans',
        status: 'filed',
        created_at: hoursAgo(30),
      });

      const events = [];
      resolver.on('transition', (evt) => events.push(evt));

      await resolver.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].disputeId, 'd-evt-trans');
      assert.equal(events[0].from, 'filed');
      assert.equal(events[0].to, 'evidence_period');
    });

    it('emits transition events for evidence_period -> under_review', async () => {
      seedDispute(store, {
        id: 'd-evt-rev',
        status: 'evidence_period',
        evidence_deadline: hoursAgo(1),
      });

      const events = [];
      resolver.on('transition', (evt) => events.push(evt));

      await resolver.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].disputeId, 'd-evt-rev');
      assert.equal(events[0].from, 'evidence_period');
      assert.equal(events[0].to, 'under_review');
    });

    it('emits resolved events with resolution details', async () => {
      seedDispute(store, {
        id: 'd-evt-res',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      const events = [];
      resolver.on('resolved', (evt) => events.push(evt));

      await resolver.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].disputeId, 'd-evt-res');
      assert.equal(events[0].resolutionType, 'full_refund');
      assert.ok(events[0].note.includes('Unauthorized'));
    });

    it('supports off() to remove listeners', async () => {
      const events = [];
      const handler = (evt) => events.push(evt);
      resolver.on('transition', handler);
      resolver.off('transition', handler);

      seedDispute(store, {
        id: 'd-evt-off',
        status: 'filed',
        created_at: hoursAgo(30),
      });

      await resolver.tick();

      assert.equal(events.length, 0);
    });
  });

  // =========================================================================
  // Under-review disputes that are not yet due for auto-resolution
  // =========================================================================

  describe('under_review disputes not yet due', () => {
    it('skips disputes with review_deadline in the future', async () => {
      const future = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
      seedDispute(store, {
        id: 'd-future',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: future,
        amount_decimal: 100,
      });

      const result = await resolver.tick();

      assert.equal(result.resolutions, 0);
      assert.equal(result.escalations, 0);
      assert.equal(disputeService._calls.resolveDispute.length, 0);
    });

    it('skips disputes with no review_deadline', async () => {
      seedDispute(store, {
        id: 'd-no-deadline',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: null,
        amount_decimal: 100,
      });

      const result = await resolver.tick();

      assert.equal(result.resolutions, 0);
      assert.equal(result.escalations, 0);
    });
  });

  // =========================================================================
  // Zero-amount disputes
  // =========================================================================

  describe('zero-amount disputes', () => {
    it('handles dispute with amount_decimal of 0', async () => {
      seedDispute(store, {
        id: 'd-zero',
        status: 'under_review',
        category: 'overcharged',
        review_deadline: hoursAgo(1),
        amount_decimal: 0,
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'partial_refund');
      assert.equal(call.params.amount, 0);
    });

    it('handles dispute with no amount_decimal (undefined)', async () => {
      seedDispute(store, {
        id: 'd-undef-amt',
        status: 'under_review',
        category: 'other',
        review_deadline: hoursAgo(1),
      });
      // Remove amount_decimal
      const d = store._disputes.get('d-undef-amt');
      delete d.amount_decimal;

      await resolver.tick();

      // amount_decimal defaults to 0 in _arbitrate, so split amount = 0
      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'split');
      assert.equal(call.params.amount, 0);
    });
  });

  // =========================================================================
  // resolveDispute failure
  // =========================================================================

  describe('resolveDispute failure', () => {
    it('handles resolveDispute throwing gracefully', async () => {
      seedDispute(store, {
        id: 'd-res-fail',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      disputeService.resolveDispute = async () => {
        throw new Error('DB failure');
      };

      // Should not throw
      const result = await resolver.tick();
      assert.equal(result.resolutions, 0);
    });
  });

  // =========================================================================
  // Mixed statuses in a single tick
  // =========================================================================

  describe('mixed statuses in one tick', () => {
    it('processes all status categories in a single tick', async () => {
      // filed -> evidence_period
      seedDispute(store, {
        id: 'd-mix-filed',
        status: 'filed',
        created_at: hoursAgo(30),
      });

      // evidence_period -> under_review
      seedDispute(store, {
        id: 'd-mix-ev',
        status: 'evidence_period',
        evidence_deadline: hoursAgo(1),
      });

      // under_review -> resolve
      seedDispute(store, {
        id: 'd-mix-ur',
        status: 'under_review',
        category: 'unauthorized',
        review_deadline: hoursAgo(1),
        amount_decimal: 100,
      });

      // under_review -> escalate
      seedDispute(store, {
        id: 'd-mix-esc',
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 1000,
      });

      const result = await resolver.tick();

      assert.equal(result.transitions, 2); // filed + evidence_period
      assert.equal(result.resolutions, 1); // unauthorized resolve
      assert.equal(result.escalations, 1); // high-value escalation
    });
  });

  // =========================================================================
  // Rule priority: category rules fire before evidence-based rules
  // =========================================================================

  describe('rule priority', () => {
    it('non_delivery rule fires before evidence-based rules', async () => {
      const disputeId = 'd-priority-nd';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'non_delivery',
        review_deadline: hoursAgo(1),
        amount_decimal: 200,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      // Buyer has evidence, seller has no delivery proof
      seedEvidence(store, {
        dispute_id: disputeId,
        submitted_by: '0xBuyer',
        evidence_type: 'text',
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      // Non-delivery with no delivery_proof wins, even though only filer has evidence
      assert.equal(call.params.resolutionType, 'full_refund');
      assert.ok(call.params.note.includes('Non-delivery'));
    });

    it('overcharged rule fires before evidence-based rules', async () => {
      const disputeId = 'd-priority-oc';
      seedDispute(store, {
        id: disputeId,
        status: 'under_review',
        category: 'overcharged',
        review_deadline: hoursAgo(1),
        amount_decimal: 300,
        filed_by: '0xBuyer',
        filed_against: '0xSeller',
      });
      // Both have evidence, but overcharged rule should fire first
      seedEvidence(store, {
        id: 'ev-oc-b',
        dispute_id: disputeId,
        submitted_by: '0xBuyer',
      });
      seedEvidence(store, {
        id: 'ev-oc-s',
        dispute_id: disputeId,
        submitted_by: '0xSeller',
      });

      await resolver.tick();

      const call = disputeService._calls.resolveDispute[0];
      assert.equal(call.params.resolutionType, 'partial_refund');
      assert.equal(call.params.amount, 60); // 300 * 0.2
    });
  });
});
