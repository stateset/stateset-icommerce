/**
 * Unit tests for Settlement integration with Agent Runtime
 *
 * Tests the settlement flow in cli/src/a2a/agent-runtime.js:
 *   - Backward compat: tick() without settlement works identically
 *   - Pre-flight: hasSufficientFunds called before accept
 *   - Settlement flow: settle() called after acceptQuote, payment updated
 *   - Events: settlement:pending, settlement:confirmed, settlement:failed
 *   - Failure handling: settlement failure → payment status 'failed'
 *   - Public API: getOnChainBalance/getChainWalletAddress
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { createAgentRuntime, makeCommerceProxy } from '../../src/a2a/agent-runtime.js';
import { createAlwaysAcceptStrategy } from '../../src/a2a/strategies.js';

// ===========================================================================
// Helpers
// ===========================================================================

const WALLET_BUYER = '0xBuyer' + crypto.randomBytes(16).toString('hex');
const WALLET_SELLER = '0xSeller' + crypto.randomBytes(16).toString('hex');

function makeKeys() {
  return {
    privateKey: crypto.randomBytes(32).toString('hex'),
    publicKey: crypto.randomBytes(32).toString('hex'),
  };
}

/**
 * Create a mock commerce proxy backed by in-memory Maps.
 */
function createMockCommerce() {
  const quotes = new Map();
  const payments = new Map();
  const services = new Map();
  const feedback = new Map();
  const reputations = new Map();
  let paymentCounter = 0;

  const a2aStore = {
    createQuote: (q) => { quotes.set(q.id, { ...q }); return { ...q }; },
    getQuote: (id) => quotes.get(id) || null,
    updateQuote: (id, u) => {
      const q = quotes.get(id);
      if (q) quotes.set(id, { ...q, ...u });
    },
    listQuotes: (filter = {}) => {
      let results = [...quotes.values()];
      if (filter.seller_address) results = results.filter(q => q.seller_address === filter.seller_address);
      if (filter.buyer_address) results = results.filter(q => q.buyer_address === filter.buyer_address);
      if (filter.status) results = results.filter(q => q.status === filter.status);
      return results;
    },
    createPayment: (p) => {
      paymentCounter++;
      const payment = { id: p.id || `pay-${paymentCounter}`, ...p };
      payments.set(payment.id, payment);
      return payment;
    },
    getPayment: (id) => payments.get(id) || null,
    updatePayment: (id, u) => {
      const p = payments.get(id);
      if (p) payments.set(id, { ...p, ...u });
    },
    listPayments: (filter = {}) => {
      let results = [...payments.values()];
      if (filter.sender_address) results = results.filter(p => p.sender_address === filter.sender_address);
      return results;
    },
    sumPayments: () => ({ total: 0 }),
    createPaymentRequest: (r) => r,
    getPaymentRequest: () => null,
    updatePaymentRequest: () => {},
    listPaymentRequests: () => [],
    createService: (s) => { services.set(s.id, { ...s }); return { ...s }; },
    getService: (id) => services.get(id) || null,
    updateService: (id, u) => {
      const s = services.get(id);
      if (s) services.set(id, { ...s, ...u });
    },
    listServices: (filter = {}) => {
      let results = [...services.values()];
      if (filter.agent_address) results = results.filter(s => s.agent_address === filter.agent_address);
      if (filter.category) results = results.filter(s => s.category === filter.category);
      if (filter.active !== undefined) results = results.filter(s => s.active === filter.active);
      return results;
    },
    createFeedback: (f) => { feedback.set(f.id || crypto.randomUUID(), f); return f; },
    getFeedback: (id) => feedback.get(id) || null,
    updateFeedback: () => {},
    listFeedback: () => [...feedback.values()],
    getReputationScore: (addr) => reputations.get(addr) || null,
    upsertReputationScore: (s) => { reputations.set(s.agent_address, s); return s; },
    createEscrow: (e) => e,
    getEscrow: () => null,
    updateEscrow: () => {},
    listEscrows: () => [],
    createDispute: (d) => d,
    getDispute: () => null,
    updateDispute: () => {},
    listDisputes: () => [],
    createEvidence: (e) => e,
    getEvidence: () => null,
    listEvidenceByDispute: () => [],
    createSubscription: (s) => s,
    getSubscription: () => null,
    updateSubscription: () => {},
    listSubscriptions: () => [],
    getDueSubscriptions: () => [],
    getExpiredTrials: () => [],
    createSplitPayment: (s) => s,
    getSplitPayment: () => null,
    updateSplitPayment: () => {},
    listSplitPayments: () => [],
    createSplitRecipient: (r) => r,
    getSplitRecipient: () => null,
    updateSplitRecipient: () => {},
    listSplitRecipients: () => [],
    createNotificationLog: (n) => n,
    getNotificationLog: () => null,
    updateNotificationLog: () => {},
    listNotificationLog: () => [],
    getPendingNotifications: () => [],
    upsertWebhookConfig: (c) => c,
    getWebhookConfig: () => null,
    listWebhookConfigs: () => [],
    createEventSubscription: (s) => s,
    getEventSubscription: () => null,
    updateEventSubscription: () => {},
    listEventSubscriptions: () => [],
    createEventLog: (e) => e,
    getEventLog: () => null,
    listEventLog: () => [],
  };

  const commerce = {
    a2a: () => a2aStore,
    x402: () => ({
      getAgent: () => null,
      getAgentByWallet: () => null,
    }),
  };

  return { commerce, a2aStore, quotes, payments, services };
}

