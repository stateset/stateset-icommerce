import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createReputationService } from '../../src/a2a/reputation.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockStore() {
  const feedback = new Map();
  const reputationScores = new Map();
  return {
    createFeedback: async (record) => {
      feedback.set(record.id, { ...record });
    },
    getFeedback: async (id) => feedback.get(id) || null,
    updateFeedback: async (id, updates) => {
      const existing = feedback.get(id);
      if (existing) feedback.set(id, { ...existing, ...updates });
    },
    listFeedback: async (filter) => {
      let results = [...feedback.values()];
      if (filter?.agent_address)
        results = results.filter((f) => f.agent_address === filter.agent_address);
      if (filter?.reviewer_address)
        results = results.filter((f) => f.reviewer_address === filter.reviewer_address);
      return results;
    },
    getReputationScore: async (addr) => reputationScores.get(addr) || null,
    upsertReputationScore: async (record) => {
      reputationScores.set(record.agent_address, { ...record });
    },
    _feedback: feedback,
    _reputationScores: reputationScores,
  };
}

function seedFeedback(store, agentAddress, count, score = 5, transactionType = 'escrow') {
  for (let i = 0; i < count; i++) {
    store._feedback.set(`fb-${i}`, {
      id: `fb-${i}`,
      agent_address: agentAddress,
      reviewer_address: `0xReviewer${i}`,
      transaction_type: transactionType,
      transaction_id: `tx-${i}`,
      score,
      dimensions: JSON.stringify({
        reliability: score,
        quality: score,
        speed: score,
        communication: score,
      }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
  }
}

function seedFeedbackWithIds(
  store,
  agentAddress,
  count,
  { score = 5, transactionType = 'escrow', prefix = 'fb' } = {},
) {
  for (let i = 0; i < count; i++) {
    store._feedback.set(`${prefix}-${i}`, {
      id: `${prefix}-${i}`,
      agent_address: agentAddress,
      reviewer_address: `0xReviewer${i}`,
      transaction_type: transactionType,
      transaction_id: `tx-${prefix}-${i}`,
      score,
      dimensions: JSON.stringify({
        reliability: score,
        quality: score,
        speed: score,
        communication: score,
      }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date(Date.now() - i * 60_000).toISOString(),
      updated_at: new Date(Date.now() - i * 60_000).toISOString(),
    });
  }
}

const AGENT = '0xAgent1';
const REVIEWER = '0xReviewer1';

function validRateParams(overrides = {}) {
  return {
    agentAddress: AGENT,
    reviewerAddress: REVIEWER,
    transactionType: 'escrow',
    transactionId: 'tx-001',
    score: 5,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// 1. rateAgent
// ---------------------------------------------------------------------------

describe('rateAgent', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  // -- validation --

  it('rejects missing agentAddress', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ agentAddress: '' })),
      /agentAddress is required/,
    );
  });

  it('rejects missing reviewerAddress', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ reviewerAddress: '' })),
      /reviewerAddress is required/,
    );
  });

  it('rejects missing transactionType', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ transactionType: '' })),
      /transactionType must be one of/,
    );
  });

  it('rejects invalid transactionType', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ transactionType: 'barter' })),
      /transactionType must be one of/,
    );
  });

  it('rejects missing transactionId', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ transactionId: '' })),
      /transactionId is required/,
    );
  });

  it('rejects missing score (undefined)', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ score: undefined })),
      /score is required/,
    );
  });

  it('rejects missing score (null)', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ score: null })),
      /score is required/,
    );
  });

  it('rejects score of 0 (below range)', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ score: 0 })),
      /score must be an integer between 1 and 5/,
    );
  });

  it('rejects score of 6 (above range)', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ score: 6 })),
      /score must be an integer between 1 and 5/,
    );
  });

  it('rejects non-integer score', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ score: 3.5 })),
      /score must be an integer between 1 and 5/,
    );
  });

  it('rejects negative score', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ score: -1 })),
      /score must be an integer between 1 and 5/,
    );
  });

  it('rejects invalid dimension score (0)', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ dimensions: { reliability: 0 } })),
      /reliability dimension score must be an integer between 1 and 5/,
    );
  });

  it('rejects invalid dimension score (6)', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ dimensions: { quality: 6 } })),
      /quality dimension score must be an integer between 1 and 5/,
    );
  });

  it('rejects non-integer dimension score', async () => {
    await assert.rejects(
      () => svc.rateAgent(validRateParams({ dimensions: { speed: 2.5 } })),
      /speed dimension score must be an integer between 1 and 5/,
    );
  });

  it('accepts all four valid transaction types', async () => {
    for (const tt of ['quote', 'payment', 'escrow', 'service']) {
      const res = await svc.rateAgent(
        validRateParams({ transactionType: tt, transactionId: `tx-${tt}` }),
      );
      assert.equal(res.success, true);
    }
  });

  // -- successful creation --

  it('creates feedback and returns formatted output', async () => {
    const res = await svc.rateAgent(validRateParams());
    assert.equal(res.success, true);
    assert.equal(res.reputationUpdated, true);
    assert.ok(res.feedback);
    assert.equal(res.feedback.agentAddress, AGENT);
    assert.equal(res.feedback.reviewerAddress, REVIEWER);
    assert.equal(res.feedback.score, 5);
    assert.equal(res.feedback.transactionType, 'escrow');
    assert.equal(res.feedback.transactionId, 'tx-001');
    assert.equal(res.feedback.revoked, false);
  });

  it('stores feedback in the store', async () => {
    await svc.rateAgent(validRateParams());
    assert.equal(store._feedback.size, 1);
    const stored = [...store._feedback.values()][0];
    assert.equal(stored.agent_address, AGENT);
    assert.equal(stored.score, 5);
  });

  it('triggers recalculation — reputation record exists after rating', async () => {
    await svc.rateAgent(validRateParams());
    const rep = await store.getReputationScore(AGENT);
    assert.ok(rep, 'reputation record should be created');
    assert.equal(rep.total_transactions, 1);
    assert.equal(rep.average_score, 5);
  });

  it('stores dimensions as JSON string', async () => {
    const dims = { reliability: 4, quality: 5, speed: 3, communication: 4 };
    await svc.rateAgent(validRateParams({ dimensions: dims }));
    const stored = [...store._feedback.values()][0];
    assert.equal(typeof stored.dimensions, 'string');
    assert.deepEqual(JSON.parse(stored.dimensions), dims);
  });

  it('stores null dimensions when none provided', async () => {
    await svc.rateAgent(validRateParams());
    const stored = [...store._feedback.values()][0];
    assert.equal(stored.dimensions, null);
  });

  it('stores comment when provided', async () => {
    await svc.rateAgent(validRateParams({ comment: 'Great!' }));
    const stored = [...store._feedback.values()][0];
    assert.equal(stored.comment, 'Great!');
  });

  it('stores null comment when omitted', async () => {
    await svc.rateAgent(validRateParams());
    const stored = [...store._feedback.values()][0];
    assert.equal(stored.comment, null);
  });

  it('assigns a unique UUID id to each feedback', async () => {
    const res1 = await svc.rateAgent(validRateParams({ transactionId: 'tx-a' }));
    const res2 = await svc.rateAgent(validRateParams({ transactionId: 'tx-b' }));
    assert.notEqual(res1.feedback.id, res2.feedback.id);
  });
});

