#!/usr/bin/env node
/**
 * Agentic Commerce Demo — Two AI Agents Trading Autonomously
 *
 * This demonstrates pure agent-to-agent commerce using StateSet iCommerce:
 *
 *   Agent A ("DataForge AI") — a data analytics service provider
 *   Agent B ("InsightBot")   — a buyer agent that needs analytics done
 *
 * Flow:
 *   1. Seller agent registers a service in the A2A marketplace
 *   2. Buyer agent discovers the service and requests a quote
 *   3. Seller agent provides a quote with pricing and terms
 *   4. Buyer agent negotiates (counter-offer)
 *   5. Seller agent revises the quote
 *   6. Buyer agent accepts and pays via escrow-backed A2A payment
 *   7. Seller agent fulfills the service
 *   8. Escrow releases payment to seller
 *   9. Both agents leave feedback
 *
 * Usage:
 *   node examples/agentic_commerce_demo.mjs
 */

import crypto from 'node:crypto';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

// ---------------------------------------------------------------------------
// Dynamic import of project modules
// ---------------------------------------------------------------------------
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cliSrc = path.join(__dirname, '..', 'src');

const { A2AStore } = await import(path.join(cliSrc, 'a2a', 'store.js'));
const { createA2AService } = await import(path.join(cliSrc, 'a2a', 'index.js'));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const walletAddress = () => '0x' + crypto.randomBytes(20).toString('hex');
const agentId = () => crypto.randomUUID();
const signingKey = () => ({
  privateKey: crypto.randomBytes(32).toString('hex'),
  publicKey: crypto.randomBytes(32).toString('hex'),
});

function banner(text) {
  const line = '═'.repeat(70);
  console.log(`\n╔${line}╗`);
  console.log(`║  ${text.padEnd(68)}║`);
  console.log(`╚${line}╝`);
}

function step(n, text) {
  console.log(`\n  ┌─── Step ${n} ───────────────────────────────────────────────────────`);
  console.log(`  │  ${text}`);
  console.log(`  └${'─'.repeat(68)}`);
}

function result(label, data) {
  console.log(`    ✓ ${label}:`);
  if (typeof data === 'object') {
    const lines = JSON.stringify(data, null, 2).split('\n');
    for (const line of lines) {
      console.log(`      ${line}`);
    }
  } else {
    console.log(`      ${data}`);
  }
}

function agentSays(name, emoji, message) {
  console.log(`\n    ${emoji}  [${name}]: "${message}"`);
}

// ---------------------------------------------------------------------------
// Agent Identities
// ---------------------------------------------------------------------------
const SELLER = {
  name: 'DataForge AI',
  emoji: '🤖',
  id: agentId(),
  wallet: walletAddress(),
  keys: signingKey(),
};

const BUYER = {
  name: 'InsightBot',
  emoji: '🧠',
  id: agentId(),
  wallet: walletAddress(),
  keys: signingKey(),
};

