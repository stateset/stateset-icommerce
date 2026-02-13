/**
 * Unit tests for A2A negotiation — counterQuote and reviseQuote
 *
 * Tests the counter-offer negotiation flow added to cli/src/a2a/index.js.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createA2AService } from '../../src/a2a/index.js';

// ===========================================================================
// Helpers
// ===========================================================================

const BUYER = '0xBuyer';
const SELLER = '0xSeller';
const OTHER = '0xOther';

/**
 * Create a mock commerce.a2a() store backed by an in-memory Map.
 */
function createMockA2A() {
  const quotes = new Map();
  const a2aStore = {
    getQuote: async (id) => quotes.get(id) || null,
    updateQuote: async (id, updates) => {
      const q = quotes.get(id);
      if (q) {
        const updated = { ...q, ...updates };
        if (updates.negotiation_history) {
          updated.negotiation_history = updates.negotiation_history;
        }
        quotes.set(id, updated);
      }
    },
    createQuote: async (record) => {
      quotes.set(record.id, { ...record });
    },
    createPayment: async () => {},
    getPayment: async () => null,
    updatePayment: async () => {},
    listPayments: async () => [],
    sumPayments: async () => ({ total: 0 }),
    createPaymentRequest: async () => {},
    getPaymentRequest: async () => null,
    updatePaymentRequest: async () => {},
    listPaymentRequests: async () => [],
    listQuotes: async () => [...quotes.values()],
  };

  const commerce = { a2a: () => a2aStore };
  return { commerce, a2aStore, quotes };
}

/**
 * Seed a quote into the in-memory store with sensible defaults.
 */
function seedQuote(quotes, overrides = {}) {
  const base = {
    id: 'quote-1',
    status: 'quoted',
    buyer_address: BUYER,
    seller_address: SELLER,
    buyer_agent_id: null,
    seller_agent_id: null,
    items: '[]',
    subtotal: 100_000_000,
    fees: 0,
    tax: 0,
    total: 100_000_000,
    total_decimal: 100,
    asset: 'USDC',
    accepted_networks: '["set_chain"]',
    expires_at: new Date(Date.now() + 86_400_000).toISOString(),
    counter_count: 0,
    max_rounds: 5,
    negotiation_history: [],
    terms: null,
    estimated_delivery: null,
    delivery_method: null,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    quoted_at: new Date().toISOString(),
  };
  const quote = { ...base, ...overrides };
  quotes.set(quote.id, quote);
  return quote;
}

// ===========================================================================
// counterQuote — validation
// ===========================================================================

