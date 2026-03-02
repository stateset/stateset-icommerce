# Stripe Integration Guide

Sync payment events from Stripe into StateSet iCommerce in real time.

## Overview

The Stripe adapter is **webhook-first**: Stripe pushes events to your webhook endpoint, and iCommerce processes them into local records. Supported events:

- `payment_intent.succeeded` / `payment_intent.payment_failed` / `payment_intent.canceled`
- `charge.succeeded` / `charge.refunded` / `charge.dispute.created`
- `customer.created` / `customer.updated`
- `customer.subscription.created` / `customer.subscription.updated` / `customer.subscription.deleted`
- `invoice.paid` / `invoice.payment_failed`

## Quick Start

### 1. Install & Initialize

```bash
npm install -g @stateset/cli
stateset-init --quickstart
```

### 2. Start the Webhook Server

```bash
stateset-webhooks --stripe-secret whsec_YOUR_SECRET --port 3000
```

### 3. Configure Stripe Webhook Endpoint

In the [Stripe Dashboard](https://dashboard.stripe.com/webhooks):

1. Click **Add endpoint**
2. Set URL to `https://your-server.com/webhooks/stripe`
3. Select events (or select all 13 supported events)
4. Copy the signing secret (`whsec_...`) — use it as `--stripe-secret`

### 4. Test with Stripe CLI

```bash
# Install Stripe CLI
brew install stripe/stripe-cli/stripe

# Forward events to local server
stripe listen --forward-to localhost:3000/webhooks/stripe

# Trigger a test event
stripe trigger payment_intent.succeeded
```

### 5. Verify

```bash
stateset "show me all payments"
stateset "list customers"
```

## Signature Verification

All webhooks are verified using Stripe's v1 signature scheme:

- Parses `Stripe-Signature` header (`t=timestamp,v1=hmac`)
- Computes HMAC-SHA256 of `${timestamp}.${rawBody}` with your webhook secret
- Uses timing-safe comparison to prevent timing attacks
- Rejects events older than 5 minutes (configurable)

## Data Mapping

| Stripe Object | StateSet Entity | Key Fields |
|---------------|-----------------|------------|
| PaymentIntent | Payment | amount (cents→decimal), status, currency, method |
| Charge | Payment | amount, status, paymentIntentId |
| Customer | Customer | email, firstName, lastName, phone |
| Subscription | Subscription | status, planId, amount, interval |
| Invoice | Invoice | amount, amountPaid, status, number |
| Refund | Payment (refund) | amount, reason, chargeId |
| Dispute | Payment (dispute) | amount, reason, status |

## Programmatic Usage

```javascript
import { getAdapter } from '@stateset/cli/standalone';

const stripe = await getAdapter('stripe', { webhookSecret: 'whsec_...' });

// Handle a webhook event
const result = stripe.handleWebhook('payment_intent.succeeded', payload);
// { externalId: 'pi_...', data: { amount: '50.00', status: 'completed', ... }, raw: {...} }

// Verify signature
const verification = stripe.verifyWebhookSignature(rawBody, signatureHeader);
// { valid: true } or { valid: false, error: '...' }
```

## MCP Tool

Configure Stripe webhooks through the AI interface:

```bash
stateset --apply "configure stripe webhooks with secret whsec_test123"
```
