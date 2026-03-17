/**
 * End-to-End Scenario Tests for A2A Agentic Commerce
 *
 * Integration tests that exercise full agent-to-agent commerce flows
 * using in-memory mocks. Proves the system works end-to-end.
 *
 * Uses Node.js built-in test runner (NOT vitest).
 *
 * Scenarios:
 *   1. Full Purchase Cycle (discover → quote → accept → escrow → fulfill → release → rate)
 *   2. Failed Payment with Saga Rollback
 *   3. Subscription Lifecycle
 *   4. Dispute Resolution with Auto-Arbitration
 *   5. Multi-Agent RFQ with Auto-Award
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';

import { createSagaOrchestrator } from '../../src/a2a/saga.js';
import { createA2ASubscriptionService } from '../../src/a2a/subscriptions.js';
import { createDisputeResolver } from '../../src/a2a/dispute-resolver.js';
import { createMarketplaceService } from '../../src/a2a/marketplace.js';

// ===========================================================================
// Shared helpers
// ===========================================================================

function hoursAgo(hours) {
  return new Date(Date.now() - hours * 60 * 60 * 1000).toISOString();
}

function daysAgo(days) {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
}

// ===========================================================================
// Scenario 1: Full Purchase Cycle
// ===========================================================================

describe('E2E Scenario 1: Full Purchase Cycle', () => {
  const BUYER = '0xBuyer';
  const SELLER = '0xSeller';

  it('buyer discovers seller, requests quote, seller provides, buyer accepts, escrow created/funded/released, seller rated', async () => {
    // -- In-memory data stores --
    const quotes = new Map();
    const escrows = new Map();
    const payments = new Map();
    const reputations = new Map();
    const feedback = [];
    const stateLog = [];

    // -- Mock store --
    const store = {
      // Quote methods
      createQuote: async (q) => { quotes.set(q.id, { ...q }); return q; },
      getQuote: async (id) => quotes.get(id) || null,
      updateQuote: async (id, updates) => {
        const q = quotes.get(id);
        if (!q) throw new Error('Quote not found');
        Object.assign(q, updates);
        return q;
      },
      listQuotes: async () => [...quotes.values()],

      // Escrow methods
      createEscrow: async (e) => {
        const escrow = { ...e, id: e.id || randomUUID(), status: 'created', created_at: new Date().toISOString() };
        escrows.set(escrow.id, escrow);
        return escrow;
      },
      getEscrow: async (id) => escrows.get(id) || null,
      updateEscrow: async (id, updates) => {
        const e = escrows.get(id);
        if (!e) throw new Error('Escrow not found');
        Object.assign(e, updates);
        return e;
      },

      // Payment methods
      createPayment: async (p) => { payments.set(p.id, { ...p }); return p; },
      updatePayment: async (id, updates) => {
        const p = payments.get(id);
        if (p) Object.assign(p, updates);
        return p;
      },

      // Reputation
      getReputationScore: (addr) => reputations.get(addr) || null,
      updateReputationScore: (addr, score) => { reputations.set(addr, score); },
      createFeedback: async (f) => { feedback.push(f); return f; },
    };

    // -- Mock a2aService --
    const a2aService = {
      async requestQuote({ seller, items, asset }) {
        stateLog.push('quote_requested');
        const quoteId = randomUUID();
        const quote = {
          id: quoteId,
          status: 'requested',
          buyer_address: BUYER,
          seller_address: seller,
          items,
          total: 0,
          total_decimal: 0,
          asset: asset || 'USDC',
          created_at: new Date().toISOString(),
        };
        await store.createQuote(quote);
        return { success: true, quote: { id: quoteId, status: 'requested', buyer: BUYER, seller, total: 0 } };
      },

      async provideQuote(quoteId, { total }) {
        stateLog.push('quote_provided');
        await store.updateQuote(quoteId, {
          status: 'quoted',
          total: Math.round(total * 1_000_000),
          total_decimal: total,
          quoted_at: new Date().toISOString(),
        });
        const q = await store.getQuote(quoteId);
        return { success: true, quote: { id: quoteId, status: 'quoted', total } };
      },

      async acceptQuote(quoteId) {
        stateLog.push('quote_accepted');
        const q = await store.getQuote(quoteId);
        await store.updateQuote(quoteId, {
          status: 'accepted',
          accepted_at: new Date().toISOString(),
        });
        return { success: true, payment: { id: randomUUID() }, quote: { id: quoteId, status: 'accepted' } };
      },

      async declineQuote(quoteId, reason) {
        stateLog.push('quote_declined');
        await store.updateQuote(quoteId, { status: 'declined' });
        return { success: true };
      },

      async createConditionalPayment({ sellerAddress, amount, quoteId }) {
        stateLog.push('escrow_created');
        const escrow = await store.createEscrow({
          buyer_address: BUYER,
          seller_address: sellerAddress,
          amount: Math.round(amount * 1_000_000),
          amount_decimal: amount,
          asset: 'USDC',
          release_conditions: [{ type: 'seller_fulfilled', quoteId }],
        });
        // Fund it
        await store.updateEscrow(escrow.id, { status: 'funded', funded_at: new Date().toISOString() });
        stateLog.push('escrow_funded');
        return { success: true, escrow: { id: escrow.id, status: 'funded' } };
      },

      async checkPaymentConditions(escrowId) {
        const escrow = await store.getEscrow(escrowId);
        const conditions = escrow.release_conditions || [];
        let allMet = true;
        const evaluated = [];
        for (const c of conditions) {
          let met = false;
          if (c.type === 'seller_fulfilled' && c.quoteId) {
            const q = await store.getQuote(c.quoteId);
            met = q?.status === 'fulfilled';
          }
          evaluated.push({ ...c, met });
          if (!met) allMet = false;
        }
        return { escrowId, allMet, conditions: evaluated };
      },

      async settleConditionalPayment(escrowId) {
        stateLog.push('escrow_released');
        await store.updateEscrow(escrowId, { status: 'released', released_at: new Date().toISOString() });
        return { success: true, escrowId, status: 'released' };
      },

      async fulfillQuote(quoteId) {
        stateLog.push('quote_fulfilled');
        await store.updateQuote(quoteId, {
          status: 'fulfilled',
          fulfilled_at: new Date().toISOString(),
        });
        return { success: true, quote: { id: quoteId, status: 'fulfilled' } };
      },

      async pay({ to, amount }) {
        stateLog.push('payment_made');
        const paymentId = randomUUID();
        await store.createPayment({
          id: paymentId,
          status: 'completed',
          sender_address: BUYER,
          recipient_address: to,
          amount: Math.round(amount * 1_000_000),
          amount_decimal: amount,
          asset: 'USDC',
        });
        return { success: true, payment: { id: paymentId } };
      },
    };

    // -- Mock reputation service --
    const reputationService = {
      async rateAgent({ agentAddress, score, comment }) {
        stateLog.push('agent_rated');
        const existing = reputations.get(agentAddress) || {
          agent_address: agentAddress,
          average_score: 0,
          total_transactions: 0,
          trust_tier: 'sandbox',
        };
        existing.total_transactions += 1;
        existing.average_score =
          (existing.average_score * (existing.total_transactions - 1) + score) /
          existing.total_transactions;
        reputations.set(agentAddress, existing);
        return { success: true, score };
      },
    };

    // ============= Execute full purchase flow =============

    // Step 1: Buyer discovers seller (simulated — in real system this uses agent cards)
    const discoveredSeller = SELLER;

    // Step 2: Buyer requests quote
    const quoteRequest = await a2aService.requestQuote({
      seller: discoveredSeller,
      items: [{ description: 'AI Data Analysis', quantity: 1 }],
      asset: 'USDC',
    });
    assert.ok(quoteRequest.success);
    const quoteId = quoteRequest.quote.id;
    const quoteAfterRequest = await store.getQuote(quoteId);
    assert.equal(quoteAfterRequest.status, 'requested');

    // Step 3: Seller provides quote
    const quoteProvided = await a2aService.provideQuote(quoteId, { total: 100 });
    assert.ok(quoteProvided.success);
    const quoteAfterProvide = await store.getQuote(quoteId);
    assert.equal(quoteAfterProvide.status, 'quoted');
    assert.equal(quoteAfterProvide.total_decimal, 100);

    // Step 4: Buyer accepts quote
    const acceptance = await a2aService.acceptQuote(quoteId);
    assert.ok(acceptance.success);
    const quoteAfterAccept = await store.getQuote(quoteId);
    assert.equal(quoteAfterAccept.status, 'accepted');

    // Step 5: Escrow created and funded
    const escrowResult = await a2aService.createConditionalPayment({
      sellerAddress: SELLER,
      amount: 100,
      quoteId,
    });
    assert.ok(escrowResult.success);
    const escrowId = escrowResult.escrow.id;
    const escrowAfterFund = await store.getEscrow(escrowId);
    assert.equal(escrowAfterFund.status, 'funded');

    // Step 6: Seller fulfills
    await a2aService.fulfillQuote(quoteId);
    const quoteAfterFulfill = await store.getQuote(quoteId);
    assert.equal(quoteAfterFulfill.status, 'fulfilled');

    // Step 7: Check conditions (should be met now)
    const conditionCheck = await a2aService.checkPaymentConditions(escrowId);
    assert.ok(conditionCheck.allMet);

    // Step 8: Release escrow
    const settlement = await a2aService.settleConditionalPayment(escrowId);
    assert.ok(settlement.success);
    const escrowAfterRelease = await store.getEscrow(escrowId);
    assert.equal(escrowAfterRelease.status, 'released');

    // Step 9: Buyer rates seller
    const rating = await reputationService.rateAgent({
      agentAddress: SELLER,
      score: 5,
      comment: 'Excellent analysis',
    });
    assert.ok(rating.success);
    const sellerRep = reputations.get(SELLER);
    assert.equal(sellerRep.average_score, 5);
    assert.equal(sellerRep.total_transactions, 1);

    // Verify full state transition log
    assert.deepEqual(stateLog, [
      'quote_requested',
      'quote_provided',
      'quote_accepted',
      'escrow_created',
      'escrow_funded',
      'quote_fulfilled',
      'escrow_released',
      'agent_rated',
    ]);

    // Verify escrow lifecycle: created → funded → released
    assert.equal(escrowAfterRelease.status, 'released');
    assert.ok(escrowAfterRelease.funded_at);
    assert.ok(escrowAfterRelease.released_at);
  });
});

// ===========================================================================
// Scenario 2: Failed Payment with Saga Rollback
// ===========================================================================

describe('E2E Scenario 2: Failed Payment with Saga Rollback', () => {
  const BUYER = '0xSagaBuyer';
  const SELLER = '0xSagaSeller';

  it('payment fails mid-saga, compensation runs in reverse, escrow refunded, quote declined', async () => {
    // -- Track compensation order --
    const compensationLog = [];
    const quotes = new Map();
    const escrows = new Map();

    // -- Build saga with actual step functions --
    const sagaDefinition = {
      name: 'purchase_with_failure',
      steps: [
        {
          name: 'request_quote',
          execute: async (ctx) => {
            const quoteId = randomUUID();
            quotes.set(quoteId, { id: quoteId, status: 'requested', total_decimal: 100 });
            return { quote: { id: quoteId, total: 100 } };
          },
          compensate: async (ctx, result) => {
            compensationLog.push('decline_quote');
            if (result?.quote?.id) {
              const q = quotes.get(result.quote.id);
              if (q) q.status = 'declined';
            }
          },
          timeoutMs: 5000,
          retries: 0,
        },
        {
          name: 'accept_quote',
          execute: async (ctx) => {
            const quoteResult = ctx.request_quote;
            const q = quotes.get(quoteResult.quote.id);
            q.status = 'accepted';
            return { quote: { id: q.id, status: 'accepted' } };
          },
          compensate: async (ctx, result) => {
            compensationLog.push('revert_acceptance');
            // Acceptance revert is handled by quote decline above
          },
          timeoutMs: 5000,
          retries: 0,
        },
        {
          name: 'create_escrow',
          execute: async (ctx) => {
            const escrowId = randomUUID();
            escrows.set(escrowId, { id: escrowId, status: 'funded', amount_decimal: 100 });
            return { escrow: { id: escrowId, status: 'funded' } };
          },
          compensate: async (ctx, result) => {
            compensationLog.push('refund_escrow');
            if (result?.escrow?.id) {
              const e = escrows.get(result.escrow.id);
              if (e) e.status = 'refunded';
            }
          },
          timeoutMs: 5000,
          retries: 0,
        },
        {
          name: 'execute_payment',
          execute: async () => {
            // This step FAILS — simulating a payment failure
            throw new Error('Payment rejected by network');
          },
          compensate: async () => {
            compensationLog.push('revert_payment');
          },
          timeoutMs: 5000,
          retries: 0,
        },
      ],
    };

    // -- Execute saga --
    const orchestrator = createSagaOrchestrator(null, {});
    const result = await orchestrator.execute(sagaDefinition, {});

    // -- Assertions --
    // Saga should be compensated
    assert.equal(result.status, 'compensated');

    // The failed step
    const failedStep = result.steps.find((s) => s.name === 'execute_payment');
    assert.equal(failedStep.status, 'failed');
    assert.ok(failedStep.error.includes('Payment rejected'));

    // Compensation ran in reverse order: create_escrow → accept_quote → request_quote
    assert.deepEqual(compensationLog, [
      'refund_escrow',
      'revert_acceptance',
      'decline_quote',
    ]);

    // Escrow should be refunded
    const escrowId = result.steps.find((s) => s.name === 'create_escrow').result.escrow.id;
    const escrow = escrows.get(escrowId);
    assert.equal(escrow.status, 'refunded');

    // Quote should be declined
    const quoteId = result.steps.find((s) => s.name === 'request_quote').result.quote.id;
    const quote = quotes.get(quoteId);
    assert.equal(quote.status, 'declined');
  });
});

// ===========================================================================
// Scenario 3: Subscription Lifecycle
// ===========================================================================

describe('E2E Scenario 3: Subscription Lifecycle', () => {
  it('subscribe → first billing → renewal → pause → resume → billing → cancel at period end → final billing → cancelled', async () => {
    // -- In-memory subscription store --
    const subscriptions = new Map();

    const mockStore = {
      createSubscription: async (sub) => {
        subscriptions.set(sub.id, { ...sub });
        return sub;
      },
      getSubscription: async (id) => subscriptions.get(id) || null,
      updateSubscription: async (id, updates) => {
        const sub = subscriptions.get(id);
        if (!sub) throw new Error('Subscription not found');
        Object.assign(sub, updates, { updated_at: new Date().toISOString() });
        return sub;
      },
      listSubscriptions: async (filter) => {
        let results = [...subscriptions.values()];
        if (filter?.status) results = results.filter((s) => s.status === filter.status);
        if (filter?.subscriber_address) {
          results = results.filter((s) => s.subscriber_address === filter.subscriber_address);
        }
        return results;
      },
      getDueSubscriptions: async (nowIso) => {
        return [...subscriptions.values()].filter(
          (s) => s.status === 'active' && s.next_billing_date && s.next_billing_date <= nowIso,
        );
      },
      getExpiredTrials: async (nowIso) => {
        return [...subscriptions.values()].filter(
          (s) => s.status === 'trial' && s.trial_end_date && s.trial_end_date <= nowIso,
        );
      },
    };

    const subService = createA2ASubscriptionService(mockStore);

    // Step 1: Create subscription (no trial for this test)
    const createResult = await subService.createSubscription({
      subscriberAddress: '0xSubscriber',
      providerAddress: '0xProvider',
      planName: 'Pro Plan',
      amount: 49.99,
      billingInterval: 'monthly',
    });
    assert.ok(createResult.success);
    const subId = createResult.subscription.id;
    assert.equal(createResult.subscription.status, 'active');

    // Step 2: First billing succeeds
    // Manually set next_billing_date to past to trigger billing
    const sub = subscriptions.get(subId);
    sub.next_billing_date = hoursAgo(1);
    sub.current_period_end = hoursAgo(1);

    const billing1 = await subService.processBilling();
    assert.equal(billing1.succeeded, 1);
    const afterBilling1 = subscriptions.get(subId);
    assert.equal(afterBilling1.billing_count, 1);
    assert.equal(afterBilling1.status, 'active');

    // Step 3: Renewal billing succeeds
    afterBilling1.next_billing_date = hoursAgo(1);
    afterBilling1.current_period_end = hoursAgo(1);

    const billing2 = await subService.processBilling();
    assert.equal(billing2.succeeded, 1);
    const afterBilling2 = subscriptions.get(subId);
    assert.equal(afterBilling2.billing_count, 2);

    // Step 4: Pause subscription
    const pauseResult = await subService.pauseSubscription(subId);
    assert.ok(pauseResult.success);
    assert.equal(pauseResult.subscription.status, 'paused');

    // Verify billing does not process paused subscriptions
    afterBilling2.next_billing_date = hoursAgo(1);
    const billingWhilePaused = await subService.processBilling();
    assert.equal(billingWhilePaused.succeeded, 0);

    // Step 5: Resume subscription
    const resumeResult = await subService.resumeSubscription(subId);
    assert.ok(resumeResult.success);
    assert.equal(resumeResult.subscription.status, 'active');

    // Step 6: Billing succeeds after resume
    const afterResume = subscriptions.get(subId);
    afterResume.next_billing_date = hoursAgo(1);
    afterResume.current_period_end = hoursAgo(1);

    const billing3 = await subService.processBilling();
    assert.equal(billing3.succeeded, 1);
    const afterBilling3 = subscriptions.get(subId);
    assert.equal(afterBilling3.billing_count, 3);

    // Step 7: Cancel at period end
    const cancelResult = await subService.cancelSubscription(subId, { immediate: false });
    assert.ok(cancelResult.success);
    const afterCancel = subscriptions.get(subId);
    assert.equal(afterCancel.cancel_at_period_end, true);
    assert.equal(afterCancel.status, 'active'); // Still active until period ends

    // Step 8: Final billing triggers cancellation
    afterCancel.next_billing_date = hoursAgo(1);
    afterCancel.current_period_end = hoursAgo(1);

    const finalBilling = await subService.processBilling();
    assert.equal(finalBilling.cancelled, 1);

    const finalSub = subscriptions.get(subId);
    assert.equal(finalSub.status, 'cancelled');
    assert.ok(finalSub.cancelled_at);

    // Verify the full status transition sequence happened
    assert.equal(finalSub.billing_count, 3); // 3 successful billings before cancellation
  });
});

// ===========================================================================
// Scenario 4: Dispute Resolution with Auto-Arbitration
// ===========================================================================

describe('E2E Scenario 4: Dispute Resolution with Auto-Arbitration', () => {
  it('buyer creates escrow, seller fails to deliver, buyer files non_delivery dispute, auto-resolves to full_refund', async () => {
    // -- Data stores --
    const disputes = new Map();
    const evidence = [];
    const escrows = new Map();
    const reputations = new Map();

    // -- Create escrow --
    const escrowId = randomUUID();
    escrows.set(escrowId, {
      id: escrowId,
      status: 'funded',
      buyer_address: '0xBuyer',
      seller_address: '0xSeller',
      amount: 100_000_000,
      amount_decimal: 100,
      asset: 'USDC',
      created_at: new Date().toISOString(),
    });

    // -- File dispute with timestamps set up for step-by-step transitions --
    // created_at is > 24h ago so tick 1 transitions filed → evidence_period,
    // but evidence_deadline is in the future so it won't also transition to
    // under_review in the same tick.
    const disputeId = randomUUID();
    disputes.set(disputeId, {
      id: disputeId,
      escrow_id: escrowId,
      status: 'filed',
      filed_by: '0xBuyer',
      filed_against: '0xSeller',
      reason: 'Never received the service',
      category: 'non_delivery',
      amount: 100_000_000,
      amount_decimal: 100,
      asset: 'USDC',
      evidence_deadline: new Date(Date.now() + 60_000).toISOString(), // Future — not yet expired
      review_deadline: new Date(Date.now() + 120_000).toISOString(),  // Future — not yet expired
      created_at: daysAgo(10),         // Filed 10 days ago (> 24h for auto-evidence-period)
      updated_at: daysAgo(10),
    });

    // -- Mock store for dispute-resolver --
    const mockStore = {
      listDisputes: async (filter) => {
        let results = [...disputes.values()];
        if (filter?.status) results = results.filter((d) => d.status === filter.status);
        return results;
      },
      getDispute: async (id) => disputes.get(id) || null,
      updateDispute: async (id, updates) => {
        const d = disputes.get(id);
        if (d) Object.assign(d, updates);
        return d;
      },
      listEvidenceByDispute: async (dId) => evidence.filter((e) => e.dispute_id === dId),
      getReputationScore: (addr) => reputations.get(addr) || null,
    };

    // -- Mock dispute service --
    const disputeService = {
      moveToEvidencePeriod: async (id) => {
        const d = disputes.get(id);
        if (d) d.status = 'evidence_period';
      },
      moveToReview: async (id) => {
        const d = disputes.get(id);
        if (d) d.status = 'under_review';
      },
      resolveDispute: async (id, params) => {
        const d = disputes.get(id);
        if (d) {
          d.status = 'resolved';
          d.resolution_type = params.resolutionType;
          d.resolution_note = params.note;
          d.resolved_by = params.resolvedBy;
        }
        return {
          success: true,
          escrowAction: {
            action: params.resolutionType === 'full_refund' ? 'refund' : 'hold',
            escrowId: d.escrow_id,
          },
        };
      },
      escalateDispute: async (id) => {
        const d = disputes.get(id);
        if (d) d.status = 'escalated';
      },
    };

    // -- Mock escrow service --
    const escrowService = {
      refundEscrow: async (id) => {
        const e = escrows.get(id);
        if (e) e.status = 'refunded';
      },
      releaseEscrow: async (id) => {
        const e = escrows.get(id);
        if (e) e.status = 'released';
      },
    };

    const resolver = createDisputeResolver(
      mockStore,
      disputeService,
      escrowService,
      null, // no notification service
      { autoResolveThreshold: 1000 },
    );

    // -- Tick 1: filed → evidence_period (24h has passed since filing) --
    const tick1 = await resolver.tick();
    assert.equal(tick1.transitions, 1);
    const disputeAfterTick1 = disputes.get(disputeId);
    assert.equal(disputeAfterTick1.status, 'evidence_period');

    // -- Now set evidence_deadline to the past so tick 2 transitions it --
    disputeAfterTick1.evidence_deadline = hoursAgo(1);

    // -- Tick 2: evidence_period → under_review (evidence deadline passed) --
    const tick2 = await resolver.tick();
    assert.equal(tick2.transitions, 1);
    const disputeAfterTick2 = disputes.get(disputeId);
    assert.equal(disputeAfterTick2.status, 'under_review');

    // -- Set review_deadline to the past so auto-resolution triggers --
    disputeAfterTick2.review_deadline = hoursAgo(1);

    // -- Tick 3: under_review → resolved (auto-arbitrate: non_delivery, no seller proof → full_refund) --
    const tick3 = await resolver.tick();
    assert.equal(tick3.resolutions, 1);
    const disputeAfterTick3 = disputes.get(disputeId);
    assert.equal(disputeAfterTick3.status, 'resolved');
    assert.equal(disputeAfterTick3.resolution_type, 'full_refund');
    assert.ok(disputeAfterTick3.resolution_note.includes('Non-delivery'));
    assert.equal(disputeAfterTick3.resolved_by, 'auto-resolver');

    // -- Verify escrow was refunded --
    const escrowAfter = escrows.get(escrowId);
    assert.equal(escrowAfter.status, 'refunded');

    // -- Verify metrics --
    const metrics = resolver.getMetrics();
    assert.equal(metrics.totalTicks, 3);
    assert.equal(metrics.autoTransitions, 2);
    assert.equal(metrics.autoResolutions, 1);
  });
});

// ===========================================================================
// Scenario 5: Multi-Agent RFQ with Auto-Award
// ===========================================================================

describe('E2E Scenario 5: Multi-Agent RFQ with Auto-Award', () => {
  it('buyer broadcasts RFQ, 3 sellers respond, deadline passes, auto-awards to cheapest, losers declined', async () => {
    // -- Data stores --
    const services = new Map();
    const rfqs = new Map();
    const rfqResponses = new Map();
    const quotes = new Map();
    const reputations = new Map();

    const BUYER = '0xRFQBuyer';
    const SELLER_1 = '0xSeller1';
    const SELLER_2 = '0xSeller2';
    const SELLER_3 = '0xSeller3';

    // Register 3 seller services
    const svcIds = [];
    for (const [addr, name, price] of [
      [SELLER_1, 'Seller A', 150],
      [SELLER_2, 'Seller B', 80],  // Cheapest
      [SELLER_3, 'Seller C', 120],
    ]) {
      const svcId = randomUUID();
      svcIds.push(svcId);
      services.set(svcId, {
        id: svcId,
        name,
        agent_address: addr,
        category: 'data',
        active: 1,
      });
    }

    // -- Mock store --
    const mockStore = {
      listServices: (filter) => {
        let results = [...services.values()];
        if (filter?.active !== undefined) results = results.filter((s) => s.active === filter.active);
        if (filter?.category) results = results.filter((s) => s.category === filter.category);
        if (filter?.agent_address) results = results.filter((s) => s.agent_address === filter.agent_address);
        return results;
      },
      getService: (id) => services.get(id) || null,

      createRFQ: (rfq) => {
        const id = randomUUID();
        const record = { ...rfq, id, status: 'open', created_at: new Date().toISOString() };
        rfqs.set(id, record);
        return record;
      },
      getRFQ: (id) => rfqs.get(id) || null,
      updateRFQ: (id, updates) => {
        const r = rfqs.get(id);
        if (r) Object.assign(r, updates);
        return r;
      },
      listRFQs: (filter) => {
        let results = [...rfqs.values()];
        if (filter?.status) results = results.filter((r) => r.status === filter.status);
        return results;
      },

      createRFQResponse: (resp) => {
        const id = randomUUID();
        const record = { ...resp, id, created_at: new Date().toISOString(), score: null, rank: null };
        rfqResponses.set(id, record);
        return record;
      },
      listRFQResponses: (filter) => {
        let results = [...rfqResponses.values()];
        if (filter?.rfq_id) results = results.filter((r) => r.rfq_id === filter.rfq_id);
        if (filter?.seller_address) results = results.filter((r) => r.seller_address === filter.seller_address);
        if (filter?.status) results = results.filter((r) => r.status === filter.status);
        return results;
      },
      updateRFQResponse: (id, updates) => {
        const r = rfqResponses.get(id);
        if (r) Object.assign(r, updates);
        return r;
      },

      // Quote support for scoring
      getQuote: (id) => quotes.get(id) || null,
      listQuotes: (filter) => {
        let results = [...quotes.values()];
        if (filter?.seller_address) results = results.filter((q) => q.seller_address === filter.seller_address);
        return results;
      },

      getReputationScore: (addr) => reputations.get(addr) || null,
      listDisputes: (filter) => [],
    };

    // Track quote operations
    const quoteActions = { accepted: [], declined: [] };

    // -- Mock a2aService --
    const a2aService = {
      async requestQuote({ seller, items, message }) {
        const quoteId = randomUUID();
        const quote = {
          id: quoteId,
          status: 'requested',
          buyer_address: BUYER,
          seller_address: seller,
          items,
          total: 0,
          total_decimal: 0,
          asset: 'USDC',
          created_at: new Date().toISOString(),
          quoted_at: null,
        };
        quotes.set(quoteId, quote);
        return { success: true, quote: { id: quoteId } };
      },
      async acceptQuote(quoteId) {
        const q = quotes.get(quoteId);
        if (q) q.status = 'accepted';
        quoteActions.accepted.push(quoteId);
        return { success: true };
      },
      async declineQuote(quoteId, reason) {
        const q = quotes.get(quoteId);
        if (q) q.status = 'declined';
        quoteActions.declined.push(quoteId);
        return { success: true };
      },
    };

    // -- Create marketplace service --
    const marketplace = createMarketplaceService(mockStore, a2aService);

    // Step 1: Buyer broadcasts RFQ
    const rfqResult = await marketplace.broadcastRFQ({
      items: [{ description: 'Data processing job', quantity: 1 }],
      deadlineMinutes: 60,
      scoringCriteria: 'cheapest',
      buyerAddress: BUYER,
      sellerFilter: 'data',
    });

    assert.ok(rfqResult.rfq);
    assert.equal(rfqResult.sellersContacted, 3);
    const rfqId = rfqResult.rfq.id;

    // Step 2: Sellers provide quotes (simulate by updating quote records)
    const responses = mockStore.listRFQResponses({ rfq_id: rfqId });
    assert.equal(responses.length, 3);

    // Simulate each seller providing their quote
    const sellerPrices = { [SELLER_1]: 150, [SELLER_2]: 80, [SELLER_3]: 120 };
    for (const resp of responses) {
      const q = quotes.get(resp.quote_id);
      const price = sellerPrices[resp.seller_address];
      q.status = 'quoted';
      q.total = Math.round(price * 1_000_000);
      q.total_decimal = price;
      q.quoted_at = new Date().toISOString();
    }

    // Step 3: Score responses
    const scored = marketplace.collectRFQResponses(rfqId);
    assert.equal(scored.scoredCount, 3);
    assert.equal(scored.ranked.length, 3);

    // With 'cheapest' scoring, highest score should be the cheapest (Seller 2 at $80)
    const winner = scored.ranked[0];
    const winnerQuote = quotes.get(winner.quote_id);
    assert.equal(winnerQuote.seller_address, SELLER_2);
    assert.equal(winnerQuote.total_decimal, 80);

    // Step 4: Deadline passes — auto-award
    // Set the deadline to the past
    const rfqRecord = rfqs.get(rfqId);
    rfqRecord.deadline = hoursAgo(1);

    const autoAwardResult = await marketplace.autoAwardExpiredRFQs();
    assert.equal(autoAwardResult.awarded, 1);
    assert.equal(autoAwardResult.awards.length, 1);
    assert.equal(autoAwardResult.awards[0].winnerAddress, SELLER_2);

    // Step 5: Verify winner accepted, losers declined
    assert.equal(quoteActions.accepted.length, 1);
    assert.equal(quoteActions.declined.length, 2);

    // Verify winner quote was accepted
    const winnerQuoteAfter = quotes.get(quoteActions.accepted[0]);
    assert.equal(winnerQuoteAfter.seller_address, SELLER_2);
    assert.equal(winnerQuoteAfter.status, 'accepted');

    // Verify loser quotes were declined
    for (const declinedId of quoteActions.declined) {
      const declinedQuote = quotes.get(declinedId);
      assert.equal(declinedQuote.status, 'declined');
      assert.notEqual(declinedQuote.seller_address, SELLER_2);
    }

    // Verify RFQ status updated
    const finalRfq = rfqs.get(rfqId);
    assert.equal(finalRfq.status, 'awarded');
    assert.ok(finalRfq.winning_quote_id);
    assert.ok(finalRfq.awarded_at);
  });
});
