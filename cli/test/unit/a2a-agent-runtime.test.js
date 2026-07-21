/**
 * Unit tests for Agent Runtime — autonomous AI agent lifecycle manager
 *
 * Tests cli/src/a2a/agent-runtime.js:
 *   - createAgentRuntime() construction and validation
 *   - Budget enforcement (canAfford, recordSpend, rollover)
 *   - Strategy delegation
 *   - Service registration and discovery
 *   - Service loop (tick) processing
 *   - Event emission
 *   - makeCommerceProxy() helper
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { createAgentRuntime, makeCommerceProxy } from '../../src/a2a/agent-runtime.js';
import { createAlwaysAcceptStrategy, createBudgetGatedStrategy } from '../../src/a2a/strategies.js';

// ===========================================================================
// Helpers
// ===========================================================================

const WALLET_A = '0xAgentA' + crypto.randomBytes(16).toString('hex');
const WALLET_B = '0xAgentB' + crypto.randomBytes(16).toString('hex');

function makeKeys() {
  return {
    privateKey: crypto.randomBytes(32).toString('hex'),
    publicKey: crypto.randomBytes(32).toString('hex'),
  };
}

/**
 * Create a mock commerce proxy backed by in-memory Maps.
 * Mimics the shape of makeCommerceProxy(A2AStore).
 */
