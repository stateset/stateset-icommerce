/**
 * Unit tests for Marketplace Service — Multi-party RFQ broadcast, scoring, and awarding
 *
 * Tests cli/src/a2a/marketplace.js:
 *   - createMarketplaceService() construction and validation
 *   - broadcastRFQ() — input validation, seller filtering, buyer exclusion, RFQ creation
 *   - collectRFQResponses() — cheapest/best_value/fastest scoring, ranking, unscored handling
 *   - awardRFQ() — winner selection, loser declination, winnerId override, state validation
 *   - expireRFQs() — past-deadline expiry, future-deadline skipping
 *   - getServiceMetrics() — success_rate, avg_response_time, totals
 *   - getAgentStatus() — services, reputation, pending RFQs
 *   - Edge cases — empty responses, no matching sellers, already awarded
 */

import { describe, it, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { A2AStore } from '../../src/a2a/store.js';
import { createMarketplaceService } from '../../src/a2a/marketplace.js';

// ===========================================================================
// Helpers
// ===========================================================================

function freshStore() {
  return new A2AStore({ dbPath: ':memory:' });
}

/** Create a service (seller) in the store. */
function createSeller(store, overrides = {}) {
  return store.createService({
    agent_address: overrides.agent_address || `0xSeller${randomUUID().slice(0, 8)}`,
    name: overrides.name || 'Test Seller Service',
    description: overrides.description || 'Provides test goods',
    category: overrides.category || 'goods',
    active: overrides.active !== undefined ? overrides.active : 1,
    ...overrides,
  });
}

/** Create a quote for a seller in the store. */
function createQuote(store, overrides = {}) {
  const id = overrides.id || randomUUID();
  return store.createQuote({
    id,
    buyer_address: overrides.buyer_address || '0xBuyer',
    seller_address: overrides.seller_address || '0xSeller',
    items: overrides.items || [{ description: 'Item A', quantity: 1 }],
    total: overrides.total ?? 100,
    total_decimal: overrides.total_decimal ?? 100.0,
    status: overrides.status || 'quoted',
    expires_at: overrides.expires_at || new Date(Date.now() + 86400000).toISOString(),
    created_at: overrides.created_at || new Date().toISOString(),
    quoted_at: overrides.quoted_at || new Date().toISOString(),
    ...overrides,
  });
}

/** Null a2aService — used when we don't need quote request forwarding. */
const NULL_A2A = null;

/** Mock a2aService that tracks calls. */
function mockA2AService(overrides = {}) {
  const calls = { requestQuote: [], acceptQuote: [], declineQuote: [] };
  return {
    calls,
    requestQuote: async (params) => {
      calls.requestQuote.push(params);
      if (overrides.requestQuoteError) throw new Error(overrides.requestQuoteError);
      return { quote: { id: overrides.quoteId || randomUUID() } };
    },
    acceptQuote: async (quoteId) => {
      calls.acceptQuote.push(quoteId);
      if (overrides.acceptQuoteError) throw new Error(overrides.acceptQuoteError);
      return { success: true };
    },
    declineQuote: async (quoteId, reason) => {
      calls.declineQuote.push({ quoteId, reason });
      if (overrides.declineQuoteError) throw new Error(overrides.declineQuoteError);
      return { success: true };
    },
  };
}

// ===========================================================================
// 1. createMarketplaceService — construction and validation
// ===========================================================================

describe('Marketplace Service — createMarketplaceService', () => {
  it('throws when store is null', () => {
    assert.throws(() => createMarketplaceService(null), /store is required/);
  });

  it('throws when store is undefined', () => {
    assert.throws(() => createMarketplaceService(undefined), /store is required/);
  });

  it('returns an object with all expected methods', () => {
    const store = freshStore();
    const mp = createMarketplaceService(store, NULL_A2A);
    assert.strictEqual(typeof mp.broadcastRFQ, 'function');
    assert.strictEqual(typeof mp.collectRFQResponses, 'function');
    assert.strictEqual(typeof mp.awardRFQ, 'function');
    assert.strictEqual(typeof mp.expireRFQs, 'function');
    assert.strictEqual(typeof mp.getServiceMetrics, 'function');
    assert.strictEqual(typeof mp.getAgentStatus, 'function');
  });

  it('accepts null a2aService gracefully', () => {
    const store = freshStore();
    const mp = createMarketplaceService(store, null);
    assert.ok(mp);
  });

  it('accepts undefined a2aService gracefully', () => {
    const store = freshStore();
    const mp = createMarketplaceService(store);
    assert.ok(mp);
  });
});

// ===========================================================================
// 2. broadcastRFQ
// ===========================================================================

describe('Marketplace Service — broadcastRFQ', () => {
  let store, mp;

  beforeEach(() => {
    store = freshStore();
    mp = createMarketplaceService(store, NULL_A2A);
  });

  it('throws when buyerAddress is missing', async () => {
    await assert.rejects(
      () => mp.broadcastRFQ({ items: [{ description: 'Widget', quantity: 1 }] }),
      /buyerAddress is required/,
    );
  });

  it('throws when items is missing', async () => {
    await assert.rejects(
      () => mp.broadcastRFQ({ buyerAddress: '0xBuyer' }),
      /items array is required/,
    );
  });

  it('throws when items is empty array', async () => {
    await assert.rejects(
      () => mp.broadcastRFQ({ buyerAddress: '0xBuyer', items: [] }),
      /items array is required/,
    );
  });

  it('throws when items is not an array', async () => {
    await assert.rejects(
      () => mp.broadcastRFQ({ buyerAddress: '0xBuyer', items: 'not-array' }),
      /items array is required/,
    );
  });

  it('creates an RFQ with correct fields', async () => {
    createSeller(store, { agent_address: '0xSeller1' });
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 2 }],
      deadlineMinutes: 30,
      scoringCriteria: 'best_value',
    });

    assert.ok(result.rfq);
    assert.strictEqual(result.rfq.buyer_address, '0xBuyer');
    assert.strictEqual(result.rfq.scoring_criteria, 'best_value');
    assert.strictEqual(result.rfq.status, 'open');
    assert.ok(result.deadline);
  });

  it('returns sellersContacted count', async () => {
    createSeller(store, { agent_address: '0xS1' });
    createSeller(store, { agent_address: '0xS2' });
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });

    assert.strictEqual(result.sellersContacted, 2);
    assert.strictEqual(result.responses.length, 2);
  });

  it('filters sellers by category when sellerFilter is provided', async () => {
    createSeller(store, { agent_address: '0xElectronics', category: 'electronics' });
    createSeller(store, { agent_address: '0xClothing', category: 'clothing' });
    createSeller(store, { agent_address: '0xElectronics2', category: 'electronics' });

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Phone', quantity: 1 }],
      sellerFilter: 'electronics',
    });

    assert.strictEqual(result.sellersContacted, 2);
    const sellerAddrs = result.responses.map((r) => r.seller_address);
    assert.ok(sellerAddrs.includes('0xElectronics'));
    assert.ok(sellerAddrs.includes('0xElectronics2'));
    assert.ok(!sellerAddrs.includes('0xClothing'));
  });

  it('excludes buyer own services', async () => {
    createSeller(store, { agent_address: '0xBuyer' }); // buyer is also a seller
    createSeller(store, { agent_address: '0xOtherSeller' });

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });

    assert.strictEqual(result.sellersContacted, 1);
    assert.strictEqual(result.responses[0].seller_address, '0xOtherSeller');
  });

  it('respects maxResponses limit', async () => {
    for (let i = 0; i < 5; i++) {
      createSeller(store, { agent_address: `0xSeller${i}` });
    }

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
      maxResponses: 3,
    });

    assert.strictEqual(result.sellersContacted, 3);
  });

  it('defaults deadlineMinutes to 60', async () => {
    createSeller(store, { agent_address: '0xS1' });
    const before = Date.now();
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });
    const deadlineTime = new Date(result.deadline).getTime();
    // deadline should be approximately 60 minutes from now
    const diffMs = deadlineTime - before;
    assert.ok(diffMs >= 59 * 60 * 1000, 'deadline should be at least 59 minutes from now');
    assert.ok(diffMs <= 61 * 60 * 1000, 'deadline should be at most 61 minutes from now');
  });

  it('defaults scoringCriteria to cheapest', async () => {
    createSeller(store, { agent_address: '0xS1' });
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });
    assert.strictEqual(result.rfq.scoring_criteria, 'cheapest');
  });

  it('stores buyerAgentId when provided', async () => {
    createSeller(store, { agent_address: '0xS1' });
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      buyerAgentId: 'agent-buyer-001',
      items: [{ description: 'Widget', quantity: 1 }],
    });
    assert.strictEqual(result.rfq.buyer_agent_id, 'agent-buyer-001');
  });

  it('creates RFQ responses with pending status', async () => {
    createSeller(store, { agent_address: '0xS1' });
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });
    assert.strictEqual(result.responses[0].status, 'pending');
    assert.strictEqual(result.responses[0].rfq_id, result.rfq.id);
  });

  it('handles zero eligible sellers gracefully', async () => {
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });
    assert.strictEqual(result.sellersContacted, 0);
    assert.strictEqual(result.responses.length, 0);
    assert.ok(result.rfq.id); // RFQ is still created
  });

  it('excludes inactive sellers', async () => {
    createSeller(store, { agent_address: '0xActive', active: 1 });
    createSeller(store, { agent_address: '0xInactive', active: 0 });

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });

    assert.strictEqual(result.sellersContacted, 1);
    assert.strictEqual(result.responses[0].seller_address, '0xActive');
  });

  it('uses a2aService to request quotes when provided', async () => {
    const a2aSvc = mockA2AService();
    const mp2 = createMarketplaceService(store, a2aSvc);
    createSeller(store, { agent_address: '0xS1' });

    await mp2.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });

    assert.strictEqual(a2aSvc.calls.requestQuote.length, 1);
    assert.strictEqual(a2aSvc.calls.requestQuote[0].seller, '0xS1');
  });

  it('continues when a2aService.requestQuote throws for one seller', async () => {
    let callCount = 0;
    const a2aSvc = {
      requestQuote: async (params) => {
        callCount++;
        if (callCount === 1) throw new Error('Network timeout');
        return { quote: { id: randomUUID() } };
      },
    };
    const mp2 = createMarketplaceService(store, a2aSvc);
    createSeller(store, { agent_address: '0xS1' });
    createSeller(store, { agent_address: '0xS2' });

    const result = await mp2.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });

    // First seller fails, second succeeds
    assert.strictEqual(result.sellersContacted, 1);
    assert.strictEqual(result.responses[0].seller_address, '0xS2');
  });

  it('persists RFQ in store and can be retrieved', async () => {
    createSeller(store, { agent_address: '0xS1' });
    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });
    const fetched = store.getRFQ(result.rfq.id);
    assert.ok(fetched);
    assert.strictEqual(fetched.buyer_address, '0xBuyer');
  });
});

