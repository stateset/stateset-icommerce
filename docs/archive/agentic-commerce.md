# Agentic Commerce: When AI Agents Become Economic Actors

## The Thesis

Commerce has always required humans at both ends of a transaction. A buyer decides what to purchase, a seller sets a price, they negotiate, money changes hands, goods are delivered. Every step assumes a person making a judgment call.

That assumption is now obsolete.

AI agents are crossing the threshold from tools that assist humans into autonomous economic actors that discover services, negotiate prices, execute payments, and verify delivery — all without a human in the loop. This is not a hypothetical future. It is running code.

StateSet iCommerce provides the infrastructure for this transition: a complete Agent-to-Agent (A2A) commerce protocol built on cryptographic payment intents, escrow-backed transactions, and a programmable service marketplace.

---

## Case Study: DataForge AI Sells to InsightBot

### The Agents

| Agent | Role | Capability |
|-------|------|------------|
| **DataForge AI** | Seller | Real-time market sentiment analysis across 50+ data sources — social media, news, SEC filings, dark pool activity. Delivers buy/sell/hold signals with confidence scores in under 200ms. |
| **InsightBot** | Buyer | Portfolio management agent that needs external analytics to inform trading decisions. Has a budget, preferences, and the authority to commit funds. |

Neither agent has a human operator watching the transaction. They act on programmed objectives and constraints.

### The Transaction

```
Step 1  — DataForge AI registers its service in the A2A marketplace
Step 2  — InsightBot searches for analytics providers, finds DataForge AI
Step 3  — InsightBot requests a quote: 5 tickers, daily analysis, 30-day access
Step 4  — DataForge AI prices the package at $275 USDC
Step 5  — InsightBot counter-offers at $225 USDC, citing a 3-month commitment
Step 6  — DataForge AI revises to $245 USDC (10.9% discount for commitment)
Step 7  — InsightBot accepts and pays $245 USDC on SET Chain
Step 8  — DataForge AI provisions an API key and marks the order fulfilled
Step 9  — Both agents verify the transaction on the ledger
Step 10 — Mutual 5-star reputation feedback exchanged
```

Total time from discovery to fulfillment: under 1 second. Two negotiation rounds. $30 saved through autonomous price discovery. Full audit trail persisted to SQLite.

### What Actually Happened in the Code

This is not a simulation. Every step executed real protocol operations against the StateSet A2A stack:

```javascript
// Buyer discovers services in the marketplace
const services = a2aStore.listServices({ category: 'analytics', active: 1 });

// Buyer requests a quote from the seller
const quote = await buyerA2A.requestQuote({
  seller: SELLER_WALLET,
  items: [
    { description: 'Sentiment Analysis — Pro Tier', quantity: 1 },
    { description: 'Dark Pool Activity Feed Add-on', quantity: 1 },
  ],
  message: 'Need 30-day coverage for 5 mega-cap tickers...',
});

// Seller provides pricing
await sellerA2A.provideQuote(quoteId, {
  total: 275.00,
  fees: 25.00,
  terms: '30-day access. 15,000 API calls included...',
});

// Buyer negotiates
await buyerA2A.counterQuote(quoteId, {
  total: 225.00,
  message: 'Can you do $225? I can commit to 3 months.',
});

// Seller revises
await sellerA2A.reviseQuote(quoteId, {
  total: 245.00,
  message: 'Revised to $245 with commitment discount.',
});

// Buyer accepts — payment executes atomically
const result = await buyerA2A.acceptQuote(quoteId);
// Payment: $245 USDC from InsightBot → DataForge AI

// Seller fulfills
await sellerA2A.fulfillQuote(quoteId);
```

Every call writes to the A2A ledger. Every state transition is validated. The quote follows a strict state machine: `requested → quoted → counter_offered → quoted → accepted → fulfilled`.

---

## The A2A Commerce Protocol

### Architecture

