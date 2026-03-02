/**
 * Integration tests for A2A Supply Chain — multi-agent pipeline
 *
 * Tests the end-to-end flow of agents composing into supply chains:
 *   - 4 agents register services, discover each other, negotiate, pay, pass work
 *   - Budget enforcement across a pipeline
 *   - Value flow verification (revenue, cost, margin per hop)
 *   - Uses real A2AStore with temp SQLite DB
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const a2aDir = path.join(__dirname, '..', '..', 'src', 'a2a');

const { A2AStore } = await import(path.join(a2aDir, 'store.js'));
const { createAgentRuntime, makeCommerceProxy } = await import(
  path.join(a2aDir, 'agent-runtime.js')
);
const { createBudgetGatedStrategy, createAlwaysAcceptStrategy } = await import(
  path.join(a2aDir, 'strategies.js')
);
const { createA2AService } = await import(path.join(a2aDir, 'index.js'));

// ===========================================================================
// Helpers
// ===========================================================================

const wallet = () => '0x' + crypto.randomBytes(20).toString('hex');
const keys = () => ({
  privateKey: crypto.randomBytes(32).toString('hex'),
  publicKey: crypto.randomBytes(32).toString('hex'),
});

let dbPath;
let store;
let commerce;

function setup() {
  dbPath = path.join(__dirname, `.test-supply-chain-${Date.now()}.db`);
  store = new A2AStore({ dbPath });
  store.init();
  commerce = makeCommerceProxy(store);
}

function teardown() {
  try { store.close(); } catch { /* ignore */ }
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }
}

// ===========================================================================
// Full Pipeline Test
// ===========================================================================