// ===========================================================================
// 3. collectRFQResponses — scoring and ranking
// ===========================================================================

describe('Marketplace Service — collectRFQResponses', () => {
  let store, mp;

  beforeEach(() => {
    store = freshStore();
    mp = createMarketplaceService(store, NULL_A2A);
  });

  it('throws when RFQ does not exist', () => {
    assert.throws(
      () => mp.collectRFQResponses('nonexistent-rfq'),
      /RFQ nonexistent-rfq not found/,
    );
  });

  it('returns empty ranked for RFQ with no responses', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });
    const result = mp.collectRFQResponses(rfq.id);
    assert.strictEqual(result.totalResponses, 0);
    assert.strictEqual(result.scoredCount, 0);
    assert.strictEqual(result.ranked.length, 0);
    assert.strictEqual(result.unscored.length, 0);
  });

  // -- Cheapest scoring --

  it('scores by cheapest — lower price gets higher score', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'cheapest',
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 100 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);

    assert.strictEqual(result.scoredCount, 2);
    assert.strictEqual(result.ranked[0].seller_address, '0xS1'); // cheaper wins
    assert.ok(result.ranked[0].score > result.ranked[1].score);
  });

  it('cheapest scoring — rank 1 is best', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'cheapest',
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 200 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 50 });
    const q3 = createQuote(store, { seller_address: '0xS3', total_decimal: 100 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS3', quote_id: q3.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);

    assert.strictEqual(result.ranked[0].rank, 1);
    assert.strictEqual(result.ranked[0].seller_address, '0xS2');
    assert.strictEqual(result.ranked[1].rank, 2);
    assert.strictEqual(result.ranked[1].seller_address, '0xS3');
    assert.strictEqual(result.ranked[2].rank, 3);
    assert.strictEqual(result.ranked[2].seller_address, '0xS1');
  });

  it('cheapest scoring — handles zero total', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'cheapest',
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 0 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    assert.strictEqual(result.ranked[0].score, 0);
  });

  // -- Best-value scoring --

  it('scores by best_value — factors in reputation', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'best_value',
    });

    // Seller 1: higher price but excellent reputation
    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 150 });
    store.upsertReputationScore({
      agent_address: '0xS1',
      average_score: 5.0,
      total_transactions: 100,
      trust_tier: 'trusted',
    });

    // Seller 2: lower price but poor reputation
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 50 });
    store.upsertReputationScore({
      agent_address: '0xS2',
      average_score: 1.0,
      total_transactions: 5,
      trust_tier: 'sandbox',
    });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);

    assert.strictEqual(result.scoringCriteria, 'best_value');
    assert.strictEqual(result.scoredCount, 2);
    // S1 has higher reputation but higher price — score should reflect the balance
    // best_value = rep * 0.4 + (1/total) * 100 * 0.6
    // S1: 5.0 * 0.4 + (1/150)*100*0.6 = 2.0 + 0.4 = 2.4
    // S2: 1.0 * 0.4 + (1/50)*100*0.6  = 0.4 + 1.2 = 1.6
    assert.strictEqual(result.ranked[0].seller_address, '0xS1');
  });

  it('best_value scoring — defaults reputation to 3 when missing', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'best_value',
    });

    const q1 = createQuote(store, { seller_address: '0xNewSeller', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xNewSeller', quote_id: q1.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    // rep defaults to 3 when not found
    // score = 3 * 0.4 + (1/100)*100*0.6 = 1.2 + 0.6 = 1.8
    const expectedScore = 3 * 0.4 + (1 / 100) * 100 * 0.6;
    assert.ok(Math.abs(result.ranked[0].score - expectedScore) < 0.001);
  });

  // -- Fastest scoring --

  it('scores by fastest — quicker response time wins', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'fastest',
    });

    const now = Date.now();
    // S1: quick response (1 second), same price
    const q1 = createQuote(store, {
      seller_address: '0xS1',
      total_decimal: 100,
      created_at: new Date(now - 10000).toISOString(),
      quoted_at: new Date(now - 9000).toISOString(), // 1s response
    });

    // S2: slow response (5 seconds), same price
    const q2 = createQuote(store, {
      seller_address: '0xS2',
      total_decimal: 100,
      created_at: new Date(now - 10000).toISOString(),
      quoted_at: new Date(now - 5000).toISOString(), // 5s response
    });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    assert.strictEqual(result.ranked[0].seller_address, '0xS1'); // faster response
  });

  it('fastest scoring — considers both time and price', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'fastest',
    });

    const now = Date.now();
    // S1: quick but expensive
    const q1 = createQuote(store, {
      seller_address: '0xS1',
      total_decimal: 500,
      created_at: new Date(now - 10000).toISOString(),
      quoted_at: new Date(now - 9000).toISOString(), // 1s
    });

    // S2: slow but cheap
    const q2 = createQuote(store, {
      seller_address: '0xS2',
      total_decimal: 10,
      created_at: new Date(now - 10000).toISOString(),
      quoted_at: new Date(now - 5000).toISOString(), // 5s
    });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    // Both time and price matter; assert both are scored
    assert.strictEqual(result.scoredCount, 2);
    assert.ok(result.ranked[0].score > result.ranked[1].score);
  });

  // -- Unscored handling --

  it('puts unscored responses (status=requested) in unscored array', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    // Quote still in "requested" state — not yet provided
    const q1 = createQuote(store, { seller_address: '0xS1', status: 'requested', total_decimal: 0 });
    const q2 = createQuote(store, { seller_address: '0xS2', status: 'quoted', total_decimal: 100 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);

    assert.strictEqual(result.totalResponses, 2);
    assert.strictEqual(result.scoredCount, 1);
    assert.strictEqual(result.unscored.length, 1);
    assert.strictEqual(result.unscored[0].seller_address, '0xS1');
    assert.strictEqual(result.unscored[0].score, null);
  });

  it('puts responses with no quote in unscored array', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    // quote_id that doesn't exist in the store
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: 'nonexistent-quote', status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    assert.strictEqual(result.unscored.length, 1);
    assert.strictEqual(result.scoredCount, 0);
  });

  it('updates response status to scored after scoring', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    const resp = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });

    mp.collectRFQResponses(rfq.id);

    const updated = store.getRFQResponse(resp.id);
    assert.strictEqual(updated.status, 'scored');
    assert.ok(updated.score !== null);
  });

  it('updates response rank in store', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 100 });

    const r1 = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    const r2 = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    mp.collectRFQResponses(rfq.id);

    const updatedR1 = store.getRFQResponse(r1.id);
    const updatedR2 = store.getRFQResponse(r2.id);
    assert.strictEqual(updatedR1.rank, 1); // cheaper = rank 1
    assert.strictEqual(updatedR2.rank, 2);
  });

  it('falls back to cheapest when scoring_criteria is unknown', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'unknown_criteria',
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    // Falls back to cheapest — S1 should rank first
    assert.strictEqual(result.ranked[0].seller_address, '0xS1');
  });

  it('includes quote object in ranked results', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    assert.ok(result.ranked[0].quote);
    assert.strictEqual(result.ranked[0].quote.id, q1.id);
  });

  it('includes reputation in ranked results when available', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'best_value',
    });

    store.upsertReputationScore({
      agent_address: '0xS1',
      average_score: 4.5,
      trust_tier: 'trusted',
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    assert.ok(result.ranked[0].reputation);
    assert.strictEqual(result.ranked[0].reputation.average_score, 4.5);
  });

  it('returns rfqId and scoringCriteria in result', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'fastest',
    });

    const result = mp.collectRFQResponses(rfq.id);
    assert.strictEqual(result.rfqId, rfq.id);
    assert.strictEqual(result.scoringCriteria, 'fastest');
  });
});

