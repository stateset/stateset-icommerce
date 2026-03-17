/**
 * End-to-End Integrated Scenario Tests — A2A Agentic Commerce Full-Stack
 *
 * These tests use REAL module instances (not mocks) to prove cross-module
 * integration across the entire A2A intelligence layer. Every service is
 * instantiated from its actual factory function and wired together, proving
 * that the system works as a cohesive whole.
 *
 * Uses Node.js built-in test runner (NOT vitest).
 *
 * Scenarios:
 *   1. Intelligent Payment with Full Stack
 *   2. Memory-Informed Quote Evaluation
 *   3. Rules Engine Cascading
 *   4. Scheduled Action + Cost Tracking
 *   5. Fan-Out Quote Collection
 *   6. Message-Driven Task Delegation
 *   7. Tracing Across Multiple Operations
 *   8. Rate Limiter + Idempotency Together
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import { createAgentMemory } from '../../src/a2a/agent-memory.js';
import { createRulesEngine } from '../../src/a2a/rules-engine.js';
import { createIdempotencyGuard } from '../../src/a2a/idempotency.js';
import { createTracingService } from '../../src/a2a/tracing.js';
import { createCostAnalytics } from '../../src/a2a/cost-analytics.js';
import { createIntrospectionService } from '../../src/a2a/introspection.js';
import { createSchedulerService } from '../../src/a2a/scheduler.js';
import { createMessagingService } from '../../src/a2a/messaging.js';
import { createSagaOrchestrator } from '../../src/a2a/saga.js';
import { createMcpRateLimiter } from '../../src/a2a/rate-limiter.js';
import { createFanOutCoordinator } from '../../src/a2a/fan-out.js';

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

const BUYER = '0xBuyer';
const SELLER_A = '0xSellerA';
const SELLER_B = '0xSellerB';
const AGENT = '0xAgent';

/**
 * Record N interactions with a counterparty for convenience.
 *
 * @param {Object} memory - Agent memory instance
 * @param {string} agent - Agent address
 * @param {string} counterparty - Counterparty address
 * @param {string} outcome - 'success' | 'failure'
 * @param {number} count - Number of interactions
 */
function recordBatch(memory, agent, counterparty, outcome, count) {
  for (let i = 0; i < count; i++) {
    memory.recordInteraction({
      agentAddress: agent,
      counterpartyAddress: counterparty,
      interactionType: 'payment_sent',
      outcome,
      amount: 100,
      responseTimeMs: 1000,
    });
  }
}

// ===========================================================================
// Scenario 1: Intelligent Payment with Full Stack
// ===========================================================================