function createMockCommerce() {
  const quotes = new Map();
  const payments = new Map();
  const services = new Map();
  const feedback = new Map();
  const reputations = new Map();

  const a2aStore = {
    // Quotes
    createQuote: (q) => {
      quotes.set(q.id, { ...q });
      return { ...q };
    },
    getQuote: (id) => quotes.get(id) || null,
    updateQuote: (id, u) => {
      const q = quotes.get(id);
      if (q) quotes.set(id, { ...q, ...u });
    },
    listQuotes: (filter = {}) => {
      let results = [...quotes.values()];
      if (filter.seller_address)
        results = results.filter((q) => q.seller_address === filter.seller_address);
      if (filter.buyer_address)
        results = results.filter((q) => q.buyer_address === filter.buyer_address);
      if (filter.status) results = results.filter((q) => q.status === filter.status);
      return results;
    },
    // Payments
    createPayment: (p) => {
      payments.set(p.id, { ...p });
      return { ...p };
    },
    getPayment: (id) => payments.get(id) || null,
    updatePayment: (id, u) => {
      const p = payments.get(id);
      if (p) payments.set(id, { ...p, ...u });
    },
    listPayments: () => [...payments.values()],
    sumPayments: () => ({ total: 0 }),
    // Payment Requests
    createPaymentRequest: (r) => r,
    getPaymentRequest: () => null,
    updatePaymentRequest: () => {},
    listPaymentRequests: () => [],
    // Services
    createService: (s) => {
      services.set(s.id, { ...s });
      return { ...s };
    },
    getService: (id) => services.get(id) || null,
    updateService: (id, u) => {
      const s = services.get(id);
      if (s) services.set(id, { ...s, ...u });
    },
    listServices: (filter = {}) => {
      let results = [...services.values()];
      if (filter.agent_address)
        results = results.filter((s) => s.agent_address === filter.agent_address);
      if (filter.category) results = results.filter((s) => s.category === filter.category);
      if (filter.active !== undefined) results = results.filter((s) => s.active === filter.active);
      return results;
    },
    // Feedback / Reputation
    createFeedback: (f) => {
      feedback.set(f.id || crypto.randomUUID(), f);
      return f;
    },
    getFeedback: (id) => feedback.get(id) || null,
    updateFeedback: () => {},
    listFeedback: () => [...feedback.values()],
    getReputationScore: (addr) => reputations.get(addr) || null,
    upsertReputationScore: (s) => {
      reputations.set(s.agent_address, s);
      return s;
    },
    // Escrow (stubs)
    createEscrow: (e) => e,
    getEscrow: () => null,
    updateEscrow: () => {},
    listEscrows: () => [],
    // Disputes (stubs)
    createDispute: (d) => d,
    getDispute: () => null,
    updateDispute: () => {},
    listDisputes: () => [],
    createEvidence: (e) => e,
    getEvidence: () => null,
    listEvidenceByDispute: () => [],
    // Subscriptions (stubs)
    createSubscription: (s) => s,
    getSubscription: () => null,
    updateSubscription: () => {},
    listSubscriptions: () => [],
    getDueSubscriptions: () => [],
    getExpiredTrials: () => [],
    // Splits (stubs)
    createSplitPayment: (s) => s,
    getSplitPayment: () => null,
    updateSplitPayment: () => {},
    listSplitPayments: () => [],
    createSplitRecipient: (r) => r,
    getSplitRecipient: () => null,
    updateSplitRecipient: () => {},
    listSplitRecipients: () => [],
    // Notifications (stubs)
    createNotificationLog: (n) => n,
    getNotificationLog: () => null,
    updateNotificationLog: () => {},
    listNotificationLog: () => [],
    getPendingNotifications: () => [],
    upsertWebhookConfig: (c) => c,
    getWebhookConfig: () => null,
    listWebhookConfigs: () => [],
    // Events (stubs)
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

function createRuntime(overrides = {}) {
  const { commerce } = createMockCommerce();
  return createAgentRuntime({
    name: 'TestAgent',
    walletAddress: WALLET_A,
    signingKey: makeKeys(),
    commerce,
    logger: () => {},
    ...overrides,
  });
}

// ===========================================================================
// Construction
// ===========================================================================

describe('AgentRuntime — Construction', () => {
  it('creates a runtime with required params', () => {
    const rt = createRuntime();
    assert.equal(rt.name, 'TestAgent');
    assert.equal(rt.walletAddress, WALLET_A);
    assert.ok(rt.agentId);
    assert.ok(rt.a2a);
  });

  it('uses custom agentId when provided', () => {
    const rt = createRuntime({ agentId: 'custom-id' });
    assert.equal(rt.agentId, 'custom-id');
  });

  it('updates on-chain accessors and default payment config when settlement is attached later', async () => {
    const rt = createRuntime();
    const settlement = {
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '0.5', balanceSmallest: 50_000_000n, symbol: 'BTC' }),
      getAddress: async () => 'bc1qruntime',
    };

    rt.settlement = settlement;

    assert.deepEqual(rt.getDefaultPaymentConfig(), {
      asset: 'BTC',
      network: 'bitcoin',
    });
    assert.deepEqual(await rt.getOnChainBalance(), {
      balance: '0.5',
      balanceSmallest: 50_000_000n,
      symbol: 'BTC',
    });
    assert.equal(await rt.getChainWalletAddress(), 'bc1qruntime');
  });

  it('supports multiple settlement services and chain-specific accessors', async () => {
    const rt = createRuntime();
    const bitcoinSettlement = {
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '0.5', balanceSmallest: 50_000_000n, symbol: 'BTC' }),
      getAddress: async () => 'bc1qmulti',
    };
    const zcashSettlement = {
      chainId: 'zcash',
      isSimulation: true,
      getBalance: async () => ({ balance: '1.25', balanceSmallest: 125_000_000n, symbol: 'ZEC' }),
      getAddress: async () => 'u1multi',
    };

    rt.setSettlement(bitcoinSettlement);
    rt.setSettlement(zcashSettlement);

    assert.deepEqual(rt.listSettlementChains().sort(), ['bitcoin', 'zcash']);
    assert.deepEqual(rt.getDefaultPaymentConfig(), {
      asset: 'ZEC',
      network: 'zcash',
    });
    assert.deepEqual(await rt.getOnChainBalance('bitcoin'), {
      balance: '0.5',
      balanceSmallest: 50_000_000n,
      symbol: 'BTC',
    });
    assert.equal(await rt.getChainWalletAddress('zcash'), 'u1multi');
  });

  it('throws when walletAddress is missing', () => {
    const { commerce } = createMockCommerce();
    assert.throws(() => createAgentRuntime({ name: 'X', commerce, signingKey: makeKeys() }), {
      message: /walletAddress is required/,
    });
  });

  it('throws when commerce is missing', () => {
    assert.throws(
      () => createAgentRuntime({ name: 'X', walletAddress: WALLET_A, signingKey: makeKeys() }),
      { message: /commerce is required/ },
    );
  });

  it('defaults strategy to AlwaysAccept', () => {
    const rt = createRuntime();
    assert.equal(rt.getStrategy().name, 'always-accept');
  });

  it('uses custom strategy when provided', () => {
    const strategy = createBudgetGatedStrategy({ markup: 1.5 });
    const rt = createRuntime({ strategy });
    assert.equal(rt.getStrategy().name, 'budget-gated');
  });
});

