# create-stateset-app

Scaffold a full-featured commerce storefront powered by [StateSet](https://stateset.com) in seconds.

Requires Node.js 20.20 or newer and npm 10 or newer.

## Quick Start

From this repository checkout, try the merchant preview on Linux x64 with
glibc. The command verifies the preview SDK and creates a new shop.

```bash
npm run storefront:preview -- my-store
cd my-store
npm run setup -- --apply
npm run dev
```

The published generator is also available via `npm create stateset-app@latest`. The merchant setup below requires the preview or generator 1.40.1.

Open the private link printed by `setup`. Choose **Add a product**, enter its name, URL slug, SKU, USD price and available stock, and select **Publish product**. Follow the product link to see it in the storefront. Choose **Set tax and shipping** to publish your checkout rates.

Setup creates or opens the configured SQLite file and issues a merchant link valid for one day. Run `npm run setup` for a read-only preview. For a sample catalog, run `npm run seed`. To enable payments, copy `.env.example` to `.env.local` if you are creating your first environment file, configure the selected processor and storefront URL, then run `npm run onboard` and restart the storefront.

## Run the web storefront and merchant commands in Docker

Build both images from the generated shop. The web image serves the storefront.
The operator image carries the CLI and worker. Both run as the same unprivileged
user and read the SQLite database at `/var/lib/stateset/store.db`.

```bash
docker build -t merchant-web .
docker build --target operator -t merchant-operator .
docker volume create merchant-store-data
docker run --rm -v merchant-store-data:/var/lib/stateset merchant-operator npm run setup -- --apply
docker run --rm -p 127.0.0.1:3000:3000 -v merchant-store-data:/var/lib/stateset merchant-web
```

Open the setup link locally to publish a product. For a deployed shop, configure
the same public HTTPS origin and provider settings on both services. If using
the generated environment file, set `STATESET_DB_PATH=/var/lib/stateset/store.db`
in that file. Run `npm run merchant:worker -- --watch --apply` in the operator
image with the same volume to process work continuously. Keep the two containers
on a host that can share the SQLite volume.

## What You Get

A production-oriented Next.js 16 starter storefront with:

- **Embedded commerce engine** — products, carts, orders, inventory, subscriptions via `@stateset/embedded`
- **USDC payments** — Base checkout with server-side receipt verification, replay protection, and crash-safe finalization
- **Cards and wallets** — Stripe-hosted checkout, eligible Apple Pay and Google Pay, and an optional Stripe/Square card form
- **Merchant and partner portals** — publish campaigns, configure checkout pricing, review renewal recovery, manage integrations, and trace commissions and seller payouts
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

## Readiness and local acceptance

`npm run doctor` checks local configuration and reads the existing SQLite file
header without opening the engine, migrating, or seeding. `npm run doctor:launch`
also requires explicit tax/shipping configuration, a public HTTPS URL, HTTPS
RPC, valid merchant address and persistent database. It exits nonzero on failed
checks and supports `-- --json`. It loads Next.js environment files with the
same precedence as the storefront and reports no secret values. Configuration
checks do not prove provider connectivity, tax compliance, or database integrity.
The optional chat API key does not block checkout readiness.

`npm run verify:journey` creates a disposable private native store and exercises
cart pricing, inventory reservation, checkout retry, partial refund and reopen
recovery. Settlement is simulated: it calls no external providers, transfers no
funds and ignores the operator database path. The normal seed script honors
`.env.local`, preserves existing stock on repeat runs, repairs missing sample
inventory and exits nonzero on catalog conflicts. Newly created samples are
activated; existing products keep their operator-selected status.

The commerce ledger and prices use USD; the USDC payment rail assumes one token
per dollar and verifies the exact token amount. `/api/store/status` exposes only
checkout availability and the configured merchant/chain. The wallet checks it
before requesting a transfer, so missing launch configuration can be corrected
before a customer sends funds.

## Options

```bash
npx create-stateset-app my-store              # interactive setup
npx create-stateset-app my-store --skip-install
npx create-stateset-app my-store --yes          # non-interactive, accepts defaults
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
Line taxes round half-up to USD cents before being summed; the tax quote,
verified USDC settlement and native order all use those same posted amounts.

Shipping uses the same operator-owned pattern. The JSON configuration replaces
the free starter method, validates method identifiers and exact USD-cent amounts, and is
recomputed by the server during settlement verification. Carrier API adapters
can implement the provider interface in `lib/shipping.js`.

## Project Structure

```
my-store/
  app/
    page.tsx                    # Homepage
    products/                   # Product listing & detail
    cart/                       # Shopping cart
    checkout/                   # Hosted and routed card checkout, USDC and receipts
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

## Import a Next Commerce product catalog

Save a page from the Next Commerce Admin API `GET /api/admin/products/?is_public=true&is_gift_card=false` using version `2024-04-01` and the `catalogue:read` permission. Keep the JSON envelope with its `results`, `next` and `previous` fields. Import each cursor page separately under the same source store code. The importer reads local files. Next's Inventory CSV covers SKUs, locations and quantities. The Products List response also supplies product and variant identities, selling prices and purchase settings. [Next Products List](https://developers.nextcommerce.com/docs/admin-api/reference/products/productsList) · [Next inventory exports](https://docs.nextcommerce.com/docs/build-a-store/catalogue/inventory)

In **Store setup → Import a product catalog**, select **Next Commerce Products List JSON**, enter a stable code for the source store and upload the saved page. Review the SKUs, exact USD prices, opening units and existing imports before selecting **Import reviewed catalog**. The same workflow runs from the generated storefront directory.

```bash
npm run onboard -- --next-catalog page-1.json --source-store my-next-store
npm run onboard -- --next-catalog page-1.json --source-store my-next-store --reviewed FINGERPRINT_FROM_PREVIEW --apply
```

- **Supported products** are public, discountable, one-time physical goods with a positive USD selling price in whole cents up to $999,999.99. An optional USD retail price imports as the comparison price. Buyers can select each variant by title and original SKU. StateSet records the source store, product, variant, stockrecord and location IDs alongside the local identities and variant option values.
- **Opening inventory** comes from one fulfillment location across this source store. Each variant must track stock, disable backorders, agree with its purchase availability and have zero units allocated to open orders. The importer preserves quantity and low-stock threshold. A replay preserves later sales, quantity adjustments, prices and product status.
- **Import scope** covers product titles, slugs, variant titles and SKUs, one-time prices and opening inventory. Descriptions, media, merchandising, unit costs, UPCs, customer or order history, payments, subscriptions and tax settings require separate setup. The whole page is checked before applying. Unsupported purchase terms, custom tax codes, conflicting source revisions and existing unrelated destination SKUs or slugs stop the import. [Next product, pricing and inventory semantics](https://developers.nextcommerce.com/docs/admin-api/guides/product-management)

## Card checkout and merchant onboarding

Set `STATESET_CHECKOUT_PROVIDER=stripe`, `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`, and `NEXT_PUBLIC_BASE_URL`. Register `/api/checkout/stripe-webhook` for `checkout.session.completed`, `checkout.session.expired`, `refund.created`, `refund.updated` and `refund.failed`. Stripe Checkout presents cards and eligible Apple Pay and Google Pay wallets according to account settings, browser, and device. The reusable `CardCheckoutButton` accepts cart, contact and delivery details. The server prices the cart and reserves stock before opening a hosted checkout. Keep the original processor credentials available while sessions and refunds are outstanding. [Stripe wallet support](https://docs.stripe.com/payments/checkout/payment-methods)

Run the onboarding command with a JSON catalog. Each row contains a name, URL slug, SKU, USD price string and initial stock. Re-importing the same catalog preserves subsequent inventory movements. Supply the exact shipping method and state rate through the shipping and tax settings above.

```json
[{"name":"Coffee","slug":"coffee","sku":"COFFEE","price":"15.00","stock":10}]
```

```bash
npm run onboard -- --catalog products.json
npm run onboard -- --catalog products.json --apply --provider
npm run verify:storefront
```

The verification scripts use disposable native stores and processor fixtures. For a provider-confirmed drill, use a test store and a Stripe test-mode key. Supply an address JSON file with the fields `firstName`, `lastName`, `line1`, `city`, `state`, `postalCode` and `country`. Complete the returned hosted checkout with a Stripe test card, then finish the same named drill. It records one local test shipment and reconciles the captured charge and partial refund with the native order and payment. The shipment uses a test tracking number.

```bash
npm run onboard -- --purchase COFFEE --key launch-01 --email buyer@example.test --address address.json --shipping ground --apply
npm run onboard -- --finish cs_test_EXAMPLE --key launch-01 --refund 0.25 --apply
npm run card -- recover --order ORDER_ID
npm run card -- refund --payment PAYMENT_ID --amount 0.25 --reason "Partial return" --key return-01 --apply
```

To test Apple Pay or Google Pay, start a fresh drill with `--wallet apple_pay` or `--wallet google_pay`, or choose the wallet in the merchant portal. Open the hosted checkout on an eligible device and pay with that wallet. Run `npm run onboard -- --finish cs_test_EXAMPLE --key launch-01` to read the captured Stripe charge and check which wallet funded the payment. The portal offers the same read-only check through **Verify payment only**. Finishing with `--apply` records the verified wallet and charge alongside the shipment and partial refund. Use a separate drill key and purchase for each wallet. [Stripe Charge fields](https://docs.stripe.com/api/charges/object)

For merchant-managed routing, set `STATESET_CHECKOUT_PROVIDER=routed` and configure the accounts in priority order. The USD starter uses provider-hosted fields for the selected account. A definitive, retryable decline can select the next eligible account. An uncertain authorization stays with its original account until reconciled. Use the operator recovery command with `--account ACCOUNT --reference PROVIDER_PAYMENT_ID --apply` to restore a confirmed original reference. Retain the original credentials for captures and refunds. Seller-owned merchandise also requires an eligible Stripe Connect platform account.

```json
[{"account":"square","provider":"square","secretEnv":"SQUARE_ACCESS_TOKEN","publicKey":"sandbox-sq0idp-EXAMPLE","location":"LOCATION","sandbox":true,"currencies":["USD"]},{"account":"stripe","provider":"stripe","secretEnv":"STRIPE_SECRET_KEY","publicKey":"pk_test_EXAMPLE","currencies":["USD"],"methods":["credit_card"],"available":true}]
```

Run `npm run verify:providers` to preview the processor drill. To exercise the native adapters against test accounts, configure `STRIPE_TEST_KEY` and/or `SQUARE_SANDBOX_ACCESS_TOKEN` plus `SQUARE_SANDBOX_LOCATION`. Run `npm run verify:providers -- --provider both --db ./processor-drill.db --key launch-01 --apply`. Each selected sandbox captures USD 1.00 and refunds USD 0.25. The report includes the processor reference and native refund. Reusing the database and key polls an outstanding result. Use `--provider stripe` or `--provider square` for a single provider. Wallet presentation requires an eligible browser, device and merchant account. [Stripe test methods](https://docs.stripe.com/testing) · [Square Sandbox payments](https://developer.squareup.com/docs/devtools/sandbox/payments)

### Import an existing Whop catalog

A merchant with an existing Whop account can stage catalog items for a StateSet storefront. Export the account’s current `GET /api/v1/products?account_id=biz_…` and `GET /api/v1/variants?account_id=biz_…` responses. Use the account’s product and variant read permissions, pin `Api-Version-Date: 2026-11-19-1`, and omit presentment currency or country overrides. Follow each forward cursor through the final page. [Whop product listing](https://docs.whop.com/api-reference/products/list-products) · [Whop variant listing and exact prices](https://docs.whop.com/api-reference/variants/list-variants)

Combine the two saved responses locally. For a multi-page listing, supply all pages for that endpoint in request order as a JSON array. The command accepts the response bodies directly and runs offline. A single-page example using `jq` follows.

```bash
jq -n --arg accountId biz_YourAccount --slurpfile products products.json --slurpfile variants variants.json \
  '{accountId:$accountId,products:$products,variants:$variants}' > whop-catalog.json
node scripts/import-whop-catalog.js whop-catalog.json | tee whop-preview.json
node scripts/import-whop-catalog.js whop-catalog.json --apply --fingerprint "$(jq -r .fingerprint whop-preview.json)"
```

The preview lists each eligible public one-time, buy-now USD variant, its exact price, its local SKU, and the reason for each exclusion. Apply creates one draft product per eligible variant with zero inventory. Source account/product/variant IDs determine stable SKUs. Replays preserve local stock and activation, and conflicting titles or prices stop the import before catalog writes. Configure digital delivery for the SKU, allocate inventory and activate the product when ready. Existing buyers, community access, renewals and billing continue in Whop. Catalog imports contribute zero orders to adoption reporting.

### Merchant portal

Run `npm run merchant -- portal shop --role merchant --days 7 --apply` to issue an expiring operator link. Deliver it privately. The portal supports catalog import, tax and shipping publication, read-only checks of each routed Stripe account or Square location, guided test purchases, integration installation and delivery replay. Publishing pricing applies the revision to storefront quotes and checkout. Environment rates supply the initial settings. The standalone doctor checks environment configuration. The portal additionally reads pricing published in the store. Revoke a link with `npm run merchant -- revoke-portal LINK_ID --apply`.

## Renewal recovery and integrations

Enable `STATESET_RENEWAL_RECOVERY=true` for a dedicated merchant Stripe Billing account and subscribe the checkout webhook to `invoice.payment_failed`. Set `STATESET_RECOVERY_SECRET` to at least 32 random bytes and enable payment method updates in Stripe's customer portal. Configure increasing retry offsets using `STATESET_RECOVERY_RETRY_DAYS`, for example `[1,3,7]`. Recovery observes Stripe's pending retry schedule before attempting its own collection. If one invoice encounters an error, the worker continues with the others and lists the affected invoice and reason. A one-shot run exits nonzero when an invoice fails. The recovery report groups failed, outstanding and recovered amounts by invoice currency in integer minor units.

Set `stateset_subscription_id` and `stateset_customer_id` in the Stripe subscription metadata. The signed failure event and a provider lookup link the invoice to its native subscription after checking customer identity and currency. To link an existing subscription explicitly, run `npm run merchant -- link-billing NATIVE_SUBSCRIPTION_ID --provider-subscription STRIPE_SUBSCRIPTION_ID --apply`. The merchant portal shows the next scheduled retry and the percentage recovered for each currency. Subscribe the webhook to `invoice.paid` and `customer.subscription.updated`, `.deleted`, `.paused` and `.resumed` for membership access. Paid recurring invoice lines supply the exact provider billing period. Cancellation and pause events update access for the linked membership.

For memberships sold through the starter's Stripe Checkout, each single-charge, fixed-price renewal creates a native payment and paid billing cycle. The merchant portal and `npm run merchant -- renewal-statement` show the invoice, period, tax, refunds and net cash collected. To recover missed events for an existing membership, run `npm run merchant -- reconcile-renewals NATIVE_SUBSCRIPTION_ID` to preview its paid invoices, then add `--apply` to post the invoices and their refunds locally. Replaying the same history reuses the original payment and billing cycle.

Renewing seller memberships retain the seller, fee and merchandise base agreed at checkout. The seller statement identifies each renewal invoice. Transfers use its verified charge, and refunds reduce or reverse the seller payout. Older checkouts with ambiguous allocations appear as `needs review` in the renewal statement. Use `npm run merchant -- resolve-renewal-seller SUBSCRIPTION_ID --seller CODE --bps FEE` to preview the original seller's agreed rate, or use `--platform-owned` for a merchant-owned membership. Add `--apply` to save that decision.

#### Verify a failed renewal against Stripe

Use the database of the sandbox storefront that sold the membership. Enable renewal recovery and deliver `invoice.payment_failed` and `invoice.paid` to its webhook. Purchase a recurring USD membership using a successful test card. In the Stripe sandbox, change the subscription's payment method to a test card that can be attached and declines on charge. Advance its billing cycle using **Run simulation** on the enrolled subscription when available. Use the resulting failed renewal event ID from that account. [Stripe Billing sandbox guide](https://docs.stripe.com/billing/testing) · [Stripe test cards](https://docs.stripe.com/testing#declined-payments)

```bash
npm run verify:recovery
npm run verify:recovery -- --check --event evt_EXAMPLE --db ./store.db
```

The first command previews the check. The second uses `STRIPE_TEST_KEY` (or a test-mode `STRIPE_SECRET_KEY`) to read that event and invoice. With the invoice open, expect `awaiting_payment`. Update the payment method for this subscription to a successful test card and let the configured retry run. When using a simulation, advance to the retry time. Rerun the same check after Stripe delivers `invoice.paid`. [Stripe payment retry settings](https://docs.stripe.com/billing/revenue-recovery/smart-retries)

A successful check matches the captured charge to the failed amount and to the native payment and billing cycle. The report includes gross recovered, refunded, and net retained amounts in USD cents. It verifies settlement regardless of who initiated the retry. Exit code 2 reports outstanding evidence. For `missing_failure_webhook`, inspect and redeliver the failed event. For `missing_native_renewal`, check delivery of `invoice.paid` and the linked membership's reconciliation history. A terminal unpaid invoice or a mismatched amount, identity, or sandbox mode exits with code 1. The verifier uses provider reads and leaves the records intact for another check.

For transactional recovery emails, configure `RESEND_API_KEY` and a verified `STATESET_RECOVERY_FROM` sender. Alternatively, configure a Klaviyo transactional flow on the `StateSet Renewal Recovery` event and use its `update_url` and `payment_url` properties. The update link opens a short-lived payment-method update session for the invoiced customer.

| Integration | Operator environment | Delivered data |
| --- | --- | --- |
| ShipStation | `SHIPSTATION_API_KEY`, `SHIPSTATION_API_SECRET` for the v1 API | Paid orders with order keys, addresses and merchandise lines |
| Klaviyo | `KLAVIYO_PRIVATE_KEY` with `accounts:read`, `events:write`, `profiles:read`, `events:read` and access to read the included metrics | Order paid and refunded events, and recovery reminders when selected |
| Gorgias | `GORGIAS_DOMAIN` tenant slug, `GORGIAS_EMAIL`, `GORGIAS_API_KEY` | Internal refund tickets associated with the buyer |

```bash
npm run merchant -- install shipstation --apply
npm run merchant -- install klaviyo --apply
npm run merchant -- install gorgias --apply
npm run merchant -- deliveries
npm run merchant -- deliveries --delivery DELIVERY_KEY --check --apply
npm run merchant -- deliveries --delivery DELIVERY_KEY --apply
npm run merchant -- recovery-report
npm run merchant:worker -- --watch --interval 60 --apply
```

Installation verifies the configured account. Events created after installation are eligible for delivery. The worker also reconciles unfinished checkouts and releases expired reservations. Use a one-shot invocation for a scheduler, or use `--watch` for a long-running process. Commands preview by default. The delivery report includes the order or refund ID and the confirmed provider ID. Choose a delivery in the portal or supply its key on the CLI to inspect the attempt history. Adding `--check --apply` looks up the remote record and saves the result locally. For a single key the command exits successfully only when the provider confirms the delivery. Checking a previously delivered key performs a fresh lookup. A missing or replaced provider record appears for review with its original ID.

ShipStation and Gorgias deliveries enter `review` when a write has an uncertain outcome. The worker searches for the original order or ticket across restarts. If the provider still shows no record, a merchant can authorize a fresh dispatch from the portal or with `npm run merchant -- deliveries --delivery DELIVERY_KEY --retry-dispatch CLAIM_ID --apply`. This applies to the specific attempt the merchant reviewed. Check the provider first because a delayed request can still create a duplicate ticket or update an existing shipping order. [ShipStation order lookup](https://www.shipstation.com/docs/api/orders/list-orders/) · [Gorgias ticket lookup](https://developers.gorgias.com/reference/list-tickets)

Klaviyo first reports `accepted`. The worker changes the status to `delivered` when the Events API returns the matching profile, metric, event ID and order or refund properties. The worker spaces automatic checks over the first day. A merchant can request a fresh lookup at any time. A confirmed event establishes that Klaviyo recorded the event. Email delivery is a separate outcome. [Klaviyo Events API](https://developers.klaviyo.com/en/v2026-07-15/reference/events_api_overview)

Run `npm run verify:integrations` in a generated storefront to exercise provider responses, lost connections, delayed event indexing and concurrent retries against a disposable native store.

## Campaigns, affiliates and digital delivery

Set `STATESET_CAMPAIGN_SECRET` to at least 32 random bytes. Define offers in `STATESET_CAMPAIGNS_JSON` using active catalog SKUs. For example:

```json
{"coffee":{"variants":[{"id":"control","weight":1,"title":"House roast","sku":"COFFEE"},{"id":"bundle","weight":1,"title":"Coffee and brewing guide","sku":"COFFEE","bumpSku":"GUIDE","upsellSku":"CUP"}]}}
```

Share `/c/coffee?utm_source=newsletter`. The landing route assigns the visitor to a weighted variant and signs the assignment in an HTTP-only cookie. Card checkout offers the selected bump and presents any follow-up offer after payment. Accepting an upsell opens its own inventory-backed order and payment review. Repeated clicks resolve to the same supplemental checkout. Reports count distinct attributed visitors and buyers. Net merchandise revenue excludes shipping and tax and allocates payment refunds proportionally. Use `--by-source` for the traffic-source breakdown.

In the merchant portal, preview variants against the live catalog before publishing. Each published revision preserves the offer for previously assigned visitors. A variant may set `couponCode` to an active native coupon. The engine consumes it with the order reservation. For hosted Stripe checkout, configure `NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY` for the same account to let a buyer consent to saving their card for the follow-up offer. The receipt then shows its exact charge including shipping and tax. The campaign report filters cohorts by first visit date in UTC, includes subsequent refunds, and lets merchants inspect the underlying orders.

```bash
npm run merchant -- campaign-report
npm run merchant -- affiliate partner-one --bps 1000 --campaign coffee --apply
npm run merchant -- affiliate-balances
npm run merchant -- export-commissions
npm run merchant -- export-payouts --batch november-01 --apply
npm run merchant -- confirm-payouts --batch november-01 --reference BANK_REFERENCE --apply
```

The campaign report counts unique visitors from the entry page through checkout, payment start, and purchase. Select a variant and source to drill into its orders and refunds. Enter a comparison variant or use the lowest observed variant ID within the same campaign revision, source, and visit-date cohort. The USD and percentage differences compare net merchandise revenue per tracked visitor. A missing control or zero control revenue leaves the percentage blank. Stage tracking starts when the updated storefront is deployed. Completed purchases supply evidence of earlier stages on older stores.

The affiliate command generates a referral URL. Attribution uses the first visit to that campaign in the visitor's attribution window. Fees are fixed in basis points at attribution. A payout batch freezes the amount per recipient and the positive or negative contribution from each order. Repeat the same batch ID to reproduce the CSV. A refund after export appears as a deduction from a later payout. Historical aggregate exports show their original totals and any carry into a new batch.

In the merchant portal, preview a batch, review its recipients and amounts, then reserve it. Select a single affiliate when different recipients use different payment methods. Choose **External payment** to pay through your bank and record the reference and time for each recipient. From the CLI, use `confirm-payouts --batch BATCH --affiliate CODE --reference BANK_REFERENCE --paid-at ISO_TIMESTAMP --apply`. Omit the affiliate flag when a single bank confirmation covers the entire batch. An affiliate’s portal shows only that recipient’s amounts and order breakdown.

For **Stripe Connect transfers**, register the affiliate and complete Express onboarding. Fund the available platform balance before transferring. Set `STATESET_AFFILIATE_PAYOUT_COUNTRIES` to allow wallet-authenticated affiliates to start or resume verification at `/partners`. Operator-managed affiliates can use an onboarding link from the CLI. A merchant can reserve the Stripe batch and transfer each recipient’s commission from the portal or CLI. Retrying the same batch and recipient reconciles the existing transfer. Reconciliation retrieves provider reversals and shows the net transferred. A reversed amount returns to the available commission balance with a reference to its original batch. If an interrupted transfer is still absent after Stripe’s idempotency window, the CLI reports its transfer group and attempt number. After verifying the group in Stripe, supply the reported `--retry-missing-attempt N` on the settlement command to retry. Bank evidence is available for automatic payouts with completed reconciliation and a matching destination payment. The portal shows Stripe’s bank status and expected arrival date for each matched transaction. [Stripe transfers](https://docs.stripe.com/api/transfers/create)

```bash
npm run merchant -- onboard-affiliate partner-one --country US --email partner@example.com --apply
npm run merchant -- export-payouts --batch november-connect --rail stripe --apply
npm run merchant -- settle-affiliate partner-one --batch november-connect --apply
npm run merchant -- affiliate-bank-payouts partner-one --apply
npm run merchant -- affiliate-payouts partner-one
```

Set `STATESET_AFFILIATE_DEFAULT_BPS` to open wallet-authenticated enrollment at `/partners`. Each affiliate signs in to retrieve referral links, referred orders, refund deductions and payout batch status. Operator-created accounts can use a scoped link issued with `npm run merchant -- portal PARTNER_CODE --role affiliate --apply`.

Map SKUs to protected resources in `STATESET_DIGITAL_PRODUCTS_JSON`, configure `STATESET_LICENSE_SECRET` with at least 32 random bytes, and place private downloads under `STATESET_DOWNLOAD_ROOT`, outside the public directory.

```json
{"GUIDE":{"resource":"brew-guide","file":"brew-guide.pdf","roles":["barista"],"validDays":30,"refundPolicy":"any"}}
```

The paid card return page displays the license and download. Wallet-authenticated buyers can retrieve their order's licenses from `GET /api/access/licenses`. A host checks a license via `POST /api/access` with `Authorization: Bearer LICENSE` and a JSON `resource`. The response supplies the authorized roles and expiry. `POST /api/access/download` verifies the same license before streaming the private file. Access checks consult the current native order and payment. The default refund policy revokes on any completed refund. Set `refundPolicy` to `full` to retain access after a partial refund.

For a prepaid membership, set `membershipPlanId` to an active USD native plan. A settled first purchase creates the buyer’s membership and records the initial cycle against its order and payment. Retrying a receipt returns the same membership. A full refund marks every paid billing cycle linked to that payment as refunded in the same database transaction. A partial refund leaves the cycle paid and access follows the product’s refund policy. An existing, unambiguous customer subscription can supply access. Resolve ambiguous matches with `npm run merchant -- link-membership RESOURCE --order ORDER_ID --subscription SUBSCRIPTION_ID --apply`. Guest checkout uses a per-cart identity.

To sell a renewing membership, use Stripe Checkout and add `"recurring": true` to that SKU’s digital policy. Set the catalog price to the native plan price. Checkout shows the regular amount and interval before payment. One membership per checkout can be combined with one-time items. The recurring price includes the tax calculated at signup. Shipping and the introductory discount apply to the first invoice. Enable Stripe’s customer portal subscription cancellation flow and subscribe the checkout webhook to `invoice.paid`, `customer.subscription.updated`, `customer.subscription.deleted`, `customer.subscription.paused`, `customer.subscription.resumed` and the refund events. The paid receipt opens cancellation for the purchased subscription. Confirmed invoice lines determine paid-through dates. A refund of a renewal revokes that period according to the purchased SKU’s `refundPolicy`. The native billing worker leaves these externally billed memberships to the processor. Run `npm run verify:membership` for the native-store lifecycle test. [Stripe subscription checkout](https://docs.stripe.com/payments/checkout/build-subscriptions)

## Seller accounts and settlement

The storefront uses separate charges and transfers on Stripe Connect. Register each seller under a stable code, deliver the returned Stripe Express onboarding link to that seller, and let Stripe collect verification and bank details. An expired link can be renewed by repeating the operator command. [Stripe Connect](https://docs.stripe.com/connect/separate-charges-and-transfers)

```bash
npm run merchant -- onboard-seller roaster --country US --email seller@example.test --apply
npm run merchant -- seller-statement
npm run merchant -- settle-sellers
npm run merchant -- settle-sellers --apply
npm run merchant -- bank-payouts roaster --apply
```

Configure SKU ownership with `STATESET_SELLER_SKUS_JSON`, for example `{"COFFEE":{"seller":"roaster","feeBps":1000}}`. Ownership and fee terms are frozen before checkout. The platform retains tax and shipping. A seller's statement shows gross merchandise, original fees, subsequent refund allocations and the adjusted payable. Configure `STATESET_SELLER_GL_JSON` with existing posting account UUIDs for `salesClearing`, `sellerPayable`, `platformFees` and `processorClearing`. The posting date needs an open period. Choose accounts that reclassify the merchant's original sales posting under the platform's accounting policy. The USD starter uses a one-to-one ledger exchange rate by default. Set `usdExchangeRate` explicitly if using another functional currency.

Settlement posts the allocation, fees, refunds, transfer and any transfer reversals to the native general ledger with unique merchant-event IDs. Each transfer is tied to the settled charge. Stripe handles bank payouts on the connected account's schedule. The bank-payout command records provider-reported amounts, arrival dates and status alongside the seller's statement. Set `STATESET_AUTOMATIC_SELLER_TRANSFERS=true` to include settlement in the scheduled merchant worker.

Set `STATESET_SELLER_SIGNUP_COUNTRIES=US,CA` to open seller enrollment for those countries at `/partners`. Sellers sign a request with their wallet and complete identity and bank verification on Stripe. Their portal shows outstanding balances, account requirements, transfers, refunds and bank payout status. `STATESET_SYNC_BANK_PAYOUTS=true` polls connected-account payouts. For automatic payouts with completed reconciliation, the statement matches balance transactions to the originating orders using Stripe's destination payment and reversal references and shows any remaining difference. Manual payouts can display a status without an order breakdown.

## Measure merchant adoption

A running store can enroll locally under its acquisition channel. The first enrollment fixes a random store ID, the channel and the start time in the store database. Enroll during setup to measure the launch interval. An existing store can start a new observation period when the operator enrolls it.

```sh
npm run merchant -- adoption-enroll --source partner --apply
npm run merchant -- adoption-report
npm run --silent merchant -- adoption-report --check > merchant-adoption.json
```

The report counts primary card orders placed after enrollment. A counted sale requires a completed storefront sale event, a native captured payment for the same order and amount, and a matching live Stripe or Square settlement. The check reads the provider using the original merchant credentials. Add `--apply` to retain verified capture receipts locally. Later reports reuse those receipts, including offline runs. Failed lookups remain retryable. The report records when the storefront finished the first sale and when another order placed at least seven days later finished. A later refund preserves the historical capture. Supplemental offers, Base transfers, test purchases and renewals contribute zero to these card-checkout milestones. Counts of unresolved evidence keep a store with missing credentials or a failed lookup visible. An explicit check exits with status 2 for partial coverage and still writes the report.

The JSON contains a pseudonymous store ID, acquisition channel, timestamps and aggregate counts. Data stays with the operator until they share the file. To compare submitted reports by acquisition channel, run the following command in a generated storefront.

```sh
node scripts/adoption-cohorts.js reports/*.json
```

The latest report for each enrolled store counts once. The output shows how many stores had their first verified sale within seven days and how many had repeat activity within thirty days. Each denominator includes submitted stores observed for that full window with complete card evidence. Include reports from stores with zero orders when measuring activation. These figures describe the participating stores. Reports are merchant-supplied JSON. For independent confirmation of a sale, review the underlying provider capture with the merchant’s authorization.