describe('Supply Chain — Full Pipeline', () => {
  beforeEach(setup);
  afterEach(teardown);

  it('4 agents form a pipeline: register → discover → negotiate → pay → fulfill', async () => {
    // Define agents
    const agents = [
      {
        name: 'DataCollector',
        wallet: wallet(),
        keys: keys(),
        service: { name: 'Data Collection', category: 'data-collection', pricingModel: 'quote' },
        nextCategory: 'data-cleaning',
        strategy: createBudgetGatedStrategy({ markup: 1.6, basePrice: 30 }),
      },
      {
        name: 'DataCleaner',
        wallet: wallet(),
        keys: keys(),
        service: { name: 'Data Cleaning', category: 'data-cleaning', pricingModel: 'quote' },
        nextCategory: 'analysis',
        strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 20 }),
      },
      {
        name: 'Analyst',
        wallet: wallet(),
        keys: keys(),
        service: { name: 'Analysis', category: 'analysis', pricingModel: 'quote' },
        nextCategory: 'reporting',
        strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 15 }),
      },
      {
        name: 'ReportWriter',
        wallet: wallet(),
        keys: keys(),
        service: { name: 'Report Writing', category: 'reporting', pricingModel: 'quote' },
        nextCategory: null,
        strategy: createBudgetGatedStrategy({ markup: 1.3, basePrice: 10 }),
      },
    ];

    // Create runtimes
    const runtimes = agents.map((a) =>
      createAgentRuntime({
        name: a.name,
        walletAddress: a.wallet,
        signingKey: a.keys,
        commerce,
        budget: { daily: 500, perTransaction: 200 },
        strategy: a.strategy,
        logger: () => {},
      })
    );

    // Step 1: Register services
    for (let i = 0; i < agents.length; i++) {
      runtimes[i].registerService(agents[i].service);
    }

    // Verify all services registered
    const allServices = store.listServices({});
    assert.equal(allServices.length, 4);

    // Step 2: Buyer initiates — request quote from DataCollector
    const buyerWallet = wallet();
    const buyerA2A = createA2AService(commerce, {
      agentId: crypto.randomUUID(),
      walletAddress: buyerWallet,
      signingKey: keys(),
    });

    // Discover first service
    const dcServices = runtimes[0].discoverServices({ category: 'data-collection' });
    assert.ok(dcServices.length >= 1);

    // Request quote (no unitPrice — let seller strategy price it)
    const quote0 = await buyerA2A.requestQuote({
      seller: agents[0].wallet,
      items: [{ description: 'Social media data collection', quantity: 1 }],
      message: 'Need 7-day sentiment data.',
    });
    assert.ok(quote0.quote.id);

    // DataCollector prices the quote
    await runtimes[0].tick();
    const priced0 = store.getQuote(quote0.quote.id);
    assert.equal(priced0.status, 'quoted');
    assert.ok(priced0.total_decimal > 0);

    // Buyer accepts and pays
    const acceptance = await buyerA2A.acceptQuote(quote0.quote.id);
    assert.ok(acceptance.payment);

    // DataCollector auto-fulfills
    await runtimes[0].tick();
    const fulfilled0 = store.getQuote(quote0.quote.id);
    assert.equal(fulfilled0.status, 'fulfilled');

    // Step 3-5: Each agent discovers next, negotiates, pays
    const payments = [{ from: 'Buyer', to: agents[0].name, amount: acceptance.payment.amount }];

    for (let i = 0; i < agents.length - 1; i++) {
      const current = agents[i];
      const next = agents[i + 1];

      // Current discovers next service
      const nextServices = runtimes[i].discoverServices({ category: current.nextCategory });
      assert.ok(nextServices.length >= 1, `${current.name} should find ${current.nextCategory} service`);

      // Request quote from next agent
      const quote = await runtimes[i].a2a.requestQuote({
        seller: next.wallet,
        items: [{ description: `Process output from ${current.name}`, quantity: 1 }],
        message: `Passing ${current.service.category} output.`,
      });

      // Next agent prices the quote
      await runtimes[i + 1].tick();
      const priced = store.getQuote(quote.quote.id);
      assert.equal(priced.status, 'quoted');

      // Current agent accepts and pays
      const payment = await runtimes[i].a2a.acceptQuote(quote.quote.id);
      runtimes[i].recordSpend(payment.payment.amount, { to: next.name });

      payments.push({
        from: current.name,
        to: next.name,
        amount: payment.payment.amount,
      });

      // Next agent fulfills
      await runtimes[i + 1].tick();
      const done = store.getQuote(quote.quote.id);
      assert.equal(done.status, 'fulfilled');
    }

    // Verify the full chain executed
    assert.equal(payments.length, 4); // Buyer→DC, DC→Cleaner, Cleaner→Analyst, Analyst→Writer

    // Each downstream payment should be less than the upstream payment
    // (because each agent keeps a margin)
    for (let i = 1; i < payments.length; i++) {
      assert.ok(
        payments[i].amount <= payments[i - 1].amount,
        `${payments[i].from} should pay less than what it received`
      );
    }
  });

  it('pipeline halts when an agent cannot afford the next hop', async () => {
    const sellerWallet = wallet();
    const buyerWalletAddr = wallet();

    // Seller with very tight budget
    const seller = createAgentRuntime({
      name: 'TightBudgetSeller',
      walletAddress: sellerWallet,
      signingKey: keys(),
      commerce,
      budget: { daily: 10, perTransaction: 5 }, // very tight
      strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 20 }),
      logger: () => {},
    });

    seller.registerService({
      name: 'Expensive Service',
      category: 'expensive',
      pricingModel: 'quote',
    });

    // Create a downstream service
    const downstreamWallet = wallet();
    const downstream = createAgentRuntime({
      name: 'Downstream',
      walletAddress: downstreamWallet,
      signingKey: keys(),
      commerce,
      budget: { daily: 500 },
      strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 50 }),
      logger: () => {},
    });

    downstream.registerService({
      name: 'Downstream Svc',
      category: 'downstream',
      pricingModel: 'quote',
    });

    // Seller tries to buy downstream — but can't afford it
    const quote = await seller.a2a.requestQuote({
      seller: downstreamWallet,
      items: [{ description: 'Process data', quantity: 1 }],
    });

    await downstream.tick(); // Downstream prices at 50 * 1.5 = $75

    // Seller's budget is $5 per-txn — way too low
    assert.ok(!seller.canAfford(75));

    // The seller's strategy should decline when evaluated
    const priced = store.getQuote(quote.quote.id);
    const decision = seller.evaluateQuote(priced);
    assert.equal(decision.action, 'decline');
  });
});

// ===========================================================================
// Service Discovery
// ===========================================================================