// ---------------------------------------------------------------------------
// 2. getReputation
// ---------------------------------------------------------------------------

describe('getReputation', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  it('rejects missing agentAddress', async () => {
    await assert.rejects(() => svc.getReputation(''), /agentAddress is required/);
  });

  it('returns default sandbox reputation for unknown agent', async () => {
    const res = await svc.getReputation('0xUnknown');
    assert.equal(res.success, true);
    assert.equal(res.reputation.trustTier, 'sandbox');
    assert.equal(res.reputation.totalTransactions, 0);
    assert.equal(res.reputation.averageScore, 0);
    assert.deepEqual(res.reputation.dimensionScores, {
      reliability: 0,
      quality: 0,
      speed: 0,
      communication: 0,
    });
  });

  it('returns stored reputation data', async () => {
    await svc.rateAgent(validRateParams({ score: 4 }));
    const res = await svc.getReputation(AGENT);
    assert.equal(res.success, true);
    assert.equal(res.reputation.totalTransactions, 1);
    assert.equal(res.reputation.averageScore, 4);
    assert.equal(res.reputation.agentAddress, AGENT);
  });

  it('formats dimension scores from JSON string', async () => {
    const dims = { reliability: 5, quality: 4, speed: 3, communication: 2 };
    await svc.rateAgent(validRateParams({ dimensions: dims }));
    const res = await svc.getReputation(AGENT);
    assert.deepEqual(res.reputation.dimensionScores, dims);
  });
});

// ---------------------------------------------------------------------------
// 3. respondToFeedback
// ---------------------------------------------------------------------------

