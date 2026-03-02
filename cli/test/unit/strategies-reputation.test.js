/**
 * Unit tests for the ReputationAware negotiation strategy.
 *
 * Tests createReputationAwareStrategy from cli/src/a2a/strategies.js —
 * trust-tier gates, reputation-based discounts, markup adjustments for
 * high-trust buyers, counter-offer margin floors, payment gating, and
 * post-fulfillment ratings.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createReputationAwareStrategy } from '../../src/a2a/strategies.js';

// ===========================================================================
// Helpers
// ===========================================================================

function makeCtx(overrides = {}) {
  const canAffordFn = overrides.canAfford ?? (() => true);
  return {
    runtime: {
      canAfford: canAffordFn,
    },
    budget: {
      perTransaction: 500,
      daily: 1000,
    },
    ...overrides,
  };
}

function makeQuote(overrides = {}) {
  return {
    id: 'quote-1',
    total: 100,
    total_decimal: 100,
    items: [
      { description: 'Service A', unitPrice: 50, quantity: 2 },
    ],
    counter_count: 0,
    ...overrides,
  };
}

// ===========================================================================
// Construction / Defaults
// ===========================================================================

describe('ReputationAware Strategy — construction', () => {
  it('has the correct name', () => {
    const strategy = createReputationAwareStrategy();
    assert.equal(strategy.name, 'reputation-aware');
  });

  it('uses default options when none are provided', () => {
    const strategy = createReputationAwareStrategy();
    // A standard-tier seller with score 4 should be accepted (not gated)
    const quote = makeQuote({
      _sellerTrustTier: 'standard',
      _sellerAvgScore: 4,
      counter_count: 0,
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    // standard tier, not verified/enterprise => no discount counter => accept
    assert.equal(decision.action, 'accept');
  });
});

// ===========================================================================
// evaluateReceivedQuote
// ===========================================================================

describe('ReputationAware Strategy — evaluateReceivedQuote', () => {
  let strategy;

  beforeEach(() => {
    strategy = createReputationAwareStrategy({
      minTrustTier: 'standard',
      minAvgScore: 3.5,
      reputationDiscount: 0.05,
      enterpriseDiscount: 0.10,
      maxRounds: 2,
    });
  });

  it('declines when budget cannot afford the quote', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const quote = makeQuote({ total: 9999, _sellerTrustTier: 'enterprise' });
    const decision = strategy.evaluateReceivedQuote(quote, ctx);
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('Cannot afford'));
  });

  it('declines a seller below the minimum trust tier', () => {
    const quote = makeQuote({ _sellerTrustTier: 'sandbox' });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('sandbox'));
    assert.ok(decision.reason.includes('below minimum'));
  });

  it('declines when seller tier is missing (defaults to sandbox)', () => {
    // No _sellerTrustTier => defaults to 'sandbox' which is rank 0 < standard (1)
    const quote = makeQuote({});
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    assert.equal(decision.action, 'decline');
  });

  it('declines a seller with positive score below minAvgScore', () => {
    const quote = makeQuote({
      _sellerTrustTier: 'verified',
      _sellerAvgScore: 2.0,
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('score'));
    assert.ok(decision.reason.includes('2'));
  });

  it('does not decline when seller score is 0 (unrated)', () => {
    // score === 0 skips the score gate (condition: score > 0 && score < min)
    const quote = makeQuote({
      _sellerTrustTier: 'standard',
      _sellerAvgScore: 0,
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    // standard tier, no discount => accept
    assert.equal(decision.action, 'accept');
  });

  it('accepts after reaching maxRounds regardless of tier', () => {
    const quote = makeQuote({
      _sellerTrustTier: 'verified',
      _sellerAvgScore: 5,
      counter_count: 2, // maxRounds = 2
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('counters with 5% discount for verified seller on round 0', () => {
    const quote = makeQuote({
      total: 200,
      _sellerTrustTier: 'verified',
      _sellerAvgScore: 4.5,
      counter_count: 0,
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    assert.equal(decision.action, 'counter');
    // 200 * (1 - 0.05) = 190
    assert.equal(decision.total, 190);
    assert.ok(decision.message.includes('5%'));
    assert.ok(decision.message.includes('verified'));
  });

  it('counters with 10% discount for enterprise seller on round 0', () => {
    const quote = makeQuote({
      total: 200,
      _sellerTrustTier: 'enterprise',
      _sellerAvgScore: 4.8,
      counter_count: 0,
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    assert.equal(decision.action, 'counter');
    // 200 * (1 - 0.10) = 180
    assert.equal(decision.total, 180);
    assert.ok(decision.message.includes('10%'));
    assert.ok(decision.message.includes('enterprise'));
  });

  it('does not counter for verified seller after round 0', () => {
    const quote = makeQuote({
      total: 200,
      _sellerTrustTier: 'verified',
      _sellerAvgScore: 4.5,
      counter_count: 1, // round 1, not 0
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    // discount only on round 0 => accept
    assert.equal(decision.action, 'accept');
  });

  it('accepts a standard-tier seller without countering', () => {
    const quote = makeQuote({
      total: 100,
      _sellerTrustTier: 'standard',
      _sellerAvgScore: 4.0,
      counter_count: 0,
    });
    const decision = strategy.evaluateReceivedQuote(quote, makeCtx());
    // standard tier => discountRate = 0 => no counter => accept
    assert.equal(decision.action, 'accept');
  });

  it('budget check runs before tier gate', () => {
    // Even a sandbox seller triggers budget decline first
    const ctx = makeCtx({ canAfford: () => false });
    const quote = makeQuote({
      total: 500,
      _sellerTrustTier: 'sandbox',
    });
    const decision = strategy.evaluateReceivedQuote(quote, ctx);
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('Cannot afford'));
  });
});

// ===========================================================================
// evaluateIncomingQuote
// ===========================================================================

describe('ReputationAware Strategy — evaluateIncomingQuote', () => {
  let strategy;

  beforeEach(() => {
    strategy = createReputationAwareStrategy({
      baseMarkup: 1.4,
      highTrustMarkdown: 0.10,
    });
  });

  it('applies full baseMarkup for sandbox buyers', () => {
    const quote = makeQuote({
      items: [{ description: 'Widget', unit_price: 100, quantity: 1 }],
      _buyerTrustTier: 'sandbox',
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 140); // 100 * 1.4
    assert.equal(pricing.fees, 0);
    assert.equal(pricing.tax, 0);
  });

  it('reduces markup for verified buyers', () => {
    const quote = makeQuote({
      items: [{ description: 'Widget', unit_price: 100, quantity: 1 }],
      _buyerTrustTier: 'verified',
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    // effectiveMarkup = 1.4 - 0.10 = 1.3
    assert.equal(pricing.total, 130);
    assert.ok(pricing.terms.includes('trust discount'));
    assert.ok(pricing.message.includes('trust discount'));
  });

  it('reduces markup for enterprise buyers', () => {
    const quote = makeQuote({
      items: [{ description: 'Widget', unit_price: 100, quantity: 1 }],
      _buyerTrustTier: 'enterprise',
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    // enterprise rank >= verified rank => gets markdown
    assert.equal(pricing.total, 130); // 100 * (1.4 - 0.1)
  });

  it('calculates cost from item totals', () => {
    const quote = makeQuote({
      items: [
        { description: 'A', unit_price: 20, quantity: 3 },
        { description: 'B', unitPrice: 10, quantity: 2 },
      ],
      _buyerTrustTier: 'sandbox',
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    // cost = 20*3 + 10*2 = 80; total = 80 * 1.4 = 112
    assert.equal(pricing.total, 112);
  });

  it('falls back to cost of 50 when items have no pricing', () => {
    const quote = makeQuote({
      items: [{ description: 'Mystery', quantity: 1 }],
      _buyerTrustTier: 'sandbox',
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    // cost = 50; total = 50 * 1.4 = 70
    assert.equal(pricing.total, 70);
  });

  it('falls back to cost of 50 when items array is missing', () => {
    const pricing = strategy.evaluateIncomingQuote({ id: 'q1' });
    // cost = 50; total = 50 * 1.4 = 70
    assert.equal(pricing.total, 70);
  });
});

// ===========================================================================
// evaluateCounterOffer
// ===========================================================================

describe('ReputationAware Strategy — evaluateCounterOffer', () => {
  let strategy;

  beforeEach(() => {
    strategy = createReputationAwareStrategy({
      baseMarkup: 1.4,
    });
  });

  it('accepts counter above floor for standard buyer (15% min margin)', () => {
    // cost = 50 (no pricing), floor = 50 * 1.15 = 57.5
    const quote = makeQuote({
      total_decimal: 60,
      items: [{ description: 'X', quantity: 1 }],
      _buyerTrustTier: 'standard',
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'accept');
  });

  it('revises when counter is below floor for standard buyer', () => {
    // cost = 50, floor = 50 * 1.15 = 57.5
    const quote = makeQuote({
      total_decimal: 40,
      items: [{ description: 'X', quantity: 1 }],
      _buyerTrustTier: 'standard',
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'revise');
    assert.ok(decision.total >= 57.5);
    assert.ok(decision.message.includes('Best I can do'));
  });

  it('uses lower 5% margin floor for verified buyers', () => {
    // cost = 50, floor = 50 * 1.05 = 52.5
    const quote = makeQuote({
      total_decimal: 53,
      items: [{ description: 'X', quantity: 1 }],
      _buyerTrustTier: 'verified',
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'accept');
  });

  it('revises to midpoint clamped at floor', () => {
    // cost = 100, floor = 100 * 1.15 = 115
    // counterTotal = 80, lastAsk = 100 * 1.4 = 140
    // midpoint = (80 + 140) / 2 = 110 < 115 => revised = 115
    const quote = makeQuote({
      total_decimal: 80,
      items: [{ description: 'A', unit_price: 100, quantity: 1 }],
      _buyerTrustTier: 'standard',
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'revise');
    assert.equal(decision.total, 115);
  });

  it('revises to midpoint when midpoint exceeds floor', () => {
    // cost = 100, floor = 100 * 1.15 = 115
    // counterTotal = 112, _lastPrice = 140
    // midpoint = (112 + 140) / 2 = 126 > 115 => revised = 126
    const quote = makeQuote({
      total_decimal: 112,
      items: [{ description: 'A', unit_price: 100, quantity: 1 }],
      _lastPrice: 140,
      _buyerTrustTier: 'standard',
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'revise');
    assert.equal(decision.total, 126);
  });
});

// ===========================================================================
// evaluatePaymentRequest
// ===========================================================================

describe('ReputationAware Strategy — evaluatePaymentRequest', () => {
  let strategy;

  beforeEach(() => {
    strategy = createReputationAwareStrategy();
  });

  it('pays when budget allows', () => {
    const decision = strategy.evaluatePaymentRequest(
      { amount_decimal: 100 },
      makeCtx()
    );
    assert.equal(decision.action, 'pay');
  });

  it('declines when budget cannot afford', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluatePaymentRequest(
      { amount_decimal: 9999 },
      ctx
    );
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('Cannot afford'));
  });

  it('uses amount field when amount_decimal is missing', () => {
    const decision = strategy.evaluatePaymentRequest(
      { amount: 50 },
      makeCtx()
    );
    assert.equal(decision.action, 'pay');
  });
});

// ===========================================================================
// postFulfillmentRating
// ===========================================================================

describe('ReputationAware Strategy — postFulfillmentRating', () => {
  it('returns the expected shape', () => {
    const strategy = createReputationAwareStrategy();
    const rating = strategy.postFulfillmentRating(makeQuote());
    assert.equal(rating.score, 4);
    assert.equal(rating.comment, 'Transaction completed successfully.');
  });

  it('returns consistent rating regardless of quote content', () => {
    const strategy = createReputationAwareStrategy();
    const rating = strategy.postFulfillmentRating({ id: 'any', total: 0 });
    assert.equal(rating.score, 4);
    assert.equal(typeof rating.comment, 'string');
    assert.ok(rating.comment.length > 0);
  });
});