describe('Supply Chain — Service Discovery', () => {
  beforeEach(setup);
  afterEach(teardown);

  it('agents discover services by category', () => {
    const rt1 = createAgentRuntime({
      name: 'Agent1',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      logger: () => {},
    });

    const rt2 = createAgentRuntime({
      name: 'Agent2',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      logger: () => {},
    });

    rt1.registerService({ name: 'Analytics', category: 'analytics' });
    rt2.registerService({ name: 'Reporting', category: 'reporting' });
    rt2.registerService({ name: 'Analytics Pro', category: 'analytics' });

    const analytics = rt1.discoverServices({ category: 'analytics' });
    assert.equal(analytics.length, 2);

    const reporting = rt1.discoverServices({ category: 'reporting' });
    assert.equal(reporting.length, 1);
  });

  it('empty category returns no results', () => {
    const rt = createAgentRuntime({
      name: 'Searcher',
      walletAddress: wallet(),
      signingKey: keys(),
      commerce,
      logger: () => {},
    });

    const found = rt.discoverServices({ category: 'nonexistent' });
    assert.equal(found.length, 0);
  });
});

// ===========================================================================
// Value Flow Verification
// ===========================================================================

describe('Supply Chain — Value Flow', () => {
  beforeEach(setup);
  afterEach(teardown);

  it('each agent in pipeline earns margin', async () => {
    // Simple 2-agent chain: Collector → Writer
    const collectorWallet = wallet();
    const writerWallet = wallet();

    const collector = createAgentRuntime({
      name: 'Collector',
      walletAddress: collectorWallet,
      signingKey: keys(),
      commerce,
      budget: { daily: 500, perTransaction: 200 },
      strategy: createBudgetGatedStrategy({ markup: 2.0, basePrice: 25 }),
      logger: () => {},
    });

    const writer = createAgentRuntime({
      name: 'Writer',
      walletAddress: writerWallet,
      signingKey: keys(),
      commerce,
      budget: { daily: 500, perTransaction: 200 },
      strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 15 }),
      logger: () => {},
    });

    collector.registerService({ name: 'Collection', category: 'collection' });
    writer.registerService({ name: 'Writing', category: 'writing' });

    // Buyer pays Collector
    const buyerA2A = createA2AService(commerce, {
      agentId: crypto.randomUUID(),
      walletAddress: wallet(),
      signingKey: keys(),
    });

    const q1 = await buyerA2A.requestQuote({
      seller: collectorWallet,
      items: [{ description: 'Collect data', quantity: 1 }],
    });
    await collector.tick(); // prices at 25 * 2.0 = $50

    const priced1 = store.getQuote(q1.quote.id);
    const buyerPaid = priced1.total_decimal;
    assert.equal(buyerPaid, 50);

    const pay1 = await buyerA2A.acceptQuote(q1.quote.id);
    await collector.tick(); // fulfills

    // Collector pays Writer
    const q2 = await collector.a2a.requestQuote({
      seller: writerWallet,
      items: [{ description: 'Write report', quantity: 1 }],
    });
    await writer.tick(); // prices at 15 * 1.5 = $22.5

    const priced2 = store.getQuote(q2.quote.id);
    const collectorPaid = priced2.total_decimal;
    assert.equal(collectorPaid, 22.5);

    await collector.a2a.acceptQuote(q2.quote.id);
    collector.recordSpend(collectorPaid, { to: 'Writer' });
    await writer.tick(); // fulfills

    // Verify margins
    const collectorMargin = buyerPaid - collectorPaid;
    assert.ok(collectorMargin > 0, 'Collector should earn positive margin');
    assert.equal(collectorMargin, 27.5); // 50 - 22.5

    // Writer gets full payment as margin (no downstream)
    assert.equal(collectorPaid, 22.5);
  });

  it('budget tracking reflects spend across pipeline', async () => {
    const agentWallet = wallet();
    const sellerWallet = wallet();

    const agent = createAgentRuntime({
      name: 'BudgetAgent',
      walletAddress: agentWallet,
      signingKey: keys(),
      commerce,
      budget: { daily: 100, perTransaction: 60 },
      strategy: createAlwaysAcceptStrategy(),
      logger: () => {},
    });

    const seller = createAgentRuntime({
      name: 'Seller',
      walletAddress: sellerWallet,
      signingKey: keys(),
      commerce,
      strategy: createBudgetGatedStrategy({ markup: 1.0, basePrice: 30 }),
      logger: () => {},
    });

    seller.registerService({ name: 'Svc', category: 'svc' });

    // First purchase: $30
    const q1 = await agent.a2a.requestQuote({
      seller: sellerWallet,
      items: [{ description: 'Service', quantity: 1 }],
    });
    await seller.tick();
    await agent.a2a.acceptQuote(q1.quote.id);
    agent.recordSpend(30, { quoteId: q1.quote.id });

    assert.equal(agent.getBudget().spentToday, 30);
    assert.equal(agent.getBudget().remainingDaily, 70);

    // Second purchase: $30
    const q2 = await agent.a2a.requestQuote({
      seller: sellerWallet,
      items: [{ description: 'Service 2', quantity: 1 }],
    });
    await seller.tick();
    await agent.a2a.acceptQuote(q2.quote.id);
    agent.recordSpend(30, { quoteId: q2.quote.id });

    assert.equal(agent.getBudget().spentToday, 60);
    assert.equal(agent.getBudget().remainingDaily, 40);

    // Third purchase would exceed daily if > $40
    assert.ok(agent.canAfford(40));
    assert.ok(!agent.canAfford(41));
  });
});