describe('E2E Integrated Scenario 1: Intelligent Payment with Full Stack', () => {
  let memory;
  let rules;
  let idempotency;
  let tracing;
  let costAnalytics;
  let introspection;
  let rateLimiter;

  beforeEach(() => {
    memory = createAgentMemory();
    rules = createRulesEngine();
    idempotency = createIdempotencyGuard({ ttlMs: 60_000 });
    tracing = createTracingService({ serviceName: 'payment-flow' });
    costAnalytics = createCostAnalytics();
    introspection = createIntrospectionService();
    rateLimiter = createMcpRateLimiter({
      defaultLimits: { requestsPerMinute: 100 },
    });
  });

  afterEach(() => {
    rateLimiter.destroy();
  });

  it('blocks $600 payment via rules, allows $100 with full stack recording, deduplicates on retry', async () => {
    // -- Add rule: block payments > $500 without escrow --
    rules.addRule({
      name: 'High-value guard',
      description: 'Block payments over $500 without escrow',
      condition: { field: 'amount', operator: 'gt', value: 500 },
      action: { type: 'block', params: { reason: 'Amount exceeds $500, escrow required' } },
      priority: 90,
      enabled: true,
      tags: ['financial', 'safety'],
    });

    // -- Step A: Agent tries to pay $600 -> rules engine blocks --
    const evalBlocked = rules.evaluate({ amount: 600, counterparty: SELLER_A });
    assert.equal(evalBlocked.allowed, false, '$600 payment should be blocked');
    assert.ok(
      evalBlocked.explanation.includes('High-value guard'),
      'Explanation should mention rule name',
    );

    // Record the blocked decision in introspection
    introspection.recordDecision({
      agentAddress: BUYER,
      type: 'payment',
      action: 'reject',
      reason: evalBlocked.explanation,
      context: { amount: 600, counterparty: SELLER_A },
    });

    // -- Step B: Agent tries to pay $100 -> allowed --
    const evalAllowed = rules.evaluate({ amount: 100, counterparty: SELLER_A });
    assert.equal(evalAllowed.allowed, true, '$100 payment should be allowed');

    // Wrap in idempotency + tracing + cost analytics + memory
    const paymentKey = 'pay-100-seller-a-001';
    const paymentResult = await idempotency.execute(paymentKey, async () => {
      // Start tracing span
      const span = tracing.startSpan('a2a_payment.execute', {
        kind: 'server',
        attributes: { amount: 100, counterparty: SELLER_A },
      });

      try {
        // Simulate payment execution
        const result = { txId: 'tx-abc-123', amount: 100, status: 'settled' };

        // Record cost
        costAnalytics.record({
          agentAddress: BUYER,
          counterparty: SELLER_A,
          direction: 'spend',
          amount: 100,
          operation: 'quote_payment',
        });

        // Record memory interaction
        memory.recordInteraction({
          agentAddress: BUYER,
          counterpartyAddress: SELLER_A,
          interactionType: 'payment_sent',
          outcome: 'success',
          amount: 100,
          responseTimeMs: 250,
        });

        // Record decision in introspection
        introspection.recordDecision({
          agentAddress: BUYER,
          type: 'payment',
          action: 'accept',
          reason: 'Payment within budget, rules passed',
          context: { amount: 100, counterparty: SELLER_A },
        });

        span.setStatus('ok');
        span.end();

        return result;
      } catch (err) {
        span.setStatus('error');
        span.setAttribute('error.message', err.message);
        span.end();
        throw err;
      }
    });

    assert.equal(paymentResult.txId, 'tx-abc-123');
    assert.equal(paymentResult.amount, 100);

    // -- Step C: Retry same $100 payment -> idempotency returns cached --
    let executorCalled = false;
    const retryResult = await idempotency.execute(paymentKey, async () => {
      executorCalled = true;
      return { txId: 'tx-SHOULD-NOT-HAPPEN' };
    });

    assert.equal(executorCalled, false, 'Executor should NOT be called on idempotent retry');
    assert.equal(retryResult.txId, 'tx-abc-123', 'Retry should return cached result');

    // -- Assertions --

    // 1 cost record
    const spendSummary = costAnalytics.getAgentSpendSummary(BUYER);
    assert.equal(spendSummary.transactionCount, 1, 'Should have exactly 1 cost record');
    assert.equal(spendSummary.totalSpent, 100);

    // 1 memory interaction
    const history = memory.getInteractionHistory(BUYER, SELLER_A);
    assert.equal(history.length, 1, 'Should have exactly 1 memory interaction');
    assert.equal(history[0].outcome, 'success');

    // 1 trace span (the payment span)
    const recentSpans = tracing.getRecentSpans(10);
    const paymentSpans = recentSpans.filter((s) => s.name === 'a2a_payment.execute');
    assert.equal(paymentSpans.length, 1, 'Should have exactly 1 payment trace span');
    assert.equal(paymentSpans[0].status, 'ok');

    // 2 introspection decisions (1 reject + 1 accept)
    const decisions = introspection.getDecisionHistory(BUYER);
    assert.equal(decisions.length, 2, 'Should have 2 introspection decisions');
    const acceptDecisions = decisions.filter((d) => d.action === 'accept');
    const rejectDecisions = decisions.filter((d) => d.action === 'reject');
    assert.equal(acceptDecisions.length, 1);
    assert.equal(rejectDecisions.length, 1);

    // Idempotency metrics: 1 miss (first exec) + 1 hit (retry)
    const idempMetrics = idempotency.getMetrics();
    assert.equal(idempMetrics.misses, 1);
    assert.equal(idempMetrics.hits, 1);
  });
});

// ===========================================================================
// Scenario 2: Memory-Informed Quote Evaluation
// ===========================================================================