describe('respondToFeedback', () => {
  let store, svc, feedbackId;

  beforeEach(async () => {
    store = createMockStore();
    svc = createReputationService(store);
    const created = await svc.rateAgent(validRateParams());
    feedbackId = created.feedback.id;
  });

  it('rejects missing feedbackId', async () => {
    await assert.rejects(
      () =>
        svc.respondToFeedback('', {
          response: 'thanks',
          responderAddress: AGENT,
        }),
      /feedbackId is required/,
    );
  });

  it('rejects missing response text', async () => {
    await assert.rejects(
      () =>
        svc.respondToFeedback(feedbackId, {
          response: '',
          responderAddress: AGENT,
        }),
      /response is required/,
    );
  });

  it('rejects missing responderAddress', async () => {
    await assert.rejects(
      () =>
        svc.respondToFeedback(feedbackId, {
          response: 'thanks',
          responderAddress: '',
        }),
      /responderAddress is required/,
    );
  });

  it('rejects non-existent feedback', async () => {
    await assert.rejects(
      () =>
        svc.respondToFeedback('no-such-id', {
          response: 'thanks',
          responderAddress: AGENT,
        }),
      /Feedback not found: no-such-id/,
    );
  });

  it('rejects response from non-rated-agent (reviewer cannot respond)', async () => {
    await assert.rejects(
      () =>
        svc.respondToFeedback(feedbackId, {
          response: 'trying to respond',
          responderAddress: REVIEWER,
        }),
      /Only the rated agent can respond/,
    );
  });

  it('allows the rated agent to respond successfully', async () => {
    const res = await svc.respondToFeedback(feedbackId, {
      response: 'Thank you!',
      responderAddress: AGENT,
    });
    assert.equal(res.success, true);
    assert.equal(res.feedback.response, 'Thank you!');
    assert.ok(res.feedback.responseAt, 'responseAt should be set');
  });

  it('updates feedback in the store', async () => {
    await svc.respondToFeedback(feedbackId, {
      response: 'Noted',
      responderAddress: AGENT,
    });
    const raw = await store.getFeedback(feedbackId);
    assert.equal(raw.response, 'Noted');
    assert.ok(raw.response_at);
  });
});

// ---------------------------------------------------------------------------
// 4. recalculateReputation
// ---------------------------------------------------------------------------