// ===========================================================================
// 4. awardRFQ
// ===========================================================================

describe('Marketplace Service — awardRFQ', () => {
  let store;

  beforeEach(() => {
    store = freshStore();
  });

  it('throws when RFQ does not exist', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    await assert.rejects(
      () => mp.awardRFQ('nonexistent'),
      /RFQ nonexistent not found/,
    );
  });

  it('throws when RFQ is not open', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });
    store.updateRFQ(rfq.id, { status: 'awarded' });

    await assert.rejects(
      () => mp.awardRFQ(rfq.id),
      /is awarded, not open/,
    );
  });

  it('throws when RFQ is expired', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });
    store.updateRFQ(rfq.id, { status: 'expired' });

    await assert.rejects(
      () => mp.awardRFQ(rfq.id),
      /is expired, not open/,
    );
  });

  it('throws when no scored responses exist', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    await assert.rejects(
      () => mp.awardRFQ(rfq.id),
      /No scored responses to award/,
    );
  });

  it('awards the highest-scored response by default', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'scored', score: 0.005 });

    const result = await mp.awardRFQ(rfq.id);

    assert.strictEqual(result.winnerAddress, '0xS1');
    assert.strictEqual(result.winnerScore, 0.02);
    assert.strictEqual(result.losersDeclined, 1);
  });

  it('updates RFQ status to awarded with winning_quote_id', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });

    await mp.awardRFQ(rfq.id);

    const updatedRFQ = store.getRFQ(rfq.id);
    assert.strictEqual(updatedRFQ.status, 'awarded');
    assert.strictEqual(updatedRFQ.winning_quote_id, q1.id);
    assert.ok(updatedRFQ.awarded_at);
  });

  it('sets winner response to awarded and losers to declined', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });

    const r1 = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });
    const r2 = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'scored', score: 0.005 });

    await mp.awardRFQ(rfq.id);

    assert.strictEqual(store.getRFQResponse(r1.id).status, 'awarded');
    assert.strictEqual(store.getRFQResponse(r2.id).status, 'declined');
  });

  it('allows winnerId override by response id', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });

    const r1 = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });
    const r2 = store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'scored', score: 0.005 });

    // Force S2 as winner despite lower score
    const result = await mp.awardRFQ(rfq.id, r2.id);

    assert.strictEqual(result.winnerAddress, '0xS2');
    assert.strictEqual(result.winnerId, r2.id);
    assert.strictEqual(store.getRFQResponse(r1.id).status, 'declined');
    assert.strictEqual(store.getRFQResponse(r2.id).status, 'awarded');
  });

  it('allows winnerId override by quote_id', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'scored', score: 0.005 });

    // Use quote_id to override
    const result = await mp.awardRFQ(rfq.id, q2.id);
    assert.strictEqual(result.winnerAddress, '0xS2');
    assert.strictEqual(result.winningQuoteId, q2.id);
  });

  it('throws when winnerId not found in scored responses', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });

    await assert.rejects(
      () => mp.awardRFQ(rfq.id, 'nonexistent-winner'),
      /Winner nonexistent-winner not found/,
    );
  });

  it('calls a2aService.acceptQuote for the winner', async () => {
    const a2aSvc = mockA2AService();
    const mp = createMarketplaceService(store, a2aSvc);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });

    await mp.awardRFQ(rfq.id);

    assert.strictEqual(a2aSvc.calls.acceptQuote.length, 1);
    assert.strictEqual(a2aSvc.calls.acceptQuote[0], q1.id);
  });

  it('calls a2aService.declineQuote for losers', async () => {
    const a2aSvc = mockA2AService();
    const mp = createMarketplaceService(store, a2aSvc);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });
    const q3 = createQuote(store, { seller_address: '0xS3', total_decimal: 300 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'scored', score: 0.005 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS3', quote_id: q3.id, status: 'scored', score: 0.003 });

    await mp.awardRFQ(rfq.id);

    assert.strictEqual(a2aSvc.calls.declineQuote.length, 2);
    const declinedQuotes = a2aSvc.calls.declineQuote.map((c) => c.quoteId);
    assert.ok(declinedQuotes.includes(q2.id));
    assert.ok(declinedQuotes.includes(q3.id));
  });

  it('handles a2aService.acceptQuote failure gracefully', async () => {
    const a2aSvc = mockA2AService({ acceptQuoteError: 'accept failed' });
    const mp = createMarketplaceService(store, a2aSvc);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });

    // Should not throw
    const result = await mp.awardRFQ(rfq.id);
    assert.ok(result.winnerId);
  });

  it('handles a2aService.declineQuote failure gracefully', async () => {
    const a2aSvc = mockA2AService({ declineQuoteError: 'decline failed' });
    const mp = createMarketplaceService(store, a2aSvc);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 50 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 200 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.02 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'scored', score: 0.005 });

    // Should not throw — decline failure is logged but doesn't block award
    const result = await mp.awardRFQ(rfq.id);
    assert.strictEqual(result.losersDeclined, 1);
  });

  it('returns correct result shape', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });

    const result = await mp.awardRFQ(rfq.id);

    assert.strictEqual(result.rfqId, rfq.id);
    assert.ok(result.winnerId);
    assert.strictEqual(result.winningQuoteId, q1.id);
    assert.strictEqual(result.winnerAddress, '0xS1');
    assert.strictEqual(result.winnerScore, 0.01);
    assert.strictEqual(result.losersDeclined, 0);
  });

  it('ignores unscored responses (score=null) when awarding', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 50 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' }); // no score

    const result = await mp.awardRFQ(rfq.id);
    assert.strictEqual(result.winnerAddress, '0xS1');
    assert.strictEqual(result.losersDeclined, 0); // unscored is not a "loser"
  });
});