describe('E2E Integrated Scenario 2: Memory-Informed Quote Evaluation', () => {
  let memory;

  beforeEach(() => {
    memory = createAgentMemory();
  });

  it('recommends reliable Seller A and warns about unreliable Seller B', () => {
    // Record 10 successful interactions with Seller A
    recordBatch(memory, BUYER, SELLER_A, 'success', 10);

    // Record 2 successful + 8 failed interactions with Seller B
    recordBatch(memory, BUYER, SELLER_B, 'success', 2);
    recordBatch(memory, BUYER, SELLER_B, 'failure', 8);

    // -- Get recommendation for Seller A --
    const recA = memory.getRecommendation(BUYER, SELLER_A, 'payment_sent');
    assert.equal(recA.recommended, true, 'Seller A should be recommended');
    assert.ok(recA.confidence > 0.5, 'Confidence for A should be high');

    // -- Get recommendation for Seller B --
    const recB = memory.getRecommendation(BUYER, SELLER_B, 'payment_sent');
    assert.equal(recB.recommended, false, 'Seller B should NOT be recommended');
    assert.ok(
      recB.reason.toLowerCase().includes('success rate') ||
      recB.reason.toLowerCase().includes('risk') ||
      recB.reason.toLowerCase().includes('dispute') ||
      recB.reason.toLowerCase().includes('poor'),
      `Reason should mention failure/risk/poor track record. Got: "${recB.reason}"`,
    );

    // -- Get counterparty profile for B --
    const profileB = memory.getCounterpartyProfile(BUYER, SELLER_B);
    assert.equal(profileB.riskLevel, 'high', 'Seller B should be high risk');
    assert.equal(profileB.totalInteractions, 10);
    assert.equal(profileB.successRate, 0.2, 'Success rate should be 20%');

    // -- Profile for A should be healthy --
    const profileA = memory.getCounterpartyProfile(BUYER, SELLER_A);
    assert.equal(profileA.riskLevel, 'low', 'Seller A should be low risk');
    assert.equal(profileA.totalInteractions, 10);
    assert.equal(profileA.successRate, 1, 'Success rate should be 100%');
  });
});

// ===========================================================================
// Scenario 3: Rules Engine Cascading
// ===========================================================================

describe('E2E Integrated Scenario 3: Rules Engine Cascading', () => {
  let rules;

  beforeEach(() => {
    rules = createRulesEngine();

    // Rule 1: block amount > $10000 (priority 100)
    rules.addRule({
      name: 'Mega-value blocker',
      condition: { field: 'amount', operator: 'gt', value: 10000 },
      action: { type: 'block', params: { reason: 'Amount exceeds $10,000 limit' } },
      priority: 100,
      enabled: true,
    });

    // Rule 2: require_escrow for first-time buyers (priority 80)
    rules.addRule({
      name: 'First-time buyer escrow',
      condition: { field: 'firstTimeBuyer', operator: 'eq', value: true },
      action: { type: 'require_escrow', params: { reason: 'first-time buyer' } },
      priority: 80,
      enabled: true,
    });

    // Rule 3: approve all (priority 10)
    rules.addRule({
      name: 'Default approve',
      condition: { field: 'amount', operator: 'gte', value: 0 },
      action: { type: 'approve', params: {} },
      priority: 10,
      enabled: true,
    });
  });

  it('cascades rules by priority: block > require_escrow > approve', () => {
    // Evaluate context: amount: 15000 -> blocked by rule 1
    const result1 = rules.evaluate({ amount: 15000, firstTimeBuyer: false });
    assert.equal(result1.allowed, false, '$15,000 should be blocked');
    assert.ok(
      result1.explanation.includes('Mega-value blocker'),
      'Should be blocked by mega-value rule',
    );

    // Evaluate context: amount: 500, firstTimeBuyer: true -> require_escrow (not blocked)
    const result2 = rules.evaluate({ amount: 500, firstTimeBuyer: true });
    assert.equal(result2.allowed, true, '$500 first-time buyer should be allowed (require_escrow is not block)');
    const escrowRule = result2.appliedRules.find(
      (r) => r.matched && r.action?.type === 'require_escrow',
    );
    assert.ok(escrowRule, 'Should match require_escrow rule');

    // Evaluate context: amount: 50, firstTimeBuyer: false -> approved by rule 3
    const result3 = rules.evaluate({ amount: 50, firstTimeBuyer: false });
    assert.equal(result3.allowed, true, '$50 should be approved');
    const approveRule = result3.appliedRules.find(
      (r) => r.matched && r.action?.type === 'approve',
    );
    assert.ok(approveRule, 'Should match approve rule');

    // Assert audit log has 3 entries
    const auditLog = rules.getAuditLog(10);
    assert.equal(auditLog.length, 3, 'Audit log should have 3 entries');

    // Verify ordering: newest first in audit log
    assert.equal(auditLog[0].allowed, true, 'Most recent (3rd eval) should be allowed');
    assert.equal(auditLog[2].allowed, false, 'Oldest (1st eval) should be blocked');
  });
});

