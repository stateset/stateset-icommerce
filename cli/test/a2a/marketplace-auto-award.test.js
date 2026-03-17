/**
 * Tests for marketplace auto-award and maintenance methods in
 * cli/src/a2a/marketplace.js
 *
 * Focuses on: autoAwardExpiredRFQs() and maintenanceTick().
 * Uses mock store to avoid native module dependency.
 */

import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createMarketplaceService } from '../../src/a2a/marketplace.js';

function createMockStore(config = {}) {
  const rfqs = config.rfqs || [];
  const rfqResponses = config.rfqResponses || [];
  const quotes = config.quotes || [];
  const services = config.services || [];

  return {
    listRFQs: (filter) =>
      rfqs.filter((r) => (!filter?.status || r.status === filter.status)),
    getRFQ: (id) => rfqs.find((r) => r.id === id) || null,
    updateRFQ: (id, updates) => {
      const r = rfqs.find((rf) => rf.id === id);
      if (r) Object.assign(r, updates);
      return r;
    },
    createRFQ: (data) => {
      const r = { id: `rfq-${rfqs.length + 1}`, status: 'open', ...data };
      rfqs.push(r);
      return r;
    },
    listRFQResponses: (filter) =>
      rfqResponses.filter((r) => (!filter?.rfq_id || r.rfq_id === filter.rfq_id)),
    createRFQResponse: (data) => {
      const r = { id: `resp-${rfqResponses.length + 1}`, ...data };
      rfqResponses.push(r);
      return r;
    },
    updateRFQResponse: (id, updates) => {
      const r = rfqResponses.find((resp) => resp.id === id);
      if (r) Object.assign(r, updates);
      return r;
    },
    getQuote: (id) => quotes.find((q) => q.id === id) || null,
    listQuotes: () => quotes,
    getReputationScore: () => ({ average_score: 4.0 }),
    listServices: () => services,
    getService: (id) => services.find((s) => s.id === id) || null,
    listDisputes: () => [],
    listFeedback: () => [],
  };
}

