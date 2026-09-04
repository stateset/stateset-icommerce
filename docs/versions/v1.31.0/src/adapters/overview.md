# Platform Adapters

iCommerce connects to existing commerce platforms via adapters that sync data in real-time through webhooks and API imports.

## Supported Platforms

| Platform | Import | Webhooks | Write-Back |
|----------|--------|----------|-----------|
| [Stripe](stripe.md) | Via webhooks | 13 events | Refunds, cancellations |
| [WooCommerce](woocommerce.md) | REST API bulk import | 8 events | Order status updates |
| [Shopify](shopify.md) | CSV + API import | Webhook sync | Order status updates |

## Architecture

```
External Platform              iCommerce
     │                           │
     │── Webhook Event ─────────►│
     │   (signed payload)        │── Verify Signature
     │                           │── Map to Domain Model
     │                           │── Store in SQLite
     │                           │── Emit Commerce Event
     │                           │
     │◄── Write-Back ───────────│
     │   (status update)         │── Update External Record
```

## Common Features

All adapters share:

- **Signature verification**: Webhook payloads are verified using platform-specific signing (HMAC-SHA256 for Stripe/WooCommerce)
- **SSRF protection**: Webhook URLs are validated against private IP blocklists
- **ID mapping**: External IDs (Shopify ID, Stripe charge ID) are mapped to internal StateSet IDs via `id-map-store.js`
- **Entity mapping**: Platform-specific data structures are normalized to iCommerce domain models
- **Event emission**: Imported/synced data triggers standard commerce events

## Base Classes

Adapters extend common base classes:

- `base-adapter.js` — Platform adapter lifecycle (connect, disconnect, sync)
- `base-importer.js` — Bulk data import (validate, transform, batch insert)
- `id-map-store.js` — Bidirectional ID translation

## Configuration

In `.stateset/config.json`:

```json
{
    "adapters": {
        "active": ["stripe", "woocommerce", "shopify"]
    },
    "webhooks": {
        "port": 3000,
        "sources": ["stripe", "woocommerce", "shopify"]
    }
}
```

## Which Adapter Should I Use?

| Scenario | Recommended |
|----------|-------------|
| Existing Shopify store, want to add AI agents | **Shopify** — CSV import for initial data, webhooks for real-time |
| Stripe for payments only (no store platform) | **Stripe** — webhook sync for payment events |
| WooCommerce / WordPress store | **WooCommerce** — REST API import + webhook sync |
| Multiple platforms simultaneously | All three — each adapter is independent |
| No existing platform (greenfield) | None needed — use iCommerce directly |

## Webhook Server

Start the standalone webhook server:

```bash
stateset-webhooks \
    --stripe-secret whsec_... \
    --wc-secret wc_... \
    --shopify-secret shpss_... \
    --port 3000
```

The server listens for webhook events from all configured platforms and syncs them into the local database.

### Webhook Server Endpoints

| Endpoint | Platform |
|----------|----------|
| `POST /webhooks/stripe` | Stripe events |
| `POST /webhooks/shopify` | Shopify events |
| `POST /webhooks/woocommerce` | WooCommerce events |
| `GET /health` | Server health check |

## Troubleshooting

### Events not appearing in the database

1. Check webhook signature: the secret must match exactly
2. Verify the server is running: `curl http://localhost:3000/health`
3. Check logs for signature verification failures
4. Ensure the database path matches your commerce instance

### Duplicate records after import + webhook

ID mapping prevents duplicates. If you see duplicates, check that the webhook event includes the same external ID as the imported record. The `id-map-store.js` bidirectional map handles translation.

## Webhook Event Mapping

Each platform's events map to iCommerce commerce events:

### Stripe Events