// ===========================================================================
// 5. expireRFQs
// ===========================================================================

describe('Marketplace Service — expireRFQs', () => {
  let store, mp;

  beforeEach(() => {
    store = freshStore();
    mp = createMarketplaceService(store, NULL_A2A);
  });

  it('expires RFQs past their deadline', () => {
    store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() - 3600000).toISOString(), // 1 hour ago
    });

    const result = mp.expireRFQs();
    assert.strictEqual(result.expired, 1);
    assert.strictEqual(result.checked, 1);
  });

  it('skips RFQs with future deadlines', () => {
    store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(), // 1 hour from now
    });

    const result = mp.expireRFQs();
    assert.strictEqual(result.expired, 0);
    assert.strictEqual(result.checked, 1);
  });

  it('only checks open RFQs', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() - 3600000).toISOString(),
    });
    store.updateRFQ(rfq.id, { status: 'awarded' });

    const result = mp.expireRFQs();
    assert.strictEqual(result.checked, 0);
    assert.strictEqual(result.expired, 0);
  });

  it('updates RFQ status to expired', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() - 3600000).toISOString(),
    });

    mp.expireRFQs();

    const updated = store.getRFQ(rfq.id);
    assert.strictEqual(updated.status, 'expired');
    assert.ok(updated.closed_at);
  });

  it('handles multiple RFQs — mixed expired and active', () => {
    store.createRFQ({
      buyer_address: '0xB1',
      items: '[]',
      deadline: new Date(Date.now() - 7200000).toISOString(), // expired
    });
    store.createRFQ({
      buyer_address: '0xB2',
      items: '[]',
      deadline: new Date(Date.now() - 1000).toISOString(), // just expired
    });
    store.createRFQ({
      buyer_address: '0xB3',
      items: '[]',
      deadline: new Date(Date.now() + 7200000).toISOString(), // still active
    });

    const result = mp.expireRFQs();
    assert.strictEqual(result.expired, 2);
    assert.strictEqual(result.checked, 3);
  });

  it('returns zero for empty store', () => {
    const result = mp.expireRFQs();
    assert.strictEqual(result.expired, 0);
    assert.strictEqual(result.checked, 0);
  });

  it('is idempotent — already expired RFQs are not re-checked', () => {
    store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() - 3600000).toISOString(),
    });

    mp.expireRFQs();
    const result = mp.expireRFQs();
    // Second call — the RFQ is now 'expired', not 'open', so not checked
    assert.strictEqual(result.checked, 0);
    assert.strictEqual(result.expired, 0);
  });
});

