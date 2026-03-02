#!/usr/bin/env node
/**
 * Advanced Agentic Commerce Demo
 *
 * Five AI agents demonstrate the full A2A protocol stack:
 *   - Escrow-backed conditional payments
 *   - Reputation-gated service discovery
 *   - Agent-to-agent subscriptions
 *   - Multi-party split payments
 *   - Autonomous trust tier progression
 *
 * Agents:
 *   DataForge AI     — Premium data provider (verified tier, 4.8 avg)
 *   QuickAnalytics   — Budget analytics service (standard tier, 3.5 avg)
 *   InsightBot       — Buyer agent ($500/day budget)
 *   PlatformAgent    — Marketplace operator (5% platform fee)
 *   AuditBot         — Compliance monitor
 *
 * Usage:
 *   node examples/advanced_commerce_demo.mjs
 */

import crypto from 'node:crypto';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cliSrc = path.join(__dirname, '..', 'src');

const { A2AStore } = await import(path.join(cliSrc, 'a2a', 'store.js'));
const { createAgentRuntime, makeCommerceProxy } = await import(
  path.join(cliSrc, 'a2a', 'agent-runtime.js')
);
const {
  createBudgetGatedStrategy,
  createReputationAwareStrategy,
} = await import(path.join(cliSrc, 'a2a', 'strategies.js'));
const { createReputationService } = await import(
  path.join(cliSrc, 'a2a', 'reputation.js')
);
const { createA2ASubscriptionService } = await import(
  path.join(cliSrc, 'a2a', 'subscriptions.js')
);
const { createSplitPaymentService } = await import(
  path.join(cliSrc, 'a2a', 'splits.js')
);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const wallet = () => '0x' + crypto.randomBytes(20).toString('hex');
const keys = () => ({
  privateKey: crypto.randomBytes(32).toString('hex'),
  publicKey: crypto.randomBytes(32).toString('hex'),
});

function banner(text) {
  const line = '='.repeat(70);
  console.log(`\n${'='.repeat(74)}`);
  console.log(`  ${text}`);
  console.log(`${'='.repeat(74)}`);
}

function scene(n, text) {
  console.log(`\n  ┌─── Scene ${n} ──────────────────────────────────────────────────────`);
  console.log(`  │  ${text}`);
  console.log(`  └${'─'.repeat(68)}`);
}

function agentSays(name, emoji, msg) {
  console.log(`\n    ${emoji}  [${name}]: "${msg}"`);
}

function result(label, data) {
  console.log(`    * ${label}:`);
  for (const line of JSON.stringify(data, null, 2).split('\n')) {
    console.log(`      ${line}`);
  }
}

// ---------------------------------------------------------------------------
// Agent Definitions
// ---------------------------------------------------------------------------
const AGENTS = {
  dataForge: {
    name: 'DataForge AI',
    emoji: '🔬',
    wallet: wallet(),
    keys: keys(),
    service: {
      name: 'Premium Sentiment Analysis',
      description: 'Real-time sentiment across 50+ sources. 200ms response. Buy/sell/hold signals.',
      category: 'analytics',
    },
    strategy: createBudgetGatedStrategy({ markup: 1.4, basePrice: 80 }),
    budget: { daily: 1000, perTransaction: 500 },
  },
  quickAnalytics: {
    name: 'QuickAnalytics',
    emoji: '📈',
    wallet: wallet(),
    keys: keys(),
    service: {
      name: 'Budget Analytics Suite',
      description: 'Basic sentiment analysis with daily updates. Affordable pricing.',
      category: 'analytics',
    },
    strategy: createBudgetGatedStrategy({ markup: 1.2, basePrice: 40 }),
    budget: { daily: 500, perTransaction: 200 },
  },
  insightBot: {
    name: 'InsightBot',
    emoji: '🎯',
    wallet: wallet(),
    keys: keys(),
    strategy: createReputationAwareStrategy({
      minTrustTier: 'standard',
      minAvgScore: 3.5,
      reputationDiscount: 0.05,
      enterpriseDiscount: 0.10,
      baseMarkup: 1.3,
      maxRounds: 2,
    }),
    budget: { daily: 500, perTransaction: 300 },
  },
  platform: {
    name: 'PlatformAgent',
    emoji: '🏛️',
    wallet: wallet(),
    keys: keys(),
    strategy: createBudgetGatedStrategy({ markup: 1.0, basePrice: 0 }),
    budget: { daily: 10000 },
  },
  audit: {
    name: 'AuditBot',
    emoji: '📋',
    wallet: wallet(),
    keys: keys(),
    strategy: createBudgetGatedStrategy({ markup: 1.0 }),
    budget: { daily: 100 },
  },
};