// ===========================================================================
// Budget
// ===========================================================================

describe('AgentRuntime — Budget', () => {
  it('canAfford returns true with default infinite limits', () => {
    const rt = createRuntime();
    assert.ok(rt.canAfford(999999));
  });

  it('respects perTransaction limit', () => {
    const rt = createRuntime({ budget: { perTransaction: 100 } });
    assert.ok(rt.canAfford(100));
    assert.ok(!rt.canAfford(101));
  });

  it('respects daily limit', () => {
    const rt = createRuntime({ budget: { daily: 200 } });
    assert.ok(rt.canAfford(200));
    rt.recordSpend(150);
    assert.ok(rt.canAfford(50));
    assert.ok(!rt.canAfford(51));
  });

  it('respects monthly limit', () => {
    const rt = createRuntime({ budget: { monthly: 500 } });
    rt.recordSpend(400);
    assert.ok(rt.canAfford(100));
    assert.ok(!rt.canAfford(101));
  });

  it('respects startingBalance', () => {
    const rt = createRuntime({ budget: { startingBalance: 300 } });
    assert.ok(rt.canAfford(300));
    assert.ok(!rt.canAfford(301));
    rt.recordSpend(200);
    assert.ok(rt.canAfford(100));
    assert.ok(!rt.canAfford(101));
  });

  it('recordSpend updates daily and monthly totals', () => {
    const rt = createRuntime({ budget: { daily: 1000, monthly: 5000 } });
    rt.recordSpend(100);
    rt.recordSpend(50);
    const b = rt.getBudget();
    assert.equal(b.spentToday, 150);
    assert.equal(b.spentThisMonth, 150);
    assert.equal(b.remainingDaily, 850);
  });

  it('recordSpend stores history with metadata', () => {
    const rt = createRuntime({ budget: { daily: 1000 } });
    rt.recordSpend(75, { type: 'test', vendor: 'acme' });
    const b = rt.getBudget();
    assert.equal(b.spentToday, 75);
  });

  it('emits budget:warning at 80% daily', () => {
    const rt = createRuntime({ budget: { daily: 100 } });
    let warned = false;
    rt.on('budget:warning', (data) => {
      warned = true;
      assert.equal(data.type, 'daily');
      assert.equal(data.limit, 100);
    });
    rt.recordSpend(81);
    assert.ok(warned);
  });

  it('getBudget returns complete budget state', () => {
    const rt = createRuntime({
      budget: { perTransaction: 100, daily: 500, monthly: 2000, startingBalance: 1000 },
    });
    const b = rt.getBudget();
    assert.equal(b.perTransaction, 100);
    assert.equal(b.daily, 500);
    assert.equal(b.monthly, 2000);
    assert.equal(b.balance, 1000);
    assert.equal(b.spentToday, 0);
    assert.equal(b.remainingDaily, 500);
    assert.equal(b.remainingMonthly, 2000);
  });

  it('multiple limits work together', () => {
    const rt = createRuntime({
      budget: { perTransaction: 50, daily: 200, monthly: 500 },
    });
    assert.ok(rt.canAfford(50));
    assert.ok(!rt.canAfford(51));
    rt.recordSpend(50);
    rt.recordSpend(50);
    rt.recordSpend(50);
    assert.ok(rt.canAfford(50));
    rt.recordSpend(50);
    assert.ok(!rt.canAfford(1));
  });

  it('tracks spend independently per payment rail', () => {
    const rt = createRuntime({
      budget: { perTransaction: 1, daily: 1, monthly: 5, startingBalance: 2 },
    });

    rt.recordSpend(0.8, { asset: 'BTC', network: 'bitcoin' });
    assert.ok(!rt.canAfford(0.3, { asset: 'BTC', network: 'bitcoin' }));
    assert.ok(rt.canAfford(0.3, { asset: 'ZEC', network: 'zcash' }));

    rt.recordSpend(0.5, { asset: 'ZEC', network: 'zcash' });

    const mixedBudget = rt.getBudget();
    assert.equal(mixedBudget.aggregateTotalsMeaningful, false);
    assert.equal(mixedBudget.spentToday, null);
    assert.deepEqual(mixedBudget.assets, ['BTC', 'ZEC']);
    assert.ok(Math.abs(mixedBudget.breakdownByAsset.BTC.networks.bitcoin.spentToday - 0.8) < 1e-12);
    assert.ok(Math.abs(mixedBudget.breakdownByAsset.ZEC.networks.zcash.spentToday - 0.5) < 1e-12);

    const btcBudget = rt.getBudget({ asset: 'BTC', network: 'bitcoin' });
    assert.equal(btcBudget.aggregateTotalsMeaningful, true);
    assert.equal(btcBudget.asset, 'BTC');
    assert.equal(btcBudget.network, 'bitcoin');
    assert.ok(Math.abs(btcBudget.spentToday - 0.8) < 1e-12);
    assert.ok(Math.abs(btcBudget.balance - 1.2) < 1e-12);
  });
});