// ===========================================================================
// 6. getServiceMetrics
// ===========================================================================

describe('Marketplace Service — getServiceMetrics', () => {
  let store, mp;

  beforeEach(() => {
    store = freshStore();
    mp = createMarketplaceService(store, NULL_A2A);
  });

  it('throws when service does not exist', () => {
    assert.throws(
      () => mp.getServiceMetrics('nonexistent'),
      /Service nonexistent not found/,
    );
  });

  it('returns correct totals for a service with no quotes', () => {
    const svc = createSeller(store, { agent_address: '0xS1', name: 'TestSvc' });
    const metrics = mp.getServiceMetrics(svc.id);

    assert.strictEqual(metrics.serviceId, svc.id);
    assert.strictEqual(metrics.serviceName, 'TestSvc');
    assert.strictEqual(metrics.agentAddress, '0xS1');
    assert.strictEqual(metrics.totalTransactions, 0);
    assert.strictEqual(metrics.fulfilledCount, 0);
    assert.strictEqual(metrics.acceptedCount, 0);
    assert.strictEqual(metrics.declinedCount, 0);
    assert.strictEqual(metrics.successRate, 0);
    assert.strictEqual(metrics.avgResponseTimeMs, null);
    assert.strictEqual(metrics.disputeRate, 0);
  });

  it('computes success_rate correctly', () => {
    const svc = createSeller(store, { agent_address: '0xS1' });

    // 3 fulfilled, 1 accepted, 1 declined = 5 total, 3 fulfilled = 60% success
    for (let i = 0; i < 3; i++) {
      createQuote(store, { seller_address: '0xS1', status: 'fulfilled' });
    }
    createQuote(store, { seller_address: '0xS1', status: 'accepted' });
    createQuote(store, { seller_address: '0xS1', status: 'declined' });

    const metrics = mp.getServiceMetrics(svc.id);

    assert.strictEqual(metrics.totalTransactions, 5);
    assert.strictEqual(metrics.fulfilledCount, 3);
    assert.strictEqual(metrics.acceptedCount, 1);
    assert.strictEqual(metrics.declinedCount, 1);
    assert.ok(Math.abs(metrics.successRate - 0.6) < 0.001);
  });

  it('computes avgResponseTimeMs', () => {
    const svc = createSeller(store, { agent_address: '0xS1' });

    const now = Date.now();
    // Quote 1: 2 seconds response
    createQuote(store, {
      seller_address: '0xS1',
      status: 'fulfilled',
      created_at: new Date(now - 5000).toISOString(),
      quoted_at: new Date(now - 3000).toISOString(),
    });
    // Quote 2: 4 seconds response
    createQuote(store, {
      seller_address: '0xS1',
      status: 'fulfilled',
      created_at: new Date(now - 10000).toISOString(),
      quoted_at: new Date(now - 6000).toISOString(),
    });

    const metrics = mp.getServiceMetrics(svc.id);
    // Average of 2000ms and 4000ms = 3000ms
    assert.strictEqual(metrics.avgResponseTimeMs, 3000);
  });

  it('ignores quotes without quoted_at for response time', () => {
    const svc = createSeller(store, { agent_address: '0xS1' });

    const now = Date.now();
    createQuote(store, {
      seller_address: '0xS1',
      status: 'fulfilled',
      created_at: new Date(now - 5000).toISOString(),
      quoted_at: new Date(now - 3000).toISOString(), // 2s
    });
    createQuote(store, {
      seller_address: '0xS1',
      status: 'requested',
      created_at: new Date(now - 5000).toISOString(),
      quoted_at: null, // no quoted_at
    });

    const metrics = mp.getServiceMetrics(svc.id);
    assert.strictEqual(metrics.avgResponseTimeMs, 2000);
  });

  it('returns null avgResponseTimeMs when no valid response times', () => {
    const svc = createSeller(store, { agent_address: '0xS1' });
    createQuote(store, { seller_address: '0xS1', status: 'requested', quoted_at: null });

    const metrics = mp.getServiceMetrics(svc.id);
    assert.strictEqual(metrics.avgResponseTimeMs, null);
  });

  it('computes disputeRate', () => {
    const svc = createSeller(store, { agent_address: '0xS1' });

    // Create 4 quotes
    for (let i = 0; i < 4; i++) {
      createQuote(store, { seller_address: '0xS1', status: 'fulfilled' });
    }

    // Create an escrow for the dispute (required field)
    const escrowId = randomUUID();
    store.createEscrow({
      id: escrowId,
      buyer_address: '0xBuyer',
      seller_address: '0xS1',
      amount: 100,
      amount_decimal: 100,
      expires_at: new Date(Date.now() + 86400000).toISOString(),
    });

    // Create 1 dispute filed against S1
    store.createDispute({
      escrow_id: escrowId,
      filed_by: '0xBuyer',
      filed_against: '0xS1',
      reason: 'Non-delivery',
      amount_disputed: 100,
      amount_decimal: 100,
      asset: 'USDC',
    });

    const metrics = mp.getServiceMetrics(svc.id);
    assert.ok(Math.abs(metrics.disputeRate - 0.25) < 0.001); // 1/4
  });
});