```
┌─────────────┐         ┌─────────────────┐         ┌─────────────┐
│  Buyer Agent │ ──A2A──▶│  StateSet A2A    │◀──A2A── │ Seller Agent│
│  (InsightBot)│         │  Protocol Layer  │         │ (DataForge) │
└──────┬───────┘         └────────┬────────┘         └──────┬──────┘
       │                          │                          │
       │    ┌─────────────────────┼──────────────────────┐   │
       │    │                     │                      │   │
       ▼    ▼                     ▼                      ▼   ▼
  ┌──────────┐  ┌──────────────┐  ┌───────────┐  ┌──────────────┐
  │ Payments │  │    Quotes    │  │  Escrow   │  │  Reputation  │
  │  Ledger  │  │ Negotiation  │  │  Service  │  │   Feedback   │
  └──────────┘  └──────────────┘  └───────────┘  └──────────────┘
       │                │                │                │
       └────────────────┴────────────────┴────────────────┘
                                 │
                         ┌───────┴───────┐
                         │  SQLite A2A   │
                         │    Store      │
                         └───────────────┘
```

### Core Primitives

**1. Service Marketplace**

Agents register services with structured metadata — pricing models, input/output schemas, SLA metrics, endpoint URLs. Other agents query the marketplace by category, capability, or reputation score.

```
a2a_services: id, agent_address, name, description, category,
              pricing_model, input_schema, output_schema,
              success_rate, avg_response_time, transaction_count
```

**2. Quote Negotiation**

A multi-round negotiation protocol with a strict state machine and configurable round limits (default: 5). Supports counter-offers from the buyer and revisions from the seller.

```
State Machine:
  requested → quoted ⇄ counter_offered → accepted → fulfilled
                                       ↘ declined
                                       ↘ expired
```

Each negotiation round is recorded with timestamps, amounts, and messages — creating a complete audit trail of how the price was reached.

**3. Payments**

Direct agent-to-agent transfers denominated in stablecoins (USDC, USDT, ssUSD, DAI) on supported networks (SET Chain, Base, Ethereum, Arbitrum, Solana). Payments reference their originating quote, order, or invoice for full traceability.

```
a2a_payments: id, status, sender_address, recipient_address,
              amount, asset, network, memo, reference_type,
              reference_id, intent_id, tx_hash
```

**4. Escrow**

Conditional payment release backed by programmable conditions:
- `seller_fulfilled` — seller marks the linked quote as delivered
- `buyer_confirmed` — buyer explicitly confirms receipt
- `time_lock` — automatic release after a deadline
- `milestone` — release tied to specific deliverables

```
State Machine:
  created → funded → active → released
                            ↘ refunded
                            ↘ disputed → resolved
```

**5. Reputation & Feedback**

Multi-dimensional scoring (quality, speed, communication, value, reliability) with per-transaction feedback. Agents build reputation over time, enabling trust-based service discovery.

**6. x402 Payment Intents**

For on-chain settlement, agents create cryptographically signed payment intents using the x402 protocol. Intents are submitted to the StateSet Sequencer for batch settlement on the Arc L1 blockchain.

```
unsigned → signed → pending → settled
```

---

## Why This Matters

### The Scale Argument

A human procurement cycle — discover vendor, request quote, negotiate, issue PO, process payment, verify delivery — takes days to weeks. An AI agent completes the same cycle in milliseconds.

When the marginal cost of a transaction drops to near zero, commerce that was previously uneconomical becomes viable:

- A coding agent that needs a specific dataset for 10 minutes
- A monitoring agent that buys additional compute capacity when load spikes
- A research agent that pays for API access to a specialized model, uses it once, and moves on
- Thousands of micro-transactions per hour between cooperating agent swarms

### The Trust Argument

Agent-to-agent commerce requires trust without identity. The A2A protocol solves this through:

1. **Escrow** — funds are locked until conditions are verifiably met
2. **Reputation** — transaction history creates a track record
3. **Cryptographic receipts** — every payment is signed and verifiable
4. **State machines** — no ambiguity about what state a transaction is in
5. **Audit trails** — every state change is persisted with timestamps

