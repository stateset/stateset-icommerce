/**
 * Integration tests for a2a/integration.js — Smart Wrappers for Automatic Intelligence
 *
 * These are TRUE integration tests: they import the REAL factory functions
 * from the actual source modules (memory, rules, idempotency, tracing,
 * costAnalytics, introspection) and wire them together via the integration layer.
 * The only mock is the coreA2A service itself (a simple object with methods
 * that return success objects).
 *
 * Covers:
 * 1.  pay() auto-applies idempotency
 * 2.  pay() auto-records cost analytics
 * 3.  pay() auto-records memory
 * 4.  pay() auto-evaluates rules (block case)
 * 5.  pay() auto-wraps in trace span
 * 6.  acceptQuote() checks rules (block case)
 * 7.  acceptQuote() checks memory (high-risk counterparty warning)
 * 8.  acceptQuote() records interaction in memory
 * 9.  evaluateQuoteWithIntelligence() enriches context
 * 10. evaluateQuoteWithIntelligence() blocks risky quotes via rules
 * 11. Graceful degradation — rules null, pay() still works
 * 12. Graceful degradation — memory null, acceptQuote() still works
 * 13. initializeServices() creates all 7 services
 * 14. Passthrough — unwrapped methods (getPayments, etc.) still work
 * 15. Introspection records decisions after operations
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createIntegratedA2AService, initializeServices } from '../../src/a2a/integration.js';
import { createAgentMemory } from '../../src/a2a/agent-memory.js';
import { createRulesEngine } from '../../src/a2a/rules-engine.js';
import { createIdempotencyGuard } from '../../src/a2a/idempotency.js';
import { createTracingService } from '../../src/a2a/tracing.js';
import { createCostAnalytics } from '../../src/a2a/cost-analytics.js';
import { createIntrospectionService } from '../../src/a2a/introspection.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const AGENT_ADDRESS = '0xBuyer';
const SELLER_ADDRESS = '0xSeller';

/**
 * Create a mock coreA2A service that returns predictable results.
 * Methods are simple stubs — no real commerce backend needed.
 */
function createMockCoreA2A() {
  let payCallCount = 0;

  return {
    walletAddress: AGENT_ADDRESS,
    agentId: 'agent-test-001',

    async pay(params) {
      payCallCount++;
      return {
        success: true,
        payment: {
          id: `pay-${payCallCount}`,
          status: 'submitted',
          from: AGENT_ADDRESS,
          to: params.to,
          amount: params.amount,
          asset: params.asset || 'USDC',
          network: 'set_chain',
          memo: params.memo || null,
          createdAt: new Date().toISOString(),
        },
      };
    },

    async acceptQuote(quoteId) {
      return {
        success: true,
        payment: {
          id: 'pay-quote-1',
          status: 'submitted',
          from: AGENT_ADDRESS,
          to: SELLER_ADDRESS,
          amount: 250,
          asset: 'USDC',
        },
        quote: {
          id: quoteId,
          status: 'accepted',
          total: 250,
          asset: 'USDC',
          seller: SELLER_ADDRESS,
        },
      };
    },

    async requestQuote(params) {
      return {
        success: true,
        quote: {
          id: 'quote-req-001',
          status: 'requested',
          buyer: AGENT_ADDRESS,
          seller: params.seller,
          items: params.items,
          total: 0,
          asset: params.asset || 'USDC',
        },
      };
    },

    async getPayments() {
      return [
        { id: 'pay-history-1', amount: 100, status: 'completed' },
        { id: 'pay-history-2', amount: 200, status: 'completed' },
      ];
    },

    async getQuotes() {
      return [{ id: 'q-1', status: 'quoted', total: 150 }];
    },

    async getBalance() {
      return { walletAddress: AGENT_ADDRESS, totalSent: 500, totalReceived: 300, netFlow: -200 };
    },

    getPayCallCount() {
      return payCallCount;
    },
  };
}

/**
 * Create the full set of REAL services for integration testing.
 */
