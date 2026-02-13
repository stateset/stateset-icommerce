# Agent-to-Agent (A2A) Commerce

Easy payments between AI agents for the iCommerce ecosystem.

## Overview

The A2A module enables AI agents to:
- **Pay each other directly** - Send stablecoins (USDC, USDT, ssUSD) to other agents
- **Request payments** - Create payment requests that other agents can pay
- **Quote/Purchase flows** - Request quotes, negotiate, accept, and fulfill

## Quick Start

### Direct Payment

```javascript
import { createA2AService } from '@stateset/cli';

const a2a = createA2AService(commerce, {
  agentId: 'my-agent-id',
  walletAddress: '0x1234...',
  signingKey: { privateKey, publicKey },
});

// Pay another agent $10 USDC
const result = await a2a.pay({
  to: '0xRecipientWallet',
  amount: 10.00,
  memo: 'API call fee',
});

console.log('Payment:', result.payment);
```

### Request Payment

```javascript
// Create a payment request
const request = await a2a.requestPayment({
  amount: 25.00,
  description: 'Data processing fee',
  expiresInHours: 48,
});

console.log('Request ID:', request.request.id);
console.log('Payment URL:', request.paymentUrl);
```

### Pay a Request

```javascript
// Pay an existing payment request
const payment = await a2a.payRequest('request-uuid');
console.log('Paid:', payment.request.fullyPaid);
```

### Quote Flow

```javascript
// 1. Buyer requests quote
const quoteRequest = await buyerA2a.requestQuote({
  seller: '0xSellerWallet',
  items: [
    { description: 'Premium Data Access', quantity: 1 },
    { description: 'API Credits', quantity: 100 },
  ],
  message: 'Need pricing for monthly access',
});

// 2. Seller provides quote
const quote = await sellerA2a.provideQuote(quoteRequest.quote.id, {
  total: 150.00,
  fees: 5.00,
  tax: 0,
  expiresInHours: 24,
  terms: 'Access valid for 30 days',
  message: 'Thanks for your interest!',
});

// 3. Buyer accepts and pays
const payment = await buyerA2a.acceptQuote(quote.quote.id);
console.log('Paid:', payment.payment);

// 4. Seller fulfills
await sellerA2a.fulfillQuote(quote.quote.id);
```

## MCP Tools

The A2A module provides these MCP tools for use in agent workflows:

### Payments
- `a2a_pay` - Pay another agent directly
- `a2a_request_payment` - Request payment from another agent
- `a2a_pay_request` - Pay an existing payment request

### Quotes
- `a2a_request_quote` - Request a price quote from a seller
- `a2a_provide_quote` - Respond to a quote request (seller)
- `a2a_accept_quote` - Accept a quote and pay
- `a2a_decline_quote` - Decline a quote
- `a2a_fulfill_quote` - Mark a quote as fulfilled (seller)

### Queries
- `a2a_list_payments` - List payments sent/received
- `a2a_list_payment_requests` - List payment requests
- `a2a_list_quotes` - List quotes
- `a2a_get_balance` - Get payment summary

### Discovery
- `a2a_discover_agents` - Find agents with specific capabilities

## CLI Usage

```bash
# Pay another agent
stateset --apply "pay 10 USDC to 0x1234..."

# Request payment
stateset --apply "request $25 for 'data processing'"

# Find sellers
stateset "find agents that can sell data services"

# Quote flow
stateset --apply "request a quote from 0xSeller for 100 API credits"
```

## Data Models

### A2APayment
Direct transfer between agents:
- `id` - Payment UUID
- `status` - pending, submitted, completed, failed, cancelled, refunded
- `sender_address` / `recipient_address` - Wallet addresses
- `amount` - Amount in smallest unit
- `asset` - USDC, USDT, ssUSD, DAI
- `network` - set_chain, base, ethereum, arbitrum
- `memo` - Human-readable description
- `tx_hash` - On-chain transaction (when settled)

### PaymentRequest
Request payment from another agent:
- `id` - Request UUID
- `status` - pending, viewed, processing, paid, declined, expired
- `requester_address` - Who's requesting payment
- `payer_address` - Who should pay (optional)
- `amount` - Requested amount
- `description` - What the payment is for
- `expires_at` - When the request expires
- `allow_partial` - Whether partial payments are accepted
- `callback_url` - Webhook for payment notifications

### A2AQuote
Price quote for goods/services:
- `id` - Quote UUID
- `status` - requested, quoted, accepted, declined, expired, fulfilled
- `buyer_address` / `seller_address` - Participants
- `items` - Line items
- `subtotal`, `fees`, `tax`, `total` - Pricing
- `expires_at` - Quote validity
- `terms` - Terms and conditions

## Integration with x402

A2A payments use the x402 protocol for settlement:
1. Payment intent is created and signed locally
2. Intent is submitted to the sequencer for batching
3. Batched payments are settled on Set Chain L2
4. Receipt with Merkle proof is returned

For immediate settlement, connect a `sequencerClient` when creating the A2A service.

## Security

- All payments require Ed25519 signatures
- Idempotency keys prevent duplicate payments
- Expiry timestamps prevent stale requests
- Trust levels from Agent Cards determine transaction limits