/**
 * Create a mock settlement service.
 */
function createMockSettlement(overrides = {}) {
  const calls = { settle: [], hasSufficientFunds: [], getBalance: [], getAddress: [] };

  return {
    calls,
    service: {
      get chainId() { return overrides.chainId || 'base'; },
      get isSimulation() { return overrides.isSimulation || false; },
      get agentId() { return overrides.agentId || 'agent-1'; },

      settle: async (params) => {
        calls.settle.push(params);
        if (overrides.settleError) throw new Error(overrides.settleError);
        return overrides.settleResult || {
          success: true,
          txHash: '0x' + 'b'.repeat(64),
          blockNumber: 54321,
          explorerUrl: `https://basescan.org/tx/0x${'b'.repeat(64)}`,
          confirmations: 5,
          simulated: false,
        };
      },

      hasSufficientFunds: async (amount) => {
        calls.hasSufficientFunds.push(amount);
        return overrides.fundsResult || {
          sufficient: true,
          balance: '1000.00',
          required: String(amount),
          symbol: 'USDC',
        };
      },

      getBalance: async () => {
        calls.getBalance.push(true);
        return overrides.balanceResult || {
          balance: '1000.00',
          balanceSmallest: 1000000000n,
          symbol: 'USDC',
        };
      },

      getAddress: async () => {
        calls.getAddress.push(true);
        return overrides.address || '0xChainWallet123';
      },
    },
  };
}

/**
 * Set up a buyer runtime with a "quoted" quote ready for processing.
 */