// ===========================================================================
// Edge Cases
// ===========================================================================

describe('Supply Chain — Edge Cases', () => {
  beforeEach(setup);
  afterEach(teardown);

  it('agent can register multiple services', () => {
    const agentWallet = wallet();
    const rt = createAgentRuntime({
      name: 'MultiService',
      walletAddress: agentWallet,
      signingKey: keys(),
      commerce,
      logger: () => {},
    });

    rt.registerService({ name: 'Svc1', category: 'cat-a' });
    rt.registerService({ name: 'Svc2', category: 'cat-b' });
    rt.registerService({ name: 'Svc3', category: 'cat-a' });

    const myServices = rt.listMyServices();
    assert.equal(myServices.length, 3);

    const catA = rt.discoverServices({ category: 'cat-a' });
    assert.equal(catA.length, 2);
  });

  it('empty pipeline — single agent with no downstream', async () => {
    const agentWallet = wallet();
    const rt = createAgentRuntime({
      name: 'Solo',
      walletAddress: agentWallet,
      signingKey: keys(),
      commerce,
      strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 20 }),
      logger: () => {},
    });

    rt.registerService({ name: 'Solo Service', category: 'solo' });

    // Buyer requests and pays
    const buyerA2A = createA2AService(commerce, {
      agentId: crypto.randomUUID(),
      walletAddress: wallet(),
      signingKey: keys(),
    });

    const q = await buyerA2A.requestQuote({
      seller: agentWallet,
      items: [{ description: 'Solo work', quantity: 1 }],
    });
    await rt.tick(); // price
    const priced = store.getQuote(q.quote.id);
    assert.equal(priced.total_decimal, 30); // 20 * 1.5

    await buyerA2A.acceptQuote(q.quote.id);
    await rt.tick(); // fulfill

    const done = store.getQuote(q.quote.id);
    assert.equal(done.status, 'fulfilled');
  });

  it('concurrent quotes from multiple buyers', async () => {
    const sellerWallet = wallet();
    const seller = createAgentRuntime({
      name: 'PopularSeller',
      walletAddress: sellerWallet,
      signingKey: keys(),
      commerce,
      strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 10 }),
      logger: () => {},
    });

    seller.registerService({ name: 'Popular Svc', category: 'popular' });

    // Two buyers request quotes simultaneously
    const buyer1 = createA2AService(commerce, {
      agentId: crypto.randomUUID(),
      walletAddress: wallet(),
      signingKey: keys(),
    });
    const buyer2 = createA2AService(commerce, {
      agentId: crypto.randomUUID(),
      walletAddress: wallet(),
      signingKey: keys(),
    });

    const q1 = await buyer1.requestQuote({
      seller: sellerWallet,
      items: [{ description: 'Service for buyer 1', quantity: 1 }],
    });
    const q2 = await buyer2.requestQuote({
      seller: sellerWallet,
      items: [{ description: 'Service for buyer 2', quantity: 1 }],
    });

    // Seller processes both in one tick
    await seller.tick();

    const priced1 = store.getQuote(q1.quote.id);
    const priced2 = store.getQuote(q2.quote.id);
    assert.equal(priced1.status, 'quoted');
    assert.equal(priced2.status, 'quoted');
    assert.equal(priced1.total_decimal, 15); // 10 * 1.5
    assert.equal(priced2.total_decimal, 15);
  });
});