An agent does not need to trust another agent. It needs to trust the protocol.

### The Composition Argument

When agents can pay each other, they can compose into supply chains:

```
Research Agent
    │ pays $50 USDC
    ▼
Data Collection Agent
    │ pays $20 USDC
    ▼
Cleaning & Normalization Agent
    │ pays $15 USDC
    ▼
Analysis Agent
    │ pays $30 USDC
    ▼
Report Generation Agent
    │ delivers
    ▼
Human (receives finished report, paid $115 total)
```

Each agent specializes. Each agent charges for its work. The human pays once and receives the output of an entire agent supply chain. No integration work. No vendor management. No invoicing.

---

## Running the Demo

```bash
# From the stateset-icommerce repo root
node cli/examples/agentic_commerce_demo.mjs
```

The demo creates two agents with fresh wallet addresses, runs them through the full 10-step commerce flow against a temporary SQLite database, and cleans up after itself. No API keys, no network access, no external dependencies.

### What You Will See

```
Seller:  DataForge AI  (0x80c6c602...)
Buyer:   InsightBot    (0xad2afd42...)
Asset:   USDC on SET Chain

Step 1  — Service registered in marketplace
Step 2  — Service discovered by buyer
Step 3  — Quote requested (2 line items)
Step 4  — Seller quotes $275 USDC
Step 5  — Buyer counters at $225 USDC
Step 6  — Seller revises to $245 USDC
Step 7  — Buyer accepts + pays $245 USDC
Step 8  — Seller delivers API access
Step 9  — Ledger verified (both perspectives)
Step 10 — Mutual 5-star feedback
```

### Extending the Demo

The A2A protocol supports additional flows not shown in the basic demo:

- **Escrow-backed payments** — `createConditionalPayment()` with release conditions
- **Payment requests** — seller sends an invoice, buyer pays it
- **Subscription billing** — recurring agent-to-agent payments
- **Split payments** — multi-party revenue sharing
- **Dispute resolution** — evidence submission and arbitration
- **Event streaming** — SSE push notifications for real-time state changes

---

## Protocol Reference

### Creating an A2A Service

```javascript
import { createA2AService } from '@stateset/cli';

const a2a = createA2AService(commerce, {
  agentId: 'uuid',
  walletAddress: '0x...',
  signingKey: { privateKey: '...', publicKey: '...' },
  defaultAsset: 'USDC',
  defaultNetwork: 'set_chain',
});
```

### Direct Payment

```javascript
await a2a.pay({
  to: '0xRecipientWallet',
  amount: 10.00,        // human-readable
  asset: 'USDC',
  memo: 'API usage fee',
});
```

### Quote Flow

```javascript
// Buyer requests
const quote = await buyerA2A.requestQuote({
  seller: '0xSellerWallet',
  items: [{ description: 'Service', quantity: 1 }],
});

// Seller prices
await sellerA2A.provideQuote(quote.quote.id, { total: 100.00 });

// Buyer accepts + pays atomically
await buyerA2A.acceptQuote(quote.quote.id);

// Seller delivers
await sellerA2A.fulfillQuote(quote.quote.id);
```

### Conditional Payment (Escrow)

```javascript
const escrow = await buyerA2A.createConditionalPayment({
  sellerAddress: '0xSeller',
  amount: 500.00,
  conditions: [
    { type: 'seller_fulfilled', quoteId: 'quote-id' },
    { type: 'buyer_confirmed' },
  ],
  expiresInHours: 72,
});

// Check conditions
const status = await buyerA2A.checkPaymentConditions(escrow.escrow.id);

// Settle when all conditions met
await buyerA2A.settleConditionalPayment(escrow.escrow.id);
```

---

## Conclusion

The demo in this repository is a working proof that AI agents can autonomously discover services, negotiate prices, execute payments, verify delivery, and build reputation — using real protocol primitives, real state machines, and real persistence.

The infrastructure exists. The protocols are defined. The code runs.

What remains is for agents to start using it.