function setupBuyerWithQuotedQuote(options = {}) {
  const { commerce, a2aStore, quotes, payments } = createMockCommerce();
  const quoteId = options.quoteId || crypto.randomUUID();

  // Insert a quote in 'quoted' state directed at the buyer
  const quote = {
    id: quoteId,
    buyer_address: WALLET_BUYER,
    seller_agent_id: options.sellerAgentId || null,
    seller_address: WALLET_SELLER,
    status: 'quoted',
    total: options.total || 50,
    total_decimal: options.total || 50,
    asset: options.asset || 'USDC',
    network: options.network || 'set_chain',
    accepted_networks: options.acceptedNetworks || undefined,
    items: JSON.stringify([{ name: 'test-service', quantity: 1, unit_price: options.total || 50 }]),
    metadata: options.quoteMetadata ? JSON.stringify(options.quoteMetadata) : null,
    created_at: new Date().toISOString(),
  };
  quotes.set(quoteId, quote);

  const mockSettlement = options.settlement || createMockSettlement();

  const runtime = createAgentRuntime({
    name: 'TestBuyer',
    walletAddress: WALLET_BUYER,
    signingKey: makeKeys(),
    commerce,
    budget: { perTransaction: options.budgetLimit || 500, daily: 2000 },
    strategy: createAlwaysAcceptStrategy(),
    settlement: options.noSettlement ? null : mockSettlement.service,
    logger: options.logger || (() => {}),
  });

  return { runtime, commerce, a2aStore, quotes, payments, quoteId, mockSettlement };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('Settlement Runtime Integration', () => {
  // =========================================================================
  // Backward Compatibility
  // =========================================================================

  describe('backward compatibility (no settlement)', () => {
    it('tick() processes quotes without settlement when settlement is null', async () => {
      const { runtime, quotes, payments, quoteId } = setupBuyerWithQuotedQuote({ noSettlement: true });

      const processed = await runtime.tick();
      assert.equal(processed, 1);

      // Quote should be accepted
      const quote = quotes.get(quoteId);
      assert.equal(quote.status, 'accepted');

      // Payment should exist but with no tx_hash (may be null or undefined)
      const allPayments = [...payments.values()];
      assert.ok(allPayments.length > 0);
      assert.ok(!allPayments[0].tx_hash, 'tx_hash should be falsy (null or undefined)');

      runtime.destroy();
    });

    it('does not emit settlement events when no settlement', async () => {
      const events = [];
      const { runtime } = setupBuyerWithQuotedQuote({ noSettlement: true });

      runtime.on('settlement:pending', (e) => events.push('pending'));
      runtime.on('settlement:confirmed', (e) => events.push('confirmed'));
      runtime.on('settlement:failed', (e) => events.push('failed'));
      runtime.on('settlement:insufficient_funds', (e) => events.push('insufficient'));

      await runtime.tick();
      assert.deepEqual(events, []);

      runtime.destroy();
    });

    it('getOnChainBalance returns null without settlement', async () => {
      const { runtime } = setupBuyerWithQuotedQuote({ noSettlement: true });
      const bal = await runtime.getOnChainBalance();
      assert.equal(bal, null);
      runtime.destroy();
    });

    it('getChainWalletAddress returns null without settlement', async () => {
      const { runtime } = setupBuyerWithQuotedQuote({ noSettlement: true });
      const addr = await runtime.getChainWalletAddress();
      assert.equal(addr, null);
      runtime.destroy();
    });
  });

  // =========================================================================
  // Pre-flight Balance Check
  // =========================================================================

  describe('pre-flight balance check', () => {
    it('calls hasSufficientFunds before accepting quote', async () => {
      const { runtime, mockSettlement } = setupBuyerWithQuotedQuote({ total: 75 });

      await runtime.tick();
      assert.ok(mockSettlement.calls.hasSufficientFunds.length > 0);
      assert.equal(mockSettlement.calls.hasSufficientFunds[0], 75);

      runtime.destroy();
    });

    it('skips quote when insufficient funds', async () => {
      const mock = createMockSettlement({
        fundsResult: { sufficient: false, balance: '10.00', required: '75', symbol: 'USDC' },
      });

      const events = [];
      const { runtime, quotes, quoteId } = setupBuyerWithQuotedQuote({
        total: 75,
        settlement: mock,
      });

      runtime.on('settlement:insufficient_funds', (e) => events.push(e));

      await runtime.tick();

      // Quote should NOT be accepted (still 'quoted')
      assert.equal(quotes.get(quoteId).status, 'quoted');

      // Insufficient funds event emitted
      assert.equal(events.length, 1);
      assert.equal(events[0].quoteId, quoteId);
      assert.equal(events[0].required, 75);
      assert.equal(events[0].available, '10.00');
      assert.equal(events[0].symbol, 'USDC');

      // settle() should NOT have been called
      assert.equal(mock.calls.settle.length, 0);

      runtime.destroy();
    });

    it('emits settlement:failed on preflight error', async () => {
      const mock = createMockSettlement();
      mock.service.hasSufficientFunds = async () => {
        throw new Error('Chain RPC unreachable');
      };

      const events = [];
      const { runtime, quotes, quoteId } = setupBuyerWithQuotedQuote({
        total: 50,
        settlement: mock,
      });

      runtime.on('settlement:failed', (e) => events.push(e));

      await runtime.tick();

      // Quote not accepted
      assert.equal(quotes.get(quoteId).status, 'quoted');

      // Failed event emitted with preflight phase
      assert.equal(events.length, 1);
      assert.equal(events[0].phase, 'preflight');
      assert.ok(events[0].error.includes('Chain RPC unreachable'));

      runtime.destroy();
    });
  });

  // =========================================================================
  // Settlement Flow
  // =========================================================================

  describe('settlement flow', () => {
    it('calls settle() after acceptQuote with correct params', async () => {
      const { runtime, mockSettlement, quoteId } = setupBuyerWithQuotedQuote({ total: 50 });

      await runtime.tick();

      assert.equal(mockSettlement.calls.settle.length, 1);
      const settleCall = mockSettlement.calls.settle[0];
      assert.equal(settleCall.toAddress, WALLET_SELLER);
      assert.equal(settleCall.amount, 50);
      assert.equal(settleCall.asset, 'USDC');
      assert.ok(settleCall.memo.includes(quoteId));
      assert.ok(settleCall.paymentId); // Payment ID assigned

      runtime.destroy();
    });

    it('uses seller payout metadata for native-chain settlement', async () => {
      const mockSettlement = createMockSettlement({
        chainId: 'bitcoin',
        fundsResult: {
          sufficient: true,
          balance: '1.00000000',
          required: '0.5',
          symbol: 'BTC',
        },
        balanceResult: {
          balance: '1.00000000',
          balanceSmallest: 100000000n,
          symbol: 'BTC',
        },
      });
      const { runtime } = setupBuyerWithQuotedQuote({
        total: 0.5,
        asset: 'BTC',
        settlement: mockSettlement,
        sellerAgentId: '11111111-1111-1111-1111-111111111111',
        quoteMetadata: {
          seller_payment_address: 'bc1qsellernative',
        },
      });

      await runtime.tick();

      assert.equal(mockSettlement.calls.settle.length, 1);
      assert.equal(mockSettlement.calls.settle[0].toAddress, 'bc1qsellernative');
      assert.equal(mockSettlement.calls.settle[0].asset, 'BTC');

      runtime.destroy();
    });

    it('selects the settlement service matching the negotiated network', async () => {
      const zcashSettlement = createMockSettlement({
        chainId: 'zcash',
        fundsResult: {
          sufficient: true,
          balance: '5.00000000',
          required: '0.5',
          symbol: 'ZEC',
        },
        balanceResult: {
          balance: '5.00000000',
          balanceSmallest: 500000000n,
          symbol: 'ZEC',
        },
      });
      const bitcoinSettlement = createMockSettlement({
        chainId: 'bitcoin',
        fundsResult: {
          sufficient: true,
          balance: '1.00000000',
          required: '0.5',
          symbol: 'BTC',
        },
        balanceResult: {
          balance: '1.00000000',
          balanceSmallest: 100000000n,
          symbol: 'BTC',
        },
      });

      const { runtime } = setupBuyerWithQuotedQuote({
        total: 0.5,
        asset: 'BTC',
        network: 'bitcoin',
        acceptedNetworks: ['bitcoin'],
        settlement: zcashSettlement,
        sellerAgentId: '11111111-1111-1111-1111-111111111111',
        quoteMetadata: {
          seller_payment_address: 'bc1qbitcoinseller',
        },
      });
      runtime.setSettlement(bitcoinSettlement.service);
      runtime.settlement = zcashSettlement.service;

      await runtime.tick();

      assert.equal(zcashSettlement.calls.settle.length, 0);
      assert.equal(bitcoinSettlement.calls.settle.length, 1);
      assert.equal(bitcoinSettlement.calls.settle[0].toAddress, 'bc1qbitcoinseller');
      assert.equal(bitcoinSettlement.calls.settle[0].asset, 'BTC');

      runtime.destroy();
    });

    it('updates payment with tx_hash and block_number on success', async () => {
      const { runtime, payments } = setupBuyerWithQuotedQuote({ total: 50 });

      await runtime.tick();

      const allPayments = [...payments.values()];
      assert.ok(allPayments.length > 0);
      const payment = allPayments[0];
      assert.equal(payment.status, 'completed');
      assert.equal(payment.tx_hash, '0x' + 'b'.repeat(64));
      assert.equal(payment.block_number, 54321);
      assert.ok(payment.completed_at);

      // Metadata should contain explorer URL
      const meta = JSON.parse(payment.metadata);
      assert.ok(meta.explorer_url);
      assert.equal(meta.confirmations, 5);
      assert.equal(meta.chain_id, 'base');

      runtime.destroy();
    });

    it('emits settlement:pending before settle()', async () => {
      const events = [];
      const { runtime, quoteId } = setupBuyerWithQuotedQuote({ total: 50 });

      runtime.on('settlement:pending', (e) => events.push(e));

      await runtime.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].quoteId, quoteId);
      assert.equal(events[0].amount, 50);
      assert.equal(events[0].toAddress, WALLET_SELLER);
      assert.equal(events[0].chainId, 'base');
      assert.ok(events[0].paymentId);

      runtime.destroy();
    });

    it('emits settlement:confirmed after successful settle()', async () => {
      const events = [];
      const { runtime, quoteId } = setupBuyerWithQuotedQuote({ total: 50 });

      runtime.on('settlement:confirmed', (e) => events.push(e));

      await runtime.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].quoteId, quoteId);
      assert.equal(events[0].txHash, '0x' + 'b'.repeat(64));
      assert.equal(events[0].blockNumber, 54321);
      assert.ok(events[0].explorerUrl);
      assert.equal(events[0].confirmations, 5);

      runtime.destroy();
    });
  });

  // =========================================================================
  // Settlement Failure
  // =========================================================================

  describe('settlement failure', () => {
    it('updates payment to failed when settle() returns failure', async () => {
      const mock = createMockSettlement({
        settleResult: { success: false, error: 'Gas estimation failed' },
      });

      const { runtime, payments } = setupBuyerWithQuotedQuote({
        total: 50,
        settlement: mock,
      });

      await runtime.tick();

      const allPayments = [...payments.values()];
      assert.ok(allPayments.length > 0);
      const payment = allPayments[0];
      assert.equal(payment.status, 'failed');
      const meta = JSON.parse(payment.metadata);
      assert.equal(meta.settlement_error, 'Gas estimation failed');

      runtime.destroy();
    });

    it('emits settlement:failed on settle() failure', async () => {
      const mock = createMockSettlement({
        settleResult: { success: false, error: 'Nonce too low' },
      });

      const events = [];
      const { runtime, quoteId } = setupBuyerWithQuotedQuote({
        total: 50,
        settlement: mock,
      });

      runtime.on('settlement:failed', (e) => events.push(e));

      await runtime.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].quoteId, quoteId);
      assert.equal(events[0].error, 'Nonce too low');

      runtime.destroy();
    });

    it('emits settlement:failed on settle() exception', async () => {
      const mock = createMockSettlement({ settleError: 'Connection reset' });

      const events = [];
      const { runtime, quoteId } = setupBuyerWithQuotedQuote({
        total: 50,
        settlement: mock,
      });

      runtime.on('settlement:failed', (e) => events.push(e));

      await runtime.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].error, 'Connection reset');

      runtime.destroy();
    });

    it('still processes quote as accepted even if settlement fails', async () => {
      const mock = createMockSettlement({
        settleResult: { success: false, error: 'tx reverted' },
      });

      const { runtime, quotes, quoteId } = setupBuyerWithQuotedQuote({
        total: 50,
        settlement: mock,
      });

      const processed = await runtime.tick();
      assert.equal(processed, 1);

      // Quote should still be accepted (payment was created)
      assert.equal(quotes.get(quoteId).status, 'accepted');

      runtime.destroy();
    });
  });

  // =========================================================================
  // Event Ordering
  // =========================================================================

  describe('event ordering', () => {
    it('emits events in correct order: accepted → pending → confirmed', async () => {
      const events = [];
      const { runtime } = setupBuyerWithQuotedQuote({ total: 50 });

      runtime.on('quote:accepted', () => events.push('accepted'));
      runtime.on('settlement:pending', () => events.push('pending'));
      runtime.on('settlement:confirmed', () => events.push('confirmed'));

      await runtime.tick();

      assert.deepEqual(events, ['accepted', 'pending', 'confirmed']);

      runtime.destroy();
    });

    it('emits accepted → pending → failed on failure', async () => {
      const events = [];
      const mock = createMockSettlement({
        settleResult: { success: false, error: 'fail' },
      });
      const { runtime } = setupBuyerWithQuotedQuote({ total: 50, settlement: mock });

      runtime.on('quote:accepted', () => events.push('accepted'));
      runtime.on('settlement:pending', () => events.push('pending'));
      runtime.on('settlement:failed', () => events.push('failed'));

      await runtime.tick();

      assert.deepEqual(events, ['accepted', 'pending', 'failed']);

      runtime.destroy();
    });
  });

  // =========================================================================
  // Public API
  // =========================================================================

  describe('public API', () => {
    it('settlement property is exposed', () => {
      const mock = createMockSettlement();
      const { commerce } = createMockCommerce();
      const runtime = createAgentRuntime({
        name: 'TestAgent',
        walletAddress: WALLET_BUYER,
        signingKey: makeKeys(),
        commerce,
        settlement: mock.service,
        logger: () => {},
      });

      assert.equal(runtime.settlement, mock.service);
      assert.equal(runtime.settlement.chainId, 'base');
      runtime.destroy();
    });

    it('settlement is null by default', () => {
      const { commerce } = createMockCommerce();
      const runtime = createAgentRuntime({
        name: 'TestAgent',
        walletAddress: WALLET_BUYER,
        signingKey: makeKeys(),
        commerce,
        logger: () => {},
      });

      assert.equal(runtime.settlement, null);
      runtime.destroy();
    });

    it('getOnChainBalance delegates to settlement.getBalance', async () => {
      const mock = createMockSettlement({
        balanceResult: { balance: '500.00', balanceSmallest: 500000000n, symbol: 'USDC' },
      });
      const { commerce } = createMockCommerce();
      const runtime = createAgentRuntime({
        name: 'TestAgent',
        walletAddress: WALLET_BUYER,
        signingKey: makeKeys(),
        commerce,
        settlement: mock.service,
        logger: () => {},
      });

      const bal = await runtime.getOnChainBalance();
      assert.equal(bal.balance, '500.00');
      assert.equal(bal.symbol, 'USDC');
      assert.equal(mock.calls.getBalance.length, 1);
      runtime.destroy();
    });

    it('getChainWalletAddress delegates to settlement.getAddress', async () => {
      const mock = createMockSettlement({ address: '0xMyChainWallet' });
      const { commerce } = createMockCommerce();
      const runtime = createAgentRuntime({
        name: 'TestAgent',
        walletAddress: WALLET_BUYER,
        signingKey: makeKeys(),
        commerce,
        settlement: mock.service,
        logger: () => {},
      });

      const addr = await runtime.getChainWalletAddress();
      assert.equal(addr, '0xMyChainWallet');
      assert.equal(mock.calls.getAddress.length, 1);
      runtime.destroy();
    });
  });

  // =========================================================================
  // Multiple Quotes
  // =========================================================================

  describe('multiple quotes', () => {
    it('settles each accepted quote independently', async () => {
      const { commerce, a2aStore, quotes, payments } = createMockCommerce();
      const mock = createMockSettlement();

      // Create 3 quotes
      const quoteIds = [];
      for (let i = 0; i < 3; i++) {
        const qid = crypto.randomUUID();
        quoteIds.push(qid);
        quotes.set(qid, {
          id: qid,
          buyer_address: WALLET_BUYER,
          seller_address: WALLET_SELLER,
          status: 'quoted',
          total: 10 + i * 10,
          total_decimal: 10 + i * 10,
          asset: 'USDC',
          network: 'set_chain',
          items: JSON.stringify([]),
          created_at: new Date().toISOString(),
        });
      }

      const runtime = createAgentRuntime({
        name: 'MultiBuyer',
        walletAddress: WALLET_BUYER,
        signingKey: makeKeys(),
        commerce,
        budget: { perTransaction: 500, daily: 2000 },
        strategy: createAlwaysAcceptStrategy(),
        settlement: mock.service,
        logger: () => {},
      });

      const processed = await runtime.tick();
      assert.equal(processed, 3);
      assert.equal(mock.calls.settle.length, 3);

      // Each settle call should have different amounts
      const amounts = mock.calls.settle.map(s => s.amount).sort((a, b) => a - b);
      assert.deepEqual(amounts, [10, 20, 30]);

      runtime.destroy();
    });
  });

  // =========================================================================
  // Simulated Settlement
  // =========================================================================

  describe('simulated settlement', () => {
    it('records simulated flag in payment metadata', async () => {
      const mock = createMockSettlement({
        settleResult: {
          success: true,
          txHash: null,
          blockNumber: null,
          explorerUrl: null,
          confirmations: 0,
          simulated: true,
        },
      });

      const { runtime, payments } = setupBuyerWithQuotedQuote({
        total: 50,
        settlement: mock,
      });

      await runtime.tick();

      const allPayments = [...payments.values()];
      assert.ok(allPayments.length > 0);
      const meta = JSON.parse(allPayments[0].metadata);
      assert.equal(meta.simulated, true);

      runtime.destroy();
    });

    it('emits settlement:confirmed with simulated flag', async () => {
      const mock = createMockSettlement({
        settleResult: {
          success: true,
          txHash: null,
          blockNumber: null,
          confirmations: 0,
          simulated: true,
        },
      });

      const events = [];
      const { runtime } = setupBuyerWithQuotedQuote({ total: 50, settlement: mock });
      runtime.on('settlement:confirmed', (e) => events.push(e));

      await runtime.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].simulated, true);

      runtime.destroy();
    });
  });

  // =========================================================================
  // Budget + Settlement interaction
  // =========================================================================

  describe('budget + settlement interaction', () => {
    it('budget check runs before settlement pre-flight', async () => {
      const mock = createMockSettlement();
      const { runtime, quotes, quoteId } = setupBuyerWithQuotedQuote({
        total: 999,
        budgetLimit: 100, // per-tx limit below total
        settlement: mock,
      });

      const events = [];
      runtime.on('budget:exceeded', (e) => events.push(e));
      runtime.on('settlement:insufficient_funds', () => events.push('funds'));

      await runtime.tick();

      // Budget check should trigger, not settlement check
      assert.ok(events.some(e => e.type === 'perTransaction'));
      assert.ok(events.some(e => e.limit === 100));
      assert.ok(events.some(e => e.attempted === 999));
      assert.ok(!events.includes('funds'));

      // hasSufficientFunds should NOT be called
      assert.equal(mock.calls.hasSufficientFunds.length, 0);

      runtime.destroy();
    });

    it('classifies daily budget failures before settlement pre-flight', async () => {
      const mock = createMockSettlement({
        chainId: 'bitcoin',
        symbol: 'BTC',
      });
      const { runtime } = setupBuyerWithQuotedQuote({
        total: 100,
        asset: 'BTC',
        network: 'bitcoin',
        acceptedNetworks: ['bitcoin'],
        settlement: mock,
      });

      runtime.recordSpend(1950, { asset: 'BTC', network: 'bitcoin' });

      const events = [];
      runtime.on('budget:exceeded', (e) => events.push(e));
      runtime.on('settlement:insufficient_funds', () => events.push('funds'));

      await runtime.tick();

      assert.ok(events.some(e => e.type === 'daily'));
      assert.ok(events.some(e => e.asset === 'BTC'));
      assert.ok(events.some(e => e.network === 'bitcoin'));
      assert.ok(events.some(e => e.limit === 2000));
      assert.ok(events.some(e => e.remaining === 50));
      assert.ok(!events.includes('funds'));
      assert.equal(mock.calls.hasSufficientFunds.length, 0);

      runtime.destroy();
    });
  });
});
