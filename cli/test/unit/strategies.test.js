/**
 * Unit tests for A2A Negotiation Strategies
 *
 * Tests the 4 pluggable negotiation strategies in cli/src/a2a/strategies.js:
 *   1. AlwaysAccept — accepts everything
 *   2. BudgetGated — budget-constrained with markup pricing
 *   3. Negotiator — counter-offers toward target discount
 *   4. BestOfN — collects quotes, picks cheapest/best-value
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  createAlwaysAcceptStrategy,
  createBudgetGatedStrategy,
  createNegotiatorStrategy,
  createBestOfNStrategy,
} from '../../src/a2a/strategies.js';

// ===========================================================================
// Helpers
// ===========================================================================

function makeCtx(overrides = {}) {
  const canAffordFn = overrides.canAfford ?? (() => true);
  return {
    runtime: {
      canAfford: canAffordFn,
      getBudget: () => ({
        perTransaction: 500,
        daily: 1000,
        monthly: 5000,
        spentToday: 0,
        spentThisMonth: 0,
        remainingDaily: 1000,
        remainingMonthly: 5000,
      }),
    },
    budget: {
      perTransaction: 500,
      daily: 1000,
      monthly: 5000,
      spentToday: 0,
      spentThisMonth: 0,
      remainingDaily: 1000,
      remainingMonthly: 5000,
    },
    ...overrides,
  };
}

function makeQuote(overrides = {}) {
  return {
    id: 'quote-1',
    total: 100,
    total_decimal: 100,
    items: [{ description: 'Service A', unitPrice: 50, quantity: 2 }],
    counter_count: 0,
    ...overrides,
  };
}

// ===========================================================================
// 1. AlwaysAccept Strategy
// ===========================================================================

describe('AlwaysAccept Strategy', () => {
  let strategy;

  beforeEach(() => {
    strategy = createAlwaysAcceptStrategy();
  });

  it('has the correct name', () => {
    assert.equal(strategy.name, 'always-accept');
  });

  it('accepts every received quote', () => {
    const decision = strategy.evaluateReceivedQuote(makeQuote(), makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('accepts received quotes regardless of price', () => {
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 999999 }), makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('provides pricing from items', () => {
    const quote = makeQuote({
      items: [
        { description: 'A', unit_price: 30, quantity: 2 },
        { description: 'B', unit_price: 10, quantity: 1 },
      ],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 70); // 30*2 + 10*1
    assert.equal(pricing.fees, 0);
    assert.equal(pricing.tax, 0);
  });

  it('uses unitPrice (camelCase) for pricing', () => {
    const quote = makeQuote({
      items: [{ description: 'A', unitPrice: 25, quantity: 4 }],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 100); // 25*4
  });

  it('falls back to default price when items have no pricing', () => {
    const quote = makeQuote({
      items: [{ description: 'Something', quantity: 1 }],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 10); // default
  });

  it('respects custom defaultPrice', () => {
    const s = createAlwaysAcceptStrategy({ defaultPrice: 42 });
    const quote = makeQuote({ items: [{ description: 'X', quantity: 1 }] });
    const pricing = s.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 42);
  });

  it('handles missing items array', () => {
    const pricing = strategy.evaluateIncomingQuote({ id: 'q1' });
    assert.equal(pricing.total, 10);
  });

  it('accepts every counter-offer', () => {
    const decision = strategy.evaluateCounterOffer(makeQuote(), makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('pays every payment request', () => {
    const decision = strategy.evaluatePaymentRequest({ amount: 100 }, makeCtx());
    assert.equal(decision.action, 'pay');
  });
});

// ===========================================================================
// 2. BudgetGated Strategy
// ===========================================================================

describe('BudgetGated Strategy', () => {
  let strategy;

  beforeEach(() => {
    strategy = createBudgetGatedStrategy({ markup: 1.5, minMargin: 0.1, basePrice: 50 });
  });

  it('has the correct name', () => {
    assert.equal(strategy.name, 'budget-gated');
  });

  // --- evaluateReceivedQuote ---

  it('accepts a quote within budget', () => {
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 100 }), makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('declines a quote exceeding budget', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 9999 }), ctx);
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('Exceeds budget'));
  });

  it('uses total_decimal when total is missing', () => {
    const decision = strategy.evaluateReceivedQuote(
      makeQuote({ total: undefined, total_decimal: 50 }),
      makeCtx(),
    );
    assert.equal(decision.action, 'accept');
  });

  it('passes quote asset and network to budget checks', () => {
    let received = null;
    const ctx = makeCtx({
      canAfford: (amount, options) => {
        received = { amount, options };
        return true;
      },
    });

    strategy.evaluateReceivedQuote(
      makeQuote({ total: 0.01, asset: 'BTC', network: 'bitcoin' }),
      ctx,
    );

    assert.deepEqual(received, {
      amount: 0.01,
      options: { asset: 'BTC', network: 'bitcoin' },
    });
  });

  // --- evaluateIncomingQuote ---

  it('prices items with markup', () => {
    const quote = makeQuote({
      items: [{ description: 'A', unit_price: 40, quantity: 1 }],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 60); // 40 * 1.5
  });

  it('uses basePrice fallback when items have no pricing', () => {
    const quote = makeQuote({ items: [{ description: 'X', quantity: 1 }] });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 75); // 50 * 1.5
  });

  it('calculates fees as 20% of margin', () => {
    const quote = makeQuote({
      items: [{ description: 'A', unit_price: 100, quantity: 1 }],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    // total = 100 * 1.5 = 150; margin = 50; fees = 50 * 0.2 = 10
    assert.equal(pricing.total, 150);
    assert.equal(pricing.fees, 10);
  });

  it('includes markup percentage in terms', () => {
    const pricing = strategy.evaluateIncomingQuote(makeQuote());
    assert.ok(pricing.terms.includes('50%'));
  });

  // --- evaluateCounterOffer ---

  it('accepts a counter at or above minimum margin', () => {
    // basePrice = 50, minMargin = 0.1, floor = 55
    const quote = makeQuote({
      total_decimal: 60,
      items: [{ description: 'X', quantity: 1 }], // no price → basePrice=50
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'accept');
  });

  it('revises when counter is below minimum margin', () => {
    const quote = makeQuote({
      total_decimal: 40, // below floor of 55
      items: [{ description: 'X', quantity: 1 }], // no price → basePrice=50
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'revise');
    assert.ok(decision.total >= 55); // at least floor
  });

  it('uses midpoint for revision but respects floor', () => {
    const quote = makeQuote({
      total_decimal: 10, // very low counter
      items: [{ description: 'A', unit_price: 50, quantity: 1 }],
      _lastPrice: 75, // 50 * 1.5
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'revise');
    // midpoint = (10 + 75) / 2 = 42.5, but floor = 55, so revised = 55
    assert.equal(decision.total, 55);
  });

  it('accepts counter exactly at floor', () => {
    const quote = makeQuote({
      total_decimal: 55, // exactly at floor (50 * 1.1)
      items: [{ description: 'X', quantity: 1 }], // no pricing → basePrice=50
    });
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'accept');
  });

  // --- evaluatePaymentRequest ---

  it('pays when within budget', () => {
    const decision = strategy.evaluatePaymentRequest({ amount_decimal: 100 }, makeCtx());
    assert.equal(decision.action, 'pay');
  });

  it('declines payment when over budget', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluatePaymentRequest({ amount_decimal: 9999 }, ctx);
    assert.equal(decision.action, 'decline');
  });

  it('passes payment request asset and network to budget checks', () => {
    let received = null;
    const ctx = makeCtx({
      canAfford: (amount, options) => {
        received = { amount, options };
        return true;
      },
    });

    strategy.evaluatePaymentRequest({ amount_decimal: 1.25, asset: 'ZEC', network: 'zcash' }, ctx);

    assert.deepEqual(received, {
      amount: 1.25,
      options: { asset: 'ZEC', network: 'zcash' },
    });
  });

  // --- Edge cases ---

  it('defaults markup to 1.3', () => {
    const s = createBudgetGatedStrategy();
    const pricing = s.evaluateIncomingQuote(
      makeQuote({ items: [{ description: 'A', unit_price: 100, quantity: 1 }] }),
    );
    assert.equal(pricing.total, 130); // 100 * 1.3
  });

  it('handles empty items array', () => {
    const pricing = strategy.evaluateIncomingQuote(makeQuote({ items: [] }));
    assert.equal(pricing.total, 75); // basePrice 50 * 1.5
  });
});

// ===========================================================================
// 3. Negotiator Strategy
// ===========================================================================

describe('Negotiator Strategy', () => {
  let strategy;

  beforeEach(() => {
    strategy = createNegotiatorStrategy({
      targetDiscount: 0.2,
      maxRounds: 3,
      walkAwayAbove: 500,
      acceptBelow: 10,
      sellerMarkup: 1.4,
      sellerFloor: 0.15,
    });
  });

  it('has the correct name', () => {
    assert.equal(strategy.name, 'negotiator');
  });

  // --- evaluateReceivedQuote ---

  it('auto-accepts quotes below acceptBelow threshold', () => {
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 5 }), makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('walks away from quotes above walkAwayAbove', () => {
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 600 }), makeCtx());
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('walk-away'));
  });

  it('declines when cannot afford', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 100 }), ctx);
    assert.equal(decision.action, 'decline');
    assert.ok(decision.reason.includes('Cannot afford'));
  });

  it('counter-offers at target discount', () => {
    const decision = strategy.evaluateReceivedQuote(
      makeQuote({ total: 100, counter_count: 0 }),
      makeCtx(),
    );
    assert.equal(decision.action, 'counter');
    assert.equal(decision.total, 80); // 100 * (1 - 0.2)
  });

  it('accepts after max negotiation rounds', () => {
    const decision = strategy.evaluateReceivedQuote(
      makeQuote({ total: 100, counter_count: 3 }),
      makeCtx(),
    );
    assert.equal(decision.action, 'accept');
  });

  it('counter message includes discount percentage', () => {
    const decision = strategy.evaluateReceivedQuote(
      makeQuote({ total: 200, counter_count: 0 }),
      makeCtx(),
    );
    assert.equal(decision.action, 'counter');
    assert.ok(decision.message.includes('20%'));
    assert.equal(decision.total, 160);
  });

  it('uses total_decimal when total is missing', () => {
    const decision = strategy.evaluateReceivedQuote(
      makeQuote({ total: undefined, total_decimal: 80, counter_count: 0 }),
      makeCtx(),
    );
    assert.equal(decision.action, 'counter');
    assert.equal(decision.total, 64); // 80 * 0.8
  });

  // --- evaluateIncomingQuote ---

  it('prices with seller markup', () => {
    const quote = makeQuote({
      items: [{ description: 'A', unit_price: 100, quantity: 1 }],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 140); // 100 * 1.4
  });

  it('falls back to $50 base when items have no pricing', () => {
    const quote = makeQuote({ items: [{ description: 'X', quantity: 1 }] });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 70); // 50 * 1.4
  });

  // --- evaluateCounterOffer ---

  it('accepts counter above seller floor', () => {
    const quote = makeQuote({
      total_decimal: 60,
      items: [{ description: 'X', quantity: 1 }], // no price → cost=50
    });
    // floor = 50 * 1.15 = 57.5
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'accept');
  });

  it('revises when counter is below seller floor', () => {
    const quote = makeQuote({
      total_decimal: 40,
      items: [{ description: 'X', quantity: 1 }], // no price → cost=50
    });
    // floor = 50 * 1.15 = 57.5
    const decision = strategy.evaluateCounterOffer(quote);
    assert.equal(decision.action, 'revise');
    assert.ok(decision.total >= 57.5);
  });

  it('splits the difference in revision', () => {
    const quote = makeQuote({
      total_decimal: 50,
      items: [{ description: 'A', unit_price: 50, quantity: 1 }],
      _lastPrice: 70, // seller's last ask
    });
    const decision = strategy.evaluateCounterOffer(quote);
    // floor = 50 * 1.15 = 57.5
    // midpoint = (50 + 70) / 2 = 60 → 60 > 57.5 → use 60
    assert.equal(decision.action, 'revise');
    assert.equal(decision.total, 60);
  });

  // --- evaluatePaymentRequest ---

  it('pays when within threshold', () => {
    const decision = strategy.evaluatePaymentRequest({ amount_decimal: 100 }, makeCtx());
    assert.equal(decision.action, 'pay');
  });

  it('declines payment above walkaway', () => {
    const decision = strategy.evaluatePaymentRequest({ amount_decimal: 600 }, makeCtx());
    assert.equal(decision.action, 'decline');
  });

  it('declines payment when cannot afford', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluatePaymentRequest({ amount_decimal: 100 }, ctx);
    assert.equal(decision.action, 'decline');
  });

  // --- Defaults ---

  it('defaults to 15% target discount', () => {
    const s = createNegotiatorStrategy();
    const decision = s.evaluateReceivedQuote(
      makeQuote({ total: 100, counter_count: 0 }),
      makeCtx(),
    );
    assert.equal(decision.action, 'counter');
    assert.equal(decision.total, 85); // 100 * (1 - 0.15)
  });
});

// ===========================================================================
// 4. BestOfN Strategy
// ===========================================================================

describe('BestOfN Strategy', () => {
  let strategy;

  beforeEach(() => {
    strategy = createBestOfNStrategy({ minQuotes: 2, selection: 'cheapest' });
  });

  it('has the correct name', () => {
    assert.equal(strategy.name, 'best-of-n');
  });

  // --- collectQuote / selectBest ---

  it('collects quotes and reports count', () => {
    strategy.collectQuote('req-1', { id: 'q1', total: 100 });
    assert.equal(strategy.getCollectedCount('req-1'), 1);
    strategy.collectQuote('req-1', { id: 'q2', total: 80 });
    assert.equal(strategy.getCollectedCount('req-1'), 2);
  });

  it('reports enough quotes correctly', () => {
    strategy.collectQuote('req-1', { id: 'q1', total: 100 });
    assert.equal(strategy.hasEnoughQuotes('req-1'), false);
    strategy.collectQuote('req-1', { id: 'q2', total: 80 });
    assert.equal(strategy.hasEnoughQuotes('req-1'), true);
  });

  it('selects cheapest quote', () => {
    strategy.collectQuote('req-1', { id: 'q1', total: 100 });
    strategy.collectQuote('req-1', { id: 'q2', total: 60 });
    strategy.collectQuote('req-1', { id: 'q3', total: 80 });

    const result = strategy.selectBest('req-1');
    assert.equal(result.winner.id, 'q2'); // cheapest at 60
    assert.equal(result.losers.length, 2);
  });

  it('uses total_decimal for scoring', () => {
    strategy.collectQuote('req-1', { id: 'q1', total_decimal: 50 });
    strategy.collectQuote('req-1', { id: 'q2', total_decimal: 30 });

    const result = strategy.selectBest('req-1');
    assert.equal(result.winner.id, 'q2');
  });

  it('returns null for unknown tag', () => {
    assert.equal(strategy.selectBest('nonexistent'), null);
  });

  it('returns 0 for unknown tag count', () => {
    assert.equal(strategy.getCollectedCount('nonexistent'), 0);
  });

  it('false for unknown tag hasEnoughQuotes', () => {
    assert.equal(strategy.hasEnoughQuotes('nonexistent'), false);
  });

  // --- reset ---

  it('resets a specific tag', () => {
    strategy.collectQuote('req-1', { id: 'q1', total: 100 });
    strategy.collectQuote('req-2', { id: 'q2', total: 80 });
    strategy.reset('req-1');
    assert.equal(strategy.getCollectedCount('req-1'), 0);
    assert.equal(strategy.getCollectedCount('req-2'), 1);
  });

  it('resets all tags', () => {
    strategy.collectQuote('req-1', { id: 'q1', total: 100 });
    strategy.collectQuote('req-2', { id: 'q2', total: 80 });
    strategy.reset();
    assert.equal(strategy.getCollectedCount('req-1'), 0);
    assert.equal(strategy.getCollectedCount('req-2'), 0);
  });

  // --- best_value selection ---

  it('selects best value using reputation', () => {
    const s = createBestOfNStrategy({ minQuotes: 2, selection: 'best_value' });
    // Cheap but low reputation
    s.collectQuote('req-1', { id: 'q1', total: 50, _sellerReputation: 1 });
    // Expensive but high reputation
    s.collectQuote('req-1', { id: 'q2', total: 80, _sellerReputation: 5 });

    const result = s.selectBest('req-1');
    // best_value weights reputation 60% and price 40% — high rep should win
    assert.equal(result.winner.id, 'q2');
  });

  it('defaults _sellerReputation to 3 when missing', () => {
    const s = createBestOfNStrategy({ minQuotes: 2, selection: 'best_value' });
    s.collectQuote('req-1', { id: 'q1', total: 100 }); // rep=3 default
    s.collectQuote('req-1', { id: 'q2', total: 100, _sellerReputation: 5 });

    const result = s.selectBest('req-1');
    assert.equal(result.winner.id, 'q2'); // higher rep wins at same price
  });

  // --- Standard interface ---

  it('defers evaluateReceivedQuote (buyer should use collectQuote)', () => {
    const decision = strategy.evaluateReceivedQuote(makeQuote(), makeCtx());
    assert.equal(decision.action, 'defer');
  });

  it('declines evaluateReceivedQuote when cannot afford', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluateReceivedQuote(makeQuote({ total: 999 }), ctx);
    assert.equal(decision.action, 'decline');
  });

  it('provides pricing as seller', () => {
    const quote = makeQuote({
      items: [{ description: 'X', unit_price: 30, quantity: 2 }],
    });
    const pricing = strategy.evaluateIncomingQuote(quote);
    assert.equal(pricing.total, 60); // 30*2
  });

  it('accepts counter-offers', () => {
    const decision = strategy.evaluateCounterOffer(makeQuote(), makeCtx());
    assert.equal(decision.action, 'accept');
  });

  it('pays when within budget', () => {
    const decision = strategy.evaluatePaymentRequest({ amount: 50 }, makeCtx());
    assert.equal(decision.action, 'pay');
  });

  it('declines payment when cannot afford', () => {
    const ctx = makeCtx({ canAfford: () => false });
    const decision = strategy.evaluatePaymentRequest({ amount: 999 }, ctx);
    assert.equal(decision.action, 'decline');
  });

  // --- minQuotes configuration ---

  it('respects custom minQuotes', () => {
    const s = createBestOfNStrategy({ minQuotes: 5 });
    for (let i = 0; i < 4; i++) {
      s.collectQuote('req-1', { id: `q${i}`, total: 100 - i });
    }
    assert.equal(s.hasEnoughQuotes('req-1'), false);
    s.collectQuote('req-1', { id: 'q4', total: 90 });
    assert.equal(s.hasEnoughQuotes('req-1'), true);
  });
});
