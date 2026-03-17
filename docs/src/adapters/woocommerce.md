# WooCommerce Adapter

Sync WooCommerce stores via REST API bulk import and real-time webhooks.

## Setup

### 1. Generate WooCommerce API Keys

In WooCommerce: Settings > Advanced > REST API > Add Key

- Permission: Read/Write
- Note the Consumer Key and Consumer Secret

### 2. Configure

```bash
stateset --apply "configure woocommerce adapter" \
    --wcUrl https://mystore.com \
    --wcKey ck_... \
    --wcSecret cs_...
```

### 3. Import Existing Data

```bash
stateset --apply "import woocommerce data"
```

This imports:
- Customers
- Products (with variants)
- Orders (with line items)
- Inventory levels

### 4. Set Up Webhooks

In WooCommerce: Settings > Advanced > Webhooks

Create webhooks for:

| Topic | Delivery URL |
|-------|-------------|
| `customer.created` | `https://your-domain/webhooks/woocommerce` |
| `customer.updated` | `https://your-domain/webhooks/woocommerce` |
| `product.created` | `https://your-domain/webhooks/woocommerce` |
| `product.updated` | `https://your-domain/webhooks/woocommerce` |
| `order.created` | `https://your-domain/webhooks/woocommerce` |
| `order.updated` | `https://your-domain/webhooks/woocommerce` |

## API Client

The adapter uses WooCommerce REST API v3 with Basic Auth over HTTPS:

```javascript
import { WooCommerceClient } from '@stateset/cli/adapters/woocommerce/client';

const client = new WooCommerceClient({
    url: 'https://mystore.com',
    consumerKey: 'ck_...',
    consumerSecret: 'cs_...'
});

const products = await client.listProducts({ per_page: 100 });
```

## Order Status Mapping

| WooCommerce Status | iCommerce Status |
|-------------------|-----------------|
| `pending` | `pending` |
| `processing` | `processing` |
| `on-hold` | `pending` |
| `completed` | `delivered` |
| `cancelled` | `cancelled` |
| `refunded` | `refunded` |
| `failed` | `failed` |

## SSRF Protection

The adapter validates all configured URLs and webhook endpoints against a comprehensive blocklist:

| Category | Blocked |
|----------|---------|
| Loopback | `127.0.0.0/8`, `::1`, `localhost` |
| Private (RFC 1918) | `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` |
| Link-local | `169.254.0.0/16`, `fe80::/10` |
| Internal TLDs | `.local`, `.internal`, `.localhost` |
| Non-HTTP schemes | `file:`, `ftp:`, `data:`, etc. |

This prevents SSRF attacks where a malicious webhook URL could probe internal infrastructure. The blocklist is applied to:

- WooCommerce store URL during configuration
- Webhook callback URLs during registration
- API redirect URLs during OAuth flows

## Write-Back

Push order status updates back to WooCommerce:

```javascript
await toolkit.executeTool('woocommerce_write_back', {
    type: 'order_status',
    wcOrderId: 123,
    status: 'completed'
});
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `configure_woocommerce` | Set up WC adapter |
| `import_woocommerce` | Bulk import data |
| `woocommerce_write_back` | Push updates to WC |