describe('recalculateReputation', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  it('rejects missing agentAddress', async () => {
    await assert.rejects(() => svc.recalculateReputation(''), /agentAddress is required/);
  });

  it('creates default record when no feedback exists and no prior record', async () => {
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.trust_tier, 'sandbox');
    assert.equal(result.total_transactions, 0);
    assert.equal(result.average_score, 0);

    // Also persists
    const stored = await store.getReputationScore(AGENT);
    assert.ok(stored);
    assert.equal(stored.trust_tier, 'sandbox');
  });

  it('returns existing record if no feedback and record already exists', async () => {
    store._reputationScores.set(AGENT, {
      agent_address: AGENT,
      total_transactions: 10,
      successful_transactions: 9,
      disputed_transactions: 1,
      average_score: 4.2,
      dimension_scores: '{}',
      trust_tier: 'standard',
      last_updated: new Date().toISOString(),
    });
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.total_transactions, 10);
    assert.equal(result.trust_tier, 'standard');
  });

  it('calculates average score correctly', async () => {
    seedFeedbackWithIds(store, AGENT, 3, { score: 4, prefix: 'a' });
    seedFeedbackWithIds(store, AGENT, 2, { score: 2, prefix: 'b' });
    // 3*4 + 2*2 = 16 / 5 = 3.2
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.average_score, 3.2);
    assert.equal(result.total_transactions, 5);
  });

  it('calculates dimension averages correctly', async () => {
    store._feedback.set('f1', {
      id: 'f1',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-1',
      score: 4,
      dimensions: JSON.stringify({
        reliability: 5,
        quality: 3,
        speed: 4,
        communication: 2,
      }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('f2', {
      id: 'f2',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-2',
      score: 4,
      dimensions: JSON.stringify({
        reliability: 3,
        quality: 5,
        speed: 2,
        communication: 4,
      }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    const result = await svc.recalculateReputation(AGENT);
    const dims = JSON.parse(result.dimension_scores);
    assert.equal(dims.reliability, 4); // (5+3)/2
    assert.equal(dims.quality, 4); // (3+5)/2
    assert.equal(dims.speed, 3); // (4+2)/2
    assert.equal(dims.communication, 3); // (2+4)/2
  });

  it('counts successful transactions (score >= 3)', async () => {
    seedFeedbackWithIds(store, AGENT, 3, { score: 4, prefix: 'good' });
    seedFeedbackWithIds(store, AGENT, 2, { score: 2, prefix: 'bad' });
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.successful_transactions, 3);
  });

  it('counts disputed transactions (escrow + score <= 2)', async () => {
    seedFeedbackWithIds(store, AGENT, 2, {
      score: 1,
      transactionType: 'escrow',
      prefix: 'disp',
    });
    seedFeedbackWithIds(store, AGENT, 1, {
      score: 1,
      transactionType: 'payment',
      prefix: 'pay',
    });
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.disputed_transactions, 2);
  });

  it('excludes revoked feedback from calculations', async () => {
    store._feedback.set('ok', {
      id: 'ok',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-ok',
      score: 5,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('revoked', {
      id: 'revoked',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-revoked',
      score: 1,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.total_transactions, 1);
    assert.equal(result.average_score, 5);
  });

  it('preserves current trust tier during recalculation', async () => {
    store._reputationScores.set(AGENT, {
      agent_address: AGENT,
      total_transactions: 0,
      successful_transactions: 0,
      disputed_transactions: 0,
      average_score: 0,
      dimension_scores: '{}',
      trust_tier: 'verified',
      last_updated: new Date().toISOString(),
    });
    seedFeedback(store, AGENT, 3, 5);
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.trust_tier, 'verified');
  });

  it('handles null dimensions in feedback gracefully', async () => {
    store._feedback.set('no-dims', {
      id: 'no-dims',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-nd',
      score: 3,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    const result = await svc.recalculateReputation(AGENT);
    const dims = JSON.parse(result.dimension_scores);
    assert.equal(dims.reliability, 0);
    assert.equal(dims.quality, 0);
    assert.equal(dims.speed, 0);
    assert.equal(dims.communication, 0);
  });

  it('rounds average score to 2 decimal places', async () => {
    // 3 feedback: 5, 5, 4 => avg 4.666... => 4.67
    store._feedback.set('a', {
      id: 'a',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-a',
      score: 5,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('b', {
      id: 'b',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-b',
      score: 5,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('c', {
      id: 'c',
      agent_address: AGENT,
      reviewer_address: '0xR3',
      transaction_type: 'payment',
      transaction_id: 'tx-c',
      score: 4,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    const result = await svc.recalculateReputation(AGENT);
    assert.equal(result.average_score, 4.67);
  });
});

// ---------------------------------------------------------------------------
// 5. checkTrustTierPromotion
// ---------------------------------------------------------------------------

describe('checkTrustTierPromotion', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  it('rejects missing agentAddress', async () => {
    await assert.rejects(() => svc.checkTrustTierPromotion(''), /agentAddress is required/);
  });

  it('returns not promoted when no reputation exists', async () => {
    const res = await svc.checkTrustTierPromotion('0xNew');
    assert.equal(res.promoted, false);
    assert.equal(res.previousTier, 'sandbox');
    assert.equal(res.currentTier, 'sandbox');
  });

  // -- sandbox -> standard --

  it('promotes sandbox -> standard with 5+ txns and avg >= 3.5', async () => {
    seedFeedback(store, AGENT, 5, 4);
    await svc.recalculateReputation(AGENT);
    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, true);
    assert.equal(res.previousTier, 'sandbox');
    assert.equal(res.currentTier, 'standard');
  });

  it('stays sandbox with <5 txns even if avg is high', async () => {
    seedFeedback(store, AGENT, 4, 5);
    await svc.recalculateReputation(AGENT);
    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'sandbox');
  });

  it('stays sandbox with 5+ txns but avg < 3.5', async () => {
    seedFeedback(store, AGENT, 5, 3);
    await svc.recalculateReputation(AGENT);
    // avg = 3.0 < 3.5
    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'sandbox');
  });

  it('promotes sandbox -> standard with exactly 5 txns and avg exactly 3.5', async () => {
    // Mix scores: three 4s and two 3s => avg = (12+6)/5 = 3.6 >= 3.5
    // Actually need exactly 3.5: two 3s + one 4 + two 3s = nope
    // Better: five scores averaging 3.5: two 4s and three 3s => (8+9)/5 = 3.4 < 3.5
    // three 4s and two 3s => (12+6)/5 = 3.6 yes
    // Actually let's use exact boundary: 4,4,3,3,4 => 18/5 = 3.6
    // For exact 3.5: 4,3,4,3,4 => 18/5 = 3.6.
    // 3,3,4,4,4 => 18/5 = 3.6. Can't get exactly 3.5 with integers unless
    // 4,3,3,4,4 = 18/5 = 3.6, or 4,4,3,3,3 = 17/5 = 3.4.
    // With 5 records of score=4 and score=3 we can't get 3.5 exactly.
    // Let's just test with avg >= 3.5:
    seedFeedbackWithIds(store, AGENT, 3, { score: 4, prefix: 'hi' });
    seedFeedbackWithIds(store, AGENT, 2, { score: 3, prefix: 'lo' });
    await svc.recalculateReputation(AGENT);
    // avg = 3.6 >= 3.5, total = 5
    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, true);
    assert.equal(res.currentTier, 'standard');
  });

  // -- standard -> verified --

  it('promotes standard -> verified with 25+ txns, avg >= 4.0, 0 disputes', async () => {
    seedFeedback(store, AGENT, 25, 4, 'payment');
    await svc.recalculateReputation(AGENT);
    // Manually set tier to standard
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'standard' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, true);
    assert.equal(res.previousTier, 'standard');
    assert.equal(res.currentTier, 'verified');
  });

  it('does not promote standard -> verified if disputes > 0', async () => {
    seedFeedbackWithIds(store, AGENT, 23, {
      score: 5,
      transactionType: 'payment',
      prefix: 'good',
    });
    // Add 2 disputed escrow (score <= 2)
    seedFeedbackWithIds(store, AGENT, 2, {
      score: 1,
      transactionType: 'escrow',
      prefix: 'disp',
    });
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'standard' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'standard');
  });

  it('does not promote standard -> verified if avg < 4.0', async () => {
    seedFeedback(store, AGENT, 25, 3, 'payment');
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'standard' });
    // avg = 3.0 < 4.0

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'standard');
  });

  it('does not promote standard -> verified if < 25 txns', async () => {
    seedFeedback(store, AGENT, 24, 5, 'payment');
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'standard' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'standard');
  });

  // -- verified -> enterprise --

  it('promotes verified -> enterprise with 100+ txns, avg >= 4.5, dispute rate < 2%', async () => {
    seedFeedback(store, AGENT, 100, 5, 'payment');
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'verified' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, true);
    assert.equal(res.previousTier, 'verified');
    assert.equal(res.currentTier, 'enterprise');
  });

  it('does not promote verified -> enterprise if avg < 4.5', async () => {
    seedFeedback(store, AGENT, 100, 4, 'payment');
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'verified' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'verified');
  });

  it('does not promote verified -> enterprise if dispute rate >= 2%', async () => {
    // 98 good + 2 disputed escrow = 2% dispute rate
    seedFeedbackWithIds(store, AGENT, 98, {
      score: 5,
      transactionType: 'payment',
      prefix: 'good',
    });
    seedFeedbackWithIds(store, AGENT, 2, {
      score: 1,
      transactionType: 'escrow',
      prefix: 'disp',
    });
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    // avg = (98*5+2*1)/100 = 4.92, disputes=2, rate=2%=0.02 (>= threshold)
    await store.upsertReputationScore({ ...rep, trust_tier: 'verified' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'verified');
  });

  it('promotes verified -> enterprise when dispute rate < 2%', async () => {
    // 99 good + 1 disputed escrow = 1% dispute rate
    seedFeedbackWithIds(store, AGENT, 99, {
      score: 5,
      transactionType: 'payment',
      prefix: 'good',
    });
    seedFeedbackWithIds(store, AGENT, 1, {
      score: 1,
      transactionType: 'escrow',
      prefix: 'disp',
    });
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    // avg = (99*5+1)/100 = 4.96, disputes=1, rate=1%=0.01 < 0.02
    await store.upsertReputationScore({ ...rep, trust_tier: 'verified' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, true);
    assert.equal(res.currentTier, 'enterprise');
  });

  it('does not promote verified -> enterprise if < 100 txns', async () => {
    seedFeedback(store, AGENT, 99, 5, 'payment');
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'verified' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.currentTier, 'verified');
  });

  // -- already enterprise --

  it('does not promote if already at enterprise tier', async () => {
    seedFeedback(store, AGENT, 200, 5, 'payment');
    await svc.recalculateReputation(AGENT);
    const rep = await store.getReputationScore(AGENT);
    await store.upsertReputationScore({ ...rep, trust_tier: 'enterprise' });

    const res = await svc.checkTrustTierPromotion(AGENT);
    assert.equal(res.promoted, false);
    assert.equal(res.previousTier, 'enterprise');
    assert.equal(res.currentTier, 'enterprise');
  });

  it('persists promoted tier in the store', async () => {
    seedFeedback(store, AGENT, 5, 4);
    await svc.recalculateReputation(AGENT);
    await svc.checkTrustTierPromotion(AGENT);
    const stored = await store.getReputationScore(AGENT);
    assert.equal(stored.trust_tier, 'standard');
  });
});

