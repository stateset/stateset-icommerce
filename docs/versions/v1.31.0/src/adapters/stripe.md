# Stripe Adapter

Real-time sync of Stripe payments, subscriptions, and customers into iCommerce via webhooks.

## Setup

### 1. Configure Webhook Secret

```bash
stateset-webhooks --stripe-secret whsec_your_secret --port 3000
```

### 2. Register Webhook in Stripe

Point your Stripe webhook endpoint to:

```
https://your-domain.com/webhooks/stripe
```

Or for local development:

```bash
stripe listen --forward-to localhost:3000/webhooks/stripe
```

### 3. Test

```bash
stripe trigger payment_intent.succeeded
```

## Supported Events

| Stripe Event | iCommerce Action |
|-------------|-----------------|
| `payment_intent.succeeded` | Create/update payment (status: captured) |
| `payment_intent.payment_failed` | Create/update payment (status: failed) |
| `payment_intent.canceled` | Create/update payment (status: cancelled) |
| `charge.succeeded` | Create payment record |
| `charge.refunded` | Create refund record |
| `customer.created` | Create customer |
| `customer.updated` | Update customer |
| `subscription.created` | Create subscription |
| `subscription.updated` | Update subscription |
| `subscription.deleted` | Cancel subscription |
| `invoice.paid` | Mark invoice as paid |
| `invoice.payment_failed` | Flag invoice payment failure |
| `dispute.created` | Create dispute record |

## Signature Verification

The adapter uses Stripe's v1 HMAC-SHA256 signature scheme:

1. Extract timestamp and signature from the `Stripe-Signature` header
2. Construct the signed payload: `${timestamp}.${rawBody}`
3. Compute HMAC-SHA256 with the webhook secret
4. Compare using constant-time comparison
5. Reject if timestamp is older than 5 minutes (replay protection)

## Data Mapping

| Stripe Object | iCommerce Entity |
|--------------|-----------------|
| PaymentIntent | Payment |
| Charge | Payment |
| Customer | Customer |
| Subscription | Subscription |
| Invoice | Invoice |
| Refund | Payment (refund) |
| Dispute | Payment (dispute) |

## Write-Back

The adapter can write status updates back to Stripe:

```javascript
// Via MCP tool
await toolkit.executeTool('stripe_write_back', {
    type: 'refund',
    stripePaymentIntentId: 'pi_...',
    amount: 29.99,
    reason: 'Customer return'
});
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `configure_stripe` | Set up Stripe adapter |
| `stripe_webhook_status` | Check webhook health |
| `stripe_write_back` | Push updates to Stripe |

## Signature Verification Deep-Dive

The adapter verifies every webhook payload using Stripe's v1 HMAC-SHA256 scheme:

```
1. Extract from Stripe-Signature header:
   t=1679000000,v1=abc123def456...

2. Construct the signed payload:
   signed_payload = "${timestamp}.${raw_body}"

3. Compute expected signature:
   expected = HMAC-SHA256(webhook_secret, signed_payload)

4. Compare:
   constant_time_compare(expected, received_v1)

5. Check timestamp freshness:
   reject if |now - timestamp| > 300 seconds (replay protection)
```

Signature comparison uses constant-time algorithms to prevent timing side-channel attacks.

## Local Development

Use the Stripe CLI to forward webhooks to your local machine:

```bash
# Terminal 1: Start webhook server
stateset-webhooks --stripe-secret whsec_... --port 3000

# Terminal 2: Forward Stripe events
stripe listen --forward-to localhost:3000/webhooks/stripe

# Terminal 3: Trigger test events
stripe trigger payment_intent.succeeded
stripe trigger customer.created
stripe trigger invoice.paid
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid signature | Returns 401, event rejected, logged |
| Unsupported event type | Returns 200 (acknowledged), no action taken |
| Database error during sync | Returns 500, Stripe retries automatically |
| Duplicate event (same ID) | Idempotent — returns 200, no duplicate created |

Stripe retries failed webhooks with exponential backoff for up to 3 days.

## Programmatic Usage

```javascript
import { StripeAdapter } from '@stateset/cli/adapters/stripe';

const adapter = new StripeAdapter({
    webhookSecret: 'whsec_...',
    commerce: commerceInstance
});

// Process a webhook event
await adapter.handleWebhook(req.headers, req.body);
```
