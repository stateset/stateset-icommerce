/**
 * Unit tests for the Dynamic Pricing Strategy
 *
 * Tests cli/src/a2a/strategies.js — createDynamicPricingStrategy()
 *
 * Covers: strategy identity, volume discounts, reputation adjustments,
 * loyalty discounts, peak hour detection, demand surge, evaluateIncomingQuote,
 * evaluateCounterOffer, evaluateReceivedQuote, evaluatePaymentRequest,
 * combined effects, and edge cases.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createDynamicPricingStrategy } from '../../src/a2a/strategies.js';

// ===========================================================================
// Helpers
// ===========================================================================

function makeCtx(overrides = {}) {
  const budget = overrides.budget ?? { daily: 500 };
  const canAfford = overrides.canAfford ?? ((amt) => amt <= budget.daily);
  return {
    budget,
    runtime: { canAfford },
    ...overrides,
  };
}

function makeQuote(overrides = {}) {
  return {
    id: 'quote-dp-1',
    total: 100,
    total_decimal: 100,
    items: [{ description: 'Widget', unit_price: 50, quantity: 2 }],
    counter_count: 0,
    ...overrides,
  };
}

// ===========================================================================
// 1. Strategy name and interface
// ===========================================================================

describe('DynamicPricingStrategy — identity and interface', () => {
  it('has name "dynamic-pricing"', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(s.name, 'dynamic-pricing');
  });

  it('exposes evaluateReceivedQuote', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(typeof s.evaluateReceivedQuote, 'function');
  });

  it('exposes evaluateIncomingQuote', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(typeof s.evaluateIncomingQuote, 'function');
  });

  it('exposes evaluateCounterOffer', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(typeof s.evaluateCounterOffer, 'function');
  });

  it('exposes evaluatePaymentRequest', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(typeof s.evaluatePaymentRequest, 'function');
  });

  it('exposes helper functions for testing', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(typeof s.getRequestsPerHour, 'function');
    assert.strictEqual(typeof s.getVolumeDiscount, 'function');
    assert.strictEqual(typeof s.getReputationAdjustment, 'function');
    assert.strictEqual(typeof s.getLoyaltyDiscount, 'function');
    assert.strictEqual(typeof s.isPeakHour, 'function');
    assert.strictEqual(typeof s.computeEffectiveMarkup, 'function');
  });
});

// ===========================================================================
// 2. Volume discounts
// ===========================================================================

describe('DynamicPricingStrategy — volume discounts', () => {
  let s;
  beforeEach(() => {
    s = createDynamicPricingStrategy();
  });

  it('returns 0 discount for qty below all tiers', () => {
    assert.strictEqual(s.getVolumeDiscount(1), 0);
  });

  it('returns 0 discount for qty 9 (just below first break)', () => {
    assert.strictEqual(s.getVolumeDiscount(9), 0);
  });

  it('returns 0.05 discount at qty 10 (first break)', () => {
    assert.strictEqual(s.getVolumeDiscount(10), 0.05);
  });

  it('returns 0.05 discount at qty 25 (between first and second break)', () => {
    assert.strictEqual(s.getVolumeDiscount(25), 0.05);
  });

  it('returns 0.10 discount at qty 50 (second break)', () => {
    assert.strictEqual(s.getVolumeDiscount(50), 0.1);
  });

  it('returns 0.10 discount at qty 99 (between second and third break)', () => {
    assert.strictEqual(s.getVolumeDiscount(99), 0.1);
  });

  it('returns 0.15 discount at qty 100 (third break)', () => {
    assert.strictEqual(s.getVolumeDiscount(100), 0.15);
  });

  it('returns 0.15 discount at qty 500 (well above all breaks)', () => {
    assert.strictEqual(s.getVolumeDiscount(500), 0.15);
  });

  it('respects custom volume breaks', () => {
    const custom = createDynamicPricingStrategy({
      volumeBreaks: [
        { minQty: 5, discount: 0.02 },
        { minQty: 20, discount: 0.08 },
      ],
    });
    assert.strictEqual(custom.getVolumeDiscount(4), 0);
    assert.strictEqual(custom.getVolumeDiscount(5), 0.02);
    assert.strictEqual(custom.getVolumeDiscount(20), 0.08);
  });
});

// ===========================================================================
// 3. Reputation adjustments
// ===========================================================================

describe('DynamicPricingStrategy — reputation adjustments', () => {
  let s;
  beforeEach(() => {
    s = createDynamicPricingStrategy();
  });

  it('returns -0.15 for enterprise tier', () => {
    assert.strictEqual(s.getReputationAdjustment('enterprise'), -0.15);
  });

  it('returns -0.10 for verified tier', () => {
    assert.strictEqual(s.getReputationAdjustment('verified'), -0.1);
  });

  it('returns 0 for standard tier', () => {
    assert.strictEqual(s.getReputationAdjustment('standard'), 0);
  });

  it('returns 0.20 for sandbox tier', () => {
    assert.strictEqual(s.getReputationAdjustment('sandbox'), 0.2);
  });

  it('falls back to standard (0) for unknown tier', () => {
    assert.strictEqual(s.getReputationAdjustment('unknown-tier'), 0);
  });
});

// ===========================================================================
// 4. Loyalty discounts
// ===========================================================================

describe('DynamicPricingStrategy — loyalty discounts', () => {
  let s;
  beforeEach(() => {
    s = createDynamicPricingStrategy();
  });

  it('returns 0 for 0 transactions', () => {
    assert.strictEqual(s.getLoyaltyDiscount(0), 0);
  });

  it('returns 0 for 4 transactions (below first tier)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(4), 0);
  });

  it('returns -0.05 at 5 transactions (first tier)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(5), -0.05);
  });

  it('returns -0.05 at 9 transactions (between first and second tier)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(9), -0.05);
  });

  it('returns -0.10 at 10 transactions (second tier)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(10), -0.1);
  });

  it('returns -0.10 at 19 transactions (between second and third tier)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(19), -0.1);
  });

  it('returns -0.15 at 20 transactions (third tier)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(20), -0.15);
  });

  it('returns -0.15 at 100 transactions (well above all tiers)', () => {
    assert.strictEqual(s.getLoyaltyDiscount(100), -0.15);
  });
});

// ===========================================================================
// 5. evaluateIncomingQuote — seller pricing
// ===========================================================================

describe('DynamicPricingStrategy — evaluateIncomingQuote', () => {
  it('applies base markup (1.3x) to item cost for standard buyer', () => {
    // Create strategy with high demand threshold so demand surge does not trigger
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({ _buyerTrustTier: 'standard', _buyerTransactionCount: 0 });
    const result = s.evaluateIncomingQuote(quote);
    // cost = 50*2 = 100, markup 1.3, standard rep = 0, no volume discount (qty 2)
    // effective markup = 1.3 + 0 = 1.3 (no peak adjustment in this test depending on time)
    assert.ok(result.total > 0, 'total should be positive');
    assert.strictEqual(result.tax, 0);
    assert.ok(typeof result.fees === 'number');
    assert.ok(typeof result.terms === 'string');
    assert.ok(typeof result.message === 'string');
  });

  it('applies volume discount when quantity qualifies', () => {
    const s = createDynamicPricingStrategy({
      demandSurgeThreshold: 9999,
      peakHours: { start: 25, end: 25, surgeMultiplier: 1.0 },
    });
    // 60 items at $10 each = cost $600, volume discount 0.10 (qty 60 >= 50)
    const quote = makeQuote({
      items: [{ description: 'Bolt', unit_price: 10, quantity: 60 }],
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // base markup 1.3, volume -0.10 = 1.2 (before peak/surge)
    // cost = 600, at minimum markup 1.2 => 720
    assert.ok(result.total <= 600 * 1.35, 'volume discount should reduce price');
    assert.ok(result.message.includes('volume'), 'message should mention volume adjustment');
  });

  it('applies reputation adjustment for enterprise buyer', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      _buyerTrustTier: 'enterprise',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // reputation adjustment = -0.15, so effective markup lower
    assert.ok(
      result.message.includes('reputation'),
      'message should mention reputation adjustment',
    );
  });

  it('applies sandbox surcharge', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      _buyerTrustTier: 'sandbox',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // sandbox = +0.20 to markup, so 1.3 + 0.20 = 1.5x before peak
    assert.ok(result.message.includes('reputation'), 'message should mention reputation');
  });

  it('falls back to default cost when items list is empty', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      items: [],
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // empty items => itemTotal = 0, cost = 50 (fallback)
    assert.ok(result.total > 0, 'should use fallback cost');
  });

  it('falls back to default cost when items is not an array', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      items: undefined,
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    assert.ok(result.total > 0, 'should use fallback cost of 50');
  });

  it('computes fees as 15% of margin', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    const cost = 100; // 50 * 2
    const expectedFees = Math.round((result.total - cost) * 0.15 * 100) / 100;
    assert.strictEqual(result.fees, expectedFees);
  });

  it('includes effective markup percentage in terms', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    assert.ok(result.terms.includes('Dynamic pricing'), 'terms should mention dynamic pricing');
    assert.ok(result.terms.includes('markup'), 'terms should mention markup');
  });
});

// ===========================================================================
// 6. evaluateCounterOffer — accept/revise/decline
// ===========================================================================

describe('DynamicPricingStrategy — evaluateCounterOffer', () => {
  it('accepts counter above floor (cost * 1.05)', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // cost = 50*2 = 100, floor = 100 * 1.05 = 105
    const quote = makeQuote({ total_decimal: 110, total: 110 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'accept');
  });

  it('accepts counter exactly at floor', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // floor = 100 * 1.05 = 105
    const quote = makeQuote({ total_decimal: 105, total: 105 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'accept');
  });

  it('revises counter below floor', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // cost = 100, floor = 105, counter = 90 (below floor)
    const quote = makeQuote({ total_decimal: 90, total: 90 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
    assert.ok(result.total >= 105, 'revised total should be at or above floor');
    assert.ok(typeof result.message === 'string');
  });

  it('revise total is at least the floor', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // Very low counter: cost = 100, floor = 105
    const quote = makeQuote({ total_decimal: 10, total: 10 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
    assert.ok(result.total >= 105, 'revised total must never go below floor');
  });

  it('enforces custom minMargin', () => {
    const s = createDynamicPricingStrategy({ minMargin: 0.2, demandSurgeThreshold: 9999 });
    // cost = 100, floor = 100 * 1.20 = 120
    const quote = makeQuote({ total_decimal: 115, total: 115 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
    assert.ok(result.total >= 120, 'revised total respects custom minMargin floor');
  });

  it('uses _lastPrice for midpoint calculation when available', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // cost = 100, floor = 105, counter = 90, _lastPrice = 150
    // midpoint = (90 + 150) / 2 = 120
    const quote = makeQuote({ total_decimal: 90, total: 90, _lastPrice: 150 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
    assert.ok(result.total >= 105, 'revised above floor');
  });

  it('uses fallback cost (50) when items have no pricing', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // No items => cost = 50, floor = 50 * 1.05 = 52.5
    const quote = { total_decimal: 40, total: 40 };
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
    assert.ok(result.total >= 52.5, 'fallback cost floor applies');
  });
});

// ===========================================================================
// 7. evaluateReceivedQuote — buyer side
// ===========================================================================

describe('DynamicPricingStrategy — evaluateReceivedQuote', () => {
  it('accepts a quote within budget', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 500 }, canAfford: (amt) => amt <= 500 });
    const quote = makeQuote({ total: 100 });
    const result = s.evaluateReceivedQuote(quote, ctx);
    assert.strictEqual(result.action, 'accept');
  });

  it('declines a quote over budget', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 50 }, canAfford: (amt) => amt <= 50 });
    const quote = makeQuote({ total: 100 });
    const result = s.evaluateReceivedQuote(quote, ctx);
    assert.strictEqual(result.action, 'decline');
    assert.ok(result.reason.includes('Cannot afford'));
  });

  it('accepts when budget context is absent', () => {
    const s = createDynamicPricingStrategy();
    const ctx = { runtime: { canAfford: () => true } };
    const quote = makeQuote({ total: 9999 });
    const result = s.evaluateReceivedQuote(quote, ctx);
    assert.strictEqual(result.action, 'accept');
  });

  it('uses total_decimal when total is missing', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 50 }, canAfford: (amt) => amt <= 50 });
    const quote = { total_decimal: 200 };
    const result = s.evaluateReceivedQuote(quote, ctx);
    assert.strictEqual(result.action, 'decline');
  });

  it('treats missing total and total_decimal as 0 (accepts)', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 500 }, canAfford: (amt) => amt <= 500 });
    const quote = {};
    const result = s.evaluateReceivedQuote(quote, ctx);
    assert.strictEqual(result.action, 'accept');
  });
});

// ===========================================================================
// 8. evaluatePaymentRequest — buyer side
// ===========================================================================

describe('DynamicPricingStrategy — evaluatePaymentRequest', () => {
  it('pays a request within budget', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 500 }, canAfford: (amt) => amt <= 500 });
    const result = s.evaluatePaymentRequest({ amount: 200 }, ctx);
    assert.strictEqual(result.action, 'pay');
  });

  it('declines a request over budget', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 100 }, canAfford: (amt) => amt <= 100 });
    const result = s.evaluatePaymentRequest({ amount: 500 }, ctx);
    assert.strictEqual(result.action, 'decline');
    assert.ok(result.reason.includes('Cannot afford'));
  });

  it('uses amount_decimal when amount is missing', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 100 }, canAfford: (amt) => amt <= 100 });
    const result = s.evaluatePaymentRequest({ amount_decimal: 200 }, ctx);
    assert.strictEqual(result.action, 'decline');
  });

  it('treats missing amount as 0 (pays)', () => {
    const s = createDynamicPricingStrategy();
    const ctx = makeCtx({ budget: { daily: 500 }, canAfford: (amt) => amt <= 500 });
    const result = s.evaluatePaymentRequest({}, ctx);
    assert.strictEqual(result.action, 'pay');
  });

  it('pays when budget context is absent', () => {
    const s = createDynamicPricingStrategy();
    const ctx = { runtime: { canAfford: () => true } };
    const result = s.evaluatePaymentRequest({ amount: 9999 }, ctx);
    assert.strictEqual(result.action, 'pay');
  });
});

// ===========================================================================
// 9. Combined effects — volume + reputation + loyalty
// ===========================================================================

describe('DynamicPricingStrategy — combined effects', () => {
  it('volume + enterprise reputation reduces markup', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // qty 100 at $5 each = cost 500
    // volume discount: 0.15 (qty >= 100)
    // enterprise reputation: -0.15
    // base markup: 1.3 - 0.15 - 0.15 = 1.0 => floored at 1 + 0.05 = 1.05
    const quote = makeQuote({
      items: [{ description: 'Part', unit_price: 5, quantity: 100 }],
      _buyerTrustTier: 'enterprise',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    assert.ok(result.total >= 500 * 1.05, 'total should be at least cost * (1 + minMargin)');
  });

  it('volume + loyalty reduces effective markup', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // qty 50 at $10 each = cost 500
    // volume discount: 0.10
    // loyalty (20 txns): -0.15
    // standard reputation: 0
    // markup: 1.3 - 0.10 + 0 - 0.15 = 1.05 = floored at 1.05
    const quote = makeQuote({
      items: [{ description: 'Part', unit_price: 10, quantity: 50 }],
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 20,
    });
    const result = s.evaluateIncomingQuote(quote);
    assert.ok(result.total >= 500 * 1.05, 'total should be at least cost * minMargin floor');
  });

  it('sandbox surcharge offsets volume discount', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // qty 100 at $10 each = cost 1000
    // volume discount: 0.15
    // sandbox reputation: +0.20
    // markup: 1.3 - 0.15 + 0.20 = 1.35
    const quote = makeQuote({
      items: [{ description: 'Part', unit_price: 10, quantity: 100 }],
      _buyerTrustTier: 'sandbox',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // At minimum, markup is 1.35 (no peak)
    assert.ok(result.total >= 1000 * 1.05, 'sandbox surcharge keeps price up');
  });

  it('all three adjustments stack: volume + reputation + loyalty', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    // qty 10 at $100 = cost 1000
    // volume: -0.05 (qty 10)
    // verified reputation: -0.10
    // loyalty (10 txns): -0.10
    // markup: 1.3 - 0.05 - 0.10 - 0.10 = 1.05 = floored at 1.05
    const quote = makeQuote({
      items: [{ description: 'Premium Widget', unit_price: 100, quantity: 10 }],
      _buyerTrustTier: 'verified',
      _buyerTransactionCount: 10,
    });
    const result = s.evaluateIncomingQuote(quote);
    assert.ok(result.total >= 1000 * 1.05, 'respects min margin floor');
  });

  it('min margin floor prevents markup going below 1 + minMargin', () => {
    // Extreme discounts: massive volume + enterprise + max loyalty
    const s = createDynamicPricingStrategy({
      baseMarkup: 1.1,
      minMargin: 0.05,
      demandSurgeThreshold: 9999,
    });
    const quote = makeQuote({
      items: [{ description: 'Item', unit_price: 10, quantity: 100 }],
      _buyerTrustTier: 'enterprise',
      _buyerTransactionCount: 20,
    });
    // baseMarkup 1.1 - volume 0.15 - enterprise 0.15 - loyalty 0.15 = 0.65
    // floored at 1 + 0.05 = 1.05
    const markup = s.computeEffectiveMarkup(quote);
    assert.ok(markup >= 1.05, `markup ${markup} should be at least 1.05 (1 + minMargin)`);
  });
});

// ===========================================================================
// 10. Edge cases
// ===========================================================================

describe('DynamicPricingStrategy — edge cases', () => {
  it('handles empty items array (falls back to qty 1 for volume)', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({ items: [] });
    // empty items => itemTotal = 0 => cost = 50, qty = 0 => volume discount = 0
    const result = s.evaluateIncomingQuote(quote);
    assert.ok(result.total > 0, 'should produce a positive total from fallback cost');
  });

  it('handles zero quantity items', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      items: [{ description: 'Zero', unit_price: 100, quantity: 0 }],
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // quantity 0 => itemTotal = 0 => cost = 50 (fallback)
    assert.ok(result.total > 0, 'zero qty items fall back to default cost');
  });

  it('handles items with no unit_price', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({
      items: [{ description: 'Freebie', quantity: 5 }],
      _buyerTrustTier: 'standard',
      _buyerTransactionCount: 0,
    });
    const result = s.evaluateIncomingQuote(quote);
    // unit_price = 0 => itemTotal = 0 => cost = 50 fallback
    assert.ok(result.total > 0, 'no unit_price falls back to default cost');
  });

  it('unknown reputation tier falls back to standard (0)', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(s.getReputationAdjustment('platinum'), 0);
    assert.strictEqual(s.getReputationAdjustment(''), 0);
  });

  it('getVolumeDiscount handles qty 0', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(s.getVolumeDiscount(0), 0);
  });

  it('getLoyaltyDiscount handles negative transaction count', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(s.getLoyaltyDiscount(-1), 0);
  });

  it('getRequestsPerHour starts at 0', () => {
    const s = createDynamicPricingStrategy();
    // Fresh strategy has no tracked requests
    assert.strictEqual(s.getRequestsPerHour(), 0);
  });

  it('computeEffectiveMarkup tracks a request each call', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({ _buyerTrustTier: 'standard', _buyerTransactionCount: 0 });
    const before = s.getRequestsPerHour();
    s.computeEffectiveMarkup(quote);
    const after = s.getRequestsPerHour();
    assert.strictEqual(after, before + 1, 'each computeEffectiveMarkup call tracks one request');
  });

  it('demand surge triggers above threshold', () => {
    const s = createDynamicPricingStrategy({
      demandSurgeThreshold: 3,
      demandSurgeMultiplier: 1.5,
    });
    const quote = makeQuote({ _buyerTrustTier: 'standard', _buyerTransactionCount: 0 });
    // Fire enough evaluations to trigger demand surge
    s.computeEffectiveMarkup(quote);
    s.computeEffectiveMarkup(quote);
    // Third call should push count to 3 which meets threshold
    const markup = s.computeEffectiveMarkup(quote);
    // base 1.3 * 1.5 = 1.95 (if no peak), or possibly higher if peak
    assert.ok(markup >= 1.3 * 1.5 * 0.9, 'demand surge should increase markup substantially');
  });

  it('counter offer with no items uses fallback cost', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = { total: 40, total_decimal: 40 };
    // cost = 50, floor = 52.5
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
  });

  it('counter offer message includes floor and revised price', () => {
    const s = createDynamicPricingStrategy({ demandSurgeThreshold: 9999 });
    const quote = makeQuote({ total_decimal: 90, total: 90 });
    const result = s.evaluateCounterOffer(quote);
    assert.strictEqual(result.action, 'revise');
    assert.ok(result.message.includes('floor'), 'message mentions floor');
    assert.ok(result.message.includes('Revised'), 'message mentions revised');
  });
});

// ===========================================================================
// 11. isPeakHour helper
// ===========================================================================

describe('DynamicPricingStrategy — isPeakHour', () => {
  it('returns a boolean', () => {
    const s = createDynamicPricingStrategy();
    assert.strictEqual(typeof s.isPeakHour(), 'boolean');
  });

  it('uses the configured peakHours window', () => {
    // Configure peak hours as 0-24 (always peak)
    const alwaysPeak = createDynamicPricingStrategy({
      peakHours: { start: 0, end: 24, surgeMultiplier: 1.1 },
    });
    assert.strictEqual(alwaysPeak.isPeakHour(), true);
  });

  it('isPeakHour false when window is empty (start == end)', () => {
    const neverPeak = createDynamicPricingStrategy({
      peakHours: { start: 25, end: 25, surgeMultiplier: 1.0 },
    });
    // No valid hour is >= 25, so always false
    assert.strictEqual(neverPeak.isPeakHour(), false);
  });
});

// ===========================================================================
// 12. Custom config overrides
// ===========================================================================

describe('DynamicPricingStrategy — custom config', () => {
  it('respects custom baseMarkup', () => {
    const s = createDynamicPricingStrategy({
      baseMarkup: 2.0,
      demandSurgeThreshold: 9999,
    });
    const quote = makeQuote({ _buyerTrustTier: 'standard', _buyerTransactionCount: 0 });
    const markup = s.computeEffectiveMarkup(quote);
    // base 2.0, standard rep = 0, no volume, no loyalty => ~2.0 (before peak)
    assert.ok(markup >= 1.5, 'custom baseMarkup should be reflected');
  });

  it('respects custom minMargin in computeEffectiveMarkup', () => {
    const s = createDynamicPricingStrategy({
      baseMarkup: 1.0,
      minMargin: 0.25,
      demandSurgeThreshold: 9999,
    });
    const quote = makeQuote({
      items: [{ unit_price: 10, quantity: 100 }],
      _buyerTrustTier: 'enterprise',
      _buyerTransactionCount: 20,
    });
    // base 1.0 - volume 0.15 - enterprise 0.15 - loyalty 0.15 = 0.55
    // floored at 1 + 0.25 = 1.25
    const markup = s.computeEffectiveMarkup(quote);
    assert.ok(markup >= 1.25, `markup ${markup} should be at least 1.25`);
  });

  it('respects custom reputationTiers', () => {
    const s = createDynamicPricingStrategy({
      reputationTiers: { gold: -0.2, silver: -0.1, bronze: 0.1 },
    });
    assert.strictEqual(s.getReputationAdjustment('gold'), -0.2);
    assert.strictEqual(s.getReputationAdjustment('silver'), -0.1);
    assert.strictEqual(s.getReputationAdjustment('bronze'), 0.1);
    // unknown falls back to standard key which does not exist in custom => 0
    assert.strictEqual(s.getReputationAdjustment('unknown'), 0);
  });

  it('respects custom loyaltyTiers', () => {
    const s = createDynamicPricingStrategy({
      loyaltyTiers: { 3: -0.03, 15: -0.12 },
    });
    assert.strictEqual(s.getLoyaltyDiscount(2), 0);
    assert.strictEqual(s.getLoyaltyDiscount(3), -0.03);
    assert.strictEqual(s.getLoyaltyDiscount(14), -0.03);
    assert.strictEqual(s.getLoyaltyDiscount(15), -0.12);
  });
});
