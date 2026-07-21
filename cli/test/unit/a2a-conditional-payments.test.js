/**
 * Tests for A2A conditional payment methods:
 *   - createConditionalPayment
 *   - checkPaymentConditions
 *   - settleConditionalPayment
 *
 * Uses node:test built-in runner (NOT vitest).
 */

import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createA2AService } from '../../src/a2a/index.js';

/** Builds a mock store (commerce.a2a()) with in-memory escrow/quote storage. */
function createMockStore() {
  const escrows = new Map();
  const quotes = new Map();

  return {
    // Escrow CRUD
    createEscrow: mock.fn(async (e) => {
      const id = e.id || `escrow-${escrows.size + 1}`;
      const record = { ...e, id, status: e.status || 'created' };
      escrows.set(id, record);
      return record;
    }),
    getEscrow: mock.fn(async (id) => {
      const e = escrows.get(id);
      return e ? { ...e } : null;
    }),
    updateEscrow: mock.fn(async (id, updates) => {
      const e = escrows.get(id);
      if (!e) throw new Error('Escrow not found');
      Object.assign(e, updates);
      return { ...e };
    }),
    listEscrows: mock.fn(async () => [...escrows.values()]),

    // Quote CRUD
    createQuote: mock.fn(async (q) => {
      const id = q.id || `quote-${quotes.size + 1}`;
      const record = { ...q, id };
      quotes.set(id, record);
      return record;
    }),
    getQuote: mock.fn(async (id) => {
      const q = quotes.get(id);
      return q ? { ...q } : null;
    }),
    updateQuote: mock.fn(async (id, updates) => {
      const q = quotes.get(id);
      if (!q) throw new Error('Quote not found');
      Object.assign(q, updates);
      return { ...q };
    }),
    listQuotes: mock.fn(async () => [...quotes.values()]),

    // Payment stubs (needed by pay() which createConditionalPayment does NOT call,
    // but the service factory still references these through other methods)
    createPayment: mock.fn(async (p) => ({ ...p })),
    getPayment: mock.fn(async () => null),
    updatePayment: mock.fn(async (id, u) => ({ id, ...u })),
    listPayments: mock.fn(async () => []),
    sumPayments: mock.fn(async () => 0),

    // Payment request stubs
    createPaymentRequest: mock.fn(async (r) => ({ ...r })),
    getPaymentRequest: mock.fn(async () => null),
    updatePaymentRequest: mock.fn(async () => ({})),
    listPaymentRequests: mock.fn(async () => []),

    // helpers for test assertions
    _escrows: escrows,
    _quotes: quotes,
  };
}

function createMockX402() {
  return {
    discoverAgents: mock.fn(async () => []),
    registerAgentCard: mock.fn(async () => ({})),
    getAgent: mock.fn(async () => null),
    getAgentByWallet: mock.fn(async () => null),
    createIntent: mock.fn(async (params) => ({
      id: 'intent-1',
      ...params,
    })),
    signIntent: mock.fn(async (id) => ({
      id,
      signing_hash: '0xsighash',
    })),
    updateIntent: mock.fn(async (id, u) => ({ id, ...u })),
  };
}