describe('counterQuote', () => {
  let svc;
  let quotes;
  let commerce;

  beforeEach(() => {
    const mock = createMockA2A();
    commerce = mock.commerce;
    quotes = mock.quotes;
    svc = createA2AService(commerce, { walletAddress: BUYER });
  });

  // ---- validation ---------------------------------------------------------

  it('throws when quote is not found', async () => {
    await assert.rejects(() => svc.counterQuote('nonexistent', { total: 80 }), {
      message: 'Quote not found',
    });
  });

  it('throws when status is not quoted', async () => {
    seedQuote(quotes, { status: 'requested' });
    await assert.rejects(
      () => svc.counterQuote('quote-1', { total: 80 }),
      (err) => {
        assert.match(err.message, /Cannot counter a quote in status: requested/);
        return true;
      },
    );
  });

  it('throws when status is counter_offered', async () => {
    seedQuote(quotes, { status: 'counter_offered' });
    await assert.rejects(
      () => svc.counterQuote('quote-1', { total: 80 }),
      (err) => {
        assert.match(err.message, /Must be 'quoted'/);
        return true;
      },
    );
  });

  it('throws when status is accepted', async () => {
    seedQuote(quotes, { status: 'accepted' });
    await assert.rejects(
      () => svc.counterQuote('quote-1', { total: 80 }),
      (err) => {
        assert.match(err.message, /Cannot counter a quote in status: accepted/);
        return true;
      },
    );
  });

  it('throws when caller is not the buyer', async () => {
    seedQuote(quotes);
    // Create a service with a different wallet (not the buyer)
    const otherSvc = createA2AService(commerce, { walletAddress: SELLER });
    await assert.rejects(() => otherSvc.counterQuote('quote-1', { total: 80 }), {
      message: 'Only the buyer can counter a quote',
    });
  });

  it('throws when a third party tries to counter', async () => {
    seedQuote(quotes);
    const thirdSvc = createA2AService(commerce, { walletAddress: OTHER });
    await assert.rejects(() => thirdSvc.counterQuote('quote-1', { total: 80 }), {
      message: 'Only the buyer can counter a quote',
    });
  });

  it('throws when max rounds are reached', async () => {
    seedQuote(quotes, { counter_count: 5, max_rounds: 5 });
    await assert.rejects(
      () => svc.counterQuote('quote-1', { total: 80 }),
      (err) => {
        assert.match(err.message, /Maximum negotiation rounds reached/);
        return true;
      },
    );
  });

  it('throws when counter_count equals max_rounds exactly', async () => {
    seedQuote(quotes, { counter_count: 3, max_rounds: 3 });
    await assert.rejects(
      () => svc.counterQuote('quote-1', { total: 80 }),
      (err) => {
        assert.match(err.message, /Maximum negotiation rounds reached \(3\)/);
        return true;
      },
    );
  });

  it('throws when quote has expired', async () => {
    seedQuote(quotes, { expires_at: new Date(Date.now() - 1000).toISOString() });
    await assert.rejects(() => svc.counterQuote('quote-1', { total: 80 }), {
      message: 'Quote has expired',
    });
  });

  // ---- success case -------------------------------------------------------

  it('succeeds and returns round number', async () => {
    seedQuote(quotes);
    const result = await svc.counterQuote('quote-1', { total: 80, message: 'Too high' });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.round, 1);
  });

  it('sets status to counter_offered', async () => {
    seedQuote(quotes);
    await svc.counterQuote('quote-1', { total: 80 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.status, 'counter_offered');
  });

  it('increments counter_count', async () => {
    seedQuote(quotes, { counter_count: 2 });
    const result = await svc.counterQuote('quote-1', { total: 80 });

    assert.strictEqual(result.round, 3);
    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.counter_count, 3);
  });

  it('converts total to smallest unit (USDC 6 decimals)', async () => {
    seedQuote(quotes);
    await svc.counterQuote('quote-1', { total: 80 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.total, 80_000_000);
    assert.strictEqual(stored.total_decimal, 80);
  });

  it('appends to negotiation_history', async () => {
    seedQuote(quotes);
    await svc.counterQuote('quote-1', { total: 80, message: 'Too expensive' });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 1);

    const entry = stored.negotiation_history[0];
    assert.strictEqual(entry.round, 1);
    assert.strictEqual(entry.type, 'counter');
    assert.strictEqual(entry.from, 'buyer');
    assert.strictEqual(entry.amount, 80);
    assert.strictEqual(entry.message, 'Too expensive');
    assert.ok(entry.timestamp);
  });

  it('preserves existing negotiation history entries', async () => {
    const priorHistory = [
      {
        round: 1,
        type: 'counter',
        from: 'buyer',
        amount: 90,
        message: null,
        timestamp: new Date().toISOString(),
      },
      {
        round: 2,
        type: 'revision',
        from: 'seller',
        amount: 95,
        message: null,
        timestamp: new Date().toISOString(),
      },
    ];
    seedQuote(quotes, { counter_count: 2, negotiation_history: priorHistory });

    await svc.counterQuote('quote-1', { total: 85 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 3);
    assert.strictEqual(stored.negotiation_history[0].round, 1);
    assert.strictEqual(stored.negotiation_history[2].round, 3);
    assert.strictEqual(stored.negotiation_history[2].amount, 85);
  });

  it('returns formatted quote in result', async () => {
    seedQuote(quotes);
    const result = await svc.counterQuote('quote-1', { total: 80 });

    assert.ok(result.quote);
    assert.strictEqual(result.quote.id, 'quote-1');
    assert.strictEqual(result.quote.status, 'counter_offered');
    assert.strictEqual(result.quote.buyer, BUYER);
    assert.strictEqual(result.quote.seller, SELLER);
  });

  it('handles null message gracefully', async () => {
    seedQuote(quotes);
    const result = await svc.counterQuote('quote-1', { total: 80 });

    assert.strictEqual(result.success, true);
    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history[0].message, null);
  });
});

// ===========================================================================
// reviseQuote — validation
// ===========================================================================