// ===========================================================================
// 7. getAgentStatus
// ===========================================================================

describe('Marketplace Service — getAgentStatus', () => {
  let store, mp;

  beforeEach(() => {
    store = freshStore();
    mp = createMarketplaceService(store, NULL_A2A);
  });

  it('returns default status for unknown agent', () => {
    const status = mp.getAgentStatus('0xUnknown');

    assert.strictEqual(status.agentAddress, '0xUnknown');
    assert.strictEqual(status.activeServices, 0);
    assert.strictEqual(status.services.length, 0);
    assert.strictEqual(status.pendingRFQs, 0);
    assert.strictEqual(status.reputation.average_score, 0);
    assert.strictEqual(status.reputation.trust_tier, 'sandbox');
    assert.strictEqual(status.reputation.total_transactions, 0);
  });

  it('returns active services for the agent', () => {
    createSeller(store, { agent_address: '0xAgent1', name: 'Svc A', category: 'goods' });
    createSeller(store, { agent_address: '0xAgent1', name: 'Svc B', category: 'services' });
    createSeller(store, { agent_address: '0xOther', name: 'Other Svc' }); // different agent

    const status = mp.getAgentStatus('0xAgent1');

    assert.strictEqual(status.activeServices, 2);
    assert.strictEqual(status.services.length, 2);
    const names = status.services.map((s) => s.name);
    assert.ok(names.includes('Svc A'));
    assert.ok(names.includes('Svc B'));
  });

  it('returns service details (id, name, category)', () => {
    const svc = createSeller(store, { agent_address: '0xAgent1', name: 'Test Svc', category: 'analytics' });

    const status = mp.getAgentStatus('0xAgent1');

    assert.strictEqual(status.services[0].id, svc.id);
    assert.strictEqual(status.services[0].name, 'Test Svc');
    assert.strictEqual(status.services[0].category, 'analytics');
  });

  it('excludes inactive services', () => {
    createSeller(store, { agent_address: '0xAgent1', name: 'Active', active: 1 });
    createSeller(store, { agent_address: '0xAgent1', name: 'Inactive', active: 0 });

    const status = mp.getAgentStatus('0xAgent1');
    assert.strictEqual(status.activeServices, 1);
    assert.strictEqual(status.services[0].name, 'Active');
  });

  it('includes reputation when available', () => {
    store.upsertReputationScore({
      agent_address: '0xAgent1',
      average_score: 4.7,
      trust_tier: 'trusted',
      total_transactions: 50,
    });

    const status = mp.getAgentStatus('0xAgent1');
    assert.strictEqual(status.reputation.average_score, 4.7);
    assert.strictEqual(status.reputation.trust_tier, 'trusted');
    assert.strictEqual(status.reputation.total_transactions, 50);
  });

  it('counts pending RFQ responses', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xAgent1', quote_id: randomUUID(), status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xAgent1', quote_id: randomUUID(), status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xAgent1', quote_id: randomUUID(), status: 'scored' }); // not pending

    const status = mp.getAgentStatus('0xAgent1');
    assert.strictEqual(status.pendingRFQs, 2);
  });

  it('does not count other agents pending RFQs', () => {
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xOther', quote_id: randomUUID(), status: 'pending' });

    const status = mp.getAgentStatus('0xAgent1');
    assert.strictEqual(status.pendingRFQs, 0);
  });
});

