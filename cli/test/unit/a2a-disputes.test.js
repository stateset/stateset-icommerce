/**
 * Tests for cli/src/a2a/disputes.js
 *
 * Covers: createDisputeService — fileDispute, submitEvidence, resolveDispute,
 * escalateDispute, moveToEvidencePeriod, moveToReview, getDispute,
 * listDisputes, getDisputeEvidence, formatDispute, formatEvidence.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

import { createDisputeService } from '../../src/a2a/disputes.js';

// ---------------------------------------------------------------------------
// Mock store factory
// ---------------------------------------------------------------------------

function createMockStore() {
  const disputes = new Map();
  const evidence = new Map();
  const escrows = new Map();

  return {
    createDispute: async (record) => {
      disputes.set(record.id, { ...record });
    },
    getDispute: async (id) => disputes.get(id) || null,
    updateDispute: async (id, updates) => {
      const existing = disputes.get(id);
      if (existing) disputes.set(id, { ...existing, ...updates });
    },
    listDisputes: async (filter) => {
      let results = [...disputes.values()];
      if (filter?.status) results = results.filter((d) => d.status === filter.status);
      if (filter?.filed_by) results = results.filter((d) => d.filed_by === filter.filed_by);
      if (filter?.filed_against)
        results = results.filter((d) => d.filed_against === filter.filed_against);
      if (filter?.escrow_id) results = results.filter((d) => d.escrow_id === filter.escrow_id);
      return results;
    },
    createEvidence: async (record) => {
      evidence.set(record.id, { ...record });
    },
    getEvidence: async (id) => evidence.get(id) || null,
    listEvidenceByDispute: async (disputeId) =>
      [...evidence.values()].filter((e) => e.dispute_id === disputeId),
    getEscrow: async (id) => escrows.get(id) || null,
    // internal refs for test seeding
    _disputes: disputes,
    _evidence: evidence,
    _escrows: escrows,
  };
}

// ---------------------------------------------------------------------------
// Helper: seed a default escrow
// ---------------------------------------------------------------------------

function seedEscrow(store, overrides = {}) {
  const escrow = {
    id: 'escrow-1',
    status: 'active',
    buyer_address: '0xBuyer',
    seller_address: '0xSeller',
    amount: 100000000,
    amount_decimal: 100,
    asset: 'USDC',
    network: 'set_chain',
    ...overrides,
  };
  store._escrows.set(escrow.id, escrow);
  return escrow;
}

// ---------------------------------------------------------------------------
// Helper: seed a dispute already in the store
// ---------------------------------------------------------------------------

function seedDispute(store, overrides = {}) {
  const dispute = {
    id: 'dispute-1',
    escrow_id: 'escrow-1',
    status: 'filed',
    filed_by: '0xBuyer',
    filed_against: '0xSeller',
    reason: 'Product not delivered',
    category: 'non_delivery',
    amount: 100000000,
    amount_decimal: 100,
    asset: 'USDC',
    evidence_deadline: new Date(Date.now() + 72 * 3600000).toISOString(),
    review_deadline: new Date(Date.now() + 7 * 86400000).toISOString(),
    resolution_type: null,
    resolution_amount: null,
    resolution_note: null,
    resolved_by: null,
    resolved_at: null,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    ...overrides,
  };
  store._disputes.set(dispute.id, dispute);
  return dispute;
}

// ---------------------------------------------------------------------------
// Default file-dispute params
// ---------------------------------------------------------------------------

const DEFAULT_FILE_PARAMS = {
  escrowId: 'escrow-1',
  filedBy: '0xBuyer',
  filedAgainst: '0xSeller',
  reason: 'Product not delivered',
  category: 'non_delivery',
};

// ===========================================================================
// 1. fileDispute
// ===========================================================================

describe('fileDispute', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    svc = createDisputeService(store);
  });

  // ---- Validation ----

  it('throws when escrowId is missing', async () => {
    await assert.rejects(() => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, escrowId: '' }), {
      message: 'escrowId is required',
    });
  });

  it('throws when filedBy is missing', async () => {
    await assert.rejects(() => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, filedBy: '' }), {
      message: 'filedBy is required',
    });
  });

  it('throws when filedAgainst is missing', async () => {
    await assert.rejects(() => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, filedAgainst: '' }), {
      message: 'filedAgainst is required',
    });
  });

  it('throws when reason is missing', async () => {
    await assert.rejects(() => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, reason: '' }), {
      message: 'reason is required',
    });
  });

  it('throws when category is missing', async () => {
    await assert.rejects(
      () => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, category: '' }),
      (err) => {
        assert.match(err.message, /category must be one of/);
        return true;
      },
    );
  });

  it('throws when category is invalid', async () => {
    await assert.rejects(
      () => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, category: 'bogus' }),
      (err) => {
        assert.match(err.message, /category must be one of/);
        return true;
      },
    );
  });

  it('throws when escrow is not found', async () => {
    await assert.rejects(
      () => svc.fileDispute({ ...DEFAULT_FILE_PARAMS, escrowId: 'nonexistent' }),
      { message: 'Escrow not found: nonexistent' },
    );
  });

  // ---- Happy paths ----

  it('creates a dispute with correct fields from escrow', async () => {
    const result = await svc.fileDispute(DEFAULT_FILE_PARAMS);

    assert.equal(result.success, true);
    assert.ok(result.dispute.id);
    assert.equal(result.dispute.escrowId, 'escrow-1');
    assert.equal(result.dispute.status, 'filed');
    assert.equal(result.dispute.filedBy, '0xBuyer');
    assert.equal(result.dispute.filedAgainst, '0xSeller');
    assert.equal(result.dispute.reason, 'Product not delivered');
    assert.equal(result.dispute.category, 'non_delivery');
    assert.equal(result.dispute.amount, 100); // amount_decimal
    assert.equal(result.dispute.asset, 'USDC');
  });

  it('sets evidence and review deadlines', async () => {
    const before = Date.now();
    const result = await svc.fileDispute(DEFAULT_FILE_PARAMS);
    const after = Date.now();

    const evidenceDeadline = new Date(result.dispute.evidenceDeadline).getTime();
    const reviewDeadline = new Date(result.dispute.reviewDeadline).getTime();

    // evidence deadline should be ~72 hours from now
    const evidenceMs = 72 * 60 * 60 * 1000;
    assert.ok(evidenceDeadline >= before + evidenceMs - 1000);
    assert.ok(evidenceDeadline <= after + evidenceMs + 1000);

    // review deadline should be ~7 days from now
    const reviewMs = 7 * 24 * 60 * 60 * 1000;
    assert.ok(reviewDeadline >= before + reviewMs - 1000);
    assert.ok(reviewDeadline <= after + reviewMs + 1000);
  });

  it('sets resolution fields to null initially', async () => {
    const result = await svc.fileDispute(DEFAULT_FILE_PARAMS);

    assert.equal(result.dispute.resolutionType, null);
    assert.equal(result.dispute.resolutionAmount, null);
    assert.equal(result.dispute.resolutionNote, null);
    assert.equal(result.dispute.resolvedBy, null);
    assert.equal(result.dispute.resolvedAt, null);
  });

  it('persists the dispute in the store', async () => {
    const result = await svc.fileDispute(DEFAULT_FILE_PARAMS);
    const stored = store._disputes.get(result.dispute.id);

    assert.ok(stored);
    assert.equal(stored.status, 'filed');
    assert.equal(stored.escrow_id, 'escrow-1');
  });

  it('accepts all valid categories', async () => {
    const categories = [
      'non_delivery',
      'poor_quality',
      'not_as_described',
      'overcharged',
      'unauthorized',
      'other',
    ];
    for (const category of categories) {
      const s = createMockStore();
      seedEscrow(s);
      const service = createDisputeService(s);
      const result = await service.fileDispute({ ...DEFAULT_FILE_PARAMS, category });
      assert.equal(result.dispute.category, category);
    }
  });

  // ---- Initial evidence ----

  it('creates evidence records when initial evidence is provided', async () => {
    const result = await svc.fileDispute({
      ...DEFAULT_FILE_PARAMS,
      evidence: [
        {
          evidenceType: 'screenshot',
          title: 'Order confirmation',
          description: 'Shows delivery date',
          content: 'base64-image-data',
        },
        {
          evidenceType: 'communication',
          title: 'Chat with seller',
          content: 'transcript of chat',
        },
      ],
    });

    const evidenceRecords = [...store._evidence.values()];
    assert.equal(evidenceRecords.length, 2);
    assert.equal(evidenceRecords[0].dispute_id, result.dispute.id);
    assert.equal(evidenceRecords[0].evidence_type, 'screenshot');
    assert.equal(evidenceRecords[0].submitted_by, '0xBuyer');
    assert.equal(evidenceRecords[1].evidence_type, 'communication');
  });

  it('uses defaults for missing evidence fields', async () => {
    await svc.fileDispute({
      ...DEFAULT_FILE_PARAMS,
      evidence: [{ content: 'some data' }],
    });

    const evidenceRecords = [...store._evidence.values()];
    assert.equal(evidenceRecords.length, 1);
    assert.equal(evidenceRecords[0].evidence_type, 'other');
    assert.equal(evidenceRecords[0].title, 'Initial evidence');
    assert.equal(evidenceRecords[0].description, null);
  });

  it('skips evidence creation when evidence array is empty', async () => {
    await svc.fileDispute({ ...DEFAULT_FILE_PARAMS, evidence: [] });
    assert.equal(store._evidence.size, 0);
  });

  it('skips evidence creation when evidence is not provided', async () => {
    await svc.fileDispute(DEFAULT_FILE_PARAMS);
    assert.equal(store._evidence.size, 0);
  });
});

// ===========================================================================
// 2. submitEvidence
// ===========================================================================

describe('submitEvidence', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store);
    svc = createDisputeService(store);
  });

  const DEFAULT_EVIDENCE = {
    submittedBy: '0xBuyer',
    evidenceType: 'screenshot',
    title: 'Proof of non-delivery',
    description: 'Tracking shows no delivery',
    content: 'base64-screenshot-data',
  };

  // ---- Validation ----

  it('throws when disputeId is missing', async () => {
    await assert.rejects(() => svc.submitEvidence('', DEFAULT_EVIDENCE), {
      message: 'disputeId is required',
    });
  });

  it('throws when submittedBy is missing', async () => {
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', { ...DEFAULT_EVIDENCE, submittedBy: '' }),
      { message: 'submittedBy is required' },
    );
  });

  it('throws when evidenceType is missing', async () => {
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', { ...DEFAULT_EVIDENCE, evidenceType: '' }),
      (err) => {
        assert.match(err.message, /evidenceType must be one of/);
        return true;
      },
    );
  });

  it('throws when evidenceType is invalid', async () => {
    await assert.rejects(
      () =>
        svc.submitEvidence('dispute-1', {
          ...DEFAULT_EVIDENCE,
          evidenceType: 'invalid_type',
        }),
      (err) => {
        assert.match(err.message, /evidenceType must be one of/);
        return true;
      },
    );
  });

  it('throws when title is missing', async () => {
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', { ...DEFAULT_EVIDENCE, title: '' }),
      { message: 'title is required' },
    );
  });

  it('throws when content is missing', async () => {
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', { ...DEFAULT_EVIDENCE, content: '' }),
      { message: 'content is required' },
    );
  });

  it('throws when dispute is not found', async () => {
    await assert.rejects(() => svc.submitEvidence('nonexistent', DEFAULT_EVIDENCE), {
      message: 'Dispute not found: nonexistent',
    });
  });

  // ---- Status restrictions ----

  it('allows evidence when status is filed', async () => {
    const result = await svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE);
    assert.equal(result.success, true);
  });

  it('allows evidence when status is evidence_period', async () => {
    store._disputes.get('dispute-1').status = 'evidence_period';
    const result = await svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE);
    assert.equal(result.success, true);
  });

  it('rejects evidence when status is under_review', async () => {
    store._disputes.get('dispute-1').status = 'under_review';
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE),
      (err) => {
        assert.match(err.message, /Cannot submit evidence when dispute status is: under_review/);
        return true;
      },
    );
  });

  it('rejects evidence when status is resolved', async () => {
    store._disputes.get('dispute-1').status = 'resolved';
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE),
      (err) => {
        assert.match(err.message, /Cannot submit evidence/);
        return true;
      },
    );
  });

  it('rejects evidence when status is escalated', async () => {
    store._disputes.get('dispute-1').status = 'escalated';
    await assert.rejects(
      () => svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE),
      (err) => {
        assert.match(err.message, /Cannot submit evidence/);
        return true;
      },
    );
  });

  // ---- Happy path ----

  it('creates evidence with SHA-256 content hash', async () => {
    const result = await svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE);

    const expectedHash = createHash('sha256').update('base64-screenshot-data').digest('hex');

    assert.equal(result.success, true);
    assert.equal(result.evidence.contentHash, expectedHash);
  });

  it('returns formatted evidence with all fields', async () => {
    const result = await svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE);

    assert.ok(result.evidence.id);
    assert.equal(result.evidence.disputeId, 'dispute-1');
    assert.equal(result.evidence.submittedBy, '0xBuyer');
    assert.equal(result.evidence.evidenceType, 'screenshot');
    assert.equal(result.evidence.title, 'Proof of non-delivery');
    assert.equal(result.evidence.description, 'Tracking shows no delivery');
    assert.ok(result.evidence.contentHash);
    assert.ok(result.evidence.createdAt);
  });

  it('persists evidence in the store', async () => {
    await svc.submitEvidence('dispute-1', DEFAULT_EVIDENCE);
    const records = [...store._evidence.values()];
    assert.equal(records.length, 1);
    assert.equal(records[0].dispute_id, 'dispute-1');
    assert.equal(records[0].content, 'base64-screenshot-data');
  });

  it('sets description to null when not provided', async () => {
    const { description, ...noDesc } = DEFAULT_EVIDENCE;
    const result = await svc.submitEvidence('dispute-1', {
      ...noDesc,
      description: undefined,
    });
    assert.equal(result.evidence.description, null);
  });

  it('accepts all valid evidence types', async () => {
    const types = ['screenshot', 'transaction_log', 'communication', 'delivery_proof', 'other'];
    for (const evidenceType of types) {
      const s = createMockStore();
      seedEscrow(s);
      seedDispute(s);
      const service = createDisputeService(s);
      const result = await service.submitEvidence('dispute-1', {
        ...DEFAULT_EVIDENCE,
        evidenceType,
      });
      assert.equal(result.evidence.evidenceType, evidenceType);
    }
  });
});

// ===========================================================================
// 3. resolveDispute
// ===========================================================================

describe('resolveDispute', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store, { status: 'under_review' });
    svc = createDisputeService(store);
  });

  // ---- Validation ----

  it('throws when disputeId is missing', async () => {
    await assert.rejects(
      () => svc.resolveDispute('', { resolutionType: 'full_refund', resolvedBy: 'arb' }),
      { message: 'disputeId is required' },
    );
  });

  it('throws when resolutionType is missing', async () => {
    await assert.rejects(
      () => svc.resolveDispute('dispute-1', { resolutionType: '', resolvedBy: 'arb' }),
      (err) => {
        assert.match(err.message, /resolutionType must be one of/);
        return true;
      },
    );
  });

  it('throws when resolutionType is invalid', async () => {
    await assert.rejects(
      () =>
        svc.resolveDispute('dispute-1', {
          resolutionType: 'magic',
          resolvedBy: 'arb',
        }),
      (err) => {
        assert.match(err.message, /resolutionType must be one of/);
        return true;
      },
    );
  });

  it('throws when resolvedBy is missing', async () => {
    await assert.rejects(
      () =>
        svc.resolveDispute('dispute-1', {
          resolutionType: 'full_refund',
          resolvedBy: '',
        }),
      { message: 'resolvedBy is required' },
    );
  });

  it('throws when dispute not found', async () => {
    await assert.rejects(
      () =>
        svc.resolveDispute('nonexistent', {
          resolutionType: 'full_refund',
          resolvedBy: 'arb',
        }),
      { message: 'Dispute not found: nonexistent' },
    );
  });

  it('throws when dispute status does not allow resolution', async () => {
    store._disputes.get('dispute-1').status = 'resolved';
    await assert.rejects(
      () =>
        svc.resolveDispute('dispute-1', {
          resolutionType: 'full_refund',
          resolvedBy: 'arb',
        }),
      (err) => {
        assert.match(err.message, /Cannot resolve dispute in status: resolved/);
        return true;
      },
    );
  });

  it('throws when dispute status is escalated', async () => {
    store._disputes.get('dispute-1').status = 'escalated';
    await assert.rejects(
      () =>
        svc.resolveDispute('dispute-1', {
          resolutionType: 'full_refund',
          resolvedBy: 'arb',
        }),
      (err) => {
        assert.match(err.message, /Cannot resolve dispute in status/);
        return true;
      },
    );
  });

  it('throws when partial_refund has no amount', async () => {
    await assert.rejects(
      () =>
        svc.resolveDispute('dispute-1', {
          resolutionType: 'partial_refund',
          resolvedBy: 'arb',
        }),
      { message: 'amount is required for partial_refund resolution' },
    );
  });

  it('throws when split has no amount', async () => {
    await assert.rejects(
      () =>
        svc.resolveDispute('dispute-1', {
          resolutionType: 'split',
          resolvedBy: 'arb',
        }),
      { message: 'amount is required for split resolution (buyer share)' },
    );
  });

  // ---- Allowed statuses for resolution ----

  it('allows resolution from filed status', async () => {
    store._disputes.get('dispute-1').status = 'filed';
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arb',
    });
    assert.equal(result.success, true);
  });

  it('allows resolution from evidence_period status', async () => {
    store._disputes.get('dispute-1').status = 'evidence_period';
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arb',
    });
    assert.equal(result.success, true);
  });

  it('allows resolution from under_review status', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arb',
    });
    assert.equal(result.success, true);
  });

  // ---- Resolution types + escrow actions ----

  it('resolves with full_refund — refund action to buyer', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arbitrator-1',
    });

    assert.equal(result.success, true);
    assert.equal(result.dispute.status, 'resolved');
    assert.equal(result.dispute.resolutionType, 'full_refund');
    assert.equal(result.dispute.resolvedBy, 'arbitrator-1');
    assert.ok(result.dispute.resolvedAt);

    // escrow action
    assert.equal(result.escrowAction.action, 'refund');
    assert.equal(result.escrowAction.escrowId, 'escrow-1');
    assert.equal(result.escrowAction.amount, 100);
    assert.equal(result.escrowAction.asset, 'USDC');
    assert.equal(result.escrowAction.to, '0xBuyer');
  });

  it('resolves with partial_refund — split refund/release', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'partial_refund',
      amount: 40,
      resolvedBy: 'arb',
    });

    assert.equal(result.escrowAction.action, 'partial_refund');
    assert.equal(result.escrowAction.refundAmount, 40);
    assert.equal(result.escrowAction.releaseAmount, 60);
    assert.equal(result.escrowAction.refundTo, '0xBuyer');
    assert.equal(result.escrowAction.releaseTo, '0xSeller');
    assert.equal(result.escrowAction.asset, 'USDC');
  });

  it('resolves with release_to_seller — release to seller', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'release_to_seller',
      resolvedBy: 'arb',
    });

    assert.equal(result.escrowAction.action, 'release');
    assert.equal(result.escrowAction.amount, 100);
    assert.equal(result.escrowAction.to, '0xSeller');
  });

  it('resolves with split — buyer and seller shares', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'split',
      amount: 55,
      resolvedBy: 'arb',
    });

    assert.equal(result.escrowAction.action, 'split');
    assert.equal(result.escrowAction.buyerAmount, 55);
    assert.equal(result.escrowAction.sellerAmount, 45);
    assert.equal(result.escrowAction.buyer, '0xBuyer');
    assert.equal(result.escrowAction.seller, '0xSeller');
  });

  it('resolves with escalated — hold action', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'escalated',
      resolvedBy: 'arb',
    });

    assert.equal(result.escrowAction.action, 'hold');
    assert.equal(result.escrowAction.escrowId, 'escrow-1');
    assert.equal(result.escrowAction.note, 'Funds held pending escalation review');
  });

  it('stores the resolution note when provided', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      note: 'Seller failed to ship',
      resolvedBy: 'arb',
    });

    assert.equal(result.dispute.resolutionNote, 'Seller failed to ship');
  });

  it('sets resolutionNote to null when no note', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arb',
    });

    assert.equal(result.dispute.resolutionNote, null);
  });

  it('stores resolution amount for partial_refund', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'partial_refund',
      amount: 30,
      resolvedBy: 'arb',
    });

    assert.equal(result.dispute.resolutionAmount, 30);
  });

  it('sets resolution amount to null for full_refund', async () => {
    const result = await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arb',
    });

    assert.equal(result.dispute.resolutionAmount, null);
  });

  it('updates the dispute status to resolved in the store', async () => {
    await svc.resolveDispute('dispute-1', {
      resolutionType: 'full_refund',
      resolvedBy: 'arb',
    });

    const stored = store._disputes.get('dispute-1');
    assert.equal(stored.status, 'resolved');
    assert.equal(stored.resolved_by, 'arb');
    assert.ok(stored.resolved_at);
  });
});

// ===========================================================================
// 4. escalateDispute
// ===========================================================================

describe('escalateDispute', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store, { status: 'under_review' });
    svc = createDisputeService(store);
  });

  it('escalates from under_review', async () => {
    const result = await svc.escalateDispute('dispute-1');

    assert.equal(result.success, true);
    assert.equal(result.escalated, true);
    assert.equal(result.dispute.status, 'escalated');
  });

  it('throws when disputeId is missing', async () => {
    await assert.rejects(() => svc.escalateDispute(''), { message: 'disputeId is required' });
  });

  it('throws when dispute not found', async () => {
    await assert.rejects(() => svc.escalateDispute('nonexistent'), {
      message: 'Dispute not found: nonexistent',
    });
  });

  it('rejects escalation from filed status', async () => {
    store._disputes.get('dispute-1').status = 'filed';
    await assert.rejects(
      () => svc.escalateDispute('dispute-1'),
      (err) => {
        assert.match(err.message, /Cannot escalate dispute in status: filed/);
        return true;
      },
    );
  });

  it('rejects escalation from evidence_period status', async () => {
    store._disputes.get('dispute-1').status = 'evidence_period';
    await assert.rejects(
      () => svc.escalateDispute('dispute-1'),
      (err) => {
        assert.match(err.message, /Cannot escalate dispute in status: evidence_period/);
        return true;
      },
    );
  });

  it('rejects escalation from resolved status', async () => {
    store._disputes.get('dispute-1').status = 'resolved';
    await assert.rejects(
      () => svc.escalateDispute('dispute-1'),
      (err) => {
        assert.match(err.message, /Cannot escalate dispute in status: resolved/);
        return true;
      },
    );
  });

  it('rejects escalation from already-escalated status', async () => {
    store._disputes.get('dispute-1').status = 'escalated';
    await assert.rejects(
      () => svc.escalateDispute('dispute-1'),
      (err) => {
        assert.match(err.message, /Cannot escalate dispute in status: escalated/);
        return true;
      },
    );
  });

  it('persists escalated status in the store', async () => {
    await svc.escalateDispute('dispute-1');
    const stored = store._disputes.get('dispute-1');
    assert.equal(stored.status, 'escalated');
    assert.ok(stored.updated_at);
  });
});

// ===========================================================================
// 5. moveToEvidencePeriod
// ===========================================================================

describe('moveToEvidencePeriod', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store, { status: 'filed' });
    svc = createDisputeService(store);
  });

  it('transitions from filed to evidence_period', async () => {
    const result = await svc.moveToEvidencePeriod('dispute-1');

    assert.equal(result.success, true);
    assert.equal(result.dispute.status, 'evidence_period');
  });

  it('throws when disputeId is missing', async () => {
    await assert.rejects(() => svc.moveToEvidencePeriod(''), { message: 'disputeId is required' });
  });

  it('throws when dispute not found', async () => {
    await assert.rejects(() => svc.moveToEvidencePeriod('nonexistent'), {
      message: 'Dispute not found: nonexistent',
    });
  });

  it('rejects when dispute is in evidence_period already', async () => {
    store._disputes.get('dispute-1').status = 'evidence_period';
    await assert.rejects(
      () => svc.moveToEvidencePeriod('dispute-1'),
      (err) => {
        assert.match(
          err.message,
          /Cannot transition to evidence_period: dispute is in status evidence_period, expected filed/,
        );
        return true;
      },
    );
  });

  it('rejects when dispute is under_review', async () => {
    store._disputes.get('dispute-1').status = 'under_review';
    await assert.rejects(
      () => svc.moveToEvidencePeriod('dispute-1'),
      (err) => {
        assert.match(err.message, /expected filed/);
        return true;
      },
    );
  });

  it('rejects when dispute is resolved', async () => {
    store._disputes.get('dispute-1').status = 'resolved';
    await assert.rejects(
      () => svc.moveToEvidencePeriod('dispute-1'),
      (err) => {
        assert.match(err.message, /expected filed/);
        return true;
      },
    );
  });

  it('updates stored dispute status', async () => {
    await svc.moveToEvidencePeriod('dispute-1');
    const stored = store._disputes.get('dispute-1');
    assert.equal(stored.status, 'evidence_period');
  });
});

// ===========================================================================
// 6. moveToReview
// ===========================================================================

describe('moveToReview', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store, { status: 'evidence_period' });
    svc = createDisputeService(store);
  });

  it('transitions from evidence_period to under_review', async () => {
    const result = await svc.moveToReview('dispute-1');

    assert.equal(result.success, true);
    assert.equal(result.dispute.status, 'under_review');
  });

  it('throws when disputeId is missing', async () => {
    await assert.rejects(() => svc.moveToReview(''), { message: 'disputeId is required' });
  });

  it('throws when dispute not found', async () => {
    await assert.rejects(() => svc.moveToReview('nonexistent'), {
      message: 'Dispute not found: nonexistent',
    });
  });

  it('rejects when dispute is filed (not evidence_period)', async () => {
    store._disputes.get('dispute-1').status = 'filed';
    await assert.rejects(
      () => svc.moveToReview('dispute-1'),
      (err) => {
        assert.match(
          err.message,
          /Cannot transition to under_review: dispute is in status filed, expected evidence_period/,
        );
        return true;
      },
    );
  });

  it('rejects when dispute is already under_review', async () => {
    store._disputes.get('dispute-1').status = 'under_review';
    await assert.rejects(
      () => svc.moveToReview('dispute-1'),
      (err) => {
        assert.match(err.message, /expected evidence_period/);
        return true;
      },
    );
  });

  it('rejects when dispute is resolved', async () => {
    store._disputes.get('dispute-1').status = 'resolved';
    await assert.rejects(
      () => svc.moveToReview('dispute-1'),
      (err) => {
        assert.match(err.message, /expected evidence_period/);
        return true;
      },
    );
  });

  it('updates stored dispute status', async () => {
    await svc.moveToReview('dispute-1');
    const stored = store._disputes.get('dispute-1');
    assert.equal(stored.status, 'under_review');
  });
});

// ===========================================================================
// 7. getDispute
// ===========================================================================

describe('getDispute', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store);
    svc = createDisputeService(store);
  });

  it('returns formatted dispute with evidence count zero', async () => {
    const result = await svc.getDispute('dispute-1');

    assert.equal(result.success, true);
    assert.equal(result.dispute.id, 'dispute-1');
    assert.equal(result.dispute.evidenceCount, 0);
    assert.equal(result.dispute.status, 'filed');
    assert.equal(result.dispute.filedBy, '0xBuyer');
  });

  it('includes correct evidence count when evidence exists', async () => {
    // Seed some evidence
    store._evidence.set('ev-1', { id: 'ev-1', dispute_id: 'dispute-1' });
    store._evidence.set('ev-2', { id: 'ev-2', dispute_id: 'dispute-1' });
    store._evidence.set('ev-3', { id: 'ev-3', dispute_id: 'other-dispute' });

    const result = await svc.getDispute('dispute-1');
    assert.equal(result.dispute.evidenceCount, 2);
  });

  it('throws when disputeId is missing', async () => {
    await assert.rejects(() => svc.getDispute(''), { message: 'disputeId is required' });
  });

  it('throws when dispute not found', async () => {
    await assert.rejects(() => svc.getDispute('nonexistent'), {
      message: 'Dispute not found: nonexistent',
    });
  });

  it('returns all formatted fields', async () => {
    const result = await svc.getDispute('dispute-1');
    const d = result.dispute;

    // Check all expected keys exist (camelCase formatted)
    const expectedKeys = [
      'id',
      'escrowId',
      'status',
      'filedBy',
      'filedAgainst',
      'reason',
      'category',
      'amount',
      'asset',
      'evidenceDeadline',
      'reviewDeadline',
      'resolutionType',
      'resolutionAmount',
      'resolutionNote',
      'resolvedBy',
      'resolvedAt',
      'createdAt',
      'updatedAt',
      'evidenceCount',
    ];
    for (const key of expectedKeys) {
      assert.ok(key in d, `missing key: ${key}`);
    }
  });
});

// ===========================================================================
// 8. listDisputes
// ===========================================================================

describe('listDisputes', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store, { id: 'd-1', status: 'filed', filed_by: '0xBuyer' });
    seedDispute(store, { id: 'd-2', status: 'under_review', filed_by: '0xBuyer' });
    seedDispute(store, { id: 'd-3', status: 'filed', filed_by: '0xOther' });
    svc = createDisputeService(store);
  });

  it('returns all disputes with no filter', async () => {
    const results = await svc.listDisputes();
    assert.equal(results.length, 3);
  });

  it('filters by status', async () => {
    const results = await svc.listDisputes({ status: 'filed' });
    assert.equal(results.length, 2);
    for (const d of results) {
      assert.equal(d.status, 'filed');
    }
  });

  it('filters by filed_by', async () => {
    const results = await svc.listDisputes({ filed_by: '0xBuyer' });
    assert.equal(results.length, 2);
    for (const d of results) {
      assert.equal(d.filedBy, '0xBuyer');
    }
  });

  it('returns empty array when no disputes match', async () => {
    const results = await svc.listDisputes({ status: 'escalated' });
    assert.equal(results.length, 0);
  });

  it('returns formatted disputes (camelCase keys)', async () => {
    const results = await svc.listDisputes();
    assert.ok(results[0].escrowId); // not escrow_id
    assert.ok(results[0].filedBy); // not filed_by
  });
});

// ===========================================================================
// 9. getDisputeEvidence
// ===========================================================================

describe('getDisputeEvidence', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    seedDispute(store);
    svc = createDisputeService(store);
  });

  it('returns empty array when no evidence exists', async () => {
    const results = await svc.getDisputeEvidence('dispute-1');
    assert.deepEqual(results, []);
  });

  it('returns formatted evidence records', async () => {
    const contentHash = createHash('sha256').update('data1').digest('hex');
    store._evidence.set('ev-1', {
      id: 'ev-1',
      dispute_id: 'dispute-1',
      submitted_by: '0xBuyer',
      evidence_type: 'screenshot',
      title: 'Screenshot 1',
      description: 'Desc',
      content: 'data1',
      content_hash: contentHash,
      created_at: new Date().toISOString(),
    });

    const results = await svc.getDisputeEvidence('dispute-1');
    assert.equal(results.length, 1);
    assert.equal(results[0].id, 'ev-1');
    assert.equal(results[0].disputeId, 'dispute-1');
    assert.equal(results[0].submittedBy, '0xBuyer');
    assert.equal(results[0].evidenceType, 'screenshot');
    assert.equal(results[0].title, 'Screenshot 1');
    assert.equal(results[0].description, 'Desc');
    assert.equal(results[0].contentHash, contentHash);
    assert.ok(results[0].createdAt);
  });

  it('only returns evidence for the specified dispute', async () => {
    store._evidence.set('ev-1', { id: 'ev-1', dispute_id: 'dispute-1' });
    store._evidence.set('ev-2', { id: 'ev-2', dispute_id: 'dispute-1' });
    store._evidence.set('ev-3', { id: 'ev-3', dispute_id: 'other-dispute' });

    const results = await svc.getDisputeEvidence('dispute-1');
    assert.equal(results.length, 2);
  });

  it('throws when disputeId is missing', async () => {
    await assert.rejects(() => svc.getDisputeEvidence(''), { message: 'disputeId is required' });
  });

  it('throws when dispute not found', async () => {
    await assert.rejects(() => svc.getDisputeEvidence('nonexistent'), {
      message: 'Dispute not found: nonexistent',
    });
  });
});

// ===========================================================================
// 10. Full state machine workflow
// ===========================================================================

describe('full dispute lifecycle', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    seedEscrow(store);
    svc = createDisputeService(store);
  });

  it('filed -> evidence_period -> under_review -> resolved', async () => {
    // Step 1: File
    const filed = await svc.fileDispute(DEFAULT_FILE_PARAMS);
    const disputeId = filed.dispute.id;
    assert.equal(filed.dispute.status, 'filed');

    // Step 2: Move to evidence period
    const ep = await svc.moveToEvidencePeriod(disputeId);
    assert.equal(ep.dispute.status, 'evidence_period');

    // Step 3: Submit evidence during evidence period
    const ev = await svc.submitEvidence(disputeId, {
      submittedBy: '0xBuyer',
      evidenceType: 'transaction_log',
      title: 'Payment proof',
      content: 'tx-hash-123',
    });
    assert.equal(ev.success, true);

    // Step 4: Move to review
    const review = await svc.moveToReview(disputeId);
    assert.equal(review.dispute.status, 'under_review');

    // Step 5: Resolve
    const resolved = await svc.resolveDispute(disputeId, {
      resolutionType: 'full_refund',
      note: 'Seller did not deliver',
      resolvedBy: 'arbitrator-bot',
    });
    assert.equal(resolved.dispute.status, 'resolved');
    assert.equal(resolved.escrowAction.action, 'refund');
  });

  it('filed -> evidence_period -> under_review -> escalated', async () => {
    const filed = await svc.fileDispute(DEFAULT_FILE_PARAMS);
    const disputeId = filed.dispute.id;

    await svc.moveToEvidencePeriod(disputeId);
    await svc.moveToReview(disputeId);

    const escalated = await svc.escalateDispute(disputeId);
    assert.equal(escalated.dispute.status, 'escalated');
    assert.equal(escalated.escalated, true);
  });

  it('cannot skip states in the lifecycle', async () => {
    const filed = await svc.fileDispute(DEFAULT_FILE_PARAMS);
    const disputeId = filed.dispute.id;

    // Cannot go straight from filed to under_review
    await assert.rejects(
      () => svc.moveToReview(disputeId),
      (err) => {
        assert.match(err.message, /expected evidence_period/);
        return true;
      },
    );

    // Cannot escalate from filed
    await assert.rejects(
      () => svc.escalateDispute(disputeId),
      (err) => {
        assert.match(err.message, /Must be under_review/);
        return true;
      },
    );
  });

  it('evidence submission is blocked after review starts', async () => {
    const filed = await svc.fileDispute(DEFAULT_FILE_PARAMS);
    const disputeId = filed.dispute.id;

    await svc.moveToEvidencePeriod(disputeId);
    await svc.moveToReview(disputeId);

    await assert.rejects(
      () =>
        svc.submitEvidence(disputeId, {
          submittedBy: '0xBuyer',
          evidenceType: 'screenshot',
          title: 'Late evidence',
          content: 'data',
        }),
      (err) => {
        assert.match(err.message, /Cannot submit evidence/);
        return true;
      },
    );
  });
});

// ===========================================================================
// 11. formatDispute / formatEvidence (exposed helpers)
// ===========================================================================

describe('formatDispute', () => {
  let svc;

  beforeEach(() => {
    const store = createMockStore();
    svc = createDisputeService(store);
  });

  it('maps snake_case store fields to camelCase', () => {
    const raw = {
      id: 'test-id',
      escrow_id: 'esc-1',
      status: 'filed',
      filed_by: '0xA',
      filed_against: '0xB',
      reason: 'test',
      category: 'other',
      amount_decimal: 50,
      asset: 'USDC',
      evidence_deadline: '2026-01-01T00:00:00.000Z',
      review_deadline: '2026-01-08T00:00:00.000Z',
      resolution_type: null,
      resolution_amount: null,
      resolution_note: null,
      resolved_by: null,
      resolved_at: null,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
    };

    const formatted = svc.formatDispute(raw);

    assert.equal(formatted.id, 'test-id');
    assert.equal(formatted.escrowId, 'esc-1');
    assert.equal(formatted.filedBy, '0xA');
    assert.equal(formatted.filedAgainst, '0xB');
    assert.equal(formatted.amount, 50); // uses amount_decimal
    assert.equal(formatted.evidenceDeadline, '2026-01-01T00:00:00.000Z');
    assert.equal(formatted.reviewDeadline, '2026-01-08T00:00:00.000Z');
    assert.equal(formatted.createdAt, '2026-01-01T00:00:00.000Z');
  });
});

describe('formatEvidence', () => {
  let svc;

  beforeEach(() => {
    const store = createMockStore();
    svc = createDisputeService(store);
  });

  it('maps snake_case store fields to camelCase', () => {
    const raw = {
      id: 'ev-1',
      dispute_id: 'disp-1',
      submitted_by: '0xA',
      evidence_type: 'screenshot',
      title: 'Test',
      description: 'Desc',
      content_hash: 'abc123',
      created_at: '2026-01-01T00:00:00.000Z',
    };

    const formatted = svc.formatEvidence(raw);

    assert.equal(formatted.id, 'ev-1');
    assert.equal(formatted.disputeId, 'disp-1');
    assert.equal(formatted.submittedBy, '0xA');
    assert.equal(formatted.evidenceType, 'screenshot');
    assert.equal(formatted.title, 'Test');
    assert.equal(formatted.description, 'Desc');
    assert.equal(formatted.contentHash, 'abc123');
    assert.equal(formatted.createdAt, '2026-01-01T00:00:00.000Z');
  });

  it('does not include raw content field in formatted output', () => {
    const raw = {
      id: 'ev-1',
      dispute_id: 'disp-1',
      submitted_by: '0xA',
      evidence_type: 'screenshot',
      title: 'Test',
      description: null,
      content: 'raw-data-should-not-appear',
      content_hash: 'abc',
      created_at: '2026-01-01T00:00:00.000Z',
    };

    const formatted = svc.formatEvidence(raw);
    assert.equal('content' in formatted, false);
  });
});