// ===========================================================================
// Strategy
// ===========================================================================

describe('AgentRuntime — Strategy', () => {
  it('evaluateQuote delegates to strategy', () => {
    const rt = createRuntime();
    const decision = rt.evaluateQuote({ total: 100, total_decimal: 100 });
    assert.equal(decision.action, 'accept');
  });

  it('evaluatePaymentRequest delegates to strategy', () => {
    const rt = createRuntime();
    const decision = rt.evaluatePaymentRequest({ amount: 100 });
    assert.equal(decision.action, 'pay');
  });

  it('setStrategy swaps the active strategy', () => {
    const rt = createRuntime();
    assert.equal(rt.getStrategy().name, 'always-accept');
    rt.setStrategy(createBudgetGatedStrategy({ markup: 2.0 }));
    assert.equal(rt.getStrategy().name, 'budget-gated');
  });
});

// ===========================================================================
// Service Management
// ===========================================================================

describe('AgentRuntime — Services', () => {
  it('registers a service in the marketplace', () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'ServiceAgent',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      logger: () => {},
    });

    const svc = rt.registerService({
      name: 'Data Analysis',
      description: 'NLP sentiment models',
      category: 'analytics',
    });

    assert.ok(svc.id);
    assert.equal(mock.services.size, 1);
    const stored = [...mock.services.values()][0];
    assert.equal(stored.name, 'Data Analysis');
    assert.equal(stored.agent_address, WALLET_A);
  });

  it('emits service:registered event', () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'ServiceAgent',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      logger: () => {},
    });

    let emitted = false;
    rt.on('service:registered', () => {
      emitted = true;
    });
    rt.registerService({ name: 'Test', category: 'test' });
    assert.ok(emitted);
  });

  it('listMyServices returns only own services', () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'ServiceAgent',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      logger: () => {},
    });

    rt.registerService({ name: 'Svc1', category: 'analytics' });

    mock.a2aStore.createService({
      id: crypto.randomUUID(),
      agent_address: WALLET_B,
      name: 'Foreign Svc',
      category: 'analytics',
      active: 1,
    });

    const myServices = rt.listMyServices();
    assert.equal(myServices.length, 1);
    assert.equal(myServices[0].name, 'Svc1');
  });

  it('discoverServices finds services by category', () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'Discoverer',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      logger: () => {},
    });

    mock.a2aStore.createService({
      id: crypto.randomUUID(),
      agent_address: WALLET_B,
      name: 'Analytics Svc',
      category: 'analytics',
      active: 1,
    });
    mock.a2aStore.createService({
      id: crypto.randomUUID(),
      agent_address: WALLET_B,
      name: 'Data Svc',
      category: 'data',
      active: 1,
    });

    const found = rt.discoverServices({ category: 'analytics' });
    assert.equal(found.length, 1);
    assert.equal(found[0].name, 'Analytics Svc');
  });

  it('registerService sets correct defaults', () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'DefaultsAgent',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      logger: () => {},
    });

    rt.registerService({ name: 'MinimalService' });
    const stored = [...mock.services.values()][0];
    assert.equal(stored.category, 'other');
    assert.equal(stored.pricing_model, 'quote');
    assert.equal(stored.active, 1);
    assert.equal(stored.transaction_count, 0);
  });
});