// ===========================================================================
// Scenario 4: Scheduled Action + Cost Tracking
// ===========================================================================

describe('E2E Integrated Scenario 4: Scheduled Action + Cost Tracking', () => {
  let scheduler;
  let costAnalytics;
  let executionLog;

  beforeEach(() => {
    executionLog = [];
    costAnalytics = createCostAnalytics();

    scheduler = createSchedulerService({
      executor: async (action) => {
        executionLog.push(action.id);
        return { paid: true, amount: action.payload?.amount ?? 0 };
      },
    });
  });

  it('processes due actions and records costs for executed actions', async () => {
    // Schedule 3 actions: 2 due now, 1 far in the future
    const pastTime1 = new Date(Date.now() - 10).toISOString();
    const pastTime2 = new Date(Date.now() - 5).toISOString();
    const futureTime = new Date(Date.now() + 100_000).toISOString();

    const { actionId: id1 } = scheduler.scheduleAction({
      agentAddress: BUYER,
      actionType: 'payment',
      payload: { amount: 50, to: SELLER_A },
      executeAt: pastTime1,
      description: 'Pay seller A $50',
    });

    const { actionId: id2 } = scheduler.scheduleAction({
      agentAddress: BUYER,
      actionType: 'payment',
      payload: { amount: 75, to: SELLER_B },
      executeAt: pastTime2,
      description: 'Pay seller B $75',
    });

    scheduler.scheduleAction({
      agentAddress: BUYER,
      actionType: 'payment',
      payload: { amount: 200, to: SELLER_A },
      executeAt: futureTime,
      description: 'Future payment - should not execute',
    });

    // Process due actions
    const result = await scheduler.processDueActions();
    assert.equal(result.executed, 2, '2 actions should be executed');
    assert.equal(result.failed, 0, 'No actions should fail');

    // Verify the correct actions executed
    assert.ok(executionLog.includes(id1), 'Action 1 should have executed');
    assert.ok(executionLog.includes(id2), 'Action 2 should have executed');

    // Record costs for executed actions
    costAnalytics.record({
      agentAddress: BUYER,
      counterparty: SELLER_A,
      direction: 'spend',
      amount: 50,
      operation: 'quote_payment',
    });
    costAnalytics.record({
      agentAddress: BUYER,
      counterparty: SELLER_B,
      direction: 'spend',
      amount: 75,
      operation: 'quote_payment',
    });

    // Assert cost analytics has 2 records
    const summary = costAnalytics.getAgentSpendSummary(BUYER);
    assert.equal(summary.transactionCount, 2, 'Should have 2 cost records');
    assert.equal(summary.totalSpent, 125, 'Total spent should be $125');

    // Assert scheduler metrics show 2 executed
    const metrics = scheduler.getMetrics();
    assert.equal(metrics.totalExecuted, 2, 'Scheduler should report 2 executed');
    assert.equal(metrics.pendingCount, 1, 'Should still have 1 pending (future) action');
  });
});

// ===========================================================================
// Scenario 5: Fan-Out Quote Collection
// ===========================================================================