// ===========================================================================
// 8. Edge cases
// ===========================================================================

describe('Marketplace Service — Edge cases', () => {
  let store;

  beforeEach(() => {
    store = freshStore();
  });

  it('broadcastRFQ with all sellers being the buyer returns empty responses', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    createSeller(store, { agent_address: '0xBuyer' });
    createSeller(store, { agent_address: '0xBuyer' }); // duplicate same address

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
    });

    assert.strictEqual(result.sellersContacted, 0);
  });

  it('collectRFQResponses handles all unscored responses', () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: 'missing-1', status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: 'missing-2', status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);

    assert.strictEqual(result.totalResponses, 2);
    assert.strictEqual(result.scoredCount, 0);
    assert.strictEqual(result.unscored.length, 2);
    assert.strictEqual(result.ranked.length, 0);
  });

  it('awardRFQ with single scored response awards it', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });

    const result = await mp.awardRFQ(rfq.id);
    assert.strictEqual(result.losersDeclined, 0);
    assert.strictEqual(result.winnerAddress, '0xS1');
  });

  it('full lifecycle: broadcast -> collect -> award', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);

    // Create sellers
    createSeller(store, { agent_address: '0xS1', category: 'goods' });
    createSeller(store, { agent_address: '0xS2', category: 'goods' });

    // Step 1: Broadcast RFQ
    const broadcast = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 10 }],
      sellerFilter: 'goods',
      scoringCriteria: 'cheapest',
      deadlineMinutes: 60,
    });

    assert.strictEqual(broadcast.sellersContacted, 2);
    const rfqId = broadcast.rfq.id;

    // Simulate sellers providing quotes — update their quotes in store
    const responses = store.listRFQResponses({ rfq_id: rfqId });
    for (const resp of responses) {
      const quote = store.getQuote(resp.quote_id);
      if (!quote) {
        // When a2aService is null, quote_id is a random UUID with no actual quote.
        // Create a real quote for each response.
        const total = resp.seller_address === '0xS1' ? 80 : 120;
        const q = createQuote(store, {
          id: resp.quote_id,
          seller_address: resp.seller_address,
          buyer_address: '0xBuyer',
          total_decimal: total,
          status: 'quoted',
        });
      }
    }

    // Step 2: Collect and score responses
    const scored = mp.collectRFQResponses(rfqId);
    assert.strictEqual(scored.scoredCount, 2);
    assert.strictEqual(scored.ranked[0].seller_address, '0xS1'); // cheaper

    // Step 3: Award
    const award = await mp.awardRFQ(rfqId);
    assert.strictEqual(award.winnerAddress, '0xS1');
    assert.strictEqual(award.losersDeclined, 1);

    // Verify final state
    const finalRFQ = store.getRFQ(rfqId);
    assert.strictEqual(finalRFQ.status, 'awarded');
  });

  it('broadcastRFQ stores items as JSON string in RFQ', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    createSeller(store, { agent_address: '0xS1' });

    const items = [
      { description: 'Widget A', quantity: 5, unitPrice: 10 },
      { description: 'Widget B', quantity: 3, unitPrice: 25 },
    ];

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items,
    });

    const rfq = store.getRFQ(result.rfq.id);
    const parsedItems = JSON.parse(rfq.items);
    assert.strictEqual(parsedItems.length, 2);
    assert.strictEqual(parsedItems[0].description, 'Widget A');
    assert.strictEqual(parsedItems[1].quantity, 3);
  });

  it('awardRFQ cannot award same RFQ twice', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'scored', score: 0.01 });

    // First award succeeds
    await mp.awardRFQ(rfq.id);

    // Second award fails — RFQ is now 'awarded'
    await assert.rejects(
      () => mp.awardRFQ(rfq.id),
      /is awarded, not open/,
    );
  });

  it('getServiceMetrics counts only quotes for the specific service agent', () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const svc1 = createSeller(store, { agent_address: '0xS1' });
    createSeller(store, { agent_address: '0xS2' });

    createQuote(store, { seller_address: '0xS1', status: 'fulfilled' });
    createQuote(store, { seller_address: '0xS1', status: 'fulfilled' });
    createQuote(store, { seller_address: '0xS2', status: 'fulfilled' }); // different seller

    const metrics = mp.getServiceMetrics(svc1.id);
    assert.strictEqual(metrics.totalTransactions, 2);
  });

  it('collectRFQResponses with mixed scored and unscored', () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100, status: 'quoted' });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 50, status: 'quoted' });
    // S3 has a quote in 'requested' state — will be unscored
    const q3 = createQuote(store, { seller_address: '0xS3', total_decimal: 0, status: 'requested' });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS3', quote_id: q3.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);

    assert.strictEqual(result.totalResponses, 3);
    assert.strictEqual(result.scoredCount, 2);
    assert.strictEqual(result.unscored.length, 1);
    assert.strictEqual(result.unscored[0].seller_address, '0xS3');
  });

  it('broadcastRFQ with maxResponses=1 limits to one seller', async () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    createSeller(store, { agent_address: '0xS1' });
    createSeller(store, { agent_address: '0xS2' });
    createSeller(store, { agent_address: '0xS3' });

    const result = await mp.broadcastRFQ({
      buyerAddress: '0xBuyer',
      items: [{ description: 'Widget', quantity: 1 }],
      maxResponses: 1,
    });

    assert.strictEqual(result.sellersContacted, 1);
  });

  it('cheapest scoring with identical prices produces equal scores', () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'cheapest',
    });

    const q1 = createQuote(store, { seller_address: '0xS1', total_decimal: 100 });
    const q2 = createQuote(store, { seller_address: '0xS2', total_decimal: 100 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    assert.strictEqual(result.ranked[0].score, result.ranked[1].score);
  });

  it('cheapest scoring uses total_decimal over total', () => {
    const mp = createMarketplaceService(store, NULL_A2A);
    const rfq = store.createRFQ({
      buyer_address: '0xBuyer',
      items: '[]',
      deadline: new Date(Date.now() + 3600000).toISOString(),
      scoring_criteria: 'cheapest',
    });

    // total_decimal differs from total — scoring should use total_decimal first
    const q1 = createQuote(store, { seller_address: '0xS1', total: 1000, total_decimal: 10 });
    const q2 = createQuote(store, { seller_address: '0xS2', total: 500, total_decimal: 50 });

    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS1', quote_id: q1.id, status: 'pending' });
    store.createRFQResponse({ rfq_id: rfq.id, seller_address: '0xS2', quote_id: q2.id, status: 'pending' });

    const result = mp.collectRFQResponses(rfq.id);
    // total_decimal: S1=10 cheaper than S2=50
    assert.strictEqual(result.ranked[0].seller_address, '0xS1');
  });
});