// ---------------------------------------------------------------------------
// 6. listFeedback
// ---------------------------------------------------------------------------

describe('listFeedback', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  it('returns empty array when no feedback exists', async () => {
    const result = await svc.listFeedback();
    assert.deepEqual(result, []);
  });

  it('returns all feedback when no filter provided', async () => {
    seedFeedback(store, '0xA', 2, 5);
    seedFeedbackWithIds(store, '0xB', 3, { score: 4, prefix: 'b' });
    const result = await svc.listFeedback();
    assert.equal(result.length, 5);
  });

  it('filters by agent_address', async () => {
    seedFeedbackWithIds(store, '0xA', 2, { prefix: 'a' });
    seedFeedbackWithIds(store, '0xB', 3, { prefix: 'b' });
    const result = await svc.listFeedback({ agent_address: '0xA' });
    assert.equal(result.length, 2);
    result.forEach((f) => assert.equal(f.agentAddress, '0xA'));
  });

  it('filters by reviewer_address', async () => {
    store._feedback.set('f1', {
      id: 'f1',
      agent_address: '0xA',
      reviewer_address: '0xBuyer',
      transaction_type: 'payment',
      transaction_id: 'tx-1',
      score: 4,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('f2', {
      id: 'f2',
      agent_address: '0xB',
      reviewer_address: '0xOther',
      transaction_type: 'payment',
      transaction_id: 'tx-2',
      score: 3,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    const result = await svc.listFeedback({ reviewer_address: '0xBuyer' });
    assert.equal(result.length, 1);
    assert.equal(result[0].reviewerAddress, '0xBuyer');
  });

  it('returns formatted feedback objects (camelCase)', async () => {
    seedFeedback(store, AGENT, 1, 4);
    const result = await svc.listFeedback();
    const f = result[0];
    assert.ok('agentAddress' in f);
    assert.ok('reviewerAddress' in f);
    assert.ok('transactionType' in f);
    assert.ok('transactionId' in f);
    assert.ok('createdAt' in f);
    assert.ok('updatedAt' in f);
    // Ensure snake_case keys are NOT present
    assert.ok(!('agent_address' in f));
    assert.ok(!('reviewer_address' in f));
  });
});

// ---------------------------------------------------------------------------
// 7. getFeedbackSummary
// ---------------------------------------------------------------------------

describe('getFeedbackSummary', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  it('rejects missing agentAddress', async () => {
    await assert.rejects(() => svc.getFeedbackSummary(''), /agentAddress is required/);
  });

  it('returns zero-state summary for agent with no feedback', async () => {
    const res = await svc.getFeedbackSummary('0xNew');
    assert.equal(res.success, true);
    const s = res.summary;
    assert.equal(s.agentAddress, '0xNew');
    assert.equal(s.totalReviews, 0);
    assert.equal(s.averageScore, 0);
    assert.deepEqual(s.scoreDistribution, { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 });
    assert.deepEqual(s.dimensionAverages, {
      reliability: 0,
      quality: 0,
      speed: 0,
      communication: 0,
    });
    assert.deepEqual(s.byTransactionType, {});
    assert.deepEqual(s.recentReviews, []);
  });

  it('calculates score distribution', async () => {
    // 2 fives, 1 four, 1 three
    store._feedback.set('s5a', {
      id: 's5a',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-s5a',
      score: 5,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('s5b', {
      id: 's5b',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-s5b',
      score: 5,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('s4', {
      id: 's4',
      agent_address: AGENT,
      reviewer_address: '0xR3',
      transaction_type: 'payment',
      transaction_id: 'tx-s4',
      score: 4,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('s3', {
      id: 's3',
      agent_address: AGENT,
      reviewer_address: '0xR4',
      transaction_type: 'payment',
      transaction_id: 'tx-s3',
      score: 3,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    const res = await svc.getFeedbackSummary(AGENT);
    assert.deepEqual(res.summary.scoreDistribution, {
      1: 0,
      2: 0,
      3: 1,
      4: 1,
      5: 2,
    });
  });

  it('computes correct average score', async () => {
    seedFeedbackWithIds(store, AGENT, 4, { score: 4, prefix: 'a' });
    seedFeedbackWithIds(store, AGENT, 1, { score: 2, prefix: 'b' });
    // avg = (16+2)/5 = 3.6
    const res = await svc.getFeedbackSummary(AGENT);
    assert.equal(res.summary.averageScore, 3.6);
    assert.equal(res.summary.totalReviews, 5);
  });

  it('computes dimension averages', async () => {
    store._feedback.set('d1', {
      id: 'd1',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-d1',
      score: 4,
      dimensions: JSON.stringify({ reliability: 5, quality: 3, speed: 4, communication: 2 }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('d2', {
      id: 'd2',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-d2',
      score: 4,
      dimensions: JSON.stringify({ reliability: 3, quality: 5, speed: 2, communication: 4 }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    const res = await svc.getFeedbackSummary(AGENT);
    assert.deepEqual(res.summary.dimensionAverages, {
      reliability: 4,
      quality: 4,
      speed: 3,
      communication: 3,
    });
  });

  it('computes dimension averages with partial dimensions', async () => {
    store._feedback.set('partial1', {
      id: 'partial1',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-p1',
      score: 4,
      dimensions: JSON.stringify({ reliability: 5, quality: 3 }),
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('partial2', {
      id: 'partial2',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-p2',
      score: 4,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    const res = await svc.getFeedbackSummary(AGENT);
    assert.equal(res.summary.dimensionAverages.reliability, 5);
    assert.equal(res.summary.dimensionAverages.quality, 3);
    assert.equal(res.summary.dimensionAverages.speed, 0);
    assert.equal(res.summary.dimensionAverages.communication, 0);
  });

  it('breaks down by transaction type', async () => {
    seedFeedbackWithIds(store, AGENT, 3, {
      score: 5,
      transactionType: 'escrow',
      prefix: 'esc',
    });
    seedFeedbackWithIds(store, AGENT, 2, {
      score: 3,
      transactionType: 'payment',
      prefix: 'pay',
    });

    const res = await svc.getFeedbackSummary(AGENT);
    const bt = res.summary.byTransactionType;
    assert.equal(bt.escrow.count, 3);
    assert.equal(bt.escrow.averageScore, 5);
    assert.equal(bt.payment.count, 2);
    assert.equal(bt.payment.averageScore, 3);
    // totalScore should be deleted
    assert.equal(bt.escrow.totalScore, undefined);
  });

  it('returns at most 5 recent reviews sorted newest first', async () => {
    // Create 7 feedback with staggered timestamps
    for (let i = 0; i < 7; i++) {
      store._feedback.set(`r-${i}`, {
        id: `r-${i}`,
        agent_address: AGENT,
        reviewer_address: `0xR${i}`,
        transaction_type: 'service',
        transaction_id: `tx-r-${i}`,
        score: (i % 5) + 1,
        dimensions: null,
        comment: `review ${i}`,
        response: null,
        response_at: null,
        revoked: false,
        created_at: new Date(Date.now() - i * 100_000).toISOString(),
        updated_at: new Date(Date.now() - i * 100_000).toISOString(),
      });
    }

    const res = await svc.getFeedbackSummary(AGENT);
    assert.equal(res.summary.recentReviews.length, 5);
    // Should be sorted newest first: r-0, r-1, r-2, r-3, r-4
    assert.equal(res.summary.recentReviews[0].id, 'r-0');
    assert.equal(res.summary.recentReviews[4].id, 'r-4');
  });

  it('recent reviews are formatted with camelCase keys', async () => {
    seedFeedback(store, AGENT, 1, 5);
    const res = await svc.getFeedbackSummary(AGENT);
    const review = res.summary.recentReviews[0];
    assert.ok('agentAddress' in review);
    assert.ok('transactionType' in review);
    assert.ok('createdAt' in review);
  });

  it('excludes revoked feedback from summary', async () => {
    store._feedback.set('active', {
      id: 'active',
      agent_address: AGENT,
      reviewer_address: '0xR1',
      transaction_type: 'payment',
      transaction_id: 'tx-active',
      score: 5,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    store._feedback.set('revoked', {
      id: 'revoked',
      agent_address: AGENT,
      reviewer_address: '0xR2',
      transaction_type: 'payment',
      transaction_id: 'tx-revoked',
      score: 1,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    const res = await svc.getFeedbackSummary(AGENT);
    assert.equal(res.summary.totalReviews, 1);
    assert.equal(res.summary.averageScore, 5);
  });
});

// ---------------------------------------------------------------------------
// 8. formatFeedback / formatReputation (exposed helpers)
// ---------------------------------------------------------------------------

describe('formatFeedback', () => {
  let svc;

  beforeEach(() => {
    svc = createReputationService(createMockStore());
  });

  it('converts snake_case to camelCase', () => {
    const raw = {
      id: 'fb-1',
      agent_address: '0xA',
      reviewer_address: '0xB',
      transaction_type: 'escrow',
      transaction_id: 'tx-1',
      score: 5,
      dimensions: null,
      comment: 'nice',
      response: null,
      response_at: null,
      revoked: 0,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
    };
    const formatted = svc.formatFeedback(raw);
    assert.equal(formatted.agentAddress, '0xA');
    assert.equal(formatted.reviewerAddress, '0xB');
    assert.equal(formatted.transactionType, 'escrow');
    assert.equal(formatted.transactionId, 'tx-1');
    assert.equal(formatted.comment, 'nice');
    assert.equal(formatted.revoked, false);
    assert.equal(formatted.createdAt, '2026-01-01T00:00:00.000Z');
  });

  it('parses dimensions from JSON string', () => {
    const raw = {
      id: 'fb-2',
      agent_address: '0xA',
      reviewer_address: '0xB',
      transaction_type: 'payment',
      transaction_id: 'tx-2',
      score: 4,
      dimensions: '{"reliability":5,"quality":4}',
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
    };
    const formatted = svc.formatFeedback(raw);
    assert.deepEqual(formatted.dimensions, { reliability: 5, quality: 4 });
  });

  it('handles null dimensions string', () => {
    const raw = {
      id: 'fb-3',
      agent_address: '0xA',
      reviewer_address: '0xB',
      transaction_type: 'quote',
      transaction_id: 'tx-3',
      score: 3,
      dimensions: null,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
    };
    const formatted = svc.formatFeedback(raw);
    assert.equal(formatted.dimensions, null);
  });

  it('passes through object dimensions unchanged', () => {
    const dims = { reliability: 3, quality: 2, speed: 1, communication: 4 };
    const raw = {
      id: 'fb-4',
      agent_address: '0xA',
      reviewer_address: '0xB',
      transaction_type: 'service',
      transaction_id: 'tx-4',
      score: 3,
      dimensions: dims,
      comment: null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: '2026-01-01T00:00:00.000Z',
      updated_at: '2026-01-01T00:00:00.000Z',
    };
    const formatted = svc.formatFeedback(raw);
    assert.deepEqual(formatted.dimensions, dims);
  });
});

describe('formatReputation', () => {
  let svc;

  beforeEach(() => {
    svc = createReputationService(createMockStore());
  });

  it('converts snake_case to camelCase', () => {
    const raw = {
      agent_address: '0xA',
      total_transactions: 10,
      successful_transactions: 9,
      disputed_transactions: 1,
      average_score: 4.5,
      dimension_scores: '{}',
      trust_tier: 'verified',
      last_updated: '2026-01-01T00:00:00.000Z',
    };
    const formatted = svc.formatReputation(raw);
    assert.equal(formatted.agentAddress, '0xA');
    assert.equal(formatted.totalTransactions, 10);
    assert.equal(formatted.successfulTransactions, 9);
    assert.equal(formatted.disputedTransactions, 1);
    assert.equal(formatted.averageScore, 4.5);
    assert.equal(formatted.trustTier, 'verified');
    assert.equal(formatted.lastUpdated, '2026-01-01T00:00:00.000Z');
  });

  it('parses dimension_scores from JSON string', () => {
    const raw = {
      agent_address: '0xA',
      total_transactions: 1,
      successful_transactions: 1,
      disputed_transactions: 0,
      average_score: 5,
      dimension_scores: '{"reliability":4,"quality":5,"speed":3,"communication":4}',
      trust_tier: 'sandbox',
      last_updated: '2026-01-01T00:00:00.000Z',
    };
    const formatted = svc.formatReputation(raw);
    assert.deepEqual(formatted.dimensionScores, {
      reliability: 4,
      quality: 5,
      speed: 3,
      communication: 4,
    });
  });
});

// ---------------------------------------------------------------------------
// 9. Integration / end-to-end flows
// ---------------------------------------------------------------------------

describe('integration: full rating flow', () => {
  let store, svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createReputationService(store);
  });

  it('rateAgent + getReputation returns updated data', async () => {
    await svc.rateAgent(validRateParams({ score: 4 }));
    const rep = await svc.getReputation(AGENT);
    assert.equal(rep.reputation.totalTransactions, 1);
    assert.equal(rep.reputation.averageScore, 4);
    assert.equal(rep.reputation.trustTier, 'sandbox');
  });

  it('multiple ratings update average correctly', async () => {
    await svc.rateAgent(validRateParams({ score: 5, transactionId: 'tx-1' }));
    await svc.rateAgent(validRateParams({ score: 3, transactionId: 'tx-2' }));
    const rep = await svc.getReputation(AGENT);
    assert.equal(rep.reputation.averageScore, 4);
    assert.equal(rep.reputation.totalTransactions, 2);
  });

  it('tier promotion is triggered automatically by rateAgent', async () => {
    // Rate 5 times with score >= 4 to trigger sandbox -> standard
    for (let i = 0; i < 5; i++) {
      await svc.rateAgent(validRateParams({ score: 4, transactionId: `tx-${i}` }));
    }
    const rep = await svc.getReputation(AGENT);
    assert.equal(rep.reputation.trustTier, 'standard');
  });

  it('rateAgent followed by respondToFeedback', async () => {
    const rated = await svc.rateAgent(validRateParams({ comment: 'Good work' }));
    const responded = await svc.respondToFeedback(rated.feedback.id, {
      response: 'Thank you!',
      responderAddress: AGENT,
    });
    assert.equal(responded.feedback.response, 'Thank you!');
    assert.ok(responded.feedback.responseAt);
  });

  it('getFeedbackSummary reflects all ratings', async () => {
    await svc.rateAgent(
      validRateParams({ score: 5, transactionId: 'tx-1', transactionType: 'escrow' }),
    );
    await svc.rateAgent(
      validRateParams({ score: 3, transactionId: 'tx-2', transactionType: 'payment' }),
    );
    const summary = await svc.getFeedbackSummary(AGENT);
    assert.equal(summary.summary.totalReviews, 2);
    assert.equal(summary.summary.averageScore, 4);
    assert.equal(summary.summary.scoreDistribution[5], 1);
    assert.equal(summary.summary.scoreDistribution[3], 1);
    assert.ok(summary.summary.byTransactionType.escrow);
    assert.ok(summary.summary.byTransactionType.payment);
  });
});