describe('E2E Integrated Scenario 5: Fan-Out Quote Collection', () => {
  let coordinator;

  beforeEach(() => {
    coordinator = createFanOutCoordinator();
  });

  afterEach(() => {
    coordinator.destroy();
  });

  it('scatters quote request to 3 agents, joins with all strategy, aggregates sorted by price', async () => {
    const AGENT_1 = '0xAgent1';
    const AGENT_2 = '0xAgent2';
    const AGENT_3 = '0xAgent3';

    // Scatter quote request to 3 agents
    const coordId = coordinator.scatter({
      agentAddress: BUYER,
      targets: [AGENT_1, AGENT_2, AGENT_3],
      taskType: 'quote',
      payload: { items: [{ sku: 'WIDGET-001', quantity: 10 }] },
      timeoutMs: 5000,
      joinStrategy: 'all',
    });

    // Verify coordination is pending
    const status1 = coordinator.getStatus(coordId);
    assert.equal(status1.status, 'pending');
    assert.equal(status1.pending.length, 3);

    // Agent 1 responds: $100
    coordinator.registerResponse(coordId, AGENT_1, { price: 100 });

    // Agent 2 responds: $80
    coordinator.registerResponse(coordId, AGENT_2, { price: 80 });

    // Agent 3 responds: $120
    coordinator.registerResponse(coordId, AGENT_3, { price: 120 });

    // Join with 'all' strategy (should complete immediately since all responded)
    const result = await coordinator.join(coordId);

    assert.equal(result.status, 'completed');
    assert.equal(result.completedCount, 3);
    assert.equal(result.timedOutCount, 0);

    // Assert aggregation sorted by price
    assert.equal(result.aggregation.type, 'ranked_quotes');
    assert.equal(result.aggregation.data.length, 3);
    assert.equal(result.aggregation.bestPrice, 80, 'Best price should be $80');
    assert.equal(result.aggregation.bestResponder, AGENT_2, 'Best responder should be Agent 2');

    // Verify sort order: $80, $100, $120
    const prices = result.aggregation.data.map((r) => r.response.price);
    assert.deepEqual(prices, [80, 100, 120], 'Prices should be sorted ascending');
  });
});

// ===========================================================================
// Scenario 6: Message-Driven Task Delegation
// ===========================================================================

describe('E2E Integrated Scenario 6: Message-Driven Task Delegation', () => {
  let messaging;

  beforeEach(() => {
    messaging = createMessagingService();
  });

  it('Agent A delegates task to Agent B, B accepts and completes, thread is traceable', () => {
    const AGENT_A = '0xAgentA';
    const AGENT_B = '0xAgentB';

    // Agent A sends task to Agent B: "process invoice #123, reward $10"
    const taskMsg = messaging.delegateTask({
      from: AGENT_A,
      to: AGENT_B,
      description: 'Process invoice #123',
      deadline: new Date(Date.now() + 3600_000).toISOString(),
      reward: 10,
      priority: 'high',
    });

    assert.equal(taskMsg.type, 'task_delegation');
    assert.equal(taskMsg.from, AGENT_A);
    assert.equal(taskMsg.to, AGENT_B);
    assert.equal(taskMsg.payload.reward, 10);

    // B gets inbox -> sees task delegation
    const inbox = messaging.getInbox(AGENT_B, { unreadOnly: true });
    assert.equal(inbox.length, 1, 'B should have 1 unread message');
    assert.equal(inbox[0].type, 'task_delegation');
    assert.equal(inbox[0].payload.description, 'Process invoice #123');

    // B accepts task
    const acceptResponse = messaging.respondToTask(taskMsg.id, { status: 'accepted' });
    assert.equal(acceptResponse.type, 'status_response');
    assert.equal(acceptResponse.payload.status, 'accepted');
    assert.equal(acceptResponse.from, AGENT_B);
    assert.equal(acceptResponse.to, AGENT_A);

    // Verify the original task message was updated
    const updatedTask = messaging.getMessage(taskMsg.id);
    assert.equal(updatedTask.taskStatus, 'accepted');

    // B completes task with result
    const completeResponse = messaging.respondToTask(taskMsg.id, {
      status: 'completed',
      result: { invoiceId: 'INV-123', processedAt: new Date().toISOString() },
    });
    assert.equal(completeResponse.payload.status, 'completed');
    assert.ok(completeResponse.payload.result.invoiceId, 'Should include invoice ID');

    // A checks thread -> sees accept + complete messages
    const thread = messaging.getThread(taskMsg.id);
    assert.ok(thread.length >= 3, 'Thread should have at least 3 messages (original + accept + complete)');

    // Verify thread contains the delegation, accept response, and complete response
    const types = thread.map((m) => m.type);
    assert.ok(types.includes('task_delegation'), 'Thread should contain task_delegation');
    assert.ok(
      types.filter((t) => t === 'status_response').length >= 2,
      'Thread should contain at least 2 status_response messages',
    );

    // Assert messaging metrics: 3+ messages
    const metrics = messaging.getMetrics();
    assert.ok(metrics.totalMessages >= 3, `Should have at least 3 messages, got ${metrics.totalMessages}`);
  });
});