function createRealServices() {
  return {
    memory: createAgentMemory(),
    rules: createRulesEngine(),
    idempotency: createIdempotencyGuard({ ttlMs: 60_000 }),
    tracing: createTracingService({ maxSpans: 1000, serviceName: 'test-agent' }),
    costAnalytics: createCostAnalytics(),
    introspection: createIntrospectionService(),
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('A2A Integration Layer', () => {
  /** @type {ReturnType<typeof createMockCoreA2A>} */
  let coreA2A;
  /** @type {ReturnType<typeof createRealServices>} */
  let services;
  /** @type {Object} */
  let a2a;

  beforeEach(() => {
    coreA2A = createMockCoreA2A();
    services = createRealServices();
    a2a = createIntegratedA2AService(coreA2A, services);
  });

  // -------------------------------------------------------------------------
  // 1. pay() auto-applies idempotency
  // -------------------------------------------------------------------------

  describe('pay() idempotency', () => {
    it('should execute payment only once for the same idempotency key', async () => {
      const params = {
        to: SELLER_ADDRESS,
        amount: 100,
        asset: 'USDC',
        memo: 'test payment',
        idempotencyKey: 'idem-key-001',
      };

      const result1 = await a2a.pay(params);
      const result2 = await a2a.pay(params);

      // Both should succeed with the same result
      assert.equal(result1.success, true);
      assert.equal(result2.success, true);
      assert.deepEqual(result1, result2);

      // Core pay should only have been called once
      assert.equal(coreA2A.getPayCallCount(), 1);
    });

    it('should generate unique idempotency keys when not provided', async () => {
      const params1 = { to: SELLER_ADDRESS, amount: 50, memo: 'first' };
      const params2 = { to: SELLER_ADDRESS, amount: 50, memo: 'second' };

      await a2a.pay(params1);
      await a2a.pay(params2);

      // Both should execute because keys are different
      assert.equal(coreA2A.getPayCallCount(), 2);
    });
  });

  // -------------------------------------------------------------------------
  // 2. pay() auto-records cost analytics
  // -------------------------------------------------------------------------

  describe('pay() cost analytics', () => {
    it('should record a spend entry in cost analytics after successful payment', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 75.50, asset: 'USDC', memo: 'widget' });

      const summary = services.costAnalytics.getAgentSpendSummary(AGENT_ADDRESS);
      assert.equal(summary.transactionCount, 1);
      assert.equal(summary.totalSpent, 75.50);
    });

    it('should record multiple payments cumulatively', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 100 });
      await a2a.pay({ to: '0xVendor', amount: 200 });

      const summary = services.costAnalytics.getAgentSpendSummary(AGENT_ADDRESS);
      assert.equal(summary.transactionCount, 2);
      assert.equal(summary.totalSpent, 300);
    });
  });

  // -------------------------------------------------------------------------
  // 3. pay() auto-records memory
  // -------------------------------------------------------------------------

  describe('pay() memory recording', () => {
    it('should record a payment_sent interaction in agent memory', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 100 });

      const history = services.memory.getInteractionHistory(AGENT_ADDRESS, SELLER_ADDRESS);
      assert.equal(history.length, 1);
      assert.equal(history[0].interactionType, 'payment_sent');
      assert.equal(history[0].outcome, 'success');
      assert.equal(history[0].amount, 100);
    });

    it('should build a counterparty profile after payment', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 50 });

      const profile = services.memory.getCounterpartyProfile(AGENT_ADDRESS, SELLER_ADDRESS);
      assert.equal(profile.totalInteractions, 1);
      assert.equal(profile.successRate, 1);
    });
  });

  // -------------------------------------------------------------------------
  // 4. pay() auto-evaluates rules (block case)
  // -------------------------------------------------------------------------

  describe('pay() rule evaluation', () => {
    it('should throw when a rule blocks the payment', async () => {
      // Add a rule that blocks payments over $500
      services.rules.addRule({
        name: 'Max payment limit',
        condition: { field: 'amount', operator: 'gt', value: 500 },
        action: { type: 'block', params: { reason: 'exceeds max payment' } },
        priority: 90,
      });

      await assert.rejects(
        () => a2a.pay({ to: SELLER_ADDRESS, amount: 1000, asset: 'USDC' }),
        (err) => {
          assert.ok(err.message.includes('blocked by rules'));
          assert.ok(err.message.includes('Max payment limit'));
          return true;
        },
      );

      // Core pay should NOT have been called
      assert.equal(coreA2A.getPayCallCount(), 0);
    });

    it('should allow payment when rules pass', async () => {
      services.rules.addRule({
        name: 'Max payment limit',
        condition: { field: 'amount', operator: 'gt', value: 500 },
        action: { type: 'block', params: { reason: 'exceeds max' } },
        priority: 90,
      });

      const result = await a2a.pay({ to: SELLER_ADDRESS, amount: 100 });
      assert.equal(result.success, true);
      assert.equal(coreA2A.getPayCallCount(), 1);
    });
  });

  // -------------------------------------------------------------------------
  // 5. pay() auto-wraps in trace span
  // -------------------------------------------------------------------------

  describe('pay() tracing', () => {
    it('should create a trace span with correct attributes after payment', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 42, asset: 'USDC', memo: 'traced' });

      const spans = services.tracing.getRecentSpans(10);
      assert.ok(spans.length > 0, 'Expected at least one span');

      const paySpan = spans.find((s) => s.name === 'a2a.pay');
      assert.ok(paySpan, 'Expected a span named "a2a.pay"');
      assert.equal(paySpan.status, 'ok');
      assert.ok(paySpan.durationMs != null, 'Span should have duration');
      assert.equal(paySpan.attributes['a2a.operation'], 'pay');
      assert.equal(paySpan.attributes['a2a.recipient'], SELLER_ADDRESS);
    });
  });

  // -------------------------------------------------------------------------
  // 6. acceptQuote() checks rules (block case)
  // -------------------------------------------------------------------------

  describe('acceptQuote() rule evaluation', () => {
    it('should throw when a rule blocks quote acceptance', async () => {
      // Add a rule that blocks accept_quote operations
      services.rules.addRule({
        name: 'Block all quote accepts',
        condition: { field: 'operationType', operator: 'eq', value: 'accept_quote' },
        action: { type: 'block', params: { reason: 'trading paused' } },
        priority: 95,
      });

      await assert.rejects(
        () => a2a.acceptQuote('quote-123'),
        (err) => {
          assert.ok(err.message.includes('blocked by rules'));
          return true;
        },
      );
    });
  });

  // -------------------------------------------------------------------------
  // 7. acceptQuote() checks memory (high-risk counterparty)
  // -------------------------------------------------------------------------

  describe('acceptQuote() memory intelligence', () => {
    it('should add a warning when counterparty is high-risk', async () => {
      // Create a high-risk profile by recording disputes
      for (let i = 0; i < 5; i++) {
        services.memory.recordInteraction({
          agentAddress: AGENT_ADDRESS,
          counterpartyAddress: SELLER_ADDRESS,
          interactionType: 'dispute',
          outcome: 'failure',
          amount: 100,
        });
      }

      const profile = services.memory.getCounterpartyProfile(AGENT_ADDRESS, SELLER_ADDRESS);
      assert.equal(profile.riskLevel, 'high');

      const result = await a2a.acceptQuote('quote-risky');
      assert.ok(result.warning, 'Expected a warning for high-risk counterparty');
      assert.ok(result.warning.includes('high risk'));
    });
  });

  // -------------------------------------------------------------------------
  // 8. acceptQuote() records interaction in memory
  // -------------------------------------------------------------------------

  describe('acceptQuote() memory recording', () => {
    it('should record a quote_received/accepted interaction', async () => {
      await a2a.acceptQuote('quote-456');

      const history = services.memory.getInteractionHistory(AGENT_ADDRESS, SELLER_ADDRESS);
      const quoteInteraction = history.find(
        (h) => h.interactionType === 'quote_received' && h.outcome === 'accepted',
      );
      assert.ok(quoteInteraction, 'Expected a quote_received/accepted interaction');
      assert.equal(quoteInteraction.amount, 250); // From mock
    });
  });

  // -------------------------------------------------------------------------
  // 9. evaluateQuoteWithIntelligence() enriches context
  // -------------------------------------------------------------------------

  describe('evaluateQuoteWithIntelligence()', () => {
    it('should return enriched result with profile, ruleResult, and recommendation', () => {
      // Seed memory with interactions so there's a profile
      for (let i = 0; i < 3; i++) {
        services.memory.recordInteraction({
          agentAddress: AGENT_ADDRESS,
          counterpartyAddress: SELLER_ADDRESS,
          interactionType: 'payment_sent',
          outcome: 'success',
          amount: 100,
          responseTimeMs: 500,
        });
      }

      const quote = {
        id: 'q-eval-1',
        seller: SELLER_ADDRESS,
        total: 200,
        asset: 'USDC',
      };

      const strategy = {
        evaluateQuote: (q, ctx) => ({
          action: 'accept',
          reason: `Price ${q.total} is within budget. Profile risk: ${ctx.profile?.riskLevel}`,
        }),
      };

      const result = a2a.evaluateQuoteWithIntelligence(quote, strategy);

      assert.equal(result.action, 'accept');
      assert.ok(result.reason.includes('within budget'));
      assert.ok(result.profile, 'Expected profile in result');
      assert.equal(result.profile.totalInteractions, 3);
      assert.equal(result.profile.riskLevel, 'low');
      assert.ok(result.ruleResult, 'Expected ruleResult in result');
      assert.ok(result.recommendation, 'Expected recommendation in result');
    });

    it('should pass enriched context to strategy evaluateQuote', () => {
      let capturedContext = null;

      const strategy = {
        evaluateQuote: (_q, ctx) => {
          capturedContext = ctx;
          return { action: 'accept', reason: 'ok' };
        },
      };

      const quote = { id: 'q-ctx', seller: SELLER_ADDRESS, total: 50 };
      a2a.evaluateQuoteWithIntelligence(quote, strategy);

      assert.ok(capturedContext, 'Strategy should receive enriched context');
      assert.ok('profile' in capturedContext);
      assert.ok('ruleResult' in capturedContext);
      assert.ok('recommendation' in capturedContext);
    });
  });

  // -------------------------------------------------------------------------
  // 10. evaluateQuoteWithIntelligence() blocks risky quotes via rules
  // -------------------------------------------------------------------------

  describe('evaluateQuoteWithIntelligence() rule blocking', () => {
    it('should return decline when rules block the quote', () => {
      services.rules.addRule({
        name: 'Block high value quotes',
        condition: { field: 'amount', operator: 'gt', value: 1000 },
        action: { type: 'block', params: { reason: 'quote too expensive' } },
        priority: 90,
      });

      const quote = {
        id: 'q-expensive',
        seller: SELLER_ADDRESS,
        total: 5000,
        asset: 'USDC',
      };

      const strategy = {
        evaluateQuote: () => ({ action: 'accept', reason: 'looks good' }),
      };

      const result = a2a.evaluateQuoteWithIntelligence(quote, strategy);

      assert.equal(result.action, 'decline');
      assert.ok(result.reason.includes('Block high value quotes'));
      assert.ok(result.ruleResult);
      assert.equal(result.ruleResult.allowed, false);
    });
  });

  // -------------------------------------------------------------------------
  // 11. Graceful degradation — rules null, pay() still works
  // -------------------------------------------------------------------------

  describe('Graceful degradation (rules = null)', () => {
    it('should still execute pay() when rules service is null', async () => {
      const degradedA2A = createIntegratedA2AService(coreA2A, {
        ...services,
        rules: null,
      });

      const result = await degradedA2A.pay({ to: SELLER_ADDRESS, amount: 100 });
      assert.equal(result.success, true);
      assert.equal(coreA2A.getPayCallCount(), 1);
    });
  });

  // -------------------------------------------------------------------------
  // 12. Graceful degradation — memory null, acceptQuote() still works
  // -------------------------------------------------------------------------

  describe('Graceful degradation (memory = null)', () => {
    it('should still execute acceptQuote() when memory is null', async () => {
      const degradedA2A = createIntegratedA2AService(coreA2A, {
        ...services,
        memory: null,
      });

      const result = await degradedA2A.acceptQuote('quote-no-memory');
      assert.equal(result.success, true);
      assert.equal(result.quote.status, 'accepted');
      // No warning since memory is null
      assert.equal(result.warning, undefined);
    });
  });

  // -------------------------------------------------------------------------
  // 13. initializeServices() creates all services
  // -------------------------------------------------------------------------

  describe('initializeServices()', () => {
    it('should create an object with all 7 services', () => {
      const svcs = initializeServices();

      assert.ok(svcs.memory, 'Expected memory service');
      assert.ok(svcs.rules, 'Expected rules service');
      assert.ok(svcs.idempotency, 'Expected idempotency service');
      assert.ok(svcs.tracing, 'Expected tracing service');
      assert.ok(svcs.costAnalytics, 'Expected costAnalytics service');
      assert.ok(svcs.introspection, 'Expected introspection service');
      assert.ok(svcs.scheduler, 'Expected scheduler service');
    });

    it('should create functional services that can be used', () => {
      const svcs = initializeServices();

      // Memory
      assert.equal(typeof svcs.memory.recordInteraction, 'function');
      assert.equal(typeof svcs.memory.getCounterpartyProfile, 'function');

      // Rules
      assert.equal(typeof svcs.rules.addRule, 'function');
      assert.equal(typeof svcs.rules.evaluate, 'function');

      // Idempotency
      assert.equal(typeof svcs.idempotency.execute, 'function');

      // Tracing
      assert.equal(typeof svcs.tracing.startSpan, 'function');
      assert.equal(typeof svcs.tracing.withSpan, 'function');

      // Cost analytics
      assert.equal(typeof svcs.costAnalytics.record, 'function');
      assert.equal(typeof svcs.costAnalytics.getAgentSpendSummary, 'function');

      // Introspection
      assert.equal(typeof svcs.introspection.recordDecision, 'function');
      assert.equal(typeof svcs.introspection.getDecisionHistory, 'function');

      // Scheduler
      assert.equal(typeof svcs.scheduler.scheduleAction, 'function');
    });

    it('should pass options through to underlying services', () => {
      const svcs = initializeServices({
        idempotency: { ttlMs: 1000, maxSize: 50 },
        tracing: { maxSpans: 100, serviceName: 'custom-test' },
      });

      // Verify tracing got the custom service name by checking a span's attributes
      const span = svcs.tracing.startSpan('test');
      span.end();
      const spans = svcs.tracing.getRecentSpans(1);
      assert.equal(spans[0].attributes['service.name'], 'custom-test');
    });
  });

  // -------------------------------------------------------------------------
  // 14. Passthrough — unwrapped methods still work
  // -------------------------------------------------------------------------

  describe('Passthrough of unwrapped methods', () => {
    it('should pass through getPayments() to coreA2A', async () => {
      const payments = await a2a.getPayments();
      assert.equal(payments.length, 2);
      assert.equal(payments[0].id, 'pay-history-1');
    });

    it('should pass through getQuotes() to coreA2A', async () => {
      const quotes = await a2a.getQuotes();
      assert.equal(quotes.length, 1);
      assert.equal(quotes[0].id, 'q-1');
    });

    it('should pass through getBalance() to coreA2A', async () => {
      const balance = await a2a.getBalance();
      assert.equal(balance.walletAddress, AGENT_ADDRESS);
      assert.equal(balance.netFlow, -200);
    });

    it('should expose walletAddress and agentId from coreA2A', () => {
      assert.equal(a2a.walletAddress, AGENT_ADDRESS);
      assert.equal(a2a.agentId, 'agent-test-001');
    });
  });

  // -------------------------------------------------------------------------
  // 15. Introspection records decisions after operations
  // -------------------------------------------------------------------------

  describe('Introspection decision recording', () => {
    it('should record a payment decision after successful pay()', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 80, asset: 'USDC' });

      const decisions = services.introspection.getDecisionHistory(AGENT_ADDRESS);
      assert.ok(decisions.length > 0, 'Expected at least one decision');

      const paymentDecision = decisions.find(
        (d) => d.type === 'payment' && d.action === 'accept',
      );
      assert.ok(paymentDecision, 'Expected a payment/accept decision');
      assert.ok(paymentDecision.context.amount === 80);
      assert.ok(paymentDecision.context.to === SELLER_ADDRESS);
    });

    it('should record a reject decision when rules block payment', async () => {
      services.rules.addRule({
        name: 'Block all',
        condition: { field: 'amount', operator: 'gt', value: 0 },
        action: { type: 'block', params: { reason: 'all blocked' } },
        priority: 99,
      });

      await assert.rejects(() => a2a.pay({ to: SELLER_ADDRESS, amount: 10 }));

      const decisions = services.introspection.getDecisionHistory(AGENT_ADDRESS);
      const rejectDecision = decisions.find(
        (d) => d.type === 'payment' && d.action === 'reject',
      );
      assert.ok(rejectDecision, 'Expected a payment/reject decision');
      assert.ok(
        rejectDecision.reason.toLowerCase().includes('block'),
        `Expected reason to mention blocking, got: ${rejectDecision.reason}`,
      );
    });

    it('should record a quote_eval decision after acceptQuote()', async () => {
      await a2a.acceptQuote('quote-789');

      const decisions = services.introspection.getDecisionHistory(AGENT_ADDRESS);
      const quoteDecision = decisions.find(
        (d) => d.type === 'quote_eval' && d.action === 'accept',
      );
      assert.ok(quoteDecision, 'Expected a quote_eval/accept decision');
    });

    it('should record a decision from evaluateQuoteWithIntelligence()', () => {
      const quote = { id: 'q-intro', seller: SELLER_ADDRESS, total: 100 };
      const strategy = {
        evaluateQuote: () => ({ action: 'accept', reason: 'good price' }),
      };

      a2a.evaluateQuoteWithIntelligence(quote, strategy);

      const decisions = services.introspection.getDecisionHistory(AGENT_ADDRESS);
      assert.ok(decisions.length > 0, 'Expected decision in introspection');
      const lastDecision = decisions[0]; // Most recent first
      assert.equal(lastDecision.type, 'quote_eval');
    });

    it('should record decisions with full context across multiple operations', async () => {
      // Do several operations
      await a2a.pay({ to: SELLER_ADDRESS, amount: 50 });
      await a2a.pay({ to: '0xVendor', amount: 75 });
      await a2a.acceptQuote('quote-multi');

      const decisions = services.introspection.getDecisionHistory(AGENT_ADDRESS);
      // We expect at least 3 decisions (two payments + one quote accept)
      assert.ok(decisions.length >= 3, `Expected >= 3 decisions, got ${decisions.length}`);
    });
  });

  // -------------------------------------------------------------------------
  // Additional edge cases
  // -------------------------------------------------------------------------

  describe('Edge cases', () => {
    it('should handle pay() with all services null (fully degraded)', async () => {
      const bareA2A = createIntegratedA2AService(coreA2A, {
        memory: null,
        rules: null,
        idempotency: null,
        tracing: null,
        costAnalytics: null,
        introspection: null,
      });

      const result = await bareA2A.pay({ to: SELLER_ADDRESS, amount: 100 });
      assert.equal(result.success, true);
    });

    it('should handle acceptQuote() with all services null', async () => {
      const bareA2A = createIntegratedA2AService(coreA2A, {
        memory: null,
        rules: null,
        idempotency: null,
        tracing: null,
        costAnalytics: null,
        introspection: null,
      });

      const result = await bareA2A.acceptQuote('quote-bare');
      assert.equal(result.success, true);
    });

    it('should expose _services for direct service access', () => {
      const svc = a2a._services;
      assert.ok(svc.memory, 'Expected memory on _services');
      assert.ok(svc.rules, 'Expected rules on _services');
    });

    it('should handle requestQuote() with tracing', async () => {
      const result = await a2a.requestQuote({
        seller: SELLER_ADDRESS,
        items: [{ description: 'Widget', quantity: 2 }],
      });

      assert.equal(result.success, true);
      assert.equal(result.quote.id, 'quote-req-001');

      // Check trace span was created
      const spans = services.tracing.getRecentSpans(10);
      const reqSpan = spans.find((s) => s.name === 'a2a.requestQuote');
      assert.ok(reqSpan, 'Expected a trace span for requestQuote');
    });

    it('should record requestQuote() in memory', async () => {
      await a2a.requestQuote({
        seller: SELLER_ADDRESS,
        items: [{ description: 'Widget' }],
      });

      const history = services.memory.getInteractionHistory(AGENT_ADDRESS, SELLER_ADDRESS);
      const quoteSent = history.find((h) => h.interactionType === 'quote_sent');
      assert.ok(quoteSent, 'Expected quote_sent interaction');
      assert.equal(quoteSent.outcome, 'success');
    });

    it('should handle evaluateQuoteWithIntelligence() with no strategy', () => {
      const quote = { id: 'q-no-strategy', seller: SELLER_ADDRESS, total: 100 };
      const result = a2a.evaluateQuoteWithIntelligence(quote, null);

      assert.equal(result.action, 'accept');
      assert.ok(result.reason.includes('No strategy configured'));
    });

    it('should handle evaluateQuoteWithIntelligence() when strategy throws', () => {
      const quote = { id: 'q-bad-strategy', seller: SELLER_ADDRESS, total: 100 };
      const strategy = {
        evaluateQuote: () => { throw new Error('strategy exploded'); },
      };

      const result = a2a.evaluateQuoteWithIntelligence(quote, strategy);
      assert.equal(result.action, 'skip');
      assert.ok(result.reason.includes('failed'));
    });

    it('should record cost analytics with correct counterparty breakdown', async () => {
      await a2a.pay({ to: SELLER_ADDRESS, amount: 100 });
      await a2a.pay({ to: '0xVendorB', amount: 200 });

      const breakdown = services.costAnalytics.getCounterpartyBreakdown(AGENT_ADDRESS);
      assert.equal(breakdown.length, 2);

      // Sorted by volume descending
      assert.equal(breakdown[0].counterparty, '0xVendorB');
      assert.equal(breakdown[0].spent, 200);
      assert.equal(breakdown[1].counterparty, SELLER_ADDRESS);
      assert.equal(breakdown[1].spent, 100);
    });
  });
});