// ===========================================================================
// Service Loop (tick)
// ===========================================================================

describe('AgentRuntime — Service Loop', () => {
  it('tick processes pending quote requests as seller', async () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'Seller',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      strategy: createAlwaysAcceptStrategy({ defaultPrice: 25 }),
      logger: () => {},
    });

    const quoteId = crypto.randomUUID();
    mock.a2aStore.createQuote({
      id: quoteId,
      seller_address: WALLET_A,
      buyer_address: WALLET_B,
      status: 'requested',
      items: JSON.stringify([{ description: 'Service', quantity: 1 }]),
    });

    const processed = await rt.tick();
    assert.ok(processed >= 1);

    const updated = mock.quotes.get(quoteId);
    assert.equal(updated.status, 'quoted');
  });

  it('tick emits quote:received for seller', async () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'Seller',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      strategy: createAlwaysAcceptStrategy(),
      logger: () => {},
    });

    mock.a2aStore.createQuote({
      id: crypto.randomUUID(),
      seller_address: WALLET_A,
      buyer_address: WALLET_B,
      status: 'requested',
      items: JSON.stringify([]),
    });

    let received = false;
    rt.on('quote:received', (data) => {
      received = true;
      assert.equal(data.role, 'seller');
    });

    await rt.tick();
    assert.ok(received);
  });

  it('tick emits loop:tick with processed count', async () => {
    const rt = createRuntime();
    let tickEvent = null;
    rt.on('loop:tick', (data) => {
      tickEvent = data;
    });
    await rt.tick();
    assert.ok(tickEvent);
    assert.equal(typeof tickEvent.processed, 'number');
  });

  it('tick does not re-process the same quote', async () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'Seller',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      strategy: createAlwaysAcceptStrategy(),
      logger: () => {},
    });

    mock.a2aStore.createQuote({
      id: 'q-once',
      seller_address: WALLET_A,
      buyer_address: WALLET_B,
      status: 'requested',
      items: JSON.stringify([]),
    });

    const first = await rt.tick();
    assert.ok(first >= 1);

    // Reset status to requested again
    mock.quotes.get('q-once').status = 'requested';
    const second = await rt.tick();
    assert.equal(second, 0);
  });

  it('tick handles errors in strategy gracefully', async () => {
    const mock = createMockCommerce();
    const badStrategy = {
      name: 'buggy',
      evaluateIncomingQuote() {
        throw new Error('Strategy bug');
      },
      evaluateReceivedQuote() {
        return { action: 'accept' };
      },
      evaluateCounterOffer() {
        return { action: 'accept' };
      },
      evaluatePaymentRequest() {
        return { action: 'pay' };
      },
    };

    const rt = createAgentRuntime({
      name: 'Seller',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      strategy: badStrategy,
      logger: () => {},
    });

    mock.a2aStore.createQuote({
      id: 'q-err',
      seller_address: WALLET_A,
      buyer_address: WALLET_B,
      status: 'requested',
      items: JSON.stringify([]),
    });

    let errorEmitted = false;
    rt.on('loop:error', (data) => {
      errorEmitted = true;
      assert.equal(data.context, 'provideQuote');
    });

    await rt.tick();
    assert.ok(errorEmitted);
  });

  it('tick auto-fulfills accepted quotes as seller', async () => {
    const mock = createMockCommerce();
    const rt = createAgentRuntime({
      name: 'Seller',
      walletAddress: WALLET_A,
      signingKey: makeKeys(),
      commerce: mock.commerce,
      strategy: createAlwaysAcceptStrategy(),
      logger: () => {},
    });

    const quoteId = crypto.randomUUID();
    mock.a2aStore.createQuote({
      id: quoteId,
      seller_address: WALLET_A,
      buyer_address: WALLET_B,
      status: 'accepted',
      total_decimal: 50,
      items: JSON.stringify([]),
    });

    let fulfilled = false;
    rt.on('service:fulfilled', (data) => {
      fulfilled = true;
      assert.equal(data.quoteId, quoteId);
    });

    await rt.tick();
    assert.ok(fulfilled);
    assert.equal(mock.quotes.get(quoteId).status, 'fulfilled');
  });

  it('tick returns 0 when nothing to process', async () => {
    const rt = createRuntime();
    const processed = await rt.tick();
    assert.equal(processed, 0);
  });

  it('tick silently skips RFQ expiry when the store does not implement RFQ methods', async () => {
    const rt = createRuntime();
    const originalDebug = console.debug;
    const debugCalls = [];
    console.debug = (...args) => {
      debugCalls.push(args);
    };

    try {
      await rt.tick();
    } finally {
      console.debug = originalDebug;
    }

    assert.deepEqual(debugCalls, []);
  });
});