// ===========================================================================
// Scenario 7: Tracing Across Multiple Operations
// ===========================================================================

describe('E2E Integrated Scenario 7: Tracing Across Multiple Operations', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService({ serviceName: 'purchase-flow' });
  });

  it('builds parent-child span hierarchy with shared traceId and computes metrics', () => {
    // Start parent span "purchase_flow"
    const parentSpan = tracing.startSpan('purchase_flow', { kind: 'server' });
    const traceId = parentSpan.traceId;

    // Create child span "request_quote"
    const quoteSpan = tracing.startSpan('request_quote', {
      kind: 'client',
      traceId,
      parentSpanId: parentSpan.spanId,
      attributes: { seller: SELLER_A },
    });
    quoteSpan.addEvent('quote_received', { price: 100 });
    quoteSpan.end();

    // Create child span "evaluate_quote"
    const evalSpan = tracing.startSpan('evaluate_quote', {
      kind: 'internal',
      traceId,
      parentSpanId: parentSpan.spanId,
      attributes: { quotePrice: 100, budget: 150 },
    });
    evalSpan.addEvent('evaluation_complete', { decision: 'accept' });
    evalSpan.end();

    // Create child span "execute_payment"
    const paySpan = tracing.startSpan('execute_payment', {
      kind: 'client',
      traceId,
      parentSpanId: parentSpan.spanId,
      attributes: { amount: 100, asset: 'USDC' },
    });
    paySpan.addEvent('payment_submitted');
    paySpan.addEvent('payment_confirmed', { txHash: '0xabc123' });
    paySpan.end();

    // End parent span
    parentSpan.end();

    // Get trace -> all 4 spans share same traceId
    const trace = tracing.getTrace(traceId);
    assert.equal(trace.length, 4, 'Trace should have exactly 4 spans');

    // All spans share the same traceId
    for (const span of trace) {
      assert.equal(span.traceId, traceId, `All spans should share traceId ${traceId}`);
    }

    // Verify span names
    const spanNames = trace.map((s) => s.name).sort();
    assert.deepEqual(spanNames, [
      'evaluate_quote',
      'execute_payment',
      'purchase_flow',
      'request_quote',
    ]);

    // Verify parent-child hierarchy: 3 child spans have parentSpanId = parent.spanId
    const childSpans = trace.filter((s) => s.parentSpanId === parentSpan.spanId);
    assert.equal(childSpans.length, 3, '3 child spans should reference the parent');

    // The parent span has no parentSpanId
    const rootSpan = trace.find((s) => s.name === 'purchase_flow');
    assert.equal(rootSpan.parentSpanId, null, 'Root span should have null parentSpanId');

    // Get metrics -> p50/p95 computed
    const metrics = tracing.getMetrics();
    assert.equal(metrics.spanCount, 4, 'Metrics should cover 4 completed spans');
    assert.ok(metrics.p50 >= 0, 'p50 should be non-negative');
    assert.ok(metrics.p95 >= metrics.p50, 'p95 should be >= p50');
    assert.equal(metrics.errorRate, 0, 'No errors expected');

    // Verify OTLP export includes all spans
    const otlp = tracing.exportOTLP();
    assert.ok(otlp.resourceSpans, 'OTLP export should have resourceSpans');
    const exportedSpans = otlp.resourceSpans[0].scopeSpans[0].spans;
    assert.equal(exportedSpans.length, 4, 'OTLP export should contain 4 spans');
  });
});

// ===========================================================================
// Scenario 8: Rate Limiter + Idempotency Together
// ===========================================================================