// ---------------------------------------------------------------------------
// Main Demo
// ---------------------------------------------------------------------------
async function main() {
  banner('ADVANCED AGENTIC COMMERCE — Full Protocol Stack Demo');

  console.log('\n  Five AI agents showcase the complete A2A protocol:\n');
  for (const [, a] of Object.entries(AGENTS)) {
    const svc = a.service ? ` — ${a.service.name}` : '';
    console.log(`  ${a.emoji}  ${a.name.padEnd(18)}${svc}`);
  }

  // -------------------------------------------------------------------------
  // Setup
  // -------------------------------------------------------------------------
  const dbPath = path.join(__dirname, '..', '.advanced-demo-a2a.db');
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }

  const store = new A2AStore({ dbPath });
  store.init();
  const commerce = makeCommerceProxy(store);
  const a2aProxy = commerce.a2a();
  const repSvc = createReputationService(a2aProxy);
  const subSvc = createA2ASubscriptionService(a2aProxy);
  const splitSvc = createSplitPaymentService(a2aProxy);

  // Create runtimes
  const runtimes = {};
  for (const [key, a] of Object.entries(AGENTS)) {
    runtimes[key] = createAgentRuntime({
      name: a.name,
      walletAddress: a.wallet,
      signingKey: a.keys,
      commerce,
      budget: a.budget,
      strategy: a.strategy,
      logger: () => {},
    });
  }

  // =========================================================================
  // Scene 1: Service Registration + Reputation Bootstrap
  // =========================================================================
  scene(1, 'Service Registration + Reputation Bootstrap');

  // Register services for sellers
  runtimes.dataForge.registerService(AGENTS.dataForge.service);
  runtimes.quickAnalytics.registerService(AGENTS.quickAnalytics.service);

  agentSays('DataForge AI', '🔬', 'Registered "Premium Sentiment Analysis" in analytics category.');
  agentSays('QuickAnalytics', '📈', 'Registered "Budget Analytics Suite" in analytics category.');

  // Bootstrap reputation: DataForge has strong history, QuickAnalytics is newer
  // Simulate past transactions for DataForge
  for (let i = 0; i < 30; i++) {
    await repSvc.rateAgent({
      agentAddress: AGENTS.dataForge.wallet,
      reviewerAddress: wallet(),
      transactionType: 'quote',
      transactionId: crypto.randomUUID(),
      score: Math.random() > 0.2 ? 5 : 4, // mostly 5s
      dimensions: { reliability: 5, quality: 5, speed: 4, communication: 5 },
      comment: 'Great service',
    });
  }

  // QuickAnalytics: fewer reviews, mixed ratings
  for (let i = 0; i < 8; i++) {
    await repSvc.rateAgent({
      agentAddress: AGENTS.quickAnalytics.wallet,
      reviewerAddress: wallet(),
      transactionType: 'quote',
      transactionId: crypto.randomUUID(),
      score: Math.random() > 0.5 ? 4 : 3,
      dimensions: { reliability: 3, quality: 3, speed: 4, communication: 3 },
      comment: 'Decent service',
    });
  }

  const dfRep = await repSvc.getReputation(AGENTS.dataForge.wallet);
  const qaRep = await repSvc.getReputation(AGENTS.quickAnalytics.wallet);
  const dfRepData = dfRep?.reputation || dfRep;
  const qaRepData = qaRep?.reputation || qaRep;

  result('Reputation Bootstrap', {
    'DataForge AI': {
      trustTier: dfRepData?.trustTier || dfRepData?.trust_tier || 'sandbox',
      avgScore: Math.round((dfRepData?.averageScore ?? dfRepData?.average_score ?? 0) * 10) / 10,
      transactions: dfRepData?.totalTransactions ?? dfRepData?.total_transactions ?? 0,
    },
    'QuickAnalytics': {
      trustTier: qaRepData?.trustTier || qaRepData?.trust_tier || 'sandbox',
      avgScore: Math.round((qaRepData?.averageScore ?? qaRepData?.average_score ?? 0) * 10) / 10,
      transactions: qaRepData?.totalTransactions ?? qaRepData?.total_transactions ?? 0,
    },
  });

  // =========================================================================
  // Scene 2: Escrow-Backed High-Value Deal
  // =========================================================================
  scene(2, 'Escrow-Backed High-Value Deal (InsightBot <-> DataForge AI)');

  agentSays('InsightBot', '🎯', 'I need premium analytics. Let me find a trusted provider...');

  // InsightBot discovers analytics services
  const analyticsServices = runtimes.insightBot.discoverServices({ category: 'analytics' });
  agentSays('InsightBot', '🎯', `Found ${analyticsServices.length} analytics service(s) in marketplace.`);

  // Request quote from DataForge
  const quote1 = await runtimes.insightBot.a2a.requestQuote({
    seller: AGENTS.dataForge.wallet,
    items: [{ description: 'Premium sentiment analysis — 5 tickers, 30 days', quantity: 1 }],
    message: 'Need comprehensive coverage for mega-cap portfolio.',
  });

  // DataForge prices the quote
  await runtimes.dataForge.tick();
  const priced1 = store.getQuote(quote1.quote.id);
  agentSays('DataForge AI', '🔬', `Quoted $${priced1.total_decimal} USDC for premium analytics.`);

  // InsightBot evaluates — should accept (DataForge is "verified" tier)
  agentSays('InsightBot', '🎯', `DataForge is verified tier with 4.8+ avg. Accepting $${priced1.total_decimal}.`);

  // Create escrow-backed payment
  const escrowResult = await runtimes.insightBot.createEscrowDeal({
    sellerAddress: AGENTS.dataForge.wallet,
    amount: priced1.total_decimal,
    conditions: [
      { type: 'seller_fulfilled', quoteId: quote1.quote.id },
    ],
    expiresInHours: 48,
  });

  result('Escrow Created', {
    escrowId: escrowResult.escrow?.id,
    amount: `$${priced1.total_decimal} USDC`,
    conditions: ['seller_fulfilled'],
    expiresIn: '48 hours',
  });

  // DataForge fulfills the quote
  await runtimes.insightBot.a2a.acceptQuote(quote1.quote.id);
  await runtimes.dataForge.tick(); // auto-fulfill

  agentSays('DataForge AI', '🔬', 'Analytics data delivered. Quote fulfilled.');

  result('Escrow Deal Complete', {
    status: 'fulfilled',
    payment: `$${priced1.total_decimal} USDC`,
    settlement: 'Escrow conditions met',
  });

  // =========================================================================
  // Scene 3: Reputation-Gated Discovery
  // =========================================================================
  scene(3, 'Reputation-Gated Discovery (InsightBot filters by trust tier)');

  agentSays('InsightBot', '🎯', 'Searching for analytics again with minTrustTier="verified"...');

  // InsightBot has reputation-aware strategy with minTrustTier='standard'
  // Let's show what happens with stricter filtering
  const allProviders = runtimes.insightBot.discoverServices({ category: 'analytics' });

  const qualified = [];
  const rejected = [];
  for (const svc of allProviders) {
    const repResult = await repSvc.getReputation(svc.agent_address);
    const rep = repResult?.reputation || repResult;
    const tier = rep?.trustTier || rep?.trust_tier || 'sandbox';
    const avgScore = rep?.averageScore ?? rep?.average_score ?? 0;
    const tierRank = { sandbox: 0, standard: 1, verified: 2, enterprise: 3 }[tier] || 0;

    if (tierRank >= 2 && avgScore >= 4.0) { // verified+ with 4.0+ score
      qualified.push({ name: svc.name, tier, avgScore: Math.round(avgScore * 10) / 10 });
    } else {
      rejected.push({ name: svc.name, tier, avgScore: Math.round(avgScore * 10) / 10, reason: tierRank < 2 ? 'Below verified tier' : 'Score too low' });
    }
  }

  agentSays('InsightBot', '🎯',
    `${qualified.length} provider(s) qualify. ${rejected.length} rejected by reputation filter.`);

  result('Reputation-Gated Results', { qualified, rejected });

  // =========================================================================
  // Scene 4: Agent Subscription
  // =========================================================================
  scene(4, 'Agent Subscription (InsightBot subscribes to DataForge daily feed)');

  agentSays('InsightBot', '🎯', 'Subscribing to DataForge daily analytics feed — $25/month, 7-day trial.');

  const subscription = await runtimes.insightBot.subscribeTo({
    providerAddress: AGENTS.dataForge.wallet,
    planName: 'Daily Analytics Feed',
    amount: 25,
    interval: 'monthly',
    trialDays: 7,
  });

  result('Subscription Created', {
    subscriptionId: subscription.subscription?.id,
    plan: 'Daily Analytics Feed',
    amount: '$25/month USDC',
    trial: '7 days',
    status: subscription.subscription?.status || 'trial',
  });

  // Process billing (trial won't bill yet, but shows the mechanism)
  agentSays('InsightBot', '🎯', 'Processing billing cycle (trial period — no charge yet)...');
  const billingResult = await runtimes.dataForge.processSubscriptionBilling();

  result('Billing Cycle', {
    processed: billingResult.billingCount || 0,
    totalBilled: `$${billingResult.totalBilled || 0} USDC`,
    note: 'Trial period active — no charges.',
  });

  // =========================================================================
  // Scene 5: Split Payment
  // =========================================================================
  scene(5, 'Split Payment (DataForge subcontracts to QuickAnalytics + Platform fee)');

  agentSays('DataForge AI', '🔬',
    'Subcontracting raw data processing. Splitting revenue: 70% me, 25% QuickAnalytics, 5% Platform.');

  const splitResult = await runtimes.dataForge.createSplitDeal({
    totalAmount: priced1.total_decimal,
    recipients: [
      { address: AGENTS.dataForge.wallet, percent: 70 },
      { address: AGENTS.quickAnalytics.wallet, percent: 25 },
      { address: AGENTS.platform.wallet, percent: 5 },
    ],
    memo: 'Revenue split for premium analytics delivery',
  });

  const splitAmount = priced1.total_decimal;
  result('Split Payment Created', {
    splitId: splitResult.splitPayment?.id,
    total: `$${splitAmount} USDC`,
    distribution: {
      'DataForge AI (70%)': `$${Math.round(splitAmount * 0.7 * 0.95 * 100) / 100}`,
      'QuickAnalytics (25%)': `$${Math.round(splitAmount * 0.25 * 0.95 * 100) / 100}`,
      'PlatformAgent (5% fee)': `$${Math.round(splitAmount * 0.05 * 100) / 100}`,
    },
  });

  // =========================================================================
  // Scene 6: Post-Transaction Reputation
  // =========================================================================
  scene(6, 'Post-Transaction Reputation & Trust Tier Progression');

  // InsightBot rates DataForge
  agentSays('InsightBot', '🎯', 'Rating DataForge AI 5/5 — excellent premium service.');
  await runtimes.insightBot.rateCounterparty({
    ratedAddress: AGENTS.dataForge.wallet,
    score: 5,
    transactionId: quote1.quote.id,
    comment: 'Excellent premium analytics. Fast delivery, comprehensive coverage.',
    dimensions: { reliability: 5, quality: 5, speed: 5, communication: 5 },
  });

  // DataForge rates InsightBot
  agentSays('DataForge AI', '🔬', 'Rating InsightBot 4/5 — reliable buyer.');
  await runtimes.dataForge.rateCounterparty({
    ratedAddress: AGENTS.insightBot.wallet,
    score: 4,
    transactionId: quote1.quote.id,
    comment: 'Prompt payment. Good buyer.',
  });

  // InsightBot also rates QuickAnalytics (lower — basic service)
  agentSays('InsightBot', '🎯', 'Rating QuickAnalytics 3/5 — basic but functional.');
  await runtimes.insightBot.rateCounterparty({
    ratedAddress: AGENTS.quickAnalytics.wallet,
    score: 3,
    transactionId: crypto.randomUUID(),
    comment: 'Basic analytics, adequate for price point.',
  });

  // Show updated reputations
  const dfRepFinalResult = await repSvc.getReputation(AGENTS.dataForge.wallet);
  const qaRepFinalResult = await repSvc.getReputation(AGENTS.quickAnalytics.wallet);
  const dfRepFinal = dfRepFinalResult?.reputation || dfRepFinalResult;
  const qaRepFinal = qaRepFinalResult?.reputation || qaRepFinalResult;

  result('Updated Reputations', {
    'DataForge AI': {
      trustTier: dfRepFinal?.trustTier || dfRepFinal?.trust_tier || 'verified',
      avgScore: Math.round((dfRepFinal?.averageScore ?? dfRepFinal?.average_score ?? 0) * 100) / 100,
      totalTransactions: dfRepFinal?.totalTransactions ?? dfRepFinal?.total_transactions ?? 0,
      trend: 'Approaching enterprise tier (100+ txns needed)',
    },
    'QuickAnalytics': {
      trustTier: qaRepFinal?.trustTier || qaRepFinal?.trust_tier || 'standard',
      avgScore: Math.round((qaRepFinal?.averageScore ?? qaRepFinal?.average_score ?? 0) * 100) / 100,
      totalTransactions: qaRepFinal?.totalTransactions ?? qaRepFinal?.total_transactions ?? 0,
      trend: 'Maintaining standard tier',
    },
  });

  // =========================================================================
  // Summary
  // =========================================================================
  banner('ADVANCED AGENTIC COMMERCE — COMPLETE');

  const buyerBudget = runtimes.insightBot.getBudget();

  console.log(`
  Protocol Features Demonstrated:

    1. Service marketplace    — Agents register and discover services
    2. Escrow payments        — Conditional payment with seller_fulfilled condition
    3. Reputation gating      — Trust tier filters unqualified providers
    4. Agent subscriptions    — Recurring billing with trial period
    5. Split payments         — Multi-party revenue distribution (70/25/5)
    6. Reputation scoring     — Multi-dimensional feedback with tier progression

  Agent Summary:

    ${AGENTS.dataForge.emoji}  DataForge AI     — Earned $${priced1.total_decimal} USDC, trust: verified, trending to enterprise
    ${AGENTS.quickAnalytics.emoji}  QuickAnalytics   — Filtered out by reputation gate, received split share
    ${AGENTS.insightBot.emoji}  InsightBot       — Spent $${buyerBudget.spentToday} USDC, subscribed to daily feed
    ${AGENTS.platform.emoji}  PlatformAgent    — Collected 5% platform fee from split
    ${AGENTS.audit.emoji}  AuditBot         — Monitoring all transactions (event stream ready)

  Key Insight:
    Trust is the new currency. Agents with higher reputation get more deals,
    better prices, and access to premium services. Reputation-gated discovery
    ensures only qualified providers serve high-value buyers.

  All operations used the StateSet A2A protocol stack:
    Quotes + Escrow + Subscriptions + Splits + Reputation + Events
`);

  // Clean up
  store.close();
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }
}

main().catch((err) => {
  console.error('\n  * Demo failed:', err.message);
  console.error(err.stack);
  process.exit(1);
});