describe('reviseQuote', () => {
  let svc;
  let quotes;
  let commerce;

  beforeEach(() => {
    const mock = createMockA2A();
    commerce = mock.commerce;
    quotes = mock.quotes;
    // Seller is the one who revises
    svc = createA2AService(commerce, { walletAddress: SELLER });
  });

  // ---- validation ---------------------------------------------------------

  it('throws when quote is not found', async () => {
    await assert.rejects(() => svc.reviseQuote('nonexistent', { total: 90 }), {
      message: 'Quote not found',
    });
  });

  it('throws when status is not counter_offered', async () => {
    seedQuote(quotes, { status: 'quoted' });
    await assert.rejects(
      () => svc.reviseQuote('quote-1', { total: 90 }),
      (err) => {
        assert.match(err.message, /Cannot revise a quote in status: quoted/);
        return true;
      },
    );
  });

  it('throws when status is requested', async () => {
    seedQuote(quotes, { status: 'requested' });
    await assert.rejects(
      () => svc.reviseQuote('quote-1', { total: 90 }),
      (err) => {
        assert.match(err.message, /Must be 'counter_offered'/);
        return true;
      },
    );
  });

  it('throws when caller is not the seller', async () => {
    seedQuote(quotes, { status: 'counter_offered' });
    const buyerSvc = createA2AService(commerce, { walletAddress: BUYER });
    await assert.rejects(() => buyerSvc.reviseQuote('quote-1', { total: 90 }), {
      message: 'Only the seller can revise a quote',
    });
  });

  it('throws when a third party tries to revise', async () => {
    seedQuote(quotes, { status: 'counter_offered' });
    const thirdSvc = createA2AService(commerce, { walletAddress: OTHER });
    await assert.rejects(() => thirdSvc.reviseQuote('quote-1', { total: 90 }), {
      message: 'Only the seller can revise a quote',
    });
  });

  it('throws when quote has expired', async () => {
    seedQuote(quotes, {
      status: 'counter_offered',
      expires_at: new Date(Date.now() - 1000).toISOString(),
    });
    await assert.rejects(() => svc.reviseQuote('quote-1', { total: 90 }), {
      message: 'Quote has expired',
    });
  });

  // ---- success case -------------------------------------------------------

  it('succeeds and returns round number', async () => {
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1 });
    const result = await svc.reviseQuote('quote-1', { total: 90, message: 'Best I can do' });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.round, 2);
  });

  it('sets status back to quoted', async () => {
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1 });
    await svc.reviseQuote('quote-1', { total: 90 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.status, 'quoted');
  });

  it('increments counter_count', async () => {
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1 });
    const result = await svc.reviseQuote('quote-1', { total: 90 });

    assert.strictEqual(result.round, 2);
    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.counter_count, 2);
  });

  it('updates pricing fields (total, fees, tax)', async () => {
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1 });
    await svc.reviseQuote('quote-1', { total: 90, fees: 5, tax: 3 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.total, 90_000_000);
    assert.strictEqual(stored.total_decimal, 90);
    assert.strictEqual(stored.fees, 5_000_000);
    assert.strictEqual(stored.tax, 3_000_000);
  });

  it('defaults fees and tax to 0 when not provided', async () => {
    seedQuote(quotes, {
      status: 'counter_offered',
      counter_count: 1,
      fees: 10_000_000,
      tax: 5_000_000,
    });
    await svc.reviseQuote('quote-1', { total: 90 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.fees, 0);
    assert.strictEqual(stored.tax, 0);
  });

  it('appends to negotiation_history', async () => {
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1 });
    await svc.reviseQuote('quote-1', { total: 90, fees: 5, tax: 3, message: 'Adjusted price' });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 1);

    const entry = stored.negotiation_history[0];
    assert.strictEqual(entry.round, 2);
    assert.strictEqual(entry.type, 'revision');
    assert.strictEqual(entry.from, 'seller');
    assert.strictEqual(entry.amount, 90);
    assert.strictEqual(entry.fees, 5);
    assert.strictEqual(entry.tax, 3);
    assert.strictEqual(entry.message, 'Adjusted price');
    assert.ok(entry.timestamp);
  });

  it('preserves existing negotiation history entries', async () => {
    const priorHistory = [
      {
        round: 1,
        type: 'counter',
        from: 'buyer',
        amount: 80,
        message: null,
        timestamp: new Date().toISOString(),
      },
    ];
    seedQuote(quotes, {
      status: 'counter_offered',
      counter_count: 1,
      negotiation_history: priorHistory,
    });

    await svc.reviseQuote('quote-1', { total: 90 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 2);
    assert.strictEqual(stored.negotiation_history[0].type, 'counter');
    assert.strictEqual(stored.negotiation_history[1].type, 'revision');
    assert.strictEqual(stored.negotiation_history[1].round, 2);
  });

  it('returns formatted quote in result', async () => {
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1 });
    const result = await svc.reviseQuote('quote-1', { total: 90 });

    assert.ok(result.quote);
    assert.strictEqual(result.quote.id, 'quote-1');
    assert.strictEqual(result.quote.status, 'quoted');
  });

  it('updates quoted_at timestamp', async () => {
    const oldQuotedAt = new Date(Date.now() - 60_000).toISOString();
    seedQuote(quotes, { status: 'counter_offered', counter_count: 1, quoted_at: oldQuotedAt });
    await svc.reviseQuote('quote-1', { total: 90 });

    const stored = quotes.get('quote-1');
    assert.notStrictEqual(stored.quoted_at, oldQuotedAt);
  });
});