describe('E2E Integrated Scenario 8: Rate Limiter + Idempotency Together', () => {
  let rateLimiter;
  let idempotency;
  let executionCount;

  beforeEach(() => {
    rateLimiter = createMcpRateLimiter({
      defaultLimits: { requestsPerMinute: 60 },
      toolOverrides: {
        a2a_pay: { requestsPerMinute: 3 },
      },
    });
    idempotency = createIdempotencyGuard({ ttlMs: 60_000 });
    executionCount = 0;
  });

  afterEach(() => {
    rateLimiter.destroy();
  });

  it('rate limits after 3 calls, idempotency cache bypasses rate limiter on retries', async () => {
    const agentId = 'agent-buyer-1';
    const toolName = 'a2a_pay';

    /**
     * Simulate a payment flow:
     * 1. Check idempotency cache first (if hit, return cached, skip rate limit)
     * 2. Check rate limit
     * 3. Execute payment
     */
    async function executePayment(idempotencyKey, amount) {
      // Step 1: check idempotency guard for existing result
      if (idempotency.has(idempotencyKey)) {
        // Return cached result directly, no rate limit consumed
        return idempotency.execute(idempotencyKey, async () => {
          throw new Error('Should not reach here');
        });
      }

      // Step 2: check rate limit (only for non-cached requests)
      const rateCheck = rateLimiter.checkLimit(agentId, toolName);
      if (!rateCheck.allowed) {
        return { error: 'rate_limited', retryAfterMs: rateCheck.retryAfterMs };
      }

      // Step 3: execute with idempotency protection
      return idempotency.execute(idempotencyKey, async () => {
        executionCount++;
        return { txId: `tx-${amount}`, amount, status: 'settled' };
      });
    }

    // Make 3 payments -> all pass rate limit
    const result1 = await executePayment('pay-1', 50);
    assert.equal(result1.txId, 'tx-50');
    assert.equal(result1.status, 'settled');

    const result2 = await executePayment('pay-2', 75);
    assert.equal(result2.txId, 'tx-75');

    const result3 = await executePayment('pay-3', 100);
    assert.equal(result3.txId, 'tx-100');

    assert.equal(executionCount, 3, '3 unique payments should have executed');

    // Make 4th payment -> rate limited
    const result4 = await executePayment('pay-4', 200);
    assert.equal(result4.error, 'rate_limited', '4th payment should be rate limited');
    assert.ok(result4.retryAfterMs >= 0, 'Should include retryAfterMs');

    // Retry 1st payment with same idempotency key -> returns cached
    // (doesn't count toward rate limit because idempotency guard returns before rate check)
    const retryResult = await executePayment('pay-1', 50);
    assert.equal(retryResult.txId, 'tx-50', 'Retry should return cached result');
    assert.equal(retryResult.status, 'settled');
    assert.equal(executionCount, 3, 'Executor should NOT have been called again');

    // Assert rate limiter metrics
    const rlMetrics = rateLimiter.getMetrics();
    assert.ok(rlMetrics.activeBuckets >= 1, 'Should have at least 1 active bucket');
    assert.ok(
      rlMetrics.topAgents.some((a) => a.agentId === agentId),
      'Agent should appear in top agents',
    );

    // The rate limiter checkLimit was called 4 times (payments 1-4), not 5 (retry bypassed)
    // We verify that the total requests for our agent match:
    // 3 allowed + 1 blocked = 4 total requests tracked by the rate limiter
    const agentMetric = rlMetrics.topAgents.find((a) => a.agentId === agentId);
    // The limiter increments count only on allowed requests (3 increments),
    // but the 4th check sees count >= 3 so doesn't increment.
    // So totalRequests = 3 (the 3 successful increments in the bucket).
    assert.equal(agentMetric.totalRequests, 3, 'Rate limiter should show 3 counted requests');

    // Idempotency metrics: 3 misses (pay-1,2,3), 1 hit (retry of pay-1)
    const idempMetrics = idempotency.getMetrics();
    assert.equal(idempMetrics.misses, 3, 'Should have 3 idempotency misses');
    assert.equal(idempMetrics.hits, 1, 'Should have 1 idempotency hit (the retry)');
  });
});
