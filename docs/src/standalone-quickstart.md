# Standalone Quickstart

Get from zero to a running commerce engine in under 5 minutes. No cloud services, no blockchain, no API keys required.

## 1. Install (60 seconds)

```bash
# Install the CLI globally
npm install -g @stateset/cli

# Initialize a local commerce database
stateset-init --quickstart
```

This creates:
- `./store.db` — SQLite database with the full commerce schema
- `./.stateset/config.json` — Local configuration

## 2. Try It (30 seconds)

```bash
# Query your store (AI-powered, read-only by default)
stateset "show me all customers"
stateset "what products are low on stock?"
stateset "what is my revenue this month?"

# Write operations require --apply (safe by default)
stateset --apply "create a customer named Alice with email alice@example.com"
stateset --apply "create a product called Widget at $29.99 with 100 in stock"
```

Tip: `ss` is a shorthand alias for `stateset`.

## 3. Import from Shopify (2 minutes)

```bash
# Import from Shopify CSV exports
stateset --apply "import shopify data from csv" --filePath ./exports/

# Or connect via Shopify API
stateset --apply "import shopify data" \
  --shopifyDomain mystore.myshopify.com \
  --shopifyAccessToken shpat_...
```

## 4. Connect Stripe Webhooks (3 minutes)

```bash
# Start the webhook receiver
stateset-webhooks --stripe-secret whsec_... --port 3000

# In another terminal, test with Stripe CLI
stripe listen --forward-to localhost:3000/webhooks/stripe
stripe trigger payment_intent.succeeded
```

Stripe payments, refunds, and subscription events sync into your local database in real time.

## 5. Add Business Rules (2 minutes)

Create a `policies/returns.yaml` file:

```yaml
name: Auto-Approve Small Returns
domain: returns
rules:
  - name: auto-approve-under-50
    conditions:
      - field: amount
        operator: less_than
        value: 50
      - field: days_since_purchase
        operator: less_than
        value: 30
    actions:
      - type: allow
        reason: "Return is under $50 and within 30-day window"
        remediation: "Auto-approved per return policy"
  - name: require-review-large
    conditions:
      - field: amount
        operator: greater_than_or_equal
        value: 50
    actions:
      - type: require-approval
        reason: "Returns over $50 require manual review"
        remediation: "Submit for manager approval"
```

```bash
# Evaluate a return against policies
stateset "evaluate return policy for a $25 return on order ORD-001"
```

## 6. Direct CLI (No AI Required)

For scripting and automation, use the direct CLI:

```bash
# CRUD operations without AI routing
stateset-direct customers list
stateset-direct orders get ORD-001
stateset-direct inventory adjust SKU-001 --quantity 50 --reason "shipment received"
```

## 7. Agent-Specific Commands

Each commerce domain has a dedicated agent:

```bash
stateset-orders "show pending orders ready to ship"
stateset-inventory "what SKUs are below reorder point?"
stateset-returns "process return for order ORD-001"
stateset-analytics "forecast revenue for next quarter"
stateset-checkout "create a cart for alice@example.com and add 2 widgets"
```

## What's Included

| Feature | Description |
|---------|-------------|
| **MCP tools** | Orders, inventory, payments, returns, carts, analytics, tax, promotions, subscriptions, manufacturing, A2A |
| **Policy engine** | YAML business rules with explainable denials and preview-before-execute |
| **Shopify adapter** | CSV import, API sync, webhook handlers |
| **Stripe adapter** | Real-time webhook sync for payments, subscriptions, invoices |
| **WooCommerce adapter** | API import and webhook sync |
| **Multi-currency** | Exchange rates, conversions, 150+ currencies |
| **Tax engine** | US state tax, EU VAT, Canadian GST/PST/HST |
| **Analytics** | Revenue forecasts, demand prediction, inventory health |

## What's Next (Optional)

These features are available when you're ready — none are required to use iCommerce:

- **Sequencer sync** — Connect to a StateSet Sequencer for multi-agent coordination and cryptographic audit trails. Add a `.stateset/sync.json` file to enable.
- **Stablecoin payments** — Accept USDC on Base, Solana, or other chains. Run `stateset pay --chains` to see options.
- **On-chain settlement** — Anchor commerce events to SET Chain for independent verifiability.

See [Product Tiers](tiers.md) for the full feature matrix.
