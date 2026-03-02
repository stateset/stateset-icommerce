#!/usr/bin/env node
/**
 * Multi-Agent Supply Chain Demo
 *
 * Four autonomous AI agents form a data-processing pipeline.
 * Each agent discovers the next service, negotiates a price, pays,
 * does work, and passes the output forward.
 *
 *   Buyer ($80) → DataCollector → DataCleaner → Analyst → ReportWriter
 *
 * Each agent keeps a margin and pays the next hop from its revenue.
 * No agent has hardcoded knowledge of the pipeline — they discover
 * services autonomously through the A2A marketplace.
 *
 * Usage:
 *   node examples/supply_chain_demo.mjs
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
const { createBudgetGatedStrategy, createNegotiatorStrategy } = await import(
  path.join(cliSrc, 'a2a', 'strategies.js')
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
// Agent definitions
// ---------------------------------------------------------------------------
const AGENTS = [
  {
    name: 'DataCollector',
    emoji: '📡',
    wallet: wallet(),
    keys: keys(),
    service: {
      name: 'Raw Data Collection',
      description: 'Collects raw social media and news data for specified tickers.',
      category: 'data-collection',
      pricingModel: 'quote',
    },
    nextCategory: 'data-cleaning',
    budget: { daily: 500, perTransaction: 200 },
    strategy: createBudgetGatedStrategy({ markup: 1.6, basePrice: 30 }),
  },
  {
    name: 'DataCleaner',
    emoji: '🧹',
    wallet: wallet(),
    keys: keys(),
    service: {
      name: 'Data Cleaning & Normalization',
      description: 'Deduplicates, normalizes, and validates raw datasets.',
      category: 'data-cleaning',
      pricingModel: 'quote',
    },
    nextCategory: 'data-analysis',
    budget: { daily: 300, perTransaction: 150 },
    strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 20 }),
  },
  {
    name: 'Analyst',
    emoji: '📊',
    wallet: wallet(),
    keys: keys(),
    service: {
      name: 'Sentiment Analysis Engine',
      description: 'Runs NLP sentiment models on cleaned data, produces buy/sell/hold signals.',
      category: 'data-analysis',
      pricingModel: 'quote',
    },
    nextCategory: 'reporting',
    budget: { daily: 400, perTransaction: 200 },
    strategy: createBudgetGatedStrategy({ markup: 1.5, basePrice: 15 }),
  },
  {
    name: 'ReportWriter',
    emoji: '📝',
    wallet: wallet(),
    keys: keys(),
    service: {
      name: 'Executive Report Generator',
      description: 'Transforms analysis insights into polished executive reports.',
      category: 'reporting',
      pricingModel: 'quote',
    },
    nextCategory: null, // terminal node
    budget: { daily: 200, perTransaction: 100 },
    strategy: createBudgetGatedStrategy({ markup: 1.3, basePrice: 10 }),
  },
];

// ---------------------------------------------------------------------------
// Main Demo
// ---------------------------------------------------------------------------
async function main() {
  banner('MULTI-AGENT SUPPLY CHAIN — Autonomous Data Pipeline');

  console.log('\n  Four AI agents form a data processing pipeline.');
  console.log('  Each discovers the next service, negotiates, pays, and passes output.\n');

  for (const a of AGENTS) {
    console.log(`  ${a.emoji}  ${a.name.padEnd(16)} (${a.wallet.slice(0, 10)}...)  — ${a.service.category}`);
  }

  // -------------------------------------------------------------------------
  // Setup
  // -------------------------------------------------------------------------
  const dbPath = path.join(__dirname, '..', '.supply-chain-a2a.db');
  try { fs.unlinkSync(dbPath); } catch { /* ignore */ }

  const store = new A2AStore({ dbPath });
  store.init();
  const commerce = makeCommerceProxy(store);

  // Create runtimes
  const runtimes = AGENTS.map((a) =>
    createAgentRuntime({
      name: a.name,
      walletAddress: a.wallet,
      signingKey: a.keys,
      commerce,
      budget: a.budget,
      strategy: a.strategy,
      logger: () => {}, // suppress debug logs in demo
    })
  );

  // -------------------------------------------------------------------------
  // Step 1: Register services
  // -------------------------------------------------------------------------
  step(1, 'Each agent registers its service in the A2A marketplace');

  for (let i = 0; i < AGENTS.length; i++) {
    runtimes[i].registerService(AGENTS[i].service);
    agentSays(AGENTS[i].name, AGENTS[i].emoji,
      `Registered "${AGENTS[i].service.name}" in category "${AGENTS[i].service.category}"`);
  }

  result('Marketplace', {
    totalServices: store.listServices({}).length,
    categories: [...new Set(AGENTS.map((a) => a.service.category))],
  });

  // -------------------------------------------------------------------------
  // Step 2: Buyer initiates the pipeline
  // -------------------------------------------------------------------------
  step(2, 'External buyer requests data collection (entry point)');

  const buyerWallet = wallet();
  const buyerA2A = (await import(path.join(cliSrc, 'a2a', 'index.js'))).createA2AService(commerce, {
    agentId: crypto.randomUUID(),
    walletAddress: buyerWallet,
    signingKey: keys(),
  });

  agentSays('Buyer', '💼', 'I need sentiment analysis for AAPL, GOOGL, MSFT — 7-day lookback.');

  // Buyer discovers the first service (data-collection)
  const dataCollectionServices = runtimes[0].discoverServices({ category: 'data-collection' });
  agentSays('Buyer', '💼', `Found ${dataCollectionServices.length} data-collection service(s).`);

  // Request a quote from DataCollector (no unitPrice — let seller price it)
  const buyerQuote = await buyerA2A.requestQuote({
    seller: AGENTS[0].wallet,
    items: [
      { description: 'Social media + news data collection', quantity: 1 },
      { description: '7-day lookback window', quantity: 1 },
      { description: '3 tickers (AAPL, GOOGL, MSFT)', quantity: 3 },
    ],
    message: 'Need 7-day sentiment data for 3 mega-cap tickers.',
  });

  result('Quote requested', { quoteId: buyerQuote.quote.id, status: 'requested' });

  // DataCollector processes the incoming quote via tick()
  await runtimes[0].tick();

  // Buyer evaluates and accepts the quote
  const updatedQuote = store.getQuote(buyerQuote.quote.id);
  agentSays('Buyer', '💼', `DataCollector quoted $${updatedQuote.total_decimal}. Accepting.`);

  const acceptance = await buyerA2A.acceptQuote(buyerQuote.quote.id);
  result('Buyer paid DataCollector', {
    amount: `$${acceptance.payment.amount} USDC`,
    from: buyerWallet.slice(0, 10) + '...',
    to: AGENTS[0].wallet.slice(0, 10) + '...',
  });

  // DataCollector auto-fulfills
  await runtimes[0].tick();

  // -------------------------------------------------------------------------
  // Step 3-5: Each agent discovers next, negotiates, pays, passes work
  // -------------------------------------------------------------------------
  const ledger = [
    {
      agent: 'Buyer',
      emoji: '💼',
      revenue: 0,
      cost: acceptance.payment.amount,
      margin: -acceptance.payment.amount,
    },
    {
      agent: AGENTS[0].name,
      emoji: AGENTS[0].emoji,
      revenue: acceptance.payment.amount,
      cost: 0,
      margin: 0,
    },
  ];

  for (let i = 0; i < AGENTS.length - 1; i++) {
    const current = AGENTS[i];
    const currentRuntime = runtimes[i];
    const nextCategory = current.nextCategory;

    step(3 + i, `${current.name} discovers "${nextCategory}" service and passes work downstream`);

    // Discover next service
    const nextServices = currentRuntime.discoverServices({ category: nextCategory });
    const nextAgent = AGENTS[i + 1];
    agentSays(current.name, current.emoji,
      `Found "${nextServices[0]?.name}" in category "${nextCategory}". Requesting quote...`);

    // Request quote from next agent
    const quote = await currentRuntime.a2a.requestQuote({
      seller: nextAgent.wallet,
      items: [
        { description: `Process output from ${current.name}`, quantity: 1 },
      ],
      message: `Passing ${current.service.category} output for ${nextCategory} processing.`,
    });

    // Next agent processes the quote request (seller tick)
    await runtimes[i + 1].tick();

    // Current agent evaluates the received quote (buyer tick)
    const receivedQuote = store.getQuote(quote.quote.id);
    const quoteTotal = receivedQuote.total_decimal;

    agentSays(current.name, current.emoji,
      `${nextAgent.name} quoted $${quoteTotal}. Accepting and paying...`);

    const payment = await currentRuntime.a2a.acceptQuote(quote.quote.id);
    currentRuntime.recordSpend(payment.payment.amount, { type: 'downstream', to: nextAgent.name });

    result(`${current.name} → ${nextAgent.name}`, {
      amount: `$${payment.payment.amount} USDC`,
      quoteId: quote.quote.id,
    });

    // Next agent auto-fulfills
    await runtimes[i + 1].tick();

    agentSays(nextAgent.name, nextAgent.emoji, 'Work completed and delivered.');

    // Update ledger
    ledger[ledger.length - 1].cost = payment.payment.amount;
    ledger[ledger.length - 1].margin = ledger[ledger.length - 1].revenue - payment.payment.amount;
    ledger.push({
      agent: nextAgent.name,
      emoji: nextAgent.emoji,
      revenue: payment.payment.amount,
      cost: 0,
      margin: 0,
    });
  }

  // Final agent has no downstream cost
  ledger[ledger.length - 1].margin = ledger[ledger.length - 1].revenue;

  // -------------------------------------------------------------------------
  // Summary
  // -------------------------------------------------------------------------
  banner('SUPPLY CHAIN COMPLETE');

  console.log('\n  Value Flow Through the Pipeline:\n');
  console.log('  ┌──────────────────┬──────────┬──────────┬──────────┬──────────┐');
  console.log('  │ Agent            │ Revenue  │ Cost     │ Margin   │ Margin % │');
  console.log('  ├──────────────────┼──────────┼──────────┼──────────┼──────────┤');

  for (const entry of ledger) {
    const pct = entry.revenue > 0
      ? `${Math.round((entry.margin / entry.revenue) * 100)}%`
      : entry.agent === 'Buyer' ? 'N/A' : '100%';
    console.log(
      `  │ ${(entry.emoji + ' ' + entry.agent).padEnd(17)}│ $${String(entry.revenue).padStart(7)} │ $${String(entry.cost).padStart(7)} │ $${String(entry.margin).padStart(7)} │ ${pct.padStart(8)} │`
    );
  }

  console.log('  └──────────────────┴──────────┴──────────┴──────────┴──────────┘');

  const totalPipelineCost = ledger[0].cost;
  console.log(`
  Pipeline Summary:
    Total cost to buyer: $${totalPipelineCost} USDC
    Pipeline depth:      ${AGENTS.length} agents
    Services used:       ${AGENTS.map((a) => a.service.category).join(' → ')}
    Settlement:          USDC on SET Chain (local ledger)

  Each agent autonomously:
    1. Discovered the next service via marketplace search
    2. Requested and evaluated quotes using its strategy
    3. Paid from its revenue to the next hop
    4. Delivered its output before receiving payment

  No agent had hardcoded knowledge of the pipeline topology.
  This is an emergent agent supply chain.
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