// ---------------------------------------------------------------------------
// Main Demo
// ---------------------------------------------------------------------------
async function main() {
  banner('AGENTIC COMMERCE DEMO — Pure Agent-to-Agent Transaction');

  console.log('\n  Two autonomous AI agents will negotiate and complete');
  console.log('  a commercial transaction with zero human intervention.\n');
  console.log(`  Seller:  ${SELLER.emoji}  ${SELLER.name}  (${SELLER.wallet.slice(0, 10)}...)`);
  console.log(`  Buyer:   ${BUYER.emoji}  ${BUYER.name}   (${BUYER.wallet.slice(0, 10)}...)`);
  console.log(`  Asset:   USDC on SET Chain`);
  console.log(`  Service: Real-time market sentiment analysis`);

  // -------------------------------------------------------------------------
  // Initialize shared A2A store (SQLite)
  // -------------------------------------------------------------------------
  const dbPath = path.join(__dirname, '..', '.demo-a2a.db');
  // Clean up from previous runs
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }

  const a2aStore = new A2AStore({ dbPath });
  a2aStore.init();

  // Build the commerce wrapper that A2A services expect
  // In production this is wired by mcp-server.js; here we build it directly.
  function makeCommerceProxy(store) {
    return {
      a2a: () => ({
        createPayment: (p) => store.createPayment(p),
        getPayment: (id) => store.getPayment(id),
        updatePayment: (id, u) => store.updatePayment(id, u),
        listPayments: (f) => store.listPayments(f),
        sumPayments: (f) => store.sumPayments(f),
        createPaymentRequest: (r) => store.createPaymentRequest(r),
        getPaymentRequest: (id) => store.getPaymentRequest(id),
        updatePaymentRequest: (id, u) => store.updatePaymentRequest(id, u),
        listPaymentRequests: (f) => store.listPaymentRequests(f),
        createQuote: (q) => store.createQuote(q),
        getQuote: (id) => store.getQuote(id),
        updateQuote: (id, u) => store.updateQuote(id, u),
        listQuotes: (f) => store.listQuotes(f),
        createEscrow: (e) => store.createEscrow(e),
        getEscrow: (id) => store.getEscrow(id),
        updateEscrow: (id, u) => store.updateEscrow(id, u),
        listEscrows: (f) => store.listEscrows(f),
        createFeedback: (f) => store.createFeedback(f),
        getFeedback: (id) => store.getFeedback(id),
        updateFeedback: (id, u) => store.updateFeedback(id, u),
        listFeedback: (f) => store.listFeedback(f),
        getReputationScore: (addr) => store.getReputationScore(addr),
        upsertReputationScore: (s) => store.upsertReputationScore(s),
        createService: (s) => store.createService(s),
        getService: (id) => store.getService(id),
        updateService: (id, u) => store.updateService(id, u),
        listServices: (f) => store.listServices(f),
      }),
      // x402 stub — not using sequencer settlement in this demo
      x402: () => ({
        getAgent: () => null,
        getAgentByWallet: () => null,
      }),
    };
  }

  const commerce = makeCommerceProxy(a2aStore);

  // -------------------------------------------------------------------------
  // Create A2A service instances for each agent
  // -------------------------------------------------------------------------
  const sellerA2A = createA2AService(commerce, {
    agentId: SELLER.id,
    walletAddress: SELLER.wallet,
    signingKey: SELLER.keys,
    defaultAsset: 'USDC',
    defaultNetwork: 'set_chain',
  });

  const buyerA2A = createA2AService(commerce, {
    agentId: BUYER.id,
    walletAddress: BUYER.wallet,
    signingKey: BUYER.keys,
    defaultAsset: 'USDC',
    defaultNetwork: 'set_chain',
  });

  // =========================================================================
  // STEP 1: Seller registers a service in the A2A marketplace
  // =========================================================================
  step(1, 'Seller registers a service in the A2A marketplace');

  agentSays(SELLER.name, SELLER.emoji,
    'I offer real-time market sentiment analysis. Let me register my service.');

  const service = a2aStore.createService({
    id: crypto.randomUUID(),
    agent_address: SELLER.wallet,
    name: 'Real-Time Market Sentiment Analysis',
    description:
      'AI-powered sentiment analysis across 50+ data sources including social media, ' +
      'news feeds, SEC filings, and dark pool activity. Delivers actionable buy/sell/hold ' +
      'signals with confidence scores in under 200ms.',
    category: 'analytics',
    pricing_model: 'quote',
    pricing_details: JSON.stringify({
      basePrice: 150,
      currency: 'USDC',
      includes: '10,000 API calls/month',
      overageRate: 0.02,
    }),
    active: 1,
    input_schema: JSON.stringify({
      type: 'object',
      properties: {
        tickers: { type: 'array', items: { type: 'string' } },
        timeframe: { type: 'string', enum: ['1h', '4h', '1d', '1w'] },
        sources: { type: 'array', items: { type: 'string' } },
      },
    }),
    output_schema: JSON.stringify({
      type: 'object',
      properties: {
        sentiment: { type: 'string', enum: ['bullish', 'bearish', 'neutral'] },
        confidence: { type: 'number', minimum: 0, maximum: 1 },
        signals: { type: 'array' },
      },
    }),
    endpoint_url: 'https://api.dataforge.ai/v1/sentiment',
    avg_response_time: 180,
    success_rate: 0.997,
    transaction_count: 0,
    metadata: JSON.stringify({ version: '2.1', uptime: '99.97%' }),
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  });

  result('Service registered', {
    id: service.id,
    name: 'Real-Time Market Sentiment Analysis',
    pricing: '$150 USDC base / 10K API calls',
    category: 'analytics',
    latency: '180ms avg',
  });

  // =========================================================================
  // STEP 2: Buyer discovers the service
  // =========================================================================
  step(2, 'Buyer agent discovers available analytics services');

  agentSays(BUYER.name, BUYER.emoji,
    'I need sentiment analysis for my portfolio. Let me search the marketplace.');

  const services = a2aStore.listServices({ category: 'analytics', active: 1 });

  result('Services discovered', {
    count: services.length,
    topResult: {
      name: services[0].name,
      provider: services[0].agent_address.slice(0, 10) + '...',
      successRate: `${(services[0].success_rate * 100).toFixed(1)}%`,
      avgLatency: `${services[0].avg_response_time}ms`,
    },
  });

  agentSays(BUYER.name, BUYER.emoji,
    `Found "${services[0].name}" — 99.7% success rate, 180ms latency. Perfect.`);

  // =========================================================================
  // STEP 3: Buyer requests a quote from seller
  // =========================================================================
  step(3, 'Buyer requests a quote from the seller agent');

  agentSays(BUYER.name, BUYER.emoji,
    'I need coverage for 5 tickers with daily analysis. Requesting a quote...');

  const quoteRequest = await buyerA2A.requestQuote({
    seller: SELLER.wallet,
    items: [
      {
        description: 'Real-Time Market Sentiment Analysis — Pro Tier',
        quantity: 1,
        metadata: {
          tickers: ['AAPL', 'GOOGL', 'MSFT', 'AMZN', 'TSLA'],
          timeframe: '1d',
          duration: '30 days',
          apiCalls: 15000,
        },
      },
      {
        description: 'Dark Pool Activity Feed Add-on',
        quantity: 1,
        metadata: { sources: ['dark_pool', 'sec_filings'] },
      },
    ],
    message: 'Need 30-day coverage for 5 mega-cap tickers with dark pool add-on. ' +
             'Volume: ~15K API calls. Can you offer a package deal?',
  });

  result('Quote requested', {
    quoteId: quoteRequest.quote.id,
    status: quoteRequest.quote.status,
    items: 2,
    message: 'Awaiting seller response...',
  });

  const quoteId = quoteRequest.quote.id;

  // =========================================================================
  // STEP 4: Seller provides a quote
  // =========================================================================
  step(4, 'Seller reviews request and provides pricing');

  agentSays(SELLER.name, SELLER.emoji,
    'Received quote request for 5 tickers + dark pool. Let me calculate pricing...');

  // Seller "thinks" about the pricing
  agentSays(SELLER.name, SELLER.emoji,
    'Base: $150 + dark pool add-on: $75 + 5K extra API calls: $50 = $275 total');

  const provided = await sellerA2A.provideQuote(quoteId, {
    total: 275.00,
    fees: 25.00,
    tax: 0,
    terms: '30-day access. 15,000 API calls included. $0.02/call overage. ' +
           'SLA: 99.9% uptime, <200ms p95 latency. No refunds after activation.',
    estimatedDelivery: new Date(Date.now() + 5 * 60 * 1000).toISOString(), // 5 min
    expiresInHours: 24,
    message: 'Package deal for Pro Tier + Dark Pool: $275 USDC for 30 days. ' +
             'Includes 15K calls with $0.02 overage. API key delivered instantly upon payment.',
  });

  result('Quote provided', {
    quoteId: provided.quote.id,
    status: provided.quote.status,
    total: `$${provided.quote.total} USDC`,
    fees: `$${provided.quote.fees} USDC`,
    terms: provided.quote.terms?.slice(0, 60) + '...',
  });

  // =========================================================================
  // STEP 5: Buyer negotiates — counter-offer
  // =========================================================================
  step(5, 'Buyer agent negotiates a better price');

  agentSays(BUYER.name, BUYER.emoji,
    '$275 is a bit steep for my budget. Let me counter at $225...');

  const counter = await buyerA2A.counterQuote(quoteId, {
    total: 225.00,
    message: 'Can you do $225? I plan to renew monthly and can commit to 3-month minimum. ' +
             'Also willing to accept slightly higher latency SLA (300ms p95).',
  });

  result('Counter-offer sent', {
    round: counter.round,
    proposedTotal: `$${counter.quote.total} USDC`,
    status: counter.quote.status,
  });

  // =========================================================================
  // STEP 6: Seller revises the quote (meets in the middle)
  // =========================================================================
  step(6, 'Seller revises pricing after considering the counter-offer');

  agentSays(SELLER.name, SELLER.emoji,
    'A 3-month commitment is valuable. I can offer $245 with relaxed SLA.');

  const revised = await sellerA2A.reviseQuote(quoteId, {
    total: 245.00,
    fees: 20.00,
    tax: 0,
    message: 'Revised to $245 USDC with 3-month commitment discount. ' +
             'SLA relaxed to 300ms p95 as requested. Deal?',
  });

  result('Revised quote', {
    round: revised.round,
    revisedTotal: `$${revised.quote.total} USDC`,
    status: revised.quote.status,
    savings: '$30 off original price',
  });

  // =========================================================================
  // STEP 7: Buyer accepts and pays
  // =========================================================================
  step(7, 'Buyer accepts the revised quote and pays');

  agentSays(BUYER.name, BUYER.emoji,
    '$245 works for me. Accepting and sending payment now...');

  const acceptance = await buyerA2A.acceptQuote(quoteId);

  result('Quote accepted + payment sent', {
    paymentId: acceptance.payment.id,
    amount: `$${acceptance.payment.amount} USDC`,
    from: acceptance.payment.from.slice(0, 10) + '...',
    to: acceptance.payment.to.slice(0, 10) + '...',
    asset: acceptance.payment.asset,
    quoteStatus: acceptance.quote.status,
  });

  agentSays(BUYER.name, BUYER.emoji, 'Payment sent! Waiting for seller to deliver API access...');

  // =========================================================================
  // STEP 8: Seller fulfills the service
  // =========================================================================
  step(8, 'Seller fulfills the order — delivers API access');

  agentSays(SELLER.name, SELLER.emoji,
    'Payment received! Provisioning API key and activating Pro Tier access...');

  // Simulate service provisioning
  const apiKey = 'sk_live_' + crypto.randomBytes(24).toString('base64url');

  agentSays(SELLER.name, SELLER.emoji,
    `API key provisioned: ${apiKey.slice(0, 15)}... Marking quote fulfilled.`);

  const fulfilled = await sellerA2A.fulfillQuote(quoteId);

  result('Service fulfilled', {
    quoteId: fulfilled.quote.id,
    status: fulfilled.quote.status,
    deliveredAsset: 'API key + Pro Tier access',
    accessDuration: '30 days',
    apiCallLimit: '15,000',
  });

  // =========================================================================
  // STEP 9: Verify the transaction ledger
  // =========================================================================
  step(9, 'Verify the complete transaction ledger');

  // Check payments from both perspectives
  const sellerPayments = await sellerA2A.getPayments({ received: true });
  const buyerPayments = await buyerA2A.getPayments({ sent: true });

  // Get the final quote state
  const finalQuote = (await sellerA2A.getQuotes({ asSeller: true }))[0];

  result('Seller received payments', {
    count: sellerPayments.length,
    totalReceived: sellerPayments.reduce((sum, p) => sum + p.amount, 0) + ' USDC',
  });

  result('Buyer sent payments', {
    count: buyerPayments.length,
    totalSent: buyerPayments.reduce((sum, p) => sum + p.amount, 0) + ' USDC',
  });

  result('Final quote state', {
    id: finalQuote.id,
    status: finalQuote.status,
    negotiationRounds: 2,
    originalPrice: '$275 USDC',
    finalPrice: `$${finalQuote.total} USDC`,
    discount: '$30 (10.9%)',
    buyer: finalQuote.buyer?.slice(0, 10) + '...',
    seller: finalQuote.seller?.slice(0, 10) + '...',
    createdAt: finalQuote.createdAt,
    fulfilledAt: finalQuote.fulfilledAt,
  });

  // =========================================================================
  // STEP 10: Both agents leave feedback
  // =========================================================================
  step(10, 'Agents exchange reputation feedback');

  agentSays(BUYER.name, BUYER.emoji,
    'Great service! Fast provisioning and excellent data quality. 5 stars.');

  a2aStore.createFeedback({
    id: crypto.randomUUID(),
    agent_address: SELLER.wallet,
    reviewer_address: BUYER.wallet,
    transaction_type: 'quote',
    transaction_id: quoteId,
    score: 5,
    dimensions: JSON.stringify({
      quality: 5,
      speed: 5,
      communication: 4,
      value: 4,
    }),
    comment: 'Excellent sentiment analysis service. Fast API provisioning, ' +
             'accurate signals, and fair negotiation. Will renew.',
    is_revoked: 0,
    created_at: new Date().toISOString(),
  });

  agentSays(SELLER.name, SELLER.emoji,
    'Good buyer — professional negotiation and prompt payment. 5 stars.');

  a2aStore.createFeedback({
    id: crypto.randomUUID(),
    agent_address: BUYER.wallet,
    reviewer_address: SELLER.wallet,
    transaction_type: 'quote',
    transaction_id: quoteId,
    score: 5,
    dimensions: JSON.stringify({
      reliability: 5,
      communication: 5,
      payment_speed: 5,
    }),
    comment: 'Professional buyer agent. Clear requirements, reasonable negotiation, ' +
             'and instant payment upon agreement. Welcome to renew anytime.',
    is_revoked: 0,
    created_at: new Date().toISOString(),
  });

  // Read back feedback
  const sellerFeedback = a2aStore.listFeedback({ agent_address: SELLER.wallet });
  const buyerFeedback = a2aStore.listFeedback({ agent_address: BUYER.wallet });

  result('Seller reputation', {
    reviews: sellerFeedback.length,
    avgScore: sellerFeedback.reduce((s, f) => s + f.score, 0) / sellerFeedback.length,
  });

  result('Buyer reputation', {
    reviews: buyerFeedback.length,
    avgScore: buyerFeedback.reduce((s, f) => s + f.score, 0) / buyerFeedback.length,
  });

  // =========================================================================
  // Summary
  // =========================================================================
  banner('TRANSACTION COMPLETE');

  console.log(`
  Two AI agents just completed a full commercial transaction:

    ${SELLER.emoji}  ${SELLER.name}  ──── sold ────▶  Real-Time Sentiment Analysis
    ${BUYER.emoji}  ${BUYER.name}    ──── paid ────▶  $245.00 USDC on SET Chain

  ┌──────────────────────────────────────────────────────────────────────┐
  │  Flow:                                                             │
  │    1. Service listed in A2A marketplace                            │
  │    2. Buyer discovered service                                     │
  │    3. Quote requested ($0 → awaiting pricing)                      │
  │    4. Seller quoted $275 USDC                                      │
  │    5. Buyer counter-offered $225 USDC                              │
  │    6. Seller revised to $245 USDC (3-month commitment discount)    │
  │    7. Buyer accepted + paid $245 USDC                              │
  │    8. Seller delivered API access                                  │
  │    9. Transaction verified on ledger                               │
  │   10. Mutual 5-star feedback exchanged                             │
  └──────────────────────────────────────────────────────────────────────┘

  All operations used the StateSet iCommerce A2A protocol:
    • Agent-to-Agent payments (USDC on SET Chain)
    • Quote negotiation (2 rounds, $30 savings)
    • Service marketplace discovery
    • Reputation + feedback system
    • Full audit trail in SQLite

  No humans were involved. This is agentic commerce.
`);

  // Clean up
  a2aStore.close();
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------
main().catch((err) => {
  console.error('\n  ✗ Demo failed:', err.message);
  console.error(err.stack);
  process.exit(1);
});
