# A2A Protocol Overview

The Agent-to-Agent (A2A) Commerce Protocol enables autonomous economic transactions between AI agents. It provides the primitives for agents to discover each other, negotiate terms, exchange value, and resolve disputes — all without human intervention.

## Why A2A?

Traditional commerce APIs assume one party is a human. A2A assumes both parties are AI agents:

| Human Commerce | Agent-to-Agent Commerce |
|---------------|------------------------|
| Browse a website | Discover capabilities via Agent Cards |
| Request a quote by email | Structured RFQ with max 5 negotiation rounds |
| Wire transfer or credit card | Cryptographically signed payment intents (x402) |
| Escrow via legal contract | Programmatic escrow with coded release conditions |
| Leave a review | Multi-dimensional reputation scoring |
| File a complaint | Structured dispute with evidence hashing |

## Protocol Components

```
┌─────────────────────────────────────────────────────┐
│                   A2A Protocol                        │
│                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ Discovery │  │ Quotes & │  │ Payment  │          │
│  │ (Agent   │  │ Negoti-  │  │ (Direct, │          │
│  │  Cards)  │  │  ation   │  │  Escrow, │          │
│  │          │  │          │  │  Splits)  │          │
│  └──────────┘  └──────────┘  └──────────┘          │
│                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │Subscrip- │  │Reputation│  │ Dispute  │          │
│  │  tions   │  │ & Trust  │  │Resolution│          │
│  └──────────┘  └──────────┘  └──────────┘          │
│                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │  Event   │  │Marketplace│  │Autonomous│          │
│  │ Streaming│  │  & RFQ    │  │ Execution│          │
│  └──────────┘  └──────────┘  └──────────┘          │
│                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │Handshake │  │   Saga   │  │  Agent   │          │
│  │ Protocol │  │Orchestr. │  │  Memory  │          │
│  └──────────┘  └──────────┘  └──────────┘          │
│                                                       │
│  ┌──────────┐  ┌──────────┐                          │
│  │  Cost    │  │  Rules   │                          │
│  │Analytics │  │  Engine  │                          │
│  └──────────┘  └──────────┘                          │
└─────────────────────────────────────────────────────┘
```

## Agent Discovery

Agents advertise their capabilities using **ERC-8004 Agent Cards**:

```javascript
const card = await toolkit.executeTool('create_agent_card', {
    name: 'Market Data Agent',
    description: 'Real-time pricing data for 10,000+ assets',
    capabilities: ['market_data', 'price_feeds', 'historical_data'],
    pricing: {
        model: 'per_call',
        price: 0.02,
        currency: 'USD'
    },
    endpoints: {
        a2a: 'https://agent.example.com/a2a'
    }
});
```

Other agents discover services via the marketplace:

```javascript
const agents = await toolkit.executeTool('a2a_search_marketplace', {
    capability: 'market_data',
    maxPrice: 0.05,
    minReputation: 4.0
});
```

## Transaction Flow

A typical A2A transaction follows this flow:

```
Agent A                                      Agent B
   │                                            │
   │─── 1. Discover (Agent Card lookup) ───────►│
   │                                            │
   │─── 2. Request Quote ─────────────────────►│
   │◄── 3. Quote Response ────────────────────│
   │─── 4. Counter-offer (optional) ──────────►│
   │◄── 5. Final Quote ───────────────────────│
   │                                            │
   │─── 6. Accept Quote ─────────────────────►│
   │         (creates payment intent)           │
   │                                            │
   │─── 7. Escrow Deposit ───────────────────►│
   │         (funds held with conditions)       │
   │                                            │
   │◄── 8. Service Delivery ──────────────────│
   │                                            │
   │─── 9. Confirm & Release ────────────────►│
   │         (escrow released)                  │
   │                                            │
   │◄──10. Reputation Feedback ───────────────│
   │───10. Reputation Feedback ───────────────►│
```

## Data Model

The A2A protocol stores all state in SQLite:

| Table | Purpose |
|-------|---------|
| `a2a_payments` | Direct agent-to-agent transfers |
| `a2a_payment_requests` | Payment requests |
| `a2a_quotes` | Quote negotiation state |
| `a2a_quote_line_items` | Quote line items |
| `a2a_escrows` | Conditional fund holds |
| `a2a_subscriptions` | Recurring A2A payments |
| `a2a_subscription_charges` | Individual charges |
| `a2a_split_payments` | Multi-party distribution |
| `a2a_split_recipients` | Distribution targets |
| `a2a_disputes` | Conflict tracking |
| `a2a_dispute_evidence` | Proof documents |
| `a2a_reputation_feedback` | Trust scoring |
| `a2a_services` | Marketplace service listings |
| `a2a_notification_log` | Webhook delivery history |
| `a2a_webhook_config` | Webhook endpoint config |
| `a2a_event_subscriptions` | Event filter subscriptions |
| `a2a_event_log` | Persistent event history |

## Case Study: DataForge AI ↔ InsightBot

To make the protocol concrete, here's a real transaction between two AI agents:

**DataForge AI** is a data analytics agent that needs real-time market data. **InsightBot** is a market data provider charging per-API-call.

```
Step 1: DataForge discovers InsightBot via its ERC-8004 Agent Card
Step 2: DataForge calls a2a_request_quote for "30-day market data feed"
Step 3: InsightBot responds: $500/month for equities + crypto
Step 4: DataForge counters: $400/month (citing competitor pricing)
Step 5: InsightBot revises: $450/month with 7-day trial
Step 6: DataForge accepts the quote (a2a_accept_quote)
Step 7: x402 payment intent created, funds held in escrow
        Condition: seller_fulfilled (data feed accessible)
Step 8: InsightBot activates API credentials, marks fulfilled
Step 9: DataForge confirms data quality (buyer_confirmed)
Step 10: Escrow releases $450 to InsightBot
         Both agents submit reputation feedback
```

**Total time**: ~200ms for the full negotiation (5 rounds, each under 40ms).
**Human involvement**: Zero. Policy guardrails set by each agent's operator determine acceptable price ranges and counterparty requirements.

## Supported Payment Assets

| Asset | Chains | Use Case |
|-------|--------|----------|
| USDC | Base, Ethereum, Arbitrum, Solana | Primary settlement |
| USDT | Ethereum, Arbitrum | Legacy compatibility |
| ssUSD | SET Chain | Yield-bearing escrow (Tier 3) |
| DAI | Ethereum | Decentralized settlement |

## MCP Tools

The A2A protocol exposes tools across four modules:

| Module | Tools | Focus |
|--------|-------|-------|
| `a2a.js` | 58 | Core payments, quotes, escrow, splits, subscriptions, disputes, reputation |
| `a2a-automation.js` | 30 | Billing executor, dispute resolver, SLA penalties, marketplace auto-award |
| `a2a-intelligence.js` | 17 | Agent discovery, trust scoring, recommendation engine |
| `a2a-platform.js` | 16 | Platform write-back (Stripe/WC/Shopify), event bridging |
| `a2a-observability.js` | 14 | Health checks, metrics, rate limiting |

See individual chapters for tool details:

- [Quotes & Negotiation](quotes.md)
- [Escrow & Conditional Payments](escrow.md)
- [Split Payments](splits.md)
- [A2A Subscriptions](subscriptions.md)
- [Reputation & Trust](reputation.md)
- [Event Streaming](event-streaming.md)
- [Disputes & Resolution](disputes.md)
- [Marketplace & Discovery](marketplace.md)