// ===========================================================================
// Multi-round negotiation flow
// ===========================================================================

describe('multi-round negotiation flow', () => {
  let buyerSvc;
  let sellerSvc;
  let quotes;

  beforeEach(() => {
    const mock = createMockA2A();
    quotes = mock.quotes;
    buyerSvc = createA2AService(mock.commerce, { walletAddress: BUYER });
    sellerSvc = createA2AService(mock.commerce, { walletAddress: SELLER });
  });

  it('supports a full counter -> revise -> counter -> accept flow', async () => {
    // Seller initially quoted at 100
    seedQuote(quotes);

    // Round 1: buyer counters at 80
    const counter1 = await buyerSvc.counterQuote('quote-1', { total: 80, message: 'Too high' });
    assert.strictEqual(counter1.round, 1);
    assert.strictEqual(counter1.quote.status, 'counter_offered');

    // Round 2: seller revises to 90
    const revise1 = await sellerSvc.reviseQuote('quote-1', {
      total: 90,
      fees: 2,
      message: 'Meet in the middle',
    });
    assert.strictEqual(revise1.round, 2);
    assert.strictEqual(revise1.quote.status, 'quoted');

    // Round 3: buyer counters at 85
    const counter2 = await buyerSvc.counterQuote('quote-1', { total: 85, message: 'A bit lower' });
    assert.strictEqual(counter2.round, 3);
    assert.strictEqual(counter2.quote.status, 'counter_offered');

    // Round 4: seller revises to 87
    const revise2 = await sellerSvc.reviseQuote('quote-1', { total: 87, message: 'Final offer' });
    assert.strictEqual(revise2.round, 4);
    assert.strictEqual(revise2.quote.status, 'quoted');

    // Buyer accepts the revised quote
    const accepted = await buyerSvc.acceptQuote('quote-1');
    assert.strictEqual(accepted.success, true);
    assert.strictEqual(accepted.quote.status, 'accepted');

    // Verify full negotiation history was preserved
    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 4);
    assert.strictEqual(stored.negotiation_history[0].type, 'counter');
    assert.strictEqual(stored.negotiation_history[1].type, 'revision');
    assert.strictEqual(stored.negotiation_history[2].type, 'counter');
    assert.strictEqual(stored.negotiation_history[3].type, 'revision');
  });

  it('alternates buyer/seller correctly in history', async () => {
    seedQuote(quotes);

    await buyerSvc.counterQuote('quote-1', { total: 80 });
    await sellerSvc.reviseQuote('quote-1', { total: 95 });
    await buyerSvc.counterQuote('quote-1', { total: 88 });

    const stored = quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history[0].from, 'buyer');
    assert.strictEqual(stored.negotiation_history[1].from, 'seller');
    assert.strictEqual(stored.negotiation_history[2].from, 'buyer');
  });

  it('buyer can decline instead of accepting after revisions', async () => {
    seedQuote(quotes);

    await buyerSvc.counterQuote('quote-1', { total: 50 });
    await sellerSvc.reviseQuote('quote-1', { total: 98 });

    // Buyer decides the revision is not good enough
    const declined = await buyerSvc.declineQuote('quote-1', 'Price still too high');
    assert.strictEqual(declined.success, true);
    assert.strictEqual(declined.quote.status, 'declined');
  });
});

// ===========================================================================
// Max rounds enforcement
// ===========================================================================

