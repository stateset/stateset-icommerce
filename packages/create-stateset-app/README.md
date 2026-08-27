# create-stateset-app

Scaffold a full-featured commerce storefront powered by [StateSet](https://stateset.com) in seconds.

## Quick Start

```bash
npx create-stateset-app my-store
cd my-store
cp .env.example .env.local
npm run seed
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## What You Get

A production-oriented Next.js 16 starter storefront with:

- **Embedded commerce engine** — products, carts, orders, inventory, subscriptions via `@stateset/embedded`
- **USDC payments** — Base checkout with server-side receipt verification, replay protection, and crash-safe finalization
- **Inventory-safe checkout** — atomic reservations reject insufficient stock instead of overselling
- **AI shopping assistant** — Claude-powered chat with product recommendations
- **Full account system** — wallet-based auth, order history, subscription management
- **10 seed products** — ready to customize

### Stack

| Layer     | Tech                            |
| --------- | ------------------------------- |
| Framework | Next.js 16 (App Router)         |
| Commerce  | @stateset/embedded (SQLite)     |
| Payments  | USDC on Base (Wagmi 3 + Viem 2) |
| AI Chat   | @ai-sdk/anthropic (Claude)      |
| Styling   | Tailwind CSS 3                  |
| State     | React Query 5, React Context    |

## Options

```bash
npx create-stateset-app my-store              # interactive setup
npx create-stateset-app my-store --skip-install
npx create-stateset-app my-store --use-pnpm
npx create-stateset-app my-store --use-yarn
```

## Configuration

Edit `.env.local`:

```env
# Your wallet address for receiving USDC payments
NEXT_PUBLIC_STORE_WALLET_ADDRESS=0x...

# Server-only Base RPC used to verify settlement
STATESET_BASE_RPC_URL=https://mainnet.base.org
STATESET_USDC_CONFIRMATIONS=2

# Replace the starter tax table with your nexus jurisdictions and exact rates
STATESET_TAX_RATES_JSON={"CA":"0.0725","OR":"0"}

# Exact server-authoritative shipping methods
STATESET_SHIPPING_METHODS_JSON=[{"id":"ground","label":"Ground","amount":"7.50","carrier":"UPS","estimatedDays":"3-5 business days","countries":["US"]}]

# SQLite database path
STATESET_DB_PATH=./store.db

# Anthropic API key for AI chat
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-5
```

The tax provider is intentionally fail-closed: checkout is enabled only for
explicitly configured states. `STATESET_TAX_RATES_JSON` replaces the complete
starter table, including support for configured zero-rate jurisdictions. For
production tax engines, implement the same provider interface in `lib/tax.js`.

Shipping uses the same operator-owned pattern. The JSON configuration replaces
the free starter method, validates method identifiers and exact amounts, and is
recomputed by the server during settlement verification. Carrier API adapters
can implement the provider interface in `lib/shipping.js`.

## Project Structure

```
my-store/
  app/
    page.tsx                    # Homepage
    products/                   # Product listing & detail
    cart/                       # Shopping cart
    checkout/                   # USDC checkout + confirmation
    account/                    # Dashboard, orders, subscriptions
    api/                        # Cart, checkout, chat, tax, etc.
  components/
    commerce/                   # ProductCard, ConnectWallet, etc.
    chat/                       # AI ChatWidget
    layout/                     # Header, Footer
  hooks/                        # useCart, useUSDCPayment, useWishlist
  lib/                          # commerce.ts, wagmi.ts
  scripts/seed.js               # Seed 10 products
```

## License

MIT
