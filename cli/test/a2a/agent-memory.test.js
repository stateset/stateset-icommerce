/**
 * Unit tests for a2a/agent-memory.js — Counterparty Learning & Recommendation Engine
 *
 * Covers: recordInteraction, getInteractionHistory, getCounterpartyProfile,
 * getTopCounterparties, getRecommendation, getAgentInsights, forget, clear,
 * separate memory spaces, empty history defaults, reliability scoring.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createAgentMemory } from '../../src/a2a/agent-memory.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const AGENT = '0xAgent';
const AGENT_B = '0xAgentB';
const SELLER = '0xSeller';
const BUYER = '0xBuyer';
const RISKY = '0xRisky';

/**
 * Record N successful payment interactions quickly.
 */
function recordSuccessfulPayments(memory, agent, counterparty, count, responseTimeMs = 1000) {
  for (let i = 0; i < count; i++) {
    memory.recordInteraction({
      agentAddress: agent,
      counterpartyAddress: counterparty,
      interactionType: 'payment_sent',
      outcome: 'success',
      amount: 100,
      responseTimeMs,
      metadata: { asset: 'USDC', network: 'set_chain' },
    });
  }
}

/**
 * Record a dispute interaction.
 */
function recordDispute(memory, agent, counterparty) {
  memory.recordInteraction({
    agentAddress: agent,
    counterpartyAddress: counterparty,
    interactionType: 'dispute',
    outcome: 'failure',
    amount: 50,
    responseTimeMs: 30000,
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Agent Memory', () => {
  /** @type {ReturnType<typeof createAgentMemory>} */
  let memory;

  beforeEach(() => {
    memory = createAgentMemory();
  });

  // -------------------------------------------------------------------------
  // 1. recordInteraction stores and getInteractionHistory retrieves
  // -------------------------------------------------------------------------
  describe('recordInteraction + getInteractionHistory', () => {
    it('stores an interaction and retrieves it', () => {
      const result = memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
        amount: 250,
        responseTimeMs: 500,
        metadata: { asset: 'USDC' },
      });

      assert.ok(result.id, 'should return an id');
      assert.ok(result.timestamp, 'should return a timestamp');

      const history = memory.getInteractionHistory(AGENT, SELLER);
      assert.equal(history.length, 1);
      assert.equal(history[0].interactionType, 'payment_sent');
      assert.equal(history[0].outcome, 'success');
      assert.equal(history[0].amount, 250);
      assert.equal(history[0].responseTimeMs, 500);
    });

    it('returns newest first', () => {
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
      });
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'fulfillment',
        outcome: 'success',
      });

      const history = memory.getInteractionHistory(AGENT, SELLER);
      assert.equal(history.length, 2);
      assert.equal(history[0].interactionType, 'fulfillment');
      assert.equal(history[1].interactionType, 'payment_sent');
    });

    it('respects limit parameter', () => {
      for (let i = 0; i < 10; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
        });
      }

      const history = memory.getInteractionHistory(AGENT, SELLER, 3);
      assert.equal(history.length, 3);
    });

    it('rejects invalid interactionType', () => {
      assert.throws(
        () =>
          memory.recordInteraction({
            agentAddress: AGENT,
            counterpartyAddress: SELLER,
            interactionType: 'invalid_type',
            outcome: 'success',
          }),
        /Invalid interactionType/,
      );
    });

    it('rejects invalid outcome', () => {
      assert.throws(
        () =>
          memory.recordInteraction({
            agentAddress: AGENT,
            counterpartyAddress: SELLER,
            interactionType: 'payment_sent',
            outcome: 'unknown',
          }),
        /Invalid outcome/,
      );
    });

    it('rejects missing required fields', () => {
      assert.throws(
        () =>
          memory.recordInteraction({
            agentAddress: AGENT,
            interactionType: 'payment_sent',
            outcome: 'success',
          }),
        /Missing or invalid required field: counterpartyAddress/,
      );
    });
  });

  // -------------------------------------------------------------------------
  // 2. getCounterpartyProfile computes correct successRate
  // -------------------------------------------------------------------------
  describe('getCounterpartyProfile — successRate', () => {
    it('computes correct success rate', () => {
      // 7 successes + 3 failures = 70% success rate
      for (let i = 0; i < 7; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
        });
      }
      for (let i = 0; i < 3; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'failure',
        });
      }

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.totalInteractions, 10);
      assert.equal(profile.successRate, 0.7);
    });

    it('counts accepted as success', () => {
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'negotiation',
        outcome: 'accepted',
      });
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'negotiation',
        outcome: 'rejected',
      });

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.successRate, 0.5);
    });
  });

  // -------------------------------------------------------------------------
  // 3. getCounterpartyProfile computes avgResponseTimeMs
  // -------------------------------------------------------------------------
  describe('getCounterpartyProfile — avgResponseTimeMs', () => {
    it('computes average response time from interactions with timing data', () => {
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
        responseTimeMs: 1000,
      });
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
        responseTimeMs: 3000,
      });
      // No responseTimeMs — should be excluded from average
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'fulfillment',
        outcome: 'success',
      });

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.avgResponseTimeMs, 2000);
    });
  });

  // -------------------------------------------------------------------------
  // 4. getCounterpartyProfile determines riskLevel correctly
  // -------------------------------------------------------------------------
  describe('getCounterpartyProfile — riskLevel', () => {
    it('returns high when dispute rate exceeds 20% with sufficient history', () => {
      // 3 disputes, 7 successes = 30% dispute rate
      for (let i = 0; i < 7; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: RISKY,
          interactionType: 'payment_sent',
          outcome: 'success',
        });
      }
      for (let i = 0; i < 3; i++) {
        recordDispute(memory, AGENT, RISKY);
      }

      const profile = memory.getCounterpartyProfile(AGENT, RISKY);
      assert.equal(profile.riskLevel, 'high');
    });

    it('returns medium when dispute rate is 10-20%', () => {
      // 2 failures, 10 successes = ~16.7% rate
      for (let i = 0; i < 10; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
        });
      }
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'failure',
      });
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'failure',
      });

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.riskLevel, 'medium');
    });

    it('returns low when dispute/failure rate is under 10%', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 20);

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.riskLevel, 'low');
    });

    it('does not assign high risk with insufficient history', () => {
      // 1 dispute, 1 success = 50% but only 2 interactions
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: RISKY,
        interactionType: 'payment_sent',
        outcome: 'success',
      });
      recordDispute(memory, AGENT, RISKY);

      const profile = memory.getCounterpartyProfile(AGENT, RISKY);
      // With only 2 interactions, should be medium at worst
      assert.notEqual(profile.riskLevel, 'high');
    });
  });

  // -------------------------------------------------------------------------
  // 5. getCounterpartyProfile computes negotiationPattern
  // -------------------------------------------------------------------------
  describe('getCounterpartyProfile — negotiationPattern', () => {
    it('computes avg discount and counter-offer rate from quote interactions', () => {
      // Quote negotiation: $100 -> $85 (15% discount)
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'negotiation',
        outcome: 'accepted',
        metadata: { originalAmount: 100, finalAmount: 85 },
      });
      // Quote negotiation: $200 -> $180 (10% discount)
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'quote_received',
        outcome: 'accepted',
        metadata: { originalAmount: 200, finalAmount: 180 },
      });
      // Counter-offer that was rejected
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'quote_sent',
        outcome: 'rejected',
        metadata: { originalAmount: 150, finalAmount: 150, counterOffer: true },
      });

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      // Average discount: (15 + 10 + 0) / 3 = 8.33%
      assert.equal(profile.negotiationPattern.sampleSize, 3);
      assert.ok(profile.negotiationPattern.avgDiscountPct > 0);
      // Counter-offer rate: 2 counter-offers (1 explicit + 1 rejected) / 3 = 0.667
      assert.ok(profile.negotiationPattern.counterOfferRate > 0);
    });

    it('returns zeros when no negotiation interactions', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 5);

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.negotiationPattern.avgDiscountPct, 0);
      assert.equal(profile.negotiationPattern.counterOfferRate, 0);
      assert.equal(profile.negotiationPattern.sampleSize, 0);
    });
  });

  // -------------------------------------------------------------------------
  // 6. getTopCounterparties ranks correctly by volume
  // -------------------------------------------------------------------------
  describe('getTopCounterparties', () => {
    it('ranks by volume (total interactions) by default', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 10);
      recordSuccessfulPayments(memory, AGENT, BUYER, 5);
      recordSuccessfulPayments(memory, AGENT, RISKY, 20);

      const top = memory.getTopCounterparties(AGENT, { limit: 2 });
      assert.equal(top.length, 2);
      assert.equal(top[0].counterpartyAddress, RISKY);
      assert.equal(top[0].totalInteractions, 20);
      assert.equal(top[1].counterpartyAddress, SELLER);
    });

    it('ranks by success_rate when requested', () => {
      // SELLER: 10/10 = 100%
      recordSuccessfulPayments(memory, AGENT, SELLER, 10);
      // RISKY: 5/10 = 50%
      recordSuccessfulPayments(memory, AGENT, RISKY, 5);
      for (let i = 0; i < 5; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: RISKY,
          interactionType: 'payment_sent',
          outcome: 'failure',
        });
      }

      const top = memory.getTopCounterparties(AGENT, {
        sortBy: 'success_rate',
        limit: 10,
      });
      assert.equal(top[0].counterpartyAddress, SELLER);
      assert.equal(top[0].successRate, 1);
    });

    it('ranks by reliability when requested', () => {
      // SELLER: fast responses → high reliability
      recordSuccessfulPayments(memory, AGENT, SELLER, 10, 500);
      // BUYER: slow responses → lower reliability
      recordSuccessfulPayments(memory, AGENT, BUYER, 10, 20000);

      const top = memory.getTopCounterparties(AGENT, {
        sortBy: 'reliability',
      });
      assert.equal(top[0].counterpartyAddress, SELLER);
      assert.ok(top[0].reliabilityScore > top[1].reliabilityScore);
    });

    it('returns empty for unknown agent', () => {
      const top = memory.getTopCounterparties('0xUnknown');
      assert.deepEqual(top, []);
    });
  });

  // -------------------------------------------------------------------------
  // 7. getRecommendation returns positive for good counterparty
  // -------------------------------------------------------------------------
  describe('getRecommendation — positive', () => {
    it('recommends a counterparty with high success rate', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 20, 500);

      const rec = memory.getRecommendation(AGENT, SELLER, 'payment_sent');
      assert.equal(rec.recommended, true);
      assert.ok(rec.confidence > 0.5, `confidence should be > 0.5, got ${rec.confidence}`);
      assert.ok(rec.reason.includes('100%'), 'should mention success rate');
    });

    it('returns neutral recommendation for unknown counterparty', () => {
      const rec = memory.getRecommendation(AGENT, '0xStranger', 'payment_sent');
      assert.equal(rec.recommended, true);
      assert.ok(rec.confidence <= 0.5, 'low confidence for unknown');
      assert.ok(rec.reason.includes('No prior interaction'));
      assert.deepEqual(rec.suggestedTerms, { escrow: true });
    });
  });

  // -------------------------------------------------------------------------
  // 8. getRecommendation returns negative for risky counterparty
  // -------------------------------------------------------------------------
  describe('getRecommendation — negative', () => {
    it('does not recommend a counterparty with high dispute rate', () => {
      // 3 successes, 7 disputes = 30% success rate, 70% dispute
      for (let i = 0; i < 3; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: RISKY,
          interactionType: 'payment_sent',
          outcome: 'success',
          responseTimeMs: 2000,
        });
      }
      for (let i = 0; i < 7; i++) {
        recordDispute(memory, AGENT, RISKY);
      }

      const rec = memory.getRecommendation(AGENT, RISKY, 'payment_sent');
      assert.equal(rec.recommended, false);
      assert.ok(rec.reason.includes('HIGH risk'));
      assert.ok(rec.suggestedTerms, 'should suggest escrow terms');
      assert.equal(rec.suggestedTerms.escrow, true);
    });

    it('flags recent dispute cluster', () => {
      // 10 successes followed by 5 disputes in a row
      recordSuccessfulPayments(memory, AGENT, RISKY, 10);
      for (let i = 0; i < 5; i++) {
        recordDispute(memory, AGENT, RISKY);
      }

      const rec = memory.getRecommendation(AGENT, RISKY, 'payment_sent');
      assert.ok(rec.reason.includes('disputes in last'));
    });
  });

  // -------------------------------------------------------------------------
  // 9. getAgentInsights aggregates correctly
  // -------------------------------------------------------------------------
  describe('getAgentInsights', () => {
    it('aggregates across all counterparties', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 10);
      recordSuccessfulPayments(memory, AGENT, BUYER, 5);

      const insights = memory.getAgentInsights(AGENT);
      assert.equal(insights.totalCounterparties, 2);
      assert.equal(insights.avgSuccessRate, 1); // all successes
      assert.ok(insights.topPerformers.length > 0);
      assert.ok(Array.isArray(insights.networkPreferences));
      assert.ok(Array.isArray(insights.assetPreferences));
    });

    it('returns safe defaults for unknown agent', () => {
      const insights = memory.getAgentInsights('0xUnknown');
      assert.equal(insights.totalCounterparties, 0);
      assert.equal(insights.avgSuccessRate, 0);
      assert.deepEqual(insights.topPerformers, []);
      assert.deepEqual(insights.riskAlerts, []);
    });

    it('includes network and asset preferences', () => {
      // 8 on set_chain with USDC, 2 on base with USDT
      for (let i = 0; i < 8; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
          metadata: { asset: 'USDC', network: 'set_chain' },
        });
      }
      for (let i = 0; i < 2; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
          metadata: { asset: 'USDT', network: 'base' },
        });
      }

      const insights = memory.getAgentInsights(AGENT);
      assert.equal(insights.networkPreferences[0], 'set_chain');
      assert.equal(insights.assetPreferences[0], 'USDC');
    });
  });

  // -------------------------------------------------------------------------
  // 10. getAgentInsights flags declining counterparties in riskAlerts
  // -------------------------------------------------------------------------
  describe('getAgentInsights — riskAlerts', () => {
    it('flags counterparty whose recent success rate dropped >20%', () => {
      // First 20 interactions: all success (100% overall)
      recordSuccessfulPayments(memory, AGENT, SELLER, 20);

      // Next 10 interactions: all failures (recent window is last 10)
      for (let i = 0; i < 10; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'failure',
        });
      }

      const insights = memory.getAgentInsights(AGENT);
      assert.ok(insights.riskAlerts.length > 0, 'should have risk alerts');

      const alert = insights.riskAlerts.find(
        (a) => a.counterpartyAddress === SELLER,
      );
      assert.ok(alert, 'SELLER should be in risk alerts');
      assert.ok(
        alert.decline > 0.2,
        `decline should exceed 20%, got ${alert.decline}`,
      );
    });

    it('does not flag counterparty with stable performance', () => {
      // All 30 interactions are successful
      recordSuccessfulPayments(memory, AGENT, SELLER, 30);

      const insights = memory.getAgentInsights(AGENT);
      assert.equal(insights.riskAlerts.length, 0);
    });

    it('does not flag counterparty with insufficient history', () => {
      // Only 5 interactions (below RECENT_WINDOW of 10)
      recordSuccessfulPayments(memory, AGENT, SELLER, 3);
      recordDispute(memory, AGENT, SELLER);
      recordDispute(memory, AGENT, SELLER);

      const insights = memory.getAgentInsights(AGENT);
      assert.equal(insights.riskAlerts.length, 0);
    });
  });

  // -------------------------------------------------------------------------
  // 11. forget removes counterparty data
  // -------------------------------------------------------------------------
  describe('forget', () => {
    it('removes all memory of a counterparty', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 5);
      recordSuccessfulPayments(memory, AGENT, BUYER, 3);

      const removed = memory.forget(AGENT, SELLER);
      assert.equal(removed, true);

      // Seller data gone
      const history = memory.getInteractionHistory(AGENT, SELLER);
      assert.equal(history.length, 0);

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.totalInteractions, 0);

      // Buyer data still present
      const buyerHistory = memory.getInteractionHistory(AGENT, BUYER);
      assert.equal(buyerHistory.length, 3);
    });

    it('returns false for non-existent counterparty', () => {
      const removed = memory.forget(AGENT, '0xNonexistent');
      assert.equal(removed, false);
    });

    it('returns false for non-existent agent', () => {
      const removed = memory.forget('0xNonexistent', SELLER);
      assert.equal(removed, false);
    });
  });

  // -------------------------------------------------------------------------
  // clear
  // -------------------------------------------------------------------------
  describe('clear', () => {
    it('removes all memories for an agent', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 5);
      recordSuccessfulPayments(memory, AGENT, BUYER, 3);

      const cleared = memory.clear(AGENT);
      assert.equal(cleared, true);

      const insights = memory.getAgentInsights(AGENT);
      assert.equal(insights.totalCounterparties, 0);
    });

    it('returns false for non-existent agent', () => {
      const cleared = memory.clear('0xNonexistent');
      assert.equal(cleared, false);
    });
  });

  // -------------------------------------------------------------------------
  // 12. Different agents have separate memory spaces
  // -------------------------------------------------------------------------
  describe('agent isolation', () => {
    it('different agents have independent memory spaces', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 10);
      recordSuccessfulPayments(memory, AGENT_B, SELLER, 3);

      const profileA = memory.getCounterpartyProfile(AGENT, SELLER);
      const profileB = memory.getCounterpartyProfile(AGENT_B, SELLER);

      assert.equal(profileA.totalInteractions, 10);
      assert.equal(profileB.totalInteractions, 3);
    });

    it('clearing one agent does not affect another', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 5);
      recordSuccessfulPayments(memory, AGENT_B, SELLER, 3);

      memory.clear(AGENT);

      const profileB = memory.getCounterpartyProfile(AGENT_B, SELLER);
      assert.equal(profileB.totalInteractions, 3);
    });

    it('forgetting a counterparty for one agent preserves it for another', () => {
      recordSuccessfulPayments(memory, AGENT, SELLER, 5);
      recordSuccessfulPayments(memory, AGENT_B, SELLER, 3);

      memory.forget(AGENT, SELLER);

      const profileA = memory.getCounterpartyProfile(AGENT, SELLER);
      const profileB = memory.getCounterpartyProfile(AGENT_B, SELLER);
      assert.equal(profileA.totalInteractions, 0);
      assert.equal(profileB.totalInteractions, 3);
    });
  });

  // -------------------------------------------------------------------------
  // 13. Empty history returns safe defaults
  // -------------------------------------------------------------------------
  describe('empty history defaults', () => {
    it('getCounterpartyProfile returns safe defaults for unknown pair', () => {
      const profile = memory.getCounterpartyProfile(AGENT, '0xUnknown');
      assert.equal(profile.totalInteractions, 0);
      assert.equal(profile.successRate, 0);
      assert.equal(profile.avgResponseTimeMs, 0);
      assert.equal(profile.avgTransactionAmount, 0);
      assert.equal(profile.reliabilityScore, 0);
      assert.equal(profile.riskLevel, 'low');
      assert.equal(profile.lastInteractionAt, null);
      assert.equal(profile.firstInteractionAt, null);
      assert.equal(profile.relationship_duration_days, 0);
      assert.deepEqual(profile.negotiationPattern, {
        avgDiscountPct: 0,
        counterOfferRate: 0,
        sampleSize: 0,
      });
      assert.deepEqual(profile.preferredAssets, []);
      assert.deepEqual(profile.preferredNetworks, []);
    });

    it('getInteractionHistory returns empty array for unknown pair', () => {
      const history = memory.getInteractionHistory(AGENT, '0xUnknown');
      assert.deepEqual(history, []);
    });

    it('getTopCounterparties returns empty for agent with no data', () => {
      const top = memory.getTopCounterparties(AGENT);
      assert.deepEqual(top, []);
    });
  });

  // -------------------------------------------------------------------------
  // 14. reliabilityScore combines success rate and timeliness
  // -------------------------------------------------------------------------
  describe('reliabilityScore', () => {
    it('high reliability for fast, successful interactions', () => {
      // 100% success, all responses < 10s
      recordSuccessfulPayments(memory, AGENT, SELLER, 10, 500);

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      // reliability = 0.7 * 1.0 + 0.3 * 1.0 = 1.0
      assert.equal(profile.reliabilityScore, 1);
    });

    it('lower reliability for slow but successful interactions', () => {
      // 100% success, all responses > 10s threshold
      recordSuccessfulPayments(memory, AGENT, SELLER, 10, 20000);

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      // reliability = 0.7 * 1.0 + 0.3 * 0.0 = 0.7
      assert.equal(profile.reliabilityScore, 0.7);
    });

    it('low reliability for failed slow interactions', () => {
      // All failures with slow response times
      for (let i = 0; i < 10; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: RISKY,
          interactionType: 'payment_sent',
          outcome: 'failure',
          responseTimeMs: 30000,
        });
      }

      const profile = memory.getCounterpartyProfile(AGENT, RISKY);
      // reliability = 0.7 * 0.0 + 0.3 * 0.0 = 0.0
      assert.equal(profile.reliabilityScore, 0);
    });

    it('uses neutral timeliness when no timing data is provided', () => {
      // 100% success, no response times
      for (let i = 0; i < 10; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
        });
      }

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      // reliability = 0.7 * 1.0 + 0.3 * 0.5 = 0.85
      assert.equal(profile.reliabilityScore, 0.85);
    });

    it('mixed success and timeliness produces intermediate score', () => {
      // 5 fast successes + 5 slow failures
      for (let i = 0; i < 5; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
          responseTimeMs: 500,
        });
      }
      for (let i = 0; i < 5; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'failure',
          responseTimeMs: 20000,
        });
      }

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      // successRate = 0.5, timeliness = 5/10 = 0.5
      // reliability = 0.7 * 0.5 + 0.3 * 0.5 = 0.35 + 0.15 = 0.5
      assert.equal(profile.reliabilityScore, 0.5);
    });
  });

  // -------------------------------------------------------------------------
  // Additional edge cases
  // -------------------------------------------------------------------------
  describe('edge cases', () => {
    it('records all valid interaction types', () => {
      const types = [
        'quote_received', 'quote_sent', 'payment_sent', 'payment_received',
        'negotiation', 'dispute', 'fulfillment', 'rating',
      ];
      for (const type of types) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: type,
          outcome: 'success',
        });
      }

      const history = memory.getInteractionHistory(AGENT, SELLER);
      assert.equal(history.length, 8);
    });

    it('records all valid outcomes', () => {
      const outcomes = ['success', 'failure', 'timeout', 'rejected', 'accepted'];
      for (const outcome of outcomes) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome,
        });
      }

      const history = memory.getInteractionHistory(AGENT, SELLER);
      assert.equal(history.length, 5);
    });

    it('preferred assets and networks are sorted by frequency', () => {
      for (let i = 0; i < 3; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
          metadata: { asset: 'USDC', network: 'set_chain' },
        });
      }
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
        metadata: { asset: 'USDT', network: 'base' },
      });

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.preferredAssets[0], 'USDC');
      assert.equal(profile.preferredNetworks[0], 'set_chain');
    });

    it('avgTransactionAmount computes from interactions with amounts', () => {
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
        amount: 100,
      });
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'success',
        amount: 200,
      });
      // No amount — excluded
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'fulfillment',
        outcome: 'success',
      });

      const profile = memory.getCounterpartyProfile(AGENT, SELLER);
      assert.equal(profile.avgTransactionAmount, 150);
    });

    it('getRecommendation suggests terms for medium-risk counterparty with negotiation history', () => {
      // Build a medium-risk counterparty with negotiation history
      for (let i = 0; i < 8; i++) {
        memory.recordInteraction({
          agentAddress: AGENT,
          counterpartyAddress: SELLER,
          interactionType: 'payment_sent',
          outcome: 'success',
        });
      }
      // 2 failures out of 10 = 20% → triggers medium risk
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'failure',
      });
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'payment_sent',
        outcome: 'failure',
      });
      // Add negotiation data
      memory.recordInteraction({
        agentAddress: AGENT,
        counterpartyAddress: SELLER,
        interactionType: 'negotiation',
        outcome: 'accepted',
        metadata: { originalAmount: 100, finalAmount: 85 },
      });

      const rec = memory.getRecommendation(AGENT, SELLER, 'payment_sent');
      assert.ok(rec.suggestedTerms, 'should have suggestedTerms');
      assert.equal(rec.suggestedTerms.escrow, true);
    });

    it('topPerformers in insights are capped at 5', () => {
      for (let i = 0; i < 8; i++) {
        recordSuccessfulPayments(memory, AGENT, `0xCP${i}`, 5);
      }

      const insights = memory.getAgentInsights(AGENT);
      assert.ok(insights.topPerformers.length <= 5);
    });
  });
});