describe('A2A Conditional Payments', () => {
  let store;
  let x402;
  let commerce;
  let agentConfig;
  let service;

  beforeEach(() => {
    store = createMockStore();
    x402 = createMockX402();

    commerce = {
      a2a: () => store,
      x402: () => x402,
    };

    agentConfig = {
      walletAddress: '0xBuyer',
      agentId: 'agent-1',
    };

    service = createA2AService(commerce, agentConfig);
  });

  // ===========================================================================
  // createConditionalPayment
  // ===========================================================================
  describe('createConditionalPayment', () => {
    it('creates escrow with basic params and funds it', async () => {
      const result = await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 100,
        memo: 'test payment',
      });

      assert.equal(result.success, true);
      assert.equal(result.escrow.buyerAddress, '0xBuyer');
      assert.equal(result.escrow.sellerAddress, '0xSeller');
      assert.equal(result.escrow.amount, 100);
      assert.equal(result.escrow.asset, 'USDC');
      assert.equal(result.escrow.network, 'set_chain');
      assert.ok(result.escrow.id);

      // Should have called createEscrow
      assert.equal(store.createEscrow.mock.calls.length, 1);
      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      assert.equal(escrowArg.buyer_address, '0xBuyer');
      assert.equal(escrowArg.seller_address, '0xSeller');
      // 100 * 10^6 = 100000000
      assert.equal(escrowArg.amount, 100_000_000);
      assert.equal(escrowArg.asset, 'USDC');

      // Should have funded the escrow (updateEscrow with status: 'funded')
      const fundCall = store.updateEscrow.mock.calls.find(
        (c) => c.arguments[1].status === 'funded',
      );
      assert.ok(fundCall, 'escrow should be funded');
    });

    it('creates escrow with custom conditions', async () => {
      const conditions = [
        { type: 'buyer_confirmed', completed: false },
        { type: 'milestone', description: 'Phase 1', completed: false },
      ];

      const result = await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 50,
        conditions,
      });

      assert.equal(result.success, true);
      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      assert.equal(escrowArg.release_conditions.length, 2);
      assert.equal(escrowArg.release_conditions[0].type, 'buyer_confirmed');
      assert.equal(escrowArg.release_conditions[1].type, 'milestone');
    });

    it('auto-adds seller_fulfilled condition when quoteId is provided', async () => {
      const result = await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 200,
        quoteId: 'quote-abc',
      });

      assert.equal(result.success, true);
      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      const sellerCond = escrowArg.release_conditions.find((c) => c.type === 'seller_fulfilled');
      assert.ok(sellerCond, 'should auto-add seller_fulfilled condition');
      assert.equal(sellerCond.quoteId, 'quote-abc');
      assert.equal(escrowArg.quote_id, 'quote-abc');
    });

    it('does not duplicate seller_fulfilled when already in conditions', async () => {
      const conditions = [{ type: 'seller_fulfilled', quoteId: 'quote-abc' }];

      await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 75,
        quoteId: 'quote-abc',
        conditions,
      });

      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      const sellerConds = escrowArg.release_conditions.filter((c) => c.type === 'seller_fulfilled');
      assert.equal(sellerConds.length, 1, 'should not duplicate seller_fulfilled');
    });

    it('throws when sellerAddress is missing', async () => {
      await assert.rejects(() => service.createConditionalPayment({ amount: 100 }), {
        message: 'sellerAddress is required',
      });
    });

    it('throws when amount is zero', async () => {
      await assert.rejects(
        () =>
          service.createConditionalPayment({
            sellerAddress: '0xSeller',
            amount: 0,
          }),
        { message: 'amount must be positive' },
      );
    });

    it('throws when amount is negative', async () => {
      await assert.rejects(
        () =>
          service.createConditionalPayment({
            sellerAddress: '0xSeller',
            amount: -10,
          }),
        { message: 'amount must be positive' },
      );
    });

    it('respects custom asset and computes correct smallest unit (DAI = 18 decimals)', async () => {
      await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 1,
        asset: 'DAI',
      });

      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      assert.equal(escrowArg.asset, 'DAI');
      // 1 * 10^18 = 1000000000000000000
      assert.equal(escrowArg.amount, 1_000_000_000_000_000_000);
    });

    it('respects custom expiresInHours', async () => {
      const before = Date.now();

      await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 10,
        expiresInHours: 24,
      });

      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      const expiresAt = new Date(escrowArg.expires_at).getTime();
      const expectedMin = before + 24 * 60 * 60 * 1000 - 5000;
      const expectedMax = before + 24 * 60 * 60 * 1000 + 5000;
      assert.ok(expiresAt >= expectedMin && expiresAt <= expectedMax);
    });

    it('creates x402 intent when sequencerClient and signingKey are present', async () => {
      const sequencerClient = {
        submitPaymentIntent: mock.fn(async () => ({})),
      };
      const signingKey = { privateKey: 'pk', publicKey: 'pub' };

      const svcWithSeq = createA2AService(commerce, {
        ...agentConfig,
        sequencerClient,
        signingKey,
      });

      const result = await svcWithSeq.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 100,
      });

      assert.equal(result.escrow.intentId, 'intent-1');
      assert.equal(x402.createIntent.mock.calls.length, 1);

      // Should link intent to escrow via updateEscrow
      const intentLinkCall = store.updateEscrow.mock.calls.find((c) => c.arguments[1].intent_id);
      assert.ok(intentLinkCall, 'should link intent to escrow');
      assert.equal(intentLinkCall.arguments[1].intent_id, 'intent-1');
    });

    it('still creates escrow when x402 intent creation fails', async () => {
      const sequencerClient = {
        submitPaymentIntent: mock.fn(async () => ({})),
      };
      const signingKey = { privateKey: 'pk', publicKey: 'pub' };

      x402.createIntent = mock.fn(async () => {
        throw new Error('x402 service down');
      });

      const svcWithSeq = createA2AService(commerce, {
        ...agentConfig,
        sequencerClient,
        signingKey,
      });

      const result = await svcWithSeq.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 50,
      });

      // Escrow should still be created and funded
      assert.equal(result.success, true);
      assert.equal(result.escrow.intentId, null);
      assert.equal(store.createEscrow.mock.calls.length, 1);
    });

    it('stores memo in metadata as JSON', async () => {
      await service.createConditionalPayment({
        sellerAddress: '0xSeller',
        amount: 10,
        memo: 'Widget delivery',
      });

      const escrowArg = store.createEscrow.mock.calls[0].arguments[0];
      assert.equal(escrowArg.metadata, JSON.stringify({ memo: 'Widget delivery' }));
    });
  });

  // ===========================================================================
  // checkPaymentConditions
  // ===========================================================================
  describe('checkPaymentConditions', () => {
    it('throws when escrowId is missing', async () => {
      await assert.rejects(() => service.checkPaymentConditions(undefined), {
        message: 'escrowId is required',
      });
    });

    it('throws when escrow is not found', async () => {
      await assert.rejects(() => service.checkPaymentConditions('nonexistent'), {
        message: 'Escrow not found',
      });
    });

    it('returns allMet=true when there are no conditions', async () => {
      store._escrows.set('esc-1', {
        id: 'esc-1',
        status: 'funded',
        release_conditions: [],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-1');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions.length, 0);
      assert.equal(result.escrowId, 'esc-1');
      assert.equal(result.status, 'funded');
    });

    it('evaluates seller_fulfilled as met when quote status is fulfilled', async () => {
      store._quotes.set('q-1', { id: 'q-1', status: 'fulfilled' });
      store._escrows.set('esc-2', {
        id: 'esc-2',
        status: 'funded',
        release_conditions: [{ type: 'seller_fulfilled', quoteId: 'q-1' }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-2');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions[0].met, true);
    });

    it('evaluates seller_fulfilled as unmet when quote is not fulfilled', async () => {
      store._quotes.set('q-2', { id: 'q-2', status: 'accepted' });
      store._escrows.set('esc-3', {
        id: 'esc-3',
        status: 'funded',
        release_conditions: [{ type: 'seller_fulfilled', quoteId: 'q-2' }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-3');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates seller_fulfilled as unmet when quote does not exist', async () => {
      store._escrows.set('esc-4', {
        id: 'esc-4',
        status: 'funded',
        release_conditions: [{ type: 'seller_fulfilled', quoteId: 'missing-q' }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-4');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates seller_fulfilled as unmet when no quoteId on condition', async () => {
      store._escrows.set('esc-noquote', {
        id: 'esc-noquote',
        status: 'funded',
        release_conditions: [{ type: 'seller_fulfilled' }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-noquote');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates buyer_confirmed as met when completed is true', async () => {
      store._escrows.set('esc-5', {
        id: 'esc-5',
        status: 'funded',
        release_conditions: [{ type: 'buyer_confirmed', completed: true }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-5');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions[0].met, true);
    });

    it('evaluates buyer_confirmed as unmet when completed is false', async () => {
      store._escrows.set('esc-6', {
        id: 'esc-6',
        status: 'funded',
        release_conditions: [{ type: 'buyer_confirmed', completed: false }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-6');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates time_lock as met when releaseAfter is in the past', async () => {
      const pastDate = new Date(Date.now() - 60_000).toISOString();
      store._escrows.set('esc-7', {
        id: 'esc-7',
        status: 'funded',
        release_conditions: [{ type: 'time_lock', releaseAfter: pastDate }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-7');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions[0].met, true);
    });

    it('evaluates time_lock as unmet when releaseAfter is in the future', async () => {
      const futureDate = new Date(Date.now() + 3_600_000).toISOString();
      store._escrows.set('esc-8', {
        id: 'esc-8',
        status: 'funded',
        release_conditions: [{ type: 'time_lock', releaseAfter: futureDate }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-8');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates time_lock as unmet when releaseAfter is missing', async () => {
      store._escrows.set('esc-norel', {
        id: 'esc-norel',
        status: 'funded',
        release_conditions: [{ type: 'time_lock' }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-norel');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates milestone as met when completed is true', async () => {
      store._escrows.set('esc-9', {
        id: 'esc-9',
        status: 'funded',
        release_conditions: [{ type: 'milestone', description: 'Phase 1', completed: true }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-9');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions[0].met, true);
    });

    it('evaluates milestone as unmet when completed is false', async () => {
      store._escrows.set('esc-10', {
        id: 'esc-10',
        status: 'funded',
        release_conditions: [{ type: 'milestone', description: 'Phase 2', completed: false }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-10');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('evaluates unknown condition type as unmet', async () => {
      store._escrows.set('esc-unk', {
        id: 'esc-unk',
        status: 'funded',
        release_conditions: [{ type: 'some_future_condition' }],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-unk');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, false);
    });

    it('returns allMet=false when some conditions met and others not', async () => {
      store._quotes.set('q-mix', { id: 'q-mix', status: 'fulfilled' });
      const futureDate = new Date(Date.now() + 3_600_000).toISOString();

      store._escrows.set('esc-mix', {
        id: 'esc-mix',
        status: 'funded',
        release_conditions: [
          { type: 'seller_fulfilled', quoteId: 'q-mix' },
          { type: 'time_lock', releaseAfter: futureDate },
        ],
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-mix');
      assert.equal(result.allMet, false);
      assert.equal(result.conditions[0].met, true);
      assert.equal(result.conditions[1].met, false);
    });

    it('returns allMet=true when all multiple conditions are met', async () => {
      store._quotes.set('q-all', { id: 'q-all', status: 'fulfilled' });
      const pastDate = new Date(Date.now() - 60_000).toISOString();

      store._escrows.set('esc-all', {
        id: 'esc-all',
        status: 'funded',
        release_conditions: [
          { type: 'seller_fulfilled', quoteId: 'q-all' },
          { type: 'buyer_confirmed', completed: true },
          { type: 'time_lock', releaseAfter: pastDate },
          { type: 'milestone', description: 'Done', completed: true },
        ],
        intent_id: 'int-99',
      });

      const result = await service.checkPaymentConditions('esc-all');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions.length, 4);
      assert.ok(result.conditions.every((c) => c.met));
      assert.equal(result.intentId, 'int-99');
    });

    it('parses JSON string release_conditions', async () => {
      store._escrows.set('esc-json', {
        id: 'esc-json',
        status: 'funded',
        release_conditions: JSON.stringify([{ type: 'buyer_confirmed', completed: true }]),
        intent_id: null,
      });

      const result = await service.checkPaymentConditions('esc-json');
      assert.equal(result.allMet, true);
      assert.equal(result.conditions.length, 1);
      assert.equal(result.conditions[0].met, true);
    });
  });

  // ===========================================================================
  // settleConditionalPayment
  // ===========================================================================
  describe('settleConditionalPayment', () => {
    it('throws when escrowId is missing', async () => {
      await assert.rejects(() => service.settleConditionalPayment(undefined), {
        message: 'escrowId is required',
      });
    });

    it('releases escrow when all conditions are met and status is funded', async () => {
      store._escrows.set('esc-settle', {
        id: 'esc-settle',
        status: 'funded',
        release_conditions: [{ type: 'buyer_confirmed', completed: true }],
        amount_decimal: 100,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: null,
      });

      const result = await service.settleConditionalPayment('esc-settle');

      assert.equal(result.success, true);
      assert.equal(result.escrowId, 'esc-settle');
      assert.equal(result.status, 'released');
      assert.equal(result.amount, 100);
      assert.equal(result.asset, 'USDC');
      assert.equal(result.sellerAddress, '0xSeller');
      assert.equal(result.intentSettled, false);

      // Verify updateEscrow was called with released status
      const releaseCall = store.updateEscrow.mock.calls.find(
        (c) => c.arguments[1].status === 'released',
      );
      assert.ok(releaseCall, 'escrow should be released');
      assert.ok(releaseCall.arguments[1].released_at);
    });

    it('releases escrow when status is active', async () => {
      store._escrows.set('esc-active', {
        id: 'esc-active',
        status: 'active',
        release_conditions: [{ type: 'milestone', completed: true }],
        amount_decimal: 50,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: null,
      });

      const result = await service.settleConditionalPayment('esc-active');
      assert.equal(result.success, true);
      assert.equal(result.status, 'released');
    });

    it('throws when conditions are not met', async () => {
      store._escrows.set('esc-unmet', {
        id: 'esc-unmet',
        status: 'funded',
        release_conditions: [
          { type: 'buyer_confirmed', completed: false },
          { type: 'milestone', description: 'Step 1', completed: false },
        ],
        intent_id: null,
      });

      await assert.rejects(
        () => service.settleConditionalPayment('esc-unmet'),
        (err) => {
          assert.ok(
            err.message.includes('2 condition(s) not met'),
            `Expected "2 condition(s) not met" in: ${err.message}`,
          );
          assert.ok(err.message.includes('buyer_confirmed'));
          assert.ok(err.message.includes('milestone'));
          return true;
        },
      );
    });

    it('throws when escrow status is not funded or active', async () => {
      store._escrows.set('esc-released', {
        id: 'esc-released',
        status: 'released',
        release_conditions: [],
        amount_decimal: 25,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: null,
      });

      await assert.rejects(() => service.settleConditionalPayment('esc-released'), {
        message: 'Cannot settle escrow in status: released',
      });
    });

    it('throws for escrow in created status (not yet funded)', async () => {
      store._escrows.set('esc-created', {
        id: 'esc-created',
        status: 'created',
        release_conditions: [],
        amount_decimal: 10,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: null,
      });

      await assert.rejects(() => service.settleConditionalPayment('esc-created'), {
        message: 'Cannot settle escrow in status: created',
      });
    });

    it('settles linked x402 intent when sequencerClient is present', async () => {
      const sequencerClient = {
        submitPaymentIntent: mock.fn(async () => ({})),
      };
      const signingKey = { privateKey: 'pk', publicKey: 'pub' };

      const svcWithSeq = createA2AService(commerce, {
        ...agentConfig,
        sequencerClient,
        signingKey,
      });

      store._escrows.set('esc-intent', {
        id: 'esc-intent',
        status: 'funded',
        release_conditions: [{ type: 'buyer_confirmed', completed: true }],
        amount_decimal: 200,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: 'intent-linked',
      });

      const result = await svcWithSeq.settleConditionalPayment('esc-intent');

      assert.equal(result.success, true);
      assert.equal(result.intentId, 'intent-linked');
      assert.equal(result.intentSettled, true);

      // Verify x402 updateIntent was called
      assert.equal(x402.updateIntent.mock.calls.length, 1);
      assert.equal(x402.updateIntent.mock.calls[0].arguments[0], 'intent-linked');
      assert.deepEqual(x402.updateIntent.mock.calls[0].arguments[1], {
        status: 'settled',
      });
    });

    it('does not settle x402 intent when no sequencerClient', async () => {
      store._escrows.set('esc-noseq', {
        id: 'esc-noseq',
        status: 'funded',
        release_conditions: [{ type: 'buyer_confirmed', completed: true }],
        amount_decimal: 50,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: 'intent-orphan',
      });

      const result = await service.settleConditionalPayment('esc-noseq');

      assert.equal(result.success, true);
      assert.equal(result.intentSettled, false);
      assert.equal(x402.updateIntent.mock.calls.length, 0);
    });

    it('still releases escrow when x402 intent settlement fails', async () => {
      const sequencerClient = {
        submitPaymentIntent: mock.fn(async () => ({})),
      };
      const signingKey = { privateKey: 'pk', publicKey: 'pub' };

      x402.updateIntent = mock.fn(async () => {
        throw new Error('x402 down');
      });

      const svcWithSeq = createA2AService(commerce, {
        ...agentConfig,
        sequencerClient,
        signingKey,
      });

      store._escrows.set('esc-failintent', {
        id: 'esc-failintent',
        status: 'funded',
        release_conditions: [],
        amount_decimal: 75,
        asset: 'USDC',
        seller_address: '0xSeller',
        intent_id: 'intent-fail',
      });

      const result = await svcWithSeq.settleConditionalPayment('esc-failintent');

      assert.equal(result.success, true);
      assert.equal(result.status, 'released');
      assert.equal(result.intentSettled, false);

      // Escrow should still be released
      const releaseCall = store.updateEscrow.mock.calls.find(
        (c) => c.arguments[1].status === 'released',
      );
      assert.ok(releaseCall, 'escrow should be released despite intent failure');
    });

    it('throws when escrow not found (via checkPaymentConditions)', async () => {
      await assert.rejects(() => service.settleConditionalPayment('does-not-exist'), {
        message: 'Escrow not found',
      });
    });
  });
});