| Stripe Event | iCommerce Event | Entity |
|-------------|----------------|--------|
| `payment_intent.succeeded` | `payment.captured` | Payment |
| `payment_intent.payment_failed` | `payment.failed` | Payment |
| `charge.refunded` | `payment.refunded` | Payment |
| `customer.created` | `customer.created` | Customer |
| `customer.updated` | `customer.updated` | Customer |
| `invoice.paid` | `invoice.paid` | Invoice |
| `invoice.payment_failed` | `invoice.overdue` | Invoice |
| `subscription.created` | `subscription.created` | Subscription |
| `subscription.updated` | `subscription.updated` | Subscription |
| `subscription.deleted` | `subscription.cancelled` | Subscription |
| `charge.dispute.created` | `dispute.filed` | Dispute |

### WooCommerce Events

| WooCommerce Topic | iCommerce Event | Entity |
|------------------|----------------|--------|
| `order.created` | `order.created` | Order |
| `order.updated` | `order.updated` | Order |
| `product.updated` | `product.updated` | Product |
| `customer.created` | `customer.created` | Customer |
| `customer.updated` | `customer.updated` | Customer |
| `subscription.updated` | `subscription.updated` | Subscription |

### Shopify Events

| Shopify Topic | iCommerce Event | Entity |
|--------------|----------------|--------|
| `orders/create` | `order.created` | Order |
| `orders/updated` | `order.updated` | Order |
| `products/update` | `product.updated` | Product |
| `customers/create` | `customer.created` | Customer |
| `inventory_levels/update` | `inventory.adjusted` | Inventory |

## Adapter Configuration

### Stripe Setup

```bash
# Set Stripe API key and webhook secret
export STRIPE_API_KEY=sk_live_...
export STRIPE_WEBHOOK_SECRET=whsec_...

# Start webhook listener
stateset-webhooks --stripe-secret $STRIPE_WEBHOOK_SECRET --port 3000
```

In your Stripe dashboard, configure the webhook endpoint: `https://your-server.com/webhooks/stripe`

### WooCommerce Setup

```bash
# Set WooCommerce credentials
export WC_SITE_URL=https://mystore.example.com
export WC_CONSUMER_KEY=ck_...
export WC_CONSUMER_SECRET=cs_...

# Import existing data then enable webhooks
stateset --apply "import woocommerce data"
stateset-webhooks --wc-secret $WC_CONSUMER_SECRET --port 3000
```

### Shopify Setup

```bash
# Set Shopify credentials
export SHOPIFY_SHOP=mystore.myshopify.com
export SHOPIFY_API_KEY=shpat_...
export SHOPIFY_WEBHOOK_SECRET=shpss_...

# Import from CSV exports
stateset --apply "import shopify data from ./exports/"
stateset-webhooks --shopify-secret $SHOPIFY_WEBHOOK_SECRET --port 3000
```

## Conflict Resolution

When the same entity is modified on both the external platform and iCommerce:

| Scenario | Resolution |
|----------|-----------|
| External update arrives via webhook | External wins (overwrite local) |
| Local update, then webhook arrives | External wins (most recent webhook) |
| Write-back to external platform | iCommerce pushes status to external |
| Simultaneous edits | Last-write-wins based on timestamp |

For multi-agent scenarios where conflicts need careful handling, use [VES sync](../guides/sync.md) with configurable conflict resolution strategies.

## Writing Custom Adapters

Extend `base-adapter.js` to create an adapter for a new platform:

```javascript
import { BaseAdapter } from '@stateset/cli/adapters/base-adapter';

class CustomAdapter extends BaseAdapter {
    mapToStateSet(externalEntity, entityType) {
        // Transform external format → iCommerce format
    }

    mapFromStateSet(statesetEntity, entityType) {
        // Transform iCommerce format → external format
    }

    async fetchBatches(entityType, options) {
        // Paginated API fetch from external platform
    }

    async handleWebhook(event) {
        // Process incoming webhook event
    }
}
```

## Further Reading

- [Stripe Adapter](stripe.md) — Stripe-specific events and write-back
- [WooCommerce Adapter](woocommerce.md) — REST API import and webhook topics
- [Shopify Adapter](shopify.md) — CSV import and API sync
- [Data Migration & Import](../guides/data-migration.md) — Bulk import guide