// ===========================================================================
// Start / Stop / Lifecycle
// ===========================================================================

describe('AgentRuntime — Lifecycle', () => {
  let rt;

  afterEach(() => {
    if (rt) rt.destroy();
  });

  it('starts and stops the service loop', () => {
    rt = createRuntime({ pollIntervalMs: 100 });
    assert.equal(rt.isRunning(), false);
    rt.start();
    assert.equal(rt.isRunning(), true);
    rt.stop();
    assert.equal(rt.isRunning(), false);
  });

  it('start is idempotent', () => {
    rt = createRuntime({ pollIntervalMs: 100 });
    rt.start();
    rt.start();
    assert.equal(rt.isRunning(), true);
    rt.stop();
  });

  it('stop is idempotent', () => {
    rt = createRuntime({ pollIntervalMs: 100 });
    rt.stop();
    assert.equal(rt.isRunning(), false);
  });

  it('destroy stops loop and clears listeners', () => {
    rt = createRuntime({ pollIntervalMs: 100 });
    let tickCount = 0;
    rt.on('loop:tick', () => {
      tickCount++;
    });
    rt.start();
    rt.destroy();
    assert.equal(rt.isRunning(), false);
    rt.emit('loop:tick', {});
    assert.equal(tickCount, 0);
    rt = null;
  });
});

// ===========================================================================
// Event Emission
// ===========================================================================