describe('max rounds enforcement', () => {
  let buyerSvc;
  let sellerSvc;
  let quotes;

  beforeEach(() => {
    const mock = createMockA2A();
    quotes = mock.quotes;
    buyerSvc = createA2AService(mock.commerce, { walletAddress: BUYER });
    sellerSvc = createA2AService(mock.commerce, { walletAddress: SELLER });
  });

  it('blocks counter after max_rounds is exhausted', async () => {
    seedQuote(quotes, { max_rounds: 2 });

    // Round 1: counter
    await buyerSvc.counterQuote('quote-1', { total: 80 });
    // Round 2: revise
    await sellerSvc.reviseQuote('quote-1', { total: 90 });

    // Round 2 is the counter_count now, which equals max_rounds
    // The buyer tries to counter again -- should be blocked
    await assert.rejects(
      () => buyerSvc.counterQuote('quote-1', { total: 85 }),
      (err) => {
        assert.match(err.message, /Maximum negotiation rounds reached/);
        return true;
      },
    );
  });

  it('respects custom max_rounds setting', async () => {
    seedQuote(quotes, { max_rounds: 1 });

    // Round 1 counter works
    await buyerSvc.counterQuote('quote-1', { total: 80 });

    // After revise, round 2 would violate max_rounds=1 is already reached after round 1
    // Actually counter_count=1, but revise increments to 2; the limit check is only in counterQuote.
    // Let seller revise:
    await sellerSvc.reviseQuote('quote-1', { total: 90 });

    // Now counter_count is 2, exceeds max_rounds=1
    await assert.rejects(
      () => buyerSvc.counterQuote('quote-1', { total: 85 }),
      (err) => {
        assert.match(err.message, /Maximum negotiation rounds reached \(1\)/);
        return true;
      },
    );
  });

  it('defaults to max_rounds=5 when not set on quote', async () => {
    seedQuote(quotes, { max_rounds: undefined, counter_count: 0 });

    // Both counterQuote and reviseQuote increment counter_count.
    // Sequence: counter(0→1), revise(1→2), counter(2→3), revise(3→4), counter(4→5)
    await buyerSvc.counterQuote('quote-1', { total: 80 }); // counter_count 1
    await sellerSvc.reviseQuote('quote-1', { total: 95 }); // counter_count 2
    await buyerSvc.counterQuote('quote-1', { total: 82 }); // counter_count 3
    await sellerSvc.reviseQuote('quote-1', { total: 93 }); // counter_count 4
    await buyerSvc.counterQuote('quote-1', { total: 84 }); // counter_count 5

    // Seller revises one more time
    await sellerSvc.reviseQuote('quote-1', { total: 91 }); // counter_count 6

    // counterCount=6 >= maxRounds=5 → blocked
    await assert.rejects(
      () => buyerSvc.counterQuote('quote-1', { total: 70 }),
      (err) => {
        assert.match(err.message, /Maximum negotiation rounds reached \(5\)/);
        return true;
      },
    );
  });

  it('seller can still accept or buyer can accept even at max rounds', async () => {
    seedQuote(quotes, { max_rounds: 1 });

    // Round 1
    await buyerSvc.counterQuote('quote-1', { total: 80 });
    // Seller revises (round 2)
    await sellerSvc.reviseQuote('quote-1', { total: 90 });

    // Cannot counter, but CAN accept
    const accepted = await buyerSvc.acceptQuote('quote-1');
    assert.strictEqual(accepted.success, true);
    assert.strictEqual(accepted.quote.status, 'accepted');
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('negotiation edge cases', () => {
  it('throws if walletAddress not provided to createA2AService', () => {
    const { commerce } = createMockA2A();
    assert.throws(() => createA2AService(commerce, {}), {
      message: 'walletAddress is required for A2A service',
    });
  });

  it('counterQuote handles empty negotiation_history (null)', async () => {
    const mock = createMockA2A();
    seedQuote(mock.quotes, { negotiation_history: null });
    const svc = createA2AService(mock.commerce, { walletAddress: BUYER });

    // Should not throw — treats null as empty array
    const result = await svc.counterQuote('quote-1', { total: 80 });
    assert.strictEqual(result.success, true);

    const stored = mock.quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 1);
  });

  it('reviseQuote handles empty negotiation_history (null)', async () => {
    const mock = createMockA2A();
    seedQuote(mock.quotes, {
      status: 'counter_offered',
      counter_count: 1,
      negotiation_history: null,
    });
    const svc = createA2AService(mock.commerce, { walletAddress: SELLER });

    const result = await svc.reviseQuote('quote-1', { total: 90 });
    assert.strictEqual(result.success, true);

    const stored = mock.quotes.get('quote-1');
    assert.strictEqual(stored.negotiation_history.length, 1);
  });

  it('counterQuote handles negotiation_history as a non-array (string)', async () => {
    const mock = createMockA2A();
    seedQuote(mock.quotes, { negotiation_history: 'not an array' });
    const svc = createA2AService(mock.commerce, { walletAddress: BUYER });

    // Array.isArray('not an array') === false, so it starts fresh
    const result = await svc.counterQuote('quote-1', { total: 80 });
    assert.strictEqual(result.success, true);
  });
});
