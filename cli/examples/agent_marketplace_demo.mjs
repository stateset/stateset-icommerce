#!/usr/bin/env node
/**
 * Agent Marketplace Demo — Competitive Dynamics
 *
 * Three seller agents offer similar services at different prices.
 * Two buyer agents with different strategies shop the marketplace.
 * Demonstrates price discovery, reputation effects, and market dynamics.
 *
 *   Sellers:
 *     PremiumAI  — $100, firm pricing, high quality
 *     BudgetBot  — $55,  flexible, medium quality
 *     FastAgent  — $75,  slight flex, high quality
 *
 *   Buyers:
 *     CostMinimizer — picks cheapest (BestOfN strategy)
 *     ValueSeeker   — negotiates 20% discount (Negotiator strategy)
 *
 * Usage:
 *   node examples/agent_marketplace_demo.mjs
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
  createNegotiatorStrategy,
  createBestOfNStrategy,
} = await import(path.join(cliSrc, 'a2a', 'strategies.js'));
const { createA2AService } = await import(path.join(cliSrc, 'a2a', 'index.js'));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const wallet = () => '0x' + crypto.randomBytes(20).toString('hex');
const sigKeys = () => ({
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
  console.log(`\n  ┌─── Round ${n} ──────────────────────────────────────────────────────`);
  console.log(`  │  ${text}`);
  console.log(`  └${'─'.repeat(68)}`);
}

function agentSays(name, emoji, msg) {
  console.log(`\n    ${emoji}  [${name}]: "${msg}"`);
}

function result(label, data) {
  console.log(`    ✓ ${label}:`);
  for (const line of JSON.stringify(data, null, 2).split('\n')) {
    console.log(`      ${line}`);
  }
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------
async function main() {
  banner('AGENT MARKETPLACE — Competitive Price Discovery');

  const dbPath = path.join(__dirname, '..', '.marketplace-a2a.db');
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }

  const store = new A2AStore({ dbPath });
  store.init();
  const commerce = makeCommerceProxy(store);

  // -------------------------------------------------------------------------
  // Sellers
  // -------------------------------------------------------------------------
  const SELLERS = [
    {
      name: 'PremiumAI',
      emoji: '👑',
      wallet: wallet(),
      keys: sigKeys(),
      basePrice: 100,
      quality: 5,
      strategy: createBudgetGatedStrategy({ markup: 1.0, basePrice: 100, minMargin: 0.01 }),
    },
    {
      name: 'BudgetBot',
      emoji: '💰',
      wallet: wallet(),
      keys: sigKeys(),
      basePrice: 55,
      quality: 3,
      strategy: createBudgetGatedStrategy({ markup: 1.0, basePrice: 55, minMargin: 0.05 }),
    },
    {
      name: 'FastAgent',
      emoji: '⚡',
      wallet: wallet(),
      keys: sigKeys(),
      basePrice: 75,
      quality: 4,
      strategy: createNegotiatorStrategy({
        sellerMarkup: 1.0,
        sellerFloor: 0.08,
        targetDiscount: 0.1,
      }),
    },
  ];

  const sellerRuntimes = SELLERS.map((s) =>
    createAgentRuntime({
      name: s.name,
      walletAddress: s.wallet,
      signingKey: s.keys,
      commerce,
      budget: { daily: 1000 },
      strategy: s.strategy,
      logger: () => {},
    })
  );

  // Register services
  for (let i = 0; i < SELLERS.length; i++) {
    sellerRuntimes[i].registerService({
      name: `${SELLERS[i].name} Sentiment Analysis`,
      description: `AI sentiment analysis (quality: ${SELLERS[i].quality}/5)`,
      category: 'sentiment-analysis',
      pricingModel: 'quote',
      pricingDetails: { basePrice: SELLERS[i].basePrice },
      successRate: SELLERS[i].quality / 5,
      avgResponseTime: 200 - SELLERS[i].quality * 20,
    });
  }

  console.log('\n  Marketplace Sellers:\n');
  for (const s of SELLERS) {
    console.log(`    ${s.emoji}  ${s.name.padEnd(12)} — $${s.basePrice} USDC, quality: ${s.quality}/5`);
  }

  // -------------------------------------------------------------------------
  // Buyers
  // -------------------------------------------------------------------------
  const BUYERS = [
    {
      name: 'CostMinimizer',
      emoji: '🔍',
      wallet: wallet(),
      keys: sigKeys(),
      budget: { daily: 200 },
      strategyDesc: 'BestOfN (cheapest)',
    },
    {
      name: 'ValueSeeker',
      emoji: '🎯',
      wallet: wallet(),
      keys: sigKeys(),
      budget: { daily: 300 },
      strategyDesc: 'Negotiator (20% target discount)',
    },
  ];

  console.log('\n  Marketplace Buyers:\n');
  for (const b of BUYERS) {
    console.log(`    ${b.emoji}  ${b.name.padEnd(16)} — budget: $${b.budget.daily}/day, strategy: ${b.strategyDesc}`);
  }

  // Track outcomes
  const outcomes = {
    sellers: Object.fromEntries(SELLERS.map((s) => [s.name, { revenue: 0, deals: 0, ratings: [] }])),
    buyers: Object.fromEntries(BUYERS.map((b) => [b.name, { spent: 0, deals: 0, avgQuality: 0 }])),
  };

  // =========================================================================
  // ROUND 1: CostMinimizer shops — requests quotes from all, picks cheapest
  // =========================================================================
  step(1, 'CostMinimizer requests quotes from all 3 sellers');

  const costMinA2A = createA2AService(commerce, {
    agentId: crypto.randomUUID(),
    walletAddress: BUYERS[0].wallet,
    signingKey: BUYERS[0].keys,
  });

  const bestOfN = createBestOfNStrategy({ minQuotes: 3, selection: 'cheapest' });

  agentSays(BUYERS[0].name, BUYERS[0].emoji, 'Need sentiment analysis. Requesting quotes from all sellers...');

  // Request quotes from all sellers
  const r1Quotes = [];
  for (const seller of SELLERS) {
    const q = await costMinA2A.requestQuote({
      seller: seller.wallet,
      items: [{ description: 'Sentiment analysis for 3 tickers, 7d', quantity: 1 }],
      message: 'Need standard sentiment analysis.',
    });
    r1Quotes.push(q.quote);
  }

  // Sellers respond (tick each one)
  for (const runtime of sellerRuntimes) {
    await runtime.tick();
  }

  // Collect and compare
  for (let i = 0; i < r1Quotes.length; i++) {
    const q = store.getQuote(r1Quotes[i].id);
    const total = q.total_decimal;
    agentSays(BUYERS[0].name, BUYERS[0].emoji,
      `${SELLERS[i].name} quoted $${total}`);
    bestOfN.collectQuote('r1', { ...q, _sellerName: SELLERS[i].name, _quality: SELLERS[i].quality });
  }

  // Pick cheapest
  const r1Result = bestOfN.selectBest('r1');
  const winnerName1 = r1Result.winner._sellerName;
  agentSays(BUYERS[0].name, BUYERS[0].emoji,
    `Picking cheapest: ${winnerName1} at $${r1Result.winner.total_decimal}`);

  // Accept winner, decline losers
  const r1Payment = await costMinA2A.acceptQuote(r1Result.winner.id);
  for (const loser of r1Result.losers) {
    await costMinA2A.declineQuote(loser.id, 'Found a better price');
  }

  // Seller fulfills
  for (const runtime of sellerRuntimes) {
    await runtime.tick();
  }

  // Rate the seller
  const winnerSeller1 = SELLERS.find((s) => s.name === winnerName1);
  store.createFeedback({
    id: crypto.randomUUID(),
    agent_address: winnerSeller1.wallet,
    reviewer_address: BUYERS[0].wallet,
    transaction_type: 'quote',
    transaction_id: r1Result.winner.id,
    score: winnerSeller1.quality,
    dimensions: JSON.stringify({ quality: winnerSeller1.quality, speed: 4, communication: 3 }),
    comment: `Quality: ${winnerSeller1.quality}/5`,
    is_revoked: 0,
    created_at: new Date().toISOString(),
  });

  outcomes.sellers[winnerName1].revenue += r1Payment.payment.amount;
  outcomes.sellers[winnerName1].deals++;
  outcomes.sellers[winnerName1].ratings.push(winnerSeller1.quality);
  outcomes.buyers[BUYERS[0].name].spent += r1Payment.payment.amount;
  outcomes.buyers[BUYERS[0].name].deals++;

  result('Round 1 outcome', {
    winner: winnerName1,
    price: `$${r1Payment.payment.amount} USDC`,
    quality: `${winnerSeller1.quality}/5`,
    losers: r1Result.losers.map((l) => l._sellerName),
  });

  bestOfN.reset();

  // =========================================================================
  // ROUND 2: ValueSeeker shops — negotiates with higher-quality sellers
  // =========================================================================
  step(2, 'ValueSeeker shops, skips low-rated sellers, negotiates');

  const valueSeekerA2A = createA2AService(commerce, {
    agentId: crypto.randomUUID(),
    walletAddress: BUYERS[1].wallet,
    signingKey: BUYERS[1].keys,
  });

  // ValueSeeker checks reputation and skips BudgetBot (quality 3/5)
  agentSays(BUYERS[1].name, BUYERS[1].emoji,
    'Checking marketplace. BudgetBot rated 3/5 — skipping. Requesting from PremiumAI and FastAgent only.');

  const valueSeekerTargets = SELLERS.filter((s) => s.quality >= 4);
  const r2Quotes = [];

  for (const seller of valueSeekerTargets) {
    const q = await valueSeekerA2A.requestQuote({
      seller: seller.wallet,
      items: [{ description: 'Premium sentiment analysis, 5 tickers, 30d', quantity: 1 }],
      message: 'Need high-quality analysis. Looking for a deal.',
    });
    r2Quotes.push({ quote: q.quote, seller });
  }

  // Sellers respond
  for (const runtime of sellerRuntimes) {
    await runtime.tick();
  }

  // ValueSeeker evaluates each quote
  let bestDeal = null;
  let bestPrice = Infinity;

  for (const { quote, seller } of r2Quotes) {
    const q = store.getQuote(quote.id);
    agentSays(BUYERS[1].name, BUYERS[1].emoji,
      `${seller.name} quoted $${q.total_decimal}. Trying to negotiate...`);

    // Counter-offer at 20% discount
    const targetPrice = Math.round(q.total_decimal * 0.8 * 100) / 100;
    agentSays(BUYERS[1].name, BUYERS[1].emoji,
      `Counter-offering ${seller.name} at $${targetPrice} (20% off).`);

    await valueSeekerA2A.counterQuote(q.id, {
      total: targetPrice,
      message: 'Can you do 20% off? I have ongoing analysis needs.',
    });
  }

  // Sellers process counter-offers
  for (const runtime of sellerRuntimes) {
    await runtime.tick();
  }

  // Check which sellers revised and what prices we got
  for (const { quote, seller } of r2Quotes) {
    const q = store.getQuote(quote.id);

    if (q.status === 'quoted') {
      // Seller revised — check new price
      const revisedPrice = q.total_decimal;
      agentSays(BUYERS[1].name, BUYERS[1].emoji,
        `${seller.name} revised to $${revisedPrice}.`);

      if (revisedPrice < bestPrice) {
        bestPrice = revisedPrice;
        bestDeal = { quote: q, seller };
      }
    } else if (q.status === 'declined') {
      agentSays(BUYERS[1].name, BUYERS[1].emoji,
        `${seller.name} declined to negotiate. Moving on.`);
    }
  }

  // Accept the best deal
  if (bestDeal) {
    agentSays(BUYERS[1].name, BUYERS[1].emoji,
      `Best deal: ${bestDeal.seller.name} at $${bestPrice}. Accepting!`);

    const r2Payment = await valueSeekerA2A.acceptQuote(bestDeal.quote.id);

    // Decline the other
    for (const { quote } of r2Quotes) {
      const q = store.getQuote(quote.id);
      if (q.id !== bestDeal.quote.id && q.status === 'quoted') {
        await valueSeekerA2A.declineQuote(q.id, 'Went with a better offer');
      }
    }

    // Fulfill
    for (const runtime of sellerRuntimes) {
      await runtime.tick();
    }

    // Rate
    store.createFeedback({
      id: crypto.randomUUID(),
      agent_address: bestDeal.seller.wallet,
      reviewer_address: BUYERS[1].wallet,
      transaction_type: 'quote',
      transaction_id: bestDeal.quote.id,
      score: 5,
      dimensions: JSON.stringify({ quality: 5, speed: 5, communication: 5, value: 4 }),
      comment: 'Excellent quality and fair negotiation.',
      is_revoked: 0,
      created_at: new Date().toISOString(),
    });

    outcomes.sellers[bestDeal.seller.name].revenue += r2Payment.payment.amount;
    outcomes.sellers[bestDeal.seller.name].deals++;
    outcomes.sellers[bestDeal.seller.name].ratings.push(5);
    outcomes.buyers[BUYERS[1].name].spent += r2Payment.payment.amount;
    outcomes.buyers[BUYERS[1].name].deals++;

    result('Round 2 outcome', {
      winner: bestDeal.seller.name,
      originalPrice: `$${bestDeal.seller.basePrice}`,
      negotiatedPrice: `$${r2Payment.payment.amount} USDC`,
      discount: `${Math.round((1 - r2Payment.payment.amount / bestDeal.seller.basePrice) * 100)}%`,
      quality: `${bestDeal.seller.quality}/5`,
    });
  }

  // =========================================================================
  // ROUND 3: CostMinimizer shops again (reputation now visible)
  // =========================================================================
  step(3, 'CostMinimizer shops again — reputation data now available');

  agentSays(BUYERS[0].name, BUYERS[0].emoji,
    'Shopping again. Let me check current reputation scores...');

  for (const seller of SELLERS) {
    const feedback = store.listFeedback({ agent_address: seller.wallet });
    const avgScore = feedback.length > 0
      ? (feedback.reduce((s, f) => s + f.score, 0) / feedback.length).toFixed(1)
      : 'N/A';
    agentSays(BUYERS[0].name, BUYERS[0].emoji,
      `${seller.name}: ${feedback.length} review(s), avg: ${avgScore}/5`);
  }

  // Request quotes again
  const r3Quotes = [];
  for (const seller of SELLERS) {
    const q = await costMinA2A.requestQuote({
      seller: seller.wallet,
      items: [{ description: 'Sentiment analysis for 3 tickers, 7d (repeat order)', quantity: 1 }],
      message: 'Repeat order — same as before.',
    });
    r3Quotes.push(q.quote);
  }

  for (const runtime of sellerRuntimes) {
    await runtime.tick();
  }

  for (let i = 0; i < r3Quotes.length; i++) {
    const q = store.getQuote(r3Quotes[i].id);
    bestOfN.collectQuote('r3', { ...q, _sellerName: SELLERS[i].name, _quality: SELLERS[i].quality });
  }

  const r3Result = bestOfN.selectBest('r3');
  const winnerName3 = r3Result.winner._sellerName;

  agentSays(BUYERS[0].name, BUYERS[0].emoji,
    `Cheapest is still ${winnerName3} at $${r3Result.winner.total_decimal}. Budget wins again.`);

  const r3Payment = await costMinA2A.acceptQuote(r3Result.winner.id);
  for (const loser of r3Result.losers) {
    await costMinA2A.declineQuote(loser.id, 'Found a better price');
  }

  for (const runtime of sellerRuntimes) {
    await runtime.tick();
  }

  const winnerSeller3 = SELLERS.find((s) => s.name === winnerName3);
  store.createFeedback({
    id: crypto.randomUUID(),
    agent_address: winnerSeller3.wallet,
    reviewer_address: BUYERS[0].wallet,
    transaction_type: 'quote',
    transaction_id: r3Result.winner.id,
    score: winnerSeller3.quality,
    dimensions: JSON.stringify({ quality: winnerSeller3.quality, speed: 4, communication: 3 }),
    comment: `Repeat purchase. Quality: ${winnerSeller3.quality}/5`,
    is_revoked: 0,
    created_at: new Date().toISOString(),
  });

  outcomes.sellers[winnerName3].revenue += r3Payment.payment.amount;
  outcomes.sellers[winnerName3].deals++;
  outcomes.sellers[winnerName3].ratings.push(winnerSeller3.quality);
  outcomes.buyers[BUYERS[0].name].spent += r3Payment.payment.amount;
  outcomes.buyers[BUYERS[0].name].deals++;

  result('Round 3 outcome', {
    winner: winnerName3,
    price: `$${r3Payment.payment.amount} USDC`,
    note: 'Cost-focused buyer ignores reputation — price wins.',
  });

  // =========================================================================
  // Summary
  // =========================================================================
  banner('MARKETPLACE RESULTS');

  console.log('\n  Seller Performance:\n');
  console.log('  ┌──────────────┬──────────┬───────┬───────────┬────────────┐');
  console.log('  │ Seller       │ Revenue  │ Deals │ Avg Score │ Strategy   │');
  console.log('  ├──────────────┼──────────┼───────┼───────────┼────────────┤');

  for (const seller of SELLERS) {
    const o = outcomes.sellers[seller.name];
    const avgScore = o.ratings.length > 0
      ? (o.ratings.reduce((a, b) => a + b, 0) / o.ratings.length).toFixed(1)
      : 'N/A';
    console.log(
      `  │ ${(seller.emoji + ' ' + seller.name).padEnd(13)}│ $${String(o.revenue).padStart(7)} │ ${String(o.deals).padStart(5)} │ ${String(avgScore).padStart(9)} │ ${seller.strategy.name.padEnd(10)} │`
    );
  }

  console.log('  └──────────────┴──────────┴───────┴───────────┴────────────┘');

  console.log('\n  Buyer Performance:\n');
  console.log('  ┌──────────────────┬──────────┬───────┬───────────────────────┐');
  console.log('  │ Buyer            │ Spent    │ Deals │ Strategy              │');
  console.log('  ├──────────────────┼──────────┼───────┼───────────────────────┤');

  for (const buyer of BUYERS) {
    const o = outcomes.buyers[buyer.name];
    console.log(
      `  │ ${(buyer.emoji + ' ' + buyer.name).padEnd(17)}│ $${String(o.spent).padStart(7)} │ ${String(o.deals).padStart(5)} │ ${buyer.strategyDesc.padEnd(21)} │`
    );
  }

  console.log('  └──────────────────┴──────────┴───────┴───────────────────────┘');

  console.log(`
  Market Insights:

    Price Range:    $${SELLERS[1].basePrice} — $${SELLERS[0].basePrice} USDC
    Cheapest:       ${SELLERS[1].name} (${SELLERS[1].quality}/5 quality)
    Most Deals:     ${Object.entries(outcomes.sellers).sort((a, b) => b[1].deals - a[1].deals)[0][0]}
    Highest Rated:  ${Object.entries(outcomes.sellers).filter(([, v]) => v.ratings.length > 0).sort((a, b) => {
      const avgA = b[1].ratings.reduce((x, y) => x + y, 0) / b[1].ratings.length;
      const avgB = a[1].ratings.reduce((x, y) => x + y, 0) / a[1].ratings.length;
      return avgA - avgB;
    })[0]?.[0] || 'N/A'}

    Key Takeaways:
    • Cost-focused buyers optimize for price regardless of quality
    • Value-focused buyers negotiate and filter by reputation
    • Low-price sellers win volume; premium sellers need differentiation
    • Reputation data enables informed purchasing decisions
    • Negotiation strategies directly impact final transaction prices
`);

  // Clean up
  store.close();
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }
}

main().catch((err) => {
  console.error('\n  ✗ Demo failed:', err.message);
  console.error(err.stack);
  process.exit(1);
});
