# WooCommerce Integration Guide

Import data from WooCommerce and receive real-time webhook updates.

## Overview

The WooCommerce adapter supports both **API import** (bulk data sync) and **webhook sync** (real-time updates). It connects via WooCommerce REST API v3 with Basic Auth over HTTPS.

### Supported Entities

| Entity | API Import | Webhooks |
|--------|-----------|----------|
| Customers | Yes | `customer.created`, `customer.updated` |
| Products | Yes | `product.created`, `product.updated` |
| Orders | Yes | `order.created`, `order.updated` |
| Inventory | Yes (via products) | — |

## Quick Start

### 1. Generate API Keys

In WooCommerce Admin:

1. Go to **WooCommerce → Settings → Advanced → REST API**
2. Click **Add Key**
3. Set permissions to **Read** (or Read/Write for export)
4. Copy the **Consumer Key** and **Consumer Secret**

### 2. Import Existing Data

```bash
# Import via AI interface
stateset --apply "import data from WooCommerce at https://mystore.com with key ck_... and secret cs_..."

# Or via programmatic API
```

```javascript
import { getAdapter, IdMapStore, DataImporter } from '@stateset/cli/standalone';

const adapter = await getAdapter('woocommerce', {
  siteUrl: 'https://mystore.example.com',
  consumerKey: 'ck_...',
  consumerSecret: 'cs_...',
});

// Test connection
const connected = await adapter.testConnection();
console.log('Connected:', connected);
```

### 3. Configure Webhooks

In WooCommerce Admin:

1. Go to **WooCommerce → Settings → Advanced → Webhooks**
2. Add webhooks for each topic:
   - `order.created` → `https://your-server.com/webhooks/woocommerce`
   - `order.updated` → same URL
   - `product.created` → same URL
   - `product.updated` → same URL
   - `customer.created` → same URL
   - `customer.updated` → same URL
3. Set **Secret** — use this as `--woocommerce-secret`

### 4. Start Webhook Server

```bash
stateset-webhooks --woocommerce-secret YOUR_SECRET --port 3000
```

### 5. Verify

```bash
stateset "show me all products"
stateset "list orders"
stateset "check inventory levels"
```

## Signature Verification

WooCommerce webhooks are verified using HMAC-SHA256:

- Reads `X-WC-Webhook-Signature` header (base64-encoded)
- Computes HMAC-SHA256 of the raw body with your webhook secret
- Uses timing-safe comparison via `crypto.timingSafeEqual`

## Data Mapping

| WooCommerce | StateSet | Status Mapping |
|-------------|----------|----------------|
| Customer | Customer | Active |
| Product | Product | publish→active, draft→draft |
| Order | Order | pending→pending, processing→processing, completed→shipped, cancelled→cancelled |
| Product (stock) | Inventory | stock_quantity, stock_status |

### Order Status Map

| WooCommerce Status | StateSet Status |
|-------------------|-----------------|
| `pending` | `pending` |
| `processing` | `processing` |
| `on-hold` | `pending` |
| `completed` | `shipped` |
| `cancelled` | `cancelled` |
| `refunded` | `refunded` |
| `failed` | `failed` |

## SSRF Protection

The WooCommerce client validates all URLs before making requests:

- Blocks `localhost`, `127.0.0.1`, `0.0.0.0`, `::1`
- Blocks private IP ranges (`10.x`, `172.16-31.x`, `192.168.x`)
- Blocks `.local` and `.internal` TLDs

## Running Both Import + Webhooks

For a complete sync setup:

```bash
# 1. Initial bulk import
stateset --apply "import all data from WooCommerce"

# 2. Start webhook server for real-time updates
stateset-webhooks --woocommerce-secret YOUR_SECRET --port 3000

# 3. (Optional) Also accept Stripe payment webhooks
stateset-webhooks \
  --woocommerce-secret YOUR_WC_SECRET \
  --stripe-secret whsec_YOUR_STRIPE_SECRET \
  --port 3000
```