describe('AgentRuntime — Events', () => {
  it('on/off work correctly', () => {
    const rt = createRuntime();
    const events = [];
    const handler = (data) => events.push(data);

    rt.on('test:event', handler);
    rt.emit('test:event', 'first');
    rt.emit('test:event', 'second');
    rt.off('test:event', handler);
    rt.emit('test:event', 'third');

    assert.equal(events.length, 2);
    assert.equal(events[0], 'first');
    assert.equal(events[1], 'second');
    rt.destroy();
  });

  it('once fires only once', () => {
    const rt = createRuntime();
    let count = 0;
    rt.once('single', () => {
      count++;
    });
    rt.emit('single');
    rt.emit('single');
    assert.equal(count, 1);
    rt.destroy();
  });
});

// ===========================================================================
// makeCommerceProxy
// ===========================================================================

describe('makeCommerceProxy', () => {
  it('creates a proxy with a2a() and x402() methods', () => {
    const fakeStore = {
      createPayment: () => {},
      getPayment: () => null,
      updatePayment: () => {},
      listPayments: () => [],
      sumPayments: () => ({ total: 0 }),
      createPaymentRequest: () => {},
      getPaymentRequest: () => null,
      updatePaymentRequest: () => {},
      listPaymentRequests: () => [],
      createQuote: () => {},
      getQuote: () => null,
      updateQuote: () => {},
      listQuotes: () => [],
      createEscrow: () => {},
      getEscrow: () => null,
      updateEscrow: () => {},
      listEscrows: () => [],
      createFeedback: () => {},
      getFeedback: () => null,
      updateFeedback: () => {},
      listFeedback: () => [],
      getReputationScore: () => null,
      upsertReputationScore: () => {},
      createService: () => {},
      getService: () => null,
      updateService: () => {},
      listServices: () => [],
      createDispute: () => {},
      getDispute: () => null,
      updateDispute: () => {},
      listDisputes: () => [],
      createEvidence: () => {},
      getEvidence: () => null,
      listEvidenceByDispute: () => [],
      createSubscription: () => {},
      getSubscription: () => null,
      updateSubscription: () => {},
      listSubscriptions: () => [],
      getDueSubscriptions: () => [],
      getExpiredTrials: () => [],
      createSplitPayment: () => {},
      getSplitPayment: () => null,
      updateSplitPayment: () => {},
      listSplitPayments: () => [],
      createSplitRecipient: () => {},
      getSplitRecipient: () => null,
      updateSplitRecipient: () => {},
      listSplitRecipients: () => [],
      createNotificationLog: () => {},
      getNotificationLog: () => null,
      updateNotificationLog: () => {},
      listNotificationLog: () => [],
      getPendingNotifications: () => [],
      upsertWebhookConfig: () => {},
      getWebhookConfig: () => null,
      listWebhookConfigs: () => [],
      createEventSubscription: () => {},
      getEventSubscription: () => null,
      updateEventSubscription: () => {},
      listEventSubscriptions: () => [],
      createEventLog: () => {},
      getEventLog: () => null,
      listEventLog: () => [],
    };

    const proxy = makeCommerceProxy(fakeStore);
    assert.equal(typeof proxy.a2a, 'function');
    assert.equal(typeof proxy.x402, 'function');

    const a2a = proxy.a2a();
    assert.equal(typeof a2a.createQuote, 'function');
    assert.equal(typeof a2a.getQuote, 'function');
    assert.equal(typeof a2a.createPayment, 'function');
    assert.equal(typeof a2a.createService, 'function');
    assert.equal(typeof a2a.createFeedback, 'function');
    assert.equal(typeof a2a.createEscrow, 'function');
    assert.equal(typeof a2a.createSubscription, 'function');
    assert.equal(typeof a2a.createSplitPayment, 'function');
    assert.equal(typeof a2a.createNotificationLog, 'function');
    assert.equal(typeof a2a.createEventSubscription, 'function');
  });

  it('x402 stub returns null for agent lookups', () => {
    const proxy = makeCommerceProxy({});
    const x402 = proxy.x402();
    assert.equal(x402.getAgent(), null);
    assert.equal(x402.getAgentByWallet(), null);
  });
});
