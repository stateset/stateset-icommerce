# CLI Quickstart

Zero to a running commerce engine in about five minutes. No cloud services, no
blockchain, no API keys beyond a model provider key for the natural-language
commands.

This is the CLI-first path. The other two entry points are the
[Rust quickstart](https://github.com/stateset/stateset-icommerce/blob/master/QUICKSTART.md)
and the storefront generator, `npm create stateset-app@latest my-store`.

## 1. Install and initialize

```bash
npm install -g @stateset/cli
stateset-init --quickstart
```

`--quickstart` is a zero-prompt setup. It creates:

- `./store.db` — a SQLite database with the full commerce schema and demo data
- `./.stateset/config.json` — local standalone configuration

Other useful flags: `--demo` seeds demo data without the standalone config,
`--db <path>` picks a different database location, and `--force` overwrites an
existing one.

Tip: `ss` is a shorthand alias for `stateset`.

## 2. Ask it things

`stateset` is a natural-language agent over the embedded engine. Reads run
freely; **writes are previewed unless you pass `--apply`**.

```bash
stateset "show me all customers"
stateset "what products are low on stock?"
stateset "what is my revenue this month?"

stateset --apply "create a customer named Alice with email alice@example.com"
stateset --apply "create a product called Widget at $29.99 with 100 in stock"
```

`stateset --help` lists the full flag set. It uses strict argument parsing, so
an unrecognized flag is an error rather than something silently ignored —
details that belong to the request go in the request text, not in flags.

## 3. Import from Shopify

The import agent understands CSV exports and live API credentials. Put the
detail in the request; there are no `--shopify*` flags.

```bash
# From Shopify Admin → Settings → Export, then:
stateset-import --apply "import shopify data from CSV at ./shopify-exports/"

# Preview first (no --apply) to see what would be created:
stateset-import "import shopify data from CSV at ./shopify-exports/"
```

`stateset-import` also handles JSON and generic CSV, and keeps an ID-mapping
store so re-running an import updates rather than duplicates. See
[Data Migration & Import](guides/data-migration.md).

## 4. Connect Stripe webhooks

```bash
stateset-webhooks --stripe-secret whsec_... --port 3000
```

```bash
# In another terminal, with the Stripe CLI:
stripe listen --forward-to localhost:3000/webhooks/stripe
stripe trigger payment_intent.succeeded
```

Payments, refunds, and subscription events sync into the local database in real
time. `--shopify-secret` and `--woocommerce-secret` enable those receivers on
the same port.

## 5. Add business rules

Policies are YAML or JSON files in the **`policies/` subdirectory of the policy
store**. The store defaults to `.stateset/` next to your database, so the
default location is `.stateset/policies/`:

```bash
mkdir -p .stateset/policies
```

`.stateset/policies/returns.yaml`:

```yaml
name: Return Approval
domain: returns
rules:
  - name: auto-approve-small-recent
    priority: 10
    stopOnMatch: true
    conditions:
      logic: and
      conditions:
        - field: amount
          operator: lt
          value: 50
        - field: days_since_purchase
          operator: lt
          value: 30
    action:
      type: allow
      reason: Under $50 and within the 30-day window
      remediation: Auto-approved per return policy

  - name: review-large-returns
    priority: 5
    conditions:
      field: amount
      operator: gte
      value: 50
    action:
      type: deny
      reason: Returns of $50 or more need manual review
      remediation: Submit for manager approval
```

Three things the schema actually requires, each of which is easy to get wrong:

- Every rule has one **`action`** (singular), not a list of `actions`.
- Operators are the short names from `Operators` in
  `cli/src/policies/engine.js`: `eq`, `neq`, `gt`, `gte`, `lt`, `lte`,
  `contains`, `startsWith`, `endsWith`, `matches`, `in`, `notIn`, `isEmpty`,
  `isNotEmpty`, `isNull`, `isNotNull`, `isTrue`, `isFalse`, `between`,
  `divisibleBy` — not `less_than` or `greater_than_or_equal`.
- Action types are `allow`, `deny`, `agent`, `workflow`, `notify`, and
  `transform`. Precedence is deny-overrides: an explicit deny beats an
  explicit allow.

A file the loader cannot parse is skipped, so a typo shows up as a policy that
never fires rather than as an error.

To keep policies somewhere else, point `STATESET_POLICY_DIR` at the store root
(the engine still looks for `policies/` inside it):

```bash
export STATESET_POLICY_DIR=/etc/stateset
# loads /etc/stateset/policies/*.yaml|*.yml|*.json
```

Then evaluate against them:

```bash
stateset "evaluate return policy for a $25 return on order ORD-001"
```

Policy files are operator-owned. They are read from disk, never from a model's
tool arguments. See [Policy Engine](policy/engine.md).

## 6. Direct CLI, no AI

For scripting and automation, `stateset-direct` maps straight onto the engine.
Its arguments are positional:

```bash
stateset-direct customers list
stateset-direct orders get ORD-001
stateset-direct inventory stock SKU-001
stateset-direct --apply --yes inventory adjust SKU-001 50 "shipment received"
```

Writes need `--apply`, and outside a TTY they also need `--yes` to skip the
confirmation prompt — which is what you want in a script.

Run `stateset-direct` with no arguments for the full resource and action list.

## 7. Domain agents

Each commerce domain has a dedicated agent binary, with the same `--apply`
posture as `stateset`:

```bash
stateset-orders "show pending orders ready to ship"
stateset-inventory "what SKUs are below reorder point?"
stateset-returns "process return for order ORD-001"
stateset-analytics "forecast revenue for next quarter"
stateset-checkout "create a cart for alice@example.com and add 2 widgets"
```

## What's included

| Feature | Description |
|---------|-------------|
| **MCP tools** | Orders, inventory, payments, returns, carts, analytics, tax, promotions, subscriptions, manufacturing, supply chain, finance, A2A — see the generated [tool catalog](https://github.com/stateset/stateset-icommerce/blob/master/cli/docs/TOOLS.md) |
| **Policy engine** | YAML business rules with explainable denials and preview-before-execute |
| **Shopify adapter** | CSV import, API sync, webhook handlers |
| **Stripe adapter** | Real-time webhook sync for payments, subscriptions, invoices |
| **WooCommerce adapter** | API import and webhook sync |
| **Multi-currency** | Exchange rates and conversions |
| **Tax engine** | US state tax, EU VAT, Canadian GST/PST/HST |
| **Analytics** | Revenue forecasts, demand prediction, inventory health |

## What's next (optional)

None of these are required to use iCommerce:

- **MCP server** — point Claude Desktop, Cursor, or Windsurf at the same
  database with `stateset-mcp`. See [MCP Tools](guides/mcp-tools.md).
- **Sequencer sync** — multi-agent coordination and cryptographic audit trails.
  Add a `.stateset/sync.json` file to enable.
- **Stablecoin payments** — accept USDC on Base, Solana, and others. Run
  `stateset-pay --chains` to see what is supported.
- **On-chain settlement** — anchor commerce events to SET Chain for independent
  verifiability.

See [Product Tiers](tiers.md) for the full feature matrix.