describe('marketplace autoAwardExpiredRFQs()', () => {
  const pastDeadline = new Date(Date.now() - 3600_000).toISOString();
  const futureDeadline = new Date(Date.now() + 3600_000).toISOString();

  it('awards expired RFQ to highest-scored response', async () => {
    const store = createMockStore({
      rfqs: [{ id: 'rfq-1', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' }],
      rfqResponses: [
        { id: 'resp-1', rfq_id: 'rfq-1', seller_address: '0xA', quote_id: 'q-1', score: null },
        { id: 'resp-2', rfq_id: 'rfq-1', seller_address: '0xB', quote_id: 'q-2', score: null },
      ],
      quotes: [
        { id: 'q-1', status: 'quoted', total_decimal: 100, seller_address: '0xA' },
        { id: 'q-2', status: 'quoted', total_decimal: 50, seller_address: '0xB' },
      ],
    });
    const a2a = {
      acceptQuote: mock.fn(async () => ({ success: true })),
      declineQuote: mock.fn(async () => ({ success: true })),
    };
    const mp = createMarketplaceService(store, a2a);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.awarded, 1);
    assert.equal(result.awards.length, 1);
    assert.equal(result.awards[0].winnerAddress, '0xB');
  });

  it('expires RFQ with no scored responses', async () => {
    const store = createMockStore({
      rfqs: [{ id: 'rfq-1', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' }],
      rfqResponses: [
        { id: 'resp-1', rfq_id: 'rfq-1', seller_address: '0xA', quote_id: 'q-missing', score: null },
      ],
      quotes: [],
    });
    const mp = createMarketplaceService(store, null);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.expired, 1);
    assert.equal(result.awarded, 0);
  });

  it('skips RFQs not past deadline', async () => {
    const store = createMockStore({
      rfqs: [{ id: 'rfq-1', status: 'open', deadline: futureDeadline, scoring_criteria: 'cheapest' }],
    });
    const mp = createMarketplaceService(store, null);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.skipped, 1);
    assert.equal(result.awarded, 0);
    assert.equal(result.expired, 0);
  });

  it('returns zeros when there are no open RFQs', async () => {
    const store = createMockStore();
    const mp = createMarketplaceService(store, null);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.awarded, 0);
    assert.equal(result.expired, 0);
    assert.equal(result.skipped, 0);
  });

  it('expires RFQ when all responses have unquoted status', async () => {
    const store = createMockStore({
      rfqs: [{ id: 'rfq-1', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' }],
      rfqResponses: [
        { id: 'resp-1', rfq_id: 'rfq-1', seller_address: '0xA', quote_id: 'q-1', score: null },
      ],
      quotes: [
        { id: 'q-1', status: 'requested', total_decimal: 0, seller_address: '0xA' },
      ],
    });
    const mp = createMarketplaceService(store, null);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.expired, 1, 'should expire when quotes are still in requested status');
    assert.equal(result.awarded, 0);
  });

  it('skips RFQ with null deadline', async () => {
    const store = createMockStore({
      rfqs: [{ id: 'rfq-1', status: 'open', deadline: null, scoring_criteria: 'cheapest' }],
    });
    const mp = createMarketplaceService(store, null);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.skipped, 1, 'should skip RFQ with null deadline');
    assert.equal(result.awarded, 0);
    assert.equal(result.expired, 0);
  });

  it('handles mix of expired-with-responses and expired-without-responses', async () => {
    const store = createMockStore({
      rfqs: [
        { id: 'rfq-1', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' },
        { id: 'rfq-2', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' },
      ],
      rfqResponses: [
        { id: 'resp-1', rfq_id: 'rfq-1', seller_address: '0xA', quote_id: 'q-1', score: null },
        // rfq-2 has no responses
      ],
      quotes: [
        { id: 'q-1', status: 'quoted', total_decimal: 75, seller_address: '0xA' },
      ],
    });
    const mp = createMarketplaceService(store, null);

    const result = await mp.autoAwardExpiredRFQs();
    assert.equal(result.awarded, 1, 'should award rfq-1');
    assert.equal(result.expired, 1, 'should expire rfq-2');
  });

  it('does not modify skipped RFQ status in store', async () => {
    const rfq = { id: 'rfq-1', status: 'open', deadline: futureDeadline, scoring_criteria: 'cheapest' };
    const store = createMockStore({ rfqs: [rfq] });
    const mp = createMarketplaceService(store, null);

    await mp.autoAwardExpiredRFQs();
    assert.equal(rfq.status, 'open', 'future RFQ should remain open');
  });

  it('updates RFQ status to awarded after auto-award', async () => {
    const rfq = { id: 'rfq-1', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' };
    const store = createMockStore({
      rfqs: [rfq],
      rfqResponses: [
        { id: 'resp-1', rfq_id: 'rfq-1', seller_address: '0xA', quote_id: 'q-1', score: null },
      ],
      quotes: [
        { id: 'q-1', status: 'quoted', total_decimal: 50, seller_address: '0xA' },
      ],
    });
    const mp = createMarketplaceService(store, null);

    await mp.autoAwardExpiredRFQs();
    assert.equal(rfq.status, 'awarded');
  });
});

describe('marketplace maintenanceTick()', () => {
  const pastDeadline = new Date(Date.now() - 3600_000).toISOString();

  it('returns timestamp and combined results', async () => {
    const store = createMockStore();
    const mp = createMarketplaceService(store, null);

    const result = await mp.maintenanceTick();
    assert.ok(result.timestamp);
    assert.equal(typeof result.awarded, 'number');
    assert.equal(typeof result.expired, 'number');
  });

  it('timestamp is a valid ISO date', async () => {
    const store = createMockStore();
    const mp = createMarketplaceService(store, null);

    const result = await mp.maintenanceTick();
    assert.ok(!isNaN(Date.parse(result.timestamp)), 'timestamp should be valid ISO');
  });

  it('includes auto-award results in tick output', async () => {
    const store = createMockStore({
      rfqs: [{ id: 'rfq-1', status: 'open', deadline: pastDeadline, scoring_criteria: 'cheapest' }],
      rfqResponses: [
        { id: 'resp-1', rfq_id: 'rfq-1', seller_address: '0xA', quote_id: 'q-1', score: null },
      ],
      quotes: [{ id: 'q-1', status: 'quoted', total_decimal: 80, seller_address: '0xA' }],
    });
    const mp = createMarketplaceService(store, null);

    const result = await mp.maintenanceTick();
    assert.equal(result.awarded, 1);
    assert.ok(Array.isArray(result.awards));
    assert.equal(result.awards.length, 1);
  });

  it('returns zeros when no open RFQs', async () => {
    const store = createMockStore();
    const mp = createMarketplaceService(store, null);

    const result = await mp.maintenanceTick();
    assert.equal(result.awarded, 0);
    assert.equal(result.expired, 0);
    assert.equal(result.skipped, 0);
    assert.deepStrictEqual(result.awards, []);
  });
});
