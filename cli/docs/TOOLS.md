# StateSet CLI Tool Catalog

<!-- GENERATED FILE — do not edit by hand. -->
<!-- Regenerate with: npm run docs:tools (from cli/) -->

Source of truth: `cli/src/tools/domain-registry.js` (tools) and `cli/src/tools/tool-tiers.js` (tiers).

**938 tools** across **88 domains**, plus 15 agentic runtime tools.

## Stability tiers

| Tier | Tools | Meaning |
| --- | ---: | --- |
| core | 198 | The default MCP surface (no `--profile`). Smoke-gated: every tool works or refuses cleanly on a fresh store, with no backlog. |
| extended | 479 | Real, specialised domains (finance, manufacturing, WMS, B2B, engagement). Opt in with `--profile` or `--domains`. |
| experimental | 276 | Demo, external-stack-dependent (wallet, chain, API key, demo stack) or known-incomplete. Only `--profile all`, a curated profile naming the domain, or `--domains`. |

## Profiles

| Profile | Tools | Domains |
| --- | ---: | --- |
| all | 953 | every domain |
| core (default) | 198 | customers, orders, products, inventory, returns, carts, analytics, tax, promotions, payments, shipments, gift-cards, store-credits, explain |
| operations | 191 | inventory, manufacturing, shipments, suppliers, warranties, warehouse, receiving, fulfillment, quality, lots, serials, cycle-counts, transfer-orders, production-batches, supplier-skus, inbound-shipments, backorders, vendor-returns |
| finance | 134 | payments, invoices, treasury, accounts-payable, accounts-receivable, cost-accounting, credit, general-ledger, fixed-assets, revenue-recognition, prepayments, vendor-credits, payment-obligations |
| agents | 251 | agent-runtime, agent-cards, agent-receipt, a2a, a2a-platform, a2a-automation, a2a-observability, a2a-intelligence, x402, stablecoin, erc8004, treasury, payment-obligations, proofs, audit, policies |

`core` is exactly the core tier, agentic runtime tools included. The curated profiles
expose every tool in their domains, whatever its tier, plus the agentic runtime tools.

## Domains

| Domain | Tier | Tools |
| --- | --- | ---: |
| [customers](#customers) | core | 11 |
| [orders](#orders) | core | 6 |
| [products](#products) | core | 14 |
| [inventory](#inventory) | core | 6 |
| [custom-objects](#custom-objects) | extended | 12 |
| [returns](#returns) | core | 12 |
| [carts](#carts) | core | 30 |
| [analytics](#analytics) | core | 14 |
| [currency](#currency) | extended | 13 |
| [tax](#tax) | core | 29 |
| [promotions](#promotions) | core | 17 |
| [subscriptions](#subscriptions) | extended | 17 |
| [sync](#sync) | experimental | 20 |
| [manufacturing](#manufacturing) | extended | 11 |
| [payments](#payments) | core | 19 |
| [stablecoin](#stablecoin) | experimental | 4 |
| [treasury](#treasury) | experimental | 6 |
| [erc8004](#erc8004) | experimental | 5 |
| [x402](#x402) | experimental + extended | 14 |
| [agent-cards](#agent-cards) | experimental | 5 |
| [a2a](#a2a) | experimental | 59 |
| [agent-runtime](#agent-runtime) | experimental | 29 |
| [shipments](#shipments) | core | 14 |
| [suppliers](#suppliers) | extended | 10 |
| [invoices](#invoices) | extended | 7 |
| [warranties](#warranties) | extended | 7 |
| [import](#import) | experimental | 10 |
| [policies](#policies) | extended | 5 |
| [vector](#vector) | experimental | 16 |
| [gift-cards](#gift-cards) | core | 7 |
| [store-credits](#store-credits) | core | 5 |
| [segments](#segments) | extended | 5 |
| [shipping-zones](#shipping-zones) | extended | 7 |
| [units-of-measure](#units-of-measure) | extended | 10 |
| [stock-snapshots](#stock-snapshots) | extended | 5 |
| [print-stations](#print-stations) | extended | 8 |
| [integration-mappings](#integration-mappings) | extended | 7 |
| [integration-field-mappings](#integration-field-mappings) | extended | 8 |
| [payment-obligations](#payment-obligations) | extended | 7 |
| [purgatory](#purgatory) | extended | 6 |
| [topology-snapshots](#topology-snapshots) | extended | 5 |
| [vendor-returns](#vendor-returns) | extended | 6 |
| [reviews](#reviews) | extended | 7 |
| [wishlists](#wishlists) | extended | 6 |
| [loyalty](#loyalty) | extended | 8 |
| [fraud](#fraud) | extended | 6 |
| [connectors](#connectors) | extended | 11 |
| [audit](#audit) | extended | 4 |
| [proofs](#proofs) | extended | 7 |
| [circuit-breaker](#circuit-breaker) | experimental | 8 |
| [checkout](#checkout) | experimental | 8 |
| [compliance](#compliance) | experimental | 6 |
| [catalog](#catalog) | experimental | 6 |
| [a2a-automation](#a2a-automation) | experimental | 32 |
| [a2a-observability](#a2a-observability) | experimental | 15 |
| [a2a-platform](#a2a-platform) | experimental | 16 |
| [a2a-intelligence](#a2a-intelligence) | experimental | 17 |
| [quality](#quality) | extended | 15 |
| [lots](#lots) | extended | 11 |
| [search-config](#search-config) | extended | 7 |
| [serials](#serials) | extended | 8 |
| [warehouse](#warehouse) | extended | 9 |
| [receiving](#receiving) | extended | 8 |
| [fulfillment](#fulfillment) | extended | 14 |
| [accounts-payable](#accounts-payable) | extended | 11 |
| [accounts-receivable](#accounts-receivable) | extended | 8 |
| [cost-accounting](#cost-accounting) | extended | 5 |
| [credit](#credit) | extended | 8 |
| [backorders](#backorders) | extended | 20 |
| [general-ledger](#general-ledger) | extended | 17 |
| [agent-receipt](#agent-receipt) | experimental | 11 |
| [fixed-assets](#fixed-assets) | extended | 9 |
| [maintenance](#maintenance) | extended | 5 |
| [revenue-recognition](#revenue-recognition) | extended | 6 |
| [cycle-counts](#cycle-counts) | extended | 7 |
| [edi-documents](#edi-documents) | extended | 5 |
| [prepayments](#prepayments) | extended | 8 |
| [activity-logs](#activity-logs) | extended | 5 |
| [channels](#channels) | extended | 8 |
| [companies](#companies) | extended | 9 |
| [vendor-credits](#vendor-credits) | extended | 8 |
| [price-schedules](#price-schedules) | extended | 10 |
| [price-levels](#price-levels) | extended | 9 |
| [transfer-orders](#transfer-orders) | extended | 7 |
| [production-batches](#production-batches) | extended | 8 |
| [supplier-skus](#supplier-skus) | extended | 7 |
| [inbound-shipments](#inbound-shipments) | extended | 8 |
| [explain](#explain) | core | 2 |

## customers

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_customers` | core | read | List all customers in the database. Returns customer details including email, name, and status. |
| `get_customer` | core | read | Get a specific customer by ID or email address. |
| `create_customer` | core | write | Create a new customer. Requires email, first name, and last name. |
| `update_customer` | core | write | Update an existing customer. Only the fields provided are changed (email, name, phone, status, marketing opt-in). |
| `delete_customer` | core | write | Delete a customer (soft delete). |
| `find_or_create_customer` | core | write | Find a customer by email, or create one if none exists. Returns the existing or newly created customer. |
| `list_customer_addresses` | core | read | List all addresses in a customer address book. |
| `add_customer_address` | core | write | Add an address to a customer address book. |
| `update_customer_address` | core | write | Update an existing customer address. |
| `delete_customer_address` | core | write | Delete a customer address. |
| `set_default_customer_address` | core | write | Set a customer address as the default for shipping, billing, or both. |

## orders

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_orders` | core | read | List all orders. Shows order number, status, customer, total amount, and item count. |
| `get_order` | core | read | Get a specific order by ID or order number. Returns full order details including line items. |
| `create_order` | core | write | Create a new order for a customer with line items. |
| `update_order_status` | core | write | Update the status of an order. Valid statuses: pending, confirmed, processing, shipped, delivered, cancelled, refunded. |
| `ship_order` | core | write | Mark an order as shipped with optional tracking number. |
| `cancel_order` | core | delete | Cancel an order. Only pending or confirmed orders can be cancelled. |

## products

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_products` | core | read | List all products in the catalog. |
| `get_product` | core | read | Get a specific product by ID. |
| `get_product_by_slug` | core | read | Get a specific product by its URL slug. |
| `search_products` | core | read | Search active products by name or description. |
| `get_product_variant` | core | read | Get a product variant by SKU. |
| `list_product_variants` | core | read | List all variants for a product. |
| `create_product` | core | write | Create a new product with optional variants. |
| `update_product` | core | write | Update an existing product. Only the fields provided are changed (name, slug, description, status). |
| `activate_product` | core | write | Activate a product, making it available for purchase. |
| `archive_product` | core | write | Archive a product, removing it from sale. |
| `delete_product` | core | write | Delete a product (archives it). |
| `add_product_variant` | core | write | Add a variant to an existing product. |
| `update_product_variant` | core | write | Update an existing product variant. |
| `delete_product_variant` | core | write | Delete a product variant. |

## inventory

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `get_stock` | core | read | Get current stock level for a SKU. Shows on-hand, allocated, and available quantities. |
| `create_inventory_item` | core | write | Create a new inventory item for a SKU. |
| `adjust_inventory` | core | write | Adjust inventory quantity for a SKU. Use positive numbers to add stock, negative to remove. |
| `reserve_inventory` | core | write | Reserve inventory for an order. Reserved stock is allocated but not yet deducted. |
| `confirm_reservation` | core | write | Confirm an inventory reservation, deducting the reserved quantity from stock. |
| `release_reservation` | core | write | Release an inventory reservation, returning the reserved quantity to available stock. |

## custom-objects

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_custom_object_types` | extended | read | List custom object types (schemas). Custom objects are similar to Shopify metaobjects / Salesforce custom objects. |
| `get_custom_object_type` | extended | read | Get a custom object type (schema) by ID. |
| `get_custom_object_type_by_handle` | extended | read | Get a custom object type (schema) by handle. |
| `create_custom_object_type` | extended | write | Create a custom object type (schema). Fields define allowed keys and types; record values are validated deterministically. |
| `update_custom_object_type` | extended | write | Update a custom object type (schema). Updating fields replaces the full field definition list. |
| `delete_custom_object_type` | extended | delete | Delete a custom object type (schema). Records of this type must be deleted first. |
| `list_custom_objects` | extended | read | List custom object records (entries). |
| `get_custom_object` | extended | read | Get a custom object record by ID. |
| `get_custom_object_by_handle` | extended | read | Get a custom object record by (typeHandle, objectHandle). |
| `create_custom_object` | extended | write | Create a custom object record. Provide `values` (object) or `valuesJson` (string). Values are validated against the type schema. |
| `update_custom_object` | extended | write | Update a custom object record. Provide `values` (object) or `valuesJson` (string) to update record values. |
| `delete_custom_object` | extended | delete | Delete a custom object record by ID. |

## returns

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_returns` | core | read | List all returns. Shows return status, order, and reason. |
| `get_return` | core | read | Get a specific return by ID. |
| `list_returns_for_order` | core | read | List all returns filed against a specific order. |
| `list_returns_for_customer` | core | read | List all returns filed by a specific customer. |
| `list_pending_returns` | core | read | List returns awaiting approval (status requested). |
| `create_return` | core | write | Create a return request for an order. |
| `approve_return` | core | write | Approve a return request. |
| `reject_return` | core | write | Reject a return request with a reason. |
| `mark_return_received` | core | write | Mark a return as physically received at the warehouse. |
| `complete_return` | core | write | Complete a return and process the refund. |
| `cancel_return` | core | write | Cancel a return request. |
| `add_return_tracking` | core | write | Add a return-shipping tracking number and mark the return in transit. |

## carts

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_carts` | core | read | List all shopping carts. Shows cart status, customer, totals, and item count. |
| `get_cart` | core | read | Get a specific cart by ID or cart number. Returns full cart details including items. |
| `create_cart` | core | write | Create a new shopping cart. Can be for a guest or authenticated customer. |
| `update_cart` | core | write | Update cart customer details, shipping method, coupon code, or notes. |
| `list_customer_carts` | core | read | List carts for a specific customer. |
| `delete_cart` | core | delete | Delete a cart permanently. |
| `add_cart_item` | core | write | Add an item to a shopping cart. |
| `update_cart_item` | core | write | Update the quantity of an item in the cart. |
| `remove_cart_item` | core | delete | Remove an item from the cart. |
| `list_cart_items` | core | read | List items currently in a cart. |
| `clear_cart_items` | core | delete | Remove all items from a cart. |
| `set_cart_shipping_address` | core | write | Set the shipping address for a cart. |
| `set_cart_shipping` | core | write | Set shipping address and shipping selection for a cart. |
| `set_cart_billing_address` | core | write | Set the billing address for a cart. |
| `set_cart_payment` | core | write | Set the payment method for a cart. |
| `apply_cart_discount` | core | write | Apply a coupon/discount code to the cart. |
| `remove_cart_discount` | core | delete | Remove the coupon or discount from a cart. |
| `get_shipping_rates` | core | read | Get available shipping rates for a cart based on contents and address. |
| `mark_cart_ready_for_payment` | core | write | Mark a cart as ready for payment processing. |
| `begin_cart_checkout` | core | write | Begin the checkout process for a cart. |
| `complete_checkout` | core | write | Complete the checkout process and convert the cart to an order. This is the final step in the checkout flow. |
| `cancel_cart` | core | delete | Cancel a shopping cart. |
| `abandon_cart` | core | write | Mark a cart as abandoned (for recovery campaigns). |
| `expire_cart` | core | write | Mark a cart as expired. |
| `reserve_cart_inventory` | core | write | Reserve inventory for all cart items. |
| `release_cart_inventory` | core | write | Release reserved inventory for all cart items. |
| `recalculate_cart` | core | write | Recalculate cart totals after pricing or address changes. |
| `set_cart_tax` | core | write | Set the tax amount for a cart explicitly. |
| `get_abandoned_carts` | core | read | Get all abandoned carts for recovery campaigns. |
| `get_expired_carts` | core | read | Get all expired carts. |

## analytics

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `get_sales_summary` | core | read | Get sales summary for a time period. Returns total revenue, order count, average order value, items sold, and unique customers. |
| `get_revenue_by_period` | core | read | Get revenue broken down by day, week, or month for a selected period. |
| `get_top_products` | core | read | Get top selling products by revenue or units sold. |
| `get_product_performance` | core | read | Get product performance metrics for a time period, optionally filtered by SKU. |
| `get_customer_metrics` | core | read | Get customer metrics including total customers, new customers, returning customers, and average lifetime value. |
| `get_top_customers` | core | read | Get top customers by total spend. |
| `get_inventory_health` | core | read | Get inventory health summary showing total SKUs, in-stock, low stock, and out of stock counts. |
| `get_low_stock_items` | core | read | Get items that are low in stock or approaching reorder point. |
| `get_inventory_movement` | core | read | Get inventory movement history and net change over a selected period. |
| `get_demand_forecast` | core | read | Get demand forecast for inventory items based on historical sales. Predicts future demand and days until stockout. |
| `get_revenue_forecast` | core | read | Get revenue forecast based on historical trends. |
| `get_order_status_breakdown` | core | read | Get breakdown of orders by status. |
| `get_fulfillment_metrics` | core | read | Get fulfillment performance metrics for a selected period. |
| `get_return_metrics` | core | read | Get return metrics including return rate and total refunds. |

## currency

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `get_exchange_rate` | extended | read | Get the exchange rate between two currencies. |
| `list_exchange_rates` | extended | read | List all available exchange rates, optionally filtered by base currency. |
| `convert_currency` | extended | read | Convert an amount from one currency to another using current exchange rates. |
| `set_exchange_rate` | extended | admin | Set or update an exchange rate between two currencies. |
| `set_exchange_rates` | extended | admin | Set multiple exchange rates in a single operation. |
| `delete_exchange_rate` | extended | delete | Delete an exchange rate by ID. |
| `get_currency_settings` | extended | read | Get the store currency settings including base currency and enabled currencies. |
| `update_currency_settings` | extended | admin | Update store currency settings including enabled currencies and rounding behavior. |
| `set_base_currency` | extended | admin | Set the store's base currency. |
| `enable_currencies` | extended | admin | Enable currencies for the store. |
| `check_currency_enabled` | extended | read | Check whether a currency is enabled for the store. |
| `get_currency_decimal_places` | extended | read | How many decimal places a currency permits. The engine refuses an amount with more places than this, so check it before formatting or validating money rather than assuming two: JPY, KRW and VND have none, BTC and ETH have eight. |
| `format_currency` | extended | read | Format an amount with currency symbol. |

## tax

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `calculate_tax` | core | read | Calculate tax for a transaction based on shipping address and line items. Supports US sales tax, EU VAT, and Canadian GST/HST/PST. |
| `get_tax_rate` | core | read | Get the effective tax rate for a shipping address and product category. |
| `calculate_item_tax` | core | read | Calculate tax for a single line item and destination address. |
| `get_tax_jurisdiction` | core | read | Get a tax jurisdiction by ID or code. |
| `list_tax_jurisdictions` | core | read | List tax jurisdictions with optional filtering by country or level. |
| `create_tax_jurisdiction` | core | write | Create a tax jurisdiction. Requires --apply flag. |
| `list_tax_rates` | core | read | List tax rates for a jurisdiction or all active rates. |
| `get_tax_rate_record` | core | read | Get a tax rate record by ID. |
| `create_tax_rate` | core | write | Create a tax rate record. Requires --apply flag. |
| `get_tax_settings` | core | read | Get the store tax calculation settings. |
| `get_us_state_tax_info` | core | read | Get pre-configured US state sales tax information including rates and rules. |
| `get_customer_tax_exemptions` | core | read | Get active tax exemptions for a customer. |
| `get_tax_exemption` | core | read | Get a tax exemption by ID. |
| `create_tax_exemption` | core | write | Create a tax exemption certificate for a customer. |
| `check_customer_tax_exempt` | core | read | Check whether a customer is currently tax exempt. |
| `calculate_cart_tax` | core | write | Calculate and apply tax to a cart based on its shipping address. Must set shipping address first. Returns tax breakdown and updates cart totals. |
| `list_tax_providers` | core | read | List tax providers and capabilities for quote, commit, and void workflows. |
| `update_tax_settings` | core | write | Update store tax settings. Requires --apply flag. |
| `set_tax_enabled` | core | write | Enable or disable tax calculation. Requires --apply flag. |
| `check_tax_enabled` | core | read | Check whether tax calculation is currently enabled. |
| `validate_tax_jurisdiction_compliance` | core | read | Validate jurisdiction readiness for tax calculation (country/state/postal requirements and category checks). |
| `calculate_tax_quote` | core | read | Calculate a provider-backed tax quote with deterministic replay-safe output and optional idempotency key. |
| `calculate_tax_quote_with_failover` | core | read | Calculate a tax quote with jurisdiction compliance validation and provider failover routing. |
| `get_tax_quote` | core | read | Get a provider-backed tax quote by ID. |
| `commit_tax_transaction` | core | write | Commit a previously calculated tax quote into a provider transaction record. |
| `get_tax_transaction` | core | read | Get a provider-backed tax transaction by ID. |
| `list_tax_transactions` | core | read | List provider-backed tax transactions with optional filtering. |
| `void_tax_transaction` | core | delete | Void a committed tax transaction with optional reason. |
| `ingest_tax_provider_webhook` | core | write | Ingest a tax provider webhook event and reconcile quote/transaction state in shadow or production mode. |

## promotions

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_promotions` | core | read | List all promotions. Shows active, paused, and scheduled promotions with their discount details. |
| `get_promotion` | core | read | Get a promotion by ID or internal code. |
| `update_promotion` | core | write | Update an existing promotion. Requires --apply flag. |
| `create_promotion` | core | write | Create a new promotion. Supports percentage off, fixed amount off, BOGO, free shipping, tiered, and first-order discounts, optionally scoped to SKUs and gated by conditions. |
| `add_promotion_condition` | core | write | Add a condition to an existing promotion (minimum subtotal, first order, shipping country, SKU in cart, ...). The condition is validated before it is stored. Requires --apply. |
| `delete_promotion` | core | delete | Delete a promotion. Requires --apply flag. |
| `activate_promotion` | core | write | Activate a promotion to make it available for use. |
| `deactivate_promotion` | core | write | Pause/deactivate a promotion. |
| `create_coupon` | core | write | Create a coupon code for a promotion. |
| `get_coupon` | core | read | Get a coupon by ID or code. |
| `validate_coupon` | core | read | Check if a coupon code is valid and can be used. |
| `list_coupons` | core | read | List coupon codes with optional filters. |
| `get_active_promotions` | core | read | Get all currently active promotions. |
| `check_promotion_validity` | core | read | Check whether a promotion is currently valid and eligible to apply. |
| `apply_cart_promotions` | core | write | Calculate and apply all applicable promotions to a cart. Uses coupon codes on the cart and automatic promotions. |
| `quote_promotions` | core | read | Price a basket against every active promotion and the given coupon codes WITHOUT writing anything: returns the discount, what applied, and what was refused and why. Use it before a cart exists, or to explain why a coupon does not apply. |
| `record_promotion_usage` | core | write | Record promotion usage after checkout completion. Requires --apply flag. |

## subscriptions

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_subscription_plans` | extended | read | List all subscription plans. Filter by status (draft, active, archived) or billing interval. |
| `get_subscription_plan` | extended | read | Get details for a specific subscription plan. |
| `create_subscription_plan` | extended | write | Create a new subscription plan. Requires --apply flag. |
| `activate_subscription_plan` | extended | write | Activate a subscription plan (make it available for new subscriptions). Requires --apply flag. |
| `update_subscription_plan` | extended | write | Update an existing subscription plan. Requires --apply flag. |
| `archive_subscription_plan` | extended | delete | Archive a subscription plan (no new subscriptions, existing ones continue). Requires --apply flag. |
| `list_subscriptions` | extended | read | List subscriptions. Filter by customer, plan, or status. |
| `get_subscription` | extended | read | Get details for a specific subscription. |
| `create_subscription` | extended | write | Create a new subscription for a customer. Requires --apply flag. |
| `pause_subscription` | extended | write | Pause a subscription (stops billing, can resume later). Requires --apply flag. |
| `update_subscription` | extended | write | Update subscription fields (status, price, paymentMethodId, nextBillingDate, discountPercent as a 0-1 fraction, discountAmount, couponCode). Requires --apply flag. |
| `resume_subscription` | extended | write | Resume a paused subscription. Requires --apply flag. |
| `cancel_subscription` | extended | delete | Cancel a subscription. By default cancels at end of period. Requires --apply flag. |
| `skip_billing_cycle` | extended | write | Skip the next billing cycle for a subscription. Requires --apply flag. |
| `list_billing_cycles` | extended | read | List billing cycles for a subscription. |
| `get_billing_cycle` | extended | read | Get details for a specific billing cycle. |
| `get_subscription_events` | extended | read | Get event history (audit log) for a subscription. |

## sync

Tier: **experimental** — needs a configured sync endpoint and a database handle the binding does not expose

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `sync_status` | experimental | read | Get the current sync status between local database and remote sequencer. Shows pending events, sync lag, and connection status. |
| `sync_push` | experimental | write | Push pending local events to the remote sequencer. Requires --apply flag for actual push. |
| `sync_pull` | experimental | write | Pull events from the remote sequencer and store them locally. |
| `sync_outbox` | experimental | read | List events in the local outbox. Shows pending, synced, failed, and rejected events. |
| `sync_pulled_events` | experimental | read | List events already pulled from the sequencer and stored locally. Can optionally include plaintext payloads or decrypt encrypted payloads with local keys. |
| `sync_decrypt_event` | experimental | read | Decrypt an encrypted sync event from the local outbox or pulled-event store using the local recipient key. Supports legacy X25519 and hybrid X25519 + ML-KEM-768 wraps. |
| `sync_retry_failed` | experimental | admin | Reset failed events to pending status so they can be retried. Requires --apply flag. |
| `sync_entity_history` | experimental | read | Get the event history for a specific entity from the remote sequencer or the local pulled-event store. |
| `sync_full` | experimental | admin | Perform a full sync: push pending events then pull new events. Requires --apply flag for push. |
| `sync_conflicts` | experimental | read | List unresolved sync conflicts. Conflicts occur when local and remote events modify the same entity concurrently. |
| `sync_resolve` | experimental | admin | Resolve a specific sync conflict using a resolution strategy. Requires --apply flag. |
| `sync_rebase` | experimental | admin | Resolve all sync conflicts using a resolution strategy. Requires --apply flag. |
| `sync_verify_receipt` | experimental | read | Verify the signature on a VES event receipt. Supports legacy Ed25519 receipts and hybrid Ed25519 + ML-DSA-65 bundles. |
| `sync_verify_inclusion` | experimental | read | Verify a Merkle inclusion proof for a VES event. Proves the event is included in a committed batch. |
| `sync_inspect_commitment` | experimental | read | Inspect a VES batch commitment from the sequencer. Shows the Merkle root, sequence range, and event count. |
| `agent_key_generate` | experimental | write | Generate a new Ed25519 signing or X25519 encryption key pair for an agent. Requires --apply flag. |
| `agent_key_list` | experimental | read | List signing and/or encryption keys for an agent. Returns only public metadata — never exposes private keys. |
| `agent_key_info` | experimental | read | Get detailed info for a specific agent key. Returns metadata only — no private key. |
| `agent_key_rotate` | experimental | write | Rotate an agent key: generate a new key and revoke the current one. Requires --apply flag. |
| `agent_key_export` | experimental | read | Export an agent public key for sequencer registration. Returns public key only — never the private key. |

## manufacturing

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_boms` | extended | read | List all Bills of Materials (BOMs). BOMs define the components/ingredients needed to manufacture a product. |
| `get_bom` | extended | read | Get a Bill of Materials by ID, including all components/ingredients. |
| `create_bom` | extended | write | Create a new Bill of Materials for a product. Defines what components/ingredients are needed. |
| `add_bom_component` | extended | write | Add a component/ingredient to a Bill of Materials. |
| `activate_bom` | extended | write | Activate a BOM to make it available for work orders. |
| `list_work_orders` | extended | read | List manufacturing work orders (production runs), optionally filtered by product, BOM, status, priority, assignee or work center. |
| `get_work_order` | extended | read | Get a work order by ID with full details. |
| `create_work_order` | extended | write | Create a manufacturing work order to produce a quantity of product. |
| `start_work_order` | extended | write | Start a work order (begin production). |
| `complete_work_order` | extended | write | Complete a work order with the quantity produced. |
| `cancel_work_order` | extended | delete | Cancel a work order. |

## payments

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_payments` | core | read | List all payments in the system. |
| `get_payment` | core | read | Get a payment by ID. |
| `create_payment` | core | write | Create a payment for an order. |
| `complete_payment` | core | write | Mark a payment as completed. |
| `mark_failed_payment` | core | write | Mark a payment as failed with a required reason and optional failure code. |
| `cancel_payment` | core | delete | Cancel a payment before settlement is finalized. |
| `create_refund` | core | write | Create a refund for a payment. |
| `list_payment_providers` | core | read | List available payment providers and capabilities for agentic payment flows. |
| `create_payment_intent` | core | write | Create a provider-backed payment intent with idempotency support for governed checkout flows. |
| `get_payment_intent` | core | read | Get a provider-backed payment intent by ID. |
| `list_payment_intents` | core | read | List provider-backed payment intents with optional filtering. |
| `list_payment_settlements` | core | read | List settlement records produced by provider payout reconciliation. |
| `list_payment_settlement_batches` | core | read | List provider payout batches generated from settlement runs. |
| `create_payment_settlement_batch` | core | write | Create a settlement batch for captured/refunded payment intents to simulate provider payout reconciliation. |
| `reconcile_payment_provider` | core | read | Reconcile payment intents against settlement records to find pending settlement or over-settlement drift. |
| `capture_payment_intent` | core | write | Capture all or part of a provider-backed payment intent. |
| `cancel_payment_intent` | core | delete | Cancel an uncaptured provider-backed payment intent. |
| `refund_payment_intent` | core | write | Refund all or part of a captured provider-backed payment intent. |
| `ingest_payment_provider_webhook` | core | write | Ingest a payment provider webhook event and reconcile payment intent state in shadow or production mode. |

## stablecoin

Tier: **experimental** — needs an agent signing key and an EVM chain

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `get_agent_wallet` | experimental | read | Get the agent wallet address for a specific blockchain. Returns the wallet address derived from VES keys. |
| `get_wallet_balance` | experimental | read | Check the balance of the agent wallet on a blockchain. |
| `create_stablecoin_payment` | experimental | write | Create and execute a blockchain payment to a wallet address. Supports stablecoins plus native BTC and shielded ZEC flows. |
| `list_supported_chains` | experimental | read | List all supported blockchain networks for agent payment execution. |

## treasury

Tier: **experimental** — on-chain stablecoin treasury; needs a configured chain and token

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `treasury_balance` | experimental | read | Get treasury balances for an agent. |
| `treasury_ledger` | experimental | read | List recent treasury transactions for an agent. |
| `treasury_deposit` | experimental | write | Record a treasury deposit for an agent (funds received). |
| `treasury_buy` | experimental | write | Purchase tokens using treasury stablecoin balances. |
| `treasury_list_tokens` | experimental | read | List available tokens from chain config and custom registry. |
| `treasury_register_token` | experimental | admin | Add or update a token in the treasury registry. |

## erc8004

Tier: **experimental** — on-chain identity registry; needs a registry database and chain

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `erc8004_register_identity` | experimental | admin | Register or update an ERC-8004 agent identity record. |
| `erc8004_link_wallet` | experimental | write | Link a wallet to an existing ERC-8004 identity record. |
| `erc8004_get_identity` | experimental | read | Get an ERC-8004 identity by registry + agent id. |
| `erc8004_get_by_wallet` | experimental | read | Get an ERC-8004 identity by wallet address. |
| `erc8004_list_identities` | experimental | read | List ERC-8004 identities. |

## x402

Tier: **experimental + extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `x402_create_payment_intent` | extended | write | Create an x402 payment intent for AI agent commerce. Returns a signing hash that the payer agent must sign with Ed25519. |
| `x402_sign_intent` | extended | write | Sign an x402 payment intent with an Ed25519 signature. Supports manual signature/public key or local agent-key signing. |
| `x402_get_intent` | extended | read | Get details of an x402 payment intent. |
| `x402_list_intents` | extended | read | List x402 payment intents with optional filtering. |
| `x402_settle_intent_onchain` | experimental— submits an on-chain settlement transaction | write | Execute a signed x402 intent on-chain using an agent wallet, then mark the intent as settled. |
| `x402_execute_agent_payment` | experimental— needs an agent signing key (wallet) | write | Execute end-to-end agentic payment: create intent, locally sign with payer agent key, settle on-chain, and optionally record incoming settlement for payee agent. |
| `x402_record_incoming_settlement` | extended | write | Record a settled x402 intent as an incoming treasury deposit for a local payee agent. |
| `x402_mark_settled` | extended | write | Mark an x402 payment intent as settled on-chain. Called after blockchain confirmation. |
| `x402_get_next_nonce` | extended | read | Get the next nonce for a payer address. Used for replay protection. |
| `x402_credit_balance` | extended | read | Get x402 credit balance for a payer (prepaid meter for streaming usage). |
| `x402_get_credit_account` | extended | read | Get the x402 credit account record for a payer address. |
| `x402_credit_deposit` | extended | write | Credit (deposit) x402 balance for metered usage. Requires --apply. |
| `x402_credit_debit` | extended | write | Debit x402 balance for metered usage. Requires --apply. |
| `x402_credit_transactions` | extended | read | List x402 credit ledger transactions. |

## agent-cards

Tier: **experimental** — agent-to-agent discovery; depends on the A2A wallet identity

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `register_agent_card` | experimental | write | Register an AI agent card for A2A commerce. Advertises capabilities, supported networks, and payment assets. |
| `discover_agents` | experimental | read | Discover AI agents with specific commerce capabilities. Find sellers, buyers, or agents supporting specific networks/assets. |
| `get_agent_card` | experimental | read | Get details of a registered AI agent card. |
| `verify_agent` | experimental | write | Verify an AI agent card (admin operation). Upgrades trust level to Verified. |
| `list_agent_cards` | experimental | read | List all registered AI agent cards. |

## a2a

Tier: **experimental** — most tools need an agent wallet identity; some are in the smoke backlog

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `a2a_pay` | experimental | write | Pay another AI agent directly. Send supported payment assets including USDC, ssUSD, BTC, or shielded ZEC to another agent by identity wallet, native chain address, or agent ID. |
| `a2a_request_payment` | experimental | write | Request payment from another agent. Creates a payment request that the other agent can pay. |
| `a2a_pay_request` | experimental | write | Pay an existing payment request from another agent. |
| `a2a_request_quote` | experimental | write | Request a price quote from another agent for goods or services. |
| `a2a_provide_quote` | experimental | write | Respond to a quote request with pricing (for sellers). |
| `a2a_accept_quote` | experimental | write | Accept a quote and pay. Automatically sends payment to the seller. |
| `a2a_decline_quote` | experimental | write | Decline a quote. |
| `a2a_fulfill_quote` | experimental | write | Mark a quote as fulfilled after delivering goods/services (for sellers). |
| `a2a_get_payment` | experimental | read | Get a single A2A payment by ID. Optionally refresh native on-chain confirmation state for supported settlement networks including Bitcoin and shielded Zcash. |
| `a2a_list_payments` | experimental | read | List A2A payments sent or received by this agent. Can optionally refresh pending native-chain settlement state for payments with on-chain transaction hashes. |
| `a2a_list_payment_requests` | experimental | read | List payment requests created by or sent to this agent. |
| `a2a_list_quotes` | experimental | read | List quotes where this agent is buyer or seller. |
| `a2a_get_balance` | experimental | read | Get an A2A payment summary for this agent, with optional asset/network filters and per-rail breakdowns. |
| `a2a_discover_agents` | experimental | read | Discover AI agents that can provide goods or services. Find sellers, buyers, or agents with specific capabilities. |
| `a2a_counter_quote` | experimental | write | Counter a quote with a different price (for buyers). Initiates or continues price negotiation with the seller. |
| `a2a_revise_quote` | experimental | write | Revise a quote after a buyer counter-offer (for sellers). Adjusts pricing in response to negotiation. |
| `a2a_create_escrow` | experimental | write | Create an escrow to hold funds between buyer and seller agents. Supports conditional release, time-based expiry, and dispute escalation. |
| `a2a_fund_escrow` | experimental | write | Fund an escrow, moving it to active status so the seller can begin work. |
| `a2a_release_escrow` | experimental | write | Release escrow funds to the seller. All release conditions must be met. |
| `a2a_refund_escrow` | experimental | write | Refund escrow funds back to the buyer. |
| `a2a_dispute_escrow` | experimental | write | Dispute an escrow, escalating it to the dispute resolution system. |
| `a2a_get_escrow` | experimental | read | Get details of an escrow by ID. |
| `a2a_list_escrows` | experimental | read | List escrows with optional filters. |
| `a2a_file_dispute` | experimental | write | File a formal dispute against an escrow. Begins the dispute resolution process with evidence collection and review. |
| `a2a_submit_evidence` | experimental | write | Submit evidence for an active dispute. |
| `a2a_resolve_dispute` | experimental | write | Resolve a dispute atomically with a full refund, seller release, exact split, or escalation. |
| `a2a_get_dispute` | experimental | read | Get details of a dispute by ID, including evidence count. |
| `a2a_list_disputes` | experimental | read | List disputes with optional filters. |
| `a2a_rate_agent` | experimental | write | Rate an agent after a transaction. Scores 1-5 with optional dimension ratings (reliability, quality, speed, communication). |
| `a2a_get_reputation` | experimental | read | Get reputation and trust score for an agent. |
| `a2a_respond_to_feedback` | experimental | write | Respond to feedback left on your agent (only the rated agent can respond). |
| `a2a_register_service` | experimental | write | Register a service that this agent provides. Other agents can discover and purchase your services. |
| `a2a_list_services` | experimental | read | List available agent services with optional filters and search. |
| `a2a_get_service` | experimental | read | Get details of a specific agent service. |
| `a2a_send_notification` | experimental | write | Send a webhook notification to another agent. Delivers a signed payload to their configured endpoint. |
| `a2a_list_notification_log` | experimental | read | View the webhook notification delivery log with optional filters. |
| `a2a_configure_webhooks` | experimental | write | Configure webhook settings for an agent. Set the endpoint URL, signing secret, and which event types to receive. |
| `a2a_list_webhook_dlq` | experimental | admin | List quarantined webhook notifications that permanently failed delivery. Use to inspect and replay failed deliveries. |
| `a2a_quarantine_failed_webhooks` | experimental | admin | Move permanently failed webhook notifications to the dead letter queue. Notifications that exhausted all retry attempts are quarantined for inspection. |
| `a2a_replay_dlq_entry` | experimental | admin | Replay a dead letter queue entry by moving it back to the notification log for retry. Resets the attempt counter. |
| `a2a_purge_dlq` | experimental | admin | Purge old dead letter queue entries. Removes entries quarantined more than the specified number of days ago. |
| `a2a_dlq_count` | experimental | read | Get the count of entries in the webhook dead letter queue. |
| `a2a_create_agent_subscription` | experimental | write | Create a recurring payment subscription between two agents. Supports trial periods and configurable billing intervals. |
| `a2a_pause_agent_subscription` | experimental | write | Pause an active agent subscription. Billing is suspended until resumed. |
| `a2a_resume_agent_subscription` | experimental | write | Resume a paused agent subscription. Recalculates billing dates from now. |
| `a2a_cancel_agent_subscription` | experimental | write | Cancel an agent subscription. Can cancel immediately or at the end of the current billing period. |
| `a2a_get_agent_subscription` | experimental | read | Get details of an agent-to-agent subscription. |
| `a2a_list_agent_subscriptions` | experimental | read | List agent-to-agent subscriptions with optional filters. |
| `a2a_process_subscription_billing` | experimental | write | Process all due subscription billing cycles. Bills active subscriptions, handles past-due retries, transitions expired trials, and cancels end-of-period subscriptions. |
| `a2a_create_split_payment` | experimental | write | Create a multi-party split payment. Splits a payment across 2+ recipients by percentage or fixed amounts, with optional platform fee. |
| `a2a_execute_split_payment` | experimental | write | Execute a pending split payment, sending funds to each recipient. Tracks per-recipient status. |
| `a2a_get_split_payment` | experimental | read | Get details of a split payment including all recipient shares and statuses. |
| `a2a_list_split_payments` | experimental | read | List split payments with optional filters. |
| `a2a_create_conditional_payment` | experimental | write | Create a conditional payment that combines escrow with x402 payment intent. Funds are held in escrow until conditions are met, then automatically settled. |
| `a2a_check_payment_conditions` | experimental | read | Check whether all release conditions are met for a conditional payment (escrow). |
| `a2a_settle_conditional_payment` | experimental | write | Settle a conditional payment. Checks all conditions, releases escrow funds to the seller, and marks the x402 intent as settled. |
| `a2a_subscribe_events` | experimental | write | Subscribe an agent to receive real-time events. Supports wildcard and prefix-based event type filtering. |
| `a2a_list_event_subscriptions` | experimental | read | List active event subscriptions for an agent. |
| `a2a_get_event_history` | experimental | read | Get historical events for an agent with optional filtering. |

## agent-runtime

Tier: **experimental** — autonomous agent loops over wallets, escrow and on-chain settlement

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `agent_create_runtime` | experimental | write | Create an autonomous AI agent runtime with a wallet, negotiation strategy, and budget. The agent can then register services, discover other agents, negotiate quotes, and make payments autonomously. |
| `agent_destroy_runtime` | experimental | delete | Destroy an agent runtime and clean up resources. |
| `agent_list_runtimes` | experimental | read | List all active agent runtimes in this session, with optional asset/network budget scope. |
| `agent_get_status` | experimental | read | Get detailed status of an agent runtime including budget, strategy, registered services, and optional rail-specific settlement context. |
| `agent_set_strategy` | experimental | write | Change an agent's negotiation strategy. Available: always-accept, budget-gated, negotiator, best-of-n, reputation-aware. |
| `agent_get_budget` | experimental | read | Get the current budget status of an agent, with optional asset/network scope for multi-rail payment budgets. |
| `agent_tick` | experimental | write | Process one autonomous cycle for an agent. The agent will respond to pending quotes, evaluate received offers, and auto-fulfill accepted deals. |
| `agent_start_loop` | experimental | write | Start the agent's autonomous polling loop. The agent will continuously process incoming work. |
| `agent_stop_loop` | experimental | write | Stop the agent's autonomous polling loop. |
| `agent_register_service` | experimental | write | Register a service in the A2A marketplace so other agents can discover and purchase it. |
| `agent_discover_services` | experimental | read | Search the A2A marketplace for services by category or capability. |
| `agent_create_escrow_deal` | experimental | write | Create an escrow-backed transaction between agents. Funds are held until conditions are met (seller fulfilled, buyer confirmed, time lock, or milestone). |
| `agent_subscribe_to_service` | experimental | write | Subscribe an agent to another agent's recurring service (e.g., daily data feed, monthly analytics). |
| `agent_rate_counterparty` | experimental | write | Rate another agent after a transaction. Builds reputation in the marketplace. |
| `agent_get_reputation` | experimental | read | Get an agent's reputation score, trust tier, and feedback summary. |
| `agent_create_split_deal` | experimental | write | Create a multi-party payment split. Revenue from a deal is distributed to multiple agents. |
| `agent_get_event_history` | experimental | read | Get an agent's event stream history with optional filters for event type, time window, and payment rail. |
| `agent_enable_settlement` | experimental | write | Enable on-chain payment settlement for an agent runtime. The agent will settle payments on the specified blockchain using derived wallets. |
| `agent_get_chain_balance` | experimental | read | Get the on-chain payment-token balance for an agent runtime with settlement enabled. |
| `agent_broadcast_rfq` | experimental | write | Broadcast a Request for Quotation (RFQ) to multiple sellers in the marketplace. Sellers matching the filter will receive quote requests. |
| `agent_collect_rfq_responses` | experimental | read | Collect and score all responses for an RFQ broadcast. |
| `agent_award_rfq` | experimental | write | Award an RFQ to the best-scored (or specified) seller. Accepts the winner's quote and declines all others. |
| `agent_get_marketplace_metrics` | experimental | read | Get marketplace performance metrics for a registered service (success rate, response time, etc.). |
| `agent_attach_sla` | experimental | write | Attach a Service Level Agreement to a registered service. Defines performance thresholds and penalties. |
| `agent_check_sla_compliance` | experimental | read | Check if a service is meeting its SLA commitments. |
| `agent_create_workflow` | experimental | write | Create a multi-agent workflow with DAG-based step dependencies. Steps execute in topological order. |
| `agent_execute_workflow` | experimental | write | Execute a workflow. Steps run in dependency order with parallel fan-out where possible. |
| `agent_get_workflow_status` | experimental | read | Get the current status and progress of a workflow. |
| `agent_set_dynamic_pricing` | experimental | write | Configure dynamic pricing for an agent. Sets volume breaks, reputation tiers, peak hours, and loyalty tiers. |

## shipments

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_shipments` | core | read | List all shipments. |
| `get_shipment` | core | read | Get a shipment by ID. |
| `create_shipment` | core | write | Create a shipment for an order. |
| `ship_shipment` | core | write | Mark a shipment as shipped with an optional tracking number. |
| `deliver_shipment` | core | write | Mark a shipment as delivered. |
| `cancel_shipment` | core | delete | Cancel a shipment before delivery is completed. |
| `list_shipping_providers` | core | read | List shipping providers and capabilities for quoting, labeling, and tracking. |
| `quote_shipping_rates` | core | read | Quote carrier rates from provider adapters using structured parcel data and destination address. |
| `create_shipping_label` | core | write | Create a carrier label from quoted rates or explicit service code. |
| `void_shipping_label` | core | delete | Void a shipping label before final delivery. |
| `track_shipping_label` | core | read | Track a shipping label by label ID or tracking number. |
| `list_shipping_labels` | core | read | List provider-backed shipping labels with optional filtering. |
| `ingest_shipping_provider_webhook` | core | write | Ingest a shipping provider webhook event and reconcile label/tracking state for shadow mode operations. |
| `handle_fulfillment_exception` | core | write | Execute governed fulfillment exception workflows for carrier failure, partial shipment, split tender, and returns arbitration. |

## suppliers

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_suppliers` | extended | read | List all suppliers. |
| `get_supplier` | extended | read | Get a supplier by ID. |
| `create_supplier` | extended | write | Create a new supplier. |
| `list_purchase_orders` | extended | read | List purchase orders, optionally filtered by supplier, status, date range or total. |
| `get_purchase_order` | extended | read | Get a purchase order by ID. |
| `create_purchase_order` | extended | write | Create a purchase order to a supplier. |
| `submit_purchase_order` | extended | write | Submit a purchase order for approval. |
| `approve_purchase_order` | extended | write | Approve a purchase order. |
| `send_purchase_order` | extended | write | Send a PO to the supplier. |
| `cancel_purchase_order` | extended | write | Cancel a purchase order. |

## invoices

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_invoices` | extended | read | List all invoices. |
| `get_invoice` | extended | read | Get an invoice by ID. |
| `create_invoice` | extended | write | Create an invoice for a customer. |
| `send_invoice` | extended | write | Send an invoice to the customer. |
| `void_invoice` | extended | delete | Void an invoice so it can no longer be paid or collected. |
| `record_invoice_payment` | extended | write | Record payment on an invoice. |
| `get_overdue_invoices` | extended | read | Get all overdue invoices. |

## warranties

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_warranties` | extended | read | List all warranties. |
| `get_warranty` | extended | read | Get a warranty by ID. |
| `create_warranty` | extended | write | Create a warranty for a product. |
| `create_warranty_claim` | extended | write | File a warranty claim. |
| `approve_warranty_claim` | extended | write | Approve a warranty claim. |
| `deny_warranty_claim` | extended | write | Deny a warranty claim with a reason. |
| `complete_warranty_claim` | extended | write | Complete a warranty claim with a final resolution. |

## import

Tier: **experimental** — IdMapStore needs a database handle the binding does not expose

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `import_shopify_data` | experimental | write | Import data from a Shopify store. Supports API, CSV file, and JSON file sources. Imports customers, products, orders, and inventory in dependency order. |
| `import_shopify_shadow_data` | experimental | write | Run Shopify interop in shadow mode for products, inventory, orders, fulfillments, and customers. Produces parity-ready summaries without writes unless explicitly enabled. |
| `import_status` | experimental | read | Get the status of the most recent import operation. |
| `list_id_mappings` | experimental | read | List external ID to StateSet ID mappings for a platform. Useful for verifying imported data. |
| `import_csv` | experimental | write | Import data from a CSV file. Auto-detects Shopify format or uses generic column mapping. |
| `import_json` | experimental | write | Import data from a JSON file (Shopify REST API response format or array). |
| `export_data` | experimental | read | Export StateSet data to JSON format. Useful for parity testing after imports. |
| `import_woocommerce_data` | experimental | write | Import data from a WooCommerce store via REST API. Imports customers, products, orders, and inventory in dependency order. |
| `configure_stripe_webhooks` | experimental | write | Configure Stripe webhook endpoint in the webhook server. Sets up the Stripe v1 signature verification and registers the webhook source. |
| `configure_woocommerce_webhooks` | experimental | write | Configure WooCommerce webhook endpoint in the webhook server. Sets up HMAC-SHA256 signature verification. |

## policies

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `evaluate_policy` | extended | read | Evaluate a policy domain against a context object. Returns allow/deny decision with full explanation of which rules matched and why. |
| `list_policies` | extended | read | List all registered policy sets. Shows policy set IDs, names, domains, and rule counts. |
| `register_policy_template` | extended | write | Activate one of the built-in policy templates. Available templates: autoApproveReturns, inventoryRestock, orderFraudDetection, promotionEligibility, subscriptionRules. |
| `load_policy_file` | extended | write | Load a YAML or JSON policy file into the engine. The file must define a valid policy set with domain, rules, and actions. |
| `explain_policy_denial` | extended | read | Re-evaluate a policy domain with verbose per-condition breakdown. Shows which conditions matched, which did not, and the expected vs actual values for each. |

## vector

Tier: **experimental** — needs OPENAI_API_KEY (external embedding service)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `vector_search_products` | experimental | read | Search products using natural language query with hybrid semantic + BM25 ranking. Returns products sorted by relevance score. |
| `vector_search_customers` | experimental | read | Search customers using natural language query with hybrid semantic + BM25 ranking. |
| `vector_search_orders` | experimental | read | Search orders using natural language query with hybrid semantic + BM25 ranking. |
| `vector_search_inventory` | experimental | read | Search inventory items using natural language query with hybrid semantic + BM25 ranking. |
| `vector_index_product` | experimental | write | Index a single product for vector search by its ID. |
| `vector_index_customer` | experimental | write | Index a single customer for vector search by their ID. |
| `vector_index_order` | experimental | write | Index a single order for vector search by its ID. |
| `vector_index_inventory` | experimental | write | Index a single inventory item for vector search by its ID. |
| `vector_index_all_products` | experimental | admin | Index all products in the database for vector search. This may take a while for large catalogs. |
| `vector_index_all_customers` | experimental | admin | Index all customers in the database for vector search. |
| `vector_index_all_orders` | experimental | admin | Index all orders in the database for vector search. |
| `vector_index_all_inventory` | experimental | admin | Index all inventory items in the database for vector search. |
| `vector_stats` | experimental | read | Get statistics about vector embeddings including counts by entity type. |
| `vector_clear` | experimental | admin | Clear all vector embeddings for a specific entity type. |
| `vector_clear_all` | experimental | admin | Clear all vector embeddings across all entity types. |
| `vector_reindex_all` | experimental | admin | Rebuild all vector embeddings from scratch. Clears existing embeddings then re-indexes all products, customers, orders, and inventory items. Use this after bulk data imports or to fix stale embeddings. |

## gift-cards

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_gift_card` | core | write | Create a new gift card with an initial balance. |
| `get_gift_card` | core | read | Get a gift card by ID or code. |
| `list_gift_cards` | core | read | List all gift cards with optional filters. |
| `charge_gift_card` | core | write | Charge (deduct) an amount from a gift card balance. |
| `refund_to_gift_card` | core | write | Refund an amount back to a gift card. |
| `disable_gift_card` | core | write | Disable a gift card so it can no longer be used. |
| `check_gift_card_balance` | core | read | Check the current balance of a gift card by ID or code. |

## store-credits

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_store_credit` | core | write | Issue store credit to a customer. |
| `get_store_credit` | core | read | Get store credit details by ID. |
| `list_store_credits` | core | read | List store credits with optional filters. |
| `adjust_store_credit` | core | write | Adjust a store credit balance (add or subtract). |
| `apply_store_credit` | core | write | Apply store credit to an order. |

## segments

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_segment` | extended | write | Create a customer segment with filter conditions. |
| `get_segment` | extended | read | Get a segment by ID including its conditions and member count. |
| `list_segments` | extended | read | List all customer segments. |
| `update_segment` | extended | write | Update a segment name, description, or conditions. |
| `evaluate_segment_membership` | extended | read | Check whether a customer is a recorded member of a segment. Reads stored membership; rules are not re-evaluated. |

## shipping-zones

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_shipping_zone` | extended | write | Create a shipping zone with country/region rules. |
| `get_shipping_zone` | extended | read | Get a shipping zone by ID, including its shipping methods. |
| `list_shipping_zones` | extended | read | List all shipping zones. |
| `update_shipping_zone` | extended | write | Update a shipping zone name, countries, or regions. |
| `create_shipping_method` | extended | write | Create a shipping method within a zone (e.g., Standard, Express, Overnight). weight_based and price_based methods pick their rate from `conditions`; free always rates 0; flat and calculated use baseRate. |
| `calculate_shipping_rate` | extended | read | Calculate available shipping rates for a destination address. |
| `list_shipping_methods` | extended | read | List shipping methods for a specific zone. |

## units-of-measure

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_unit_classes` | extended | read | List unit classes (e.g. weight, volume). |
| `create_unit_class` | extended | write | Create a unit class. |
| `delete_unit_class` | extended | write | Delete a unit class. |
| `list_units_of_measure` | extended | read | List units of measure, optionally scoped to a unit class. |
| `create_unit_of_measure` | extended | write | Create a unit of measure within a unit class. |
| `set_base_unit_of_measure` | extended | write | Mark a unit of measure as the base unit for its class. |
| `delete_unit_of_measure` | extended | write | Delete a unit of measure. |
| `list_unit_conversion_rules` | extended | read | List unit conversion rules. |
| `create_unit_conversion_rule` | extended | write | Create a unit conversion rule (system-wide or SKU-specific). |
| `delete_unit_conversion_rule` | extended | write | Delete a unit conversion rule. |

## stock-snapshots

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_stock_snapshots` | extended | read | List stock snapshots (header level). |
| `get_stock_snapshot` | extended | read | Get a stock snapshot by ID. |
| `get_latest_stock_snapshot` | extended | read | Get the most recent stock snapshot. |
| `capture_stock_snapshot` | extended | write | Capture a stock snapshot; totals are computed from the supplied lines. |
| `delete_stock_snapshot` | extended | write | Delete a stock snapshot. |

## print-stations

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_print_stations` | extended | read | List paired print stations. |
| `get_print_station` | extended | read | Get a print station by ID. |
| `pair_print_station` | extended | write | Pair a new print station. Returns a one-time pairing token. |
| `revoke_print_station` | extended | write | Revoke a paired print station. |
| `list_print_jobs` | extended | read | List print jobs for a station. |
| `enqueue_print_job` | extended | write | Enqueue a print job to a station. |
| `pick_up_next_print_job` | extended | write | Pick up the next queued print job for a station. |
| `complete_print_job` | extended | write | Mark a print job printed or failed. |

## integration-mappings

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_integration_mappings` | extended | read | List integration value mappings. |
| `get_integration_mapping` | extended | read | Get an integration mapping by ID. |
| `resolve_integration_mapping` | extended | read | Resolve the internal value for an external value. |
| `create_integration_mapping` | extended | write | Create an integration value mapping. |
| `update_integration_mapping` | extended | write | Update an integration mapping. |
| `bulk_upsert_integration_mappings` | extended | write | Bulk upsert integration mappings. |
| `delete_integration_mapping` | extended | write | Delete an integration mapping. |

## integration-field-mappings

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_integration_field_mappings` | extended | read | List integration field mappings. |
| `get_integration_field_mapping` | extended | read | Get an integration field mapping by ID. |
| `list_integration_mapping_groups` | extended | read | List the distinct mapping groups for an integration account. |
| `create_integration_field_mapping` | extended | write | Create an integration field mapping. |
| `update_integration_field_mapping` | extended | write | Update an integration field mapping. |
| `bulk_create_integration_field_mappings` | extended | write | Bulk create integration field mappings. |
| `bulk_delete_integration_field_mappings` | extended | write | Bulk delete integration field mappings by ID. |
| `delete_integration_field_mapping` | extended | write | Delete an integration field mapping. |

## payment-obligations

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_payment_obligations` | extended | read | List payment obligations. |
| `get_payment_obligation` | extended | read | Get a payment obligation by ID. |
| `get_payment_obligation_dashboard` | extended | read | Aggregate payment obligation dashboard as of a date. |
| `create_payment_obligation` | extended | write | Create a payment obligation. |
| `record_payment_obligation_payment` | extended | write | Record a payment against an obligation. |
| `set_payment_obligation_status` | extended | write | Set the status of a payment obligation. |
| `link_payment_obligation_bill` | extended | write | Link an accounts-payable bill to a payment obligation. |

## purgatory

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_purgatory_orders` | extended | read | List staged purgatory orders. |
| `get_purgatory_order` | extended | read | Get a purgatory order by ID. |
| `ingest_purgatory_order` | extended | write | Ingest an external order into purgatory. |
| `map_purgatory_line` | extended | write | Map a staged line to a product and/or toggle its flags. |
| `post_purgatory_order` | extended | write | Post a fully-resolved order out of purgatory. |
| `delete_purgatory_order` | extended | write | Delete a purgatory order. |

## topology-snapshots

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_topology_snapshots` | extended | read | List operational topology snapshots. |
| `get_topology_snapshot` | extended | read | Get a topology snapshot by ID. |
| `get_latest_topology_snapshot` | extended | read | Get the most recent topology snapshot. |
| `capture_topology_snapshot` | extended | write | Capture a topology snapshot; health is derived from the supplied metrics. |
| `delete_topology_snapshot` | extended | write | Delete a topology snapshot. |

## vendor-returns

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_vendor_returns` | extended | read | List vendor returns. |
| `get_vendor_return` | extended | read | Get a vendor return by ID. |
| `create_vendor_return` | extended | write | Create a draft vendor return. |
| `submit_vendor_return` | extended | write | Submit a draft vendor return to the supplier. |
| `process_vendor_return` | extended | write | Process a vendor return, optionally generating a vendor credit. |
| `cancel_vendor_return` | extended | write | Cancel a vendor return. |

## reviews

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_review` | extended | write | Create a product review. |
| `get_review` | extended | read | Get a review by ID. |
| `list_reviews` | extended | read | List reviews with optional filters. |
| `approve_review` | extended | write | Approve a pending review for public display. |
| `reject_review` | extended | write | Reject a review with a reason. |
| `get_review_summary` | extended | read | Get aggregated review summary for a product including average rating and rating distribution. |
| `flag_review` | extended | write | Flag a review for manual moderation. |

## wishlists

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_wishlist` | extended | write | Create a new wishlist for a customer. |
| `get_wishlist` | extended | read | Get a wishlist by ID including all items. |
| `add_to_wishlist` | extended | write | Add a product to a wishlist. |
| `remove_from_wishlist` | extended | write | Remove a product from a wishlist. |
| `list_wishlists` | extended | read | List wishlists for a customer. |
| `convert_wishlist_to_cart` | extended | write | Create a cart for the wishlist's customer and add each wishlist item at its catalog variant price. Items that cannot be priced are reported, not added. |

## loyalty

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_loyalty_program` | extended | admin | Create a loyalty program with an earning rate and optional tiers. |
| `get_loyalty_program` | extended | read | Get loyalty program details including its tiers. |
| `enroll_customer` | extended | write | Enroll a customer in a loyalty program. |
| `get_loyalty_account` | extended | read | Get a customer's loyalty account in a program: points balance and tier. |
| `earn_points` | extended | write | Award loyalty points to a customer's account in a program. |
| `redeem_points` | extended | write | Redeem loyalty points from a customer's account, optionally for a reward in the program. The engine refuses a redemption larger than the balance. |
| `list_rewards` | extended | read | List rewards in a loyalty program. |
| `create_reward` | extended | admin | Create a redeemable reward in a loyalty program. |

## fraud

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `assess_order_fraud` | extended | write | Record a fraud assessment for an order from caller-supplied signals. The engine stores the signals, sets the risk score to the highest signal score, and decides accept (or review when the risk score is 0.8 or higher). One assessment per order. |
| `get_fraud_assessment` | extended | read | Get the fraud assessment for an order. |
| `list_fraud_signals` | extended | read | List the fraud signals recorded on one order, or on the most recent assessments across orders. |
| `create_fraud_rule` | extended | admin | Create a fraud rule: when a signal of `signalType` scores at or above `threshold`, apply `action`. Rules are created enabled. |
| `update_fraud_rule` | extended | admin | Update a fraud rule (name, description, threshold, action, or enabled flag). |
| `review_flagged_order` | extended | write | Record a manual review of an order's fraud assessment: set its decision, the reviewer, and why. |

## connectors

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_connector_marketplace` | extended | read | List available WASM connectors in the local marketplace catalog. |
| `publish_wasm_connector` | extended | admin | Publish a WASM connector to the local marketplace catalog (app-store style ecosystem index). |
| `install_wasm_connector` | extended | write | Install a connector from marketplace catalog into the local connector runtime. |
| `assess_wasm_connector_safety` | extended | read | Compute connector safety scorecard and risk signals for marketplace governance and installation policy. |
| `certify_wasm_connector` | extended | admin | Issue marketplace certification metadata for a connector version using automated safety score + trust policy. |
| `sign_wasm_connector_attestation` | extended | admin | Sign a marketplace connector attestation using local signing key material for trustable install/execute verification. |
| `verify_wasm_connector_attestation` | extended | read | Verify connector trust attestation in the marketplace catalog before installation or execution. |
| `uninstall_wasm_connector` | extended | delete | Uninstall a connector version from the local connector runtime. |
| `list_installed_connectors` | extended | read | List installed connectors available to agentic runtime execution. |
| `get_installed_connector` | extended | read | Get details for an installed connector and its action contract. |
| `execute_wasm_connector` | extended | write | Execute an installed WASM connector action so agents can orchestrate ecosystem apps through iCommerce. |

## audit

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `audit_query` | extended | read | Query the audit log with optional filters. Returns recent permission checks and tool executions. |
| `audit_summary` | extended | read | Get a summary of audit activity including total entries, breakdown by result type, and most active tools. |
| `audit_export` | extended | admin | Export the full audit log for compliance purposes. Returns all entries with metadata for external archival. |
| `audit_retention` | extended | admin | Run audit log retention cleanup. Removes entries older than the configured retention period (default: 90 days). |

## proofs

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `verify_receipt` | extended | read | Verify a VES commerce receipt — checks signature, hash, and Merkle inclusion proof. |
| `generate_inclusion_proof` | extended | read | Generate a Merkle inclusion proof for an event within a batch. Events carrying tenantId, storeId, sequenceNumber, eventSigningHash and agentSignature get a ves-v1 proof that the SDK spec verifier accepts; events with only id and eventSigningHash get a legacy-v0 proof and a warning saying which fields are missing. |
| `verify_inclusion_proof` | extended | read | Verify a Merkle inclusion proof — confirms that a leaf hash is included in a Merkle root. |
| `generate_receipt_bundle` | extended | read | Generate a full verifiable receipt bundle for an event — includes event data, leaf hash, Merkle inclusion proof, and anchor metadata. |
| `inspect_batch` | extended | read | Inspect a batch of events — computes Merkle root, event count, and time range. |
| `export_compliance_package` | extended | read | Generate a compliance package — a complete set of verifiable receipts for all events in a batch, suitable for regulatory export or third-party audit. |
| `verify_chain_anchor` | extended | read | Verify that an event proof matches an expected on-chain anchor transaction hash and Merkle root. Confirms the event was committed to the chain. |

## circuit-breaker

Tier: **experimental** — agent spending breakers; state lives in ~/.stateset/a2a.db, not the --db store

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `agent_get_breaker_state` | experimental | read | Get the circuit breaker state for a specific agent, including trip reason and config. |
| `agent_get_spending_summary` | experimental | read | Get the spending summary for an agent: today's spend, monthly spend, and remaining limits. |
| `agent_get_all_breaker_states` | experimental | read | Get the circuit breaker states for all known agents. |
| `agent_trip_breaker` | experimental | admin | Manually trip the circuit breaker for a specific agent. Blocks all transactions until reset. |
| `agent_trip_all_breakers` | experimental | admin | Activate the global kill switch — blocks ALL agent transactions immediately. |
| `agent_reset_breaker` | experimental | admin | Reset the circuit breaker for a specific agent, allowing transactions again. |
| `agent_reset_all_breakers` | experimental | admin | Reset ALL circuit breakers and deactivate the global kill switch. |
| `agent_set_spending_limits` | experimental | admin | Update the spending limits for agent circuit breakers: per-transaction, daily, and monthly caps. |

## checkout

Tier: **experimental** — payment links and crypto checkout; state lives in ~/.stateset/a2a.db, not the --db store

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `create_payment_link` | experimental | write | Create a shareable payment link for instant checkout. Returns a short URL that buyers or agents can use. |
| `resolve_payment_link` | experimental | read | Resolve a payment link by ID or short code. Returns the link details, items, total, and expiry status. |
| `express_checkout` | experimental | write | One-call checkout from a payment link. Converts the link into an order and payment. |
| `agent_instant_checkout` | experimental | write | Agent-to-agent instant checkout. Creates a payment link and converts it in one step. Returns order and escrow IDs for A2A settlement. |
| `get_payment_link_status` | experimental | read | Get the status and metrics (views, conversions) for a payment link. |
| `list_payment_links` | experimental | read | List payment links with optional filters by status and customer. |
| `revoke_payment_link` | experimental | write | Revoke (cancel) an active payment link. Prevents further checkouts from it. |
| `checkout_with_crypto` | experimental | write | Express checkout with a crypto wallet. Similar to express_checkout but takes a wallet address and network for on-chain payment. |

## compliance

Tier: **experimental** — reads ~/.stateset/a2a.db, not the --db store; GDPR/SOC2 exports crash (smoke backlog)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `export_audit_trail` | experimental | admin | Export a complete audit trail of agent transactions and events for compliance review. Supports JSON and CSV formats with date range, agent, and event type filters. |
| `generate_1099k` | experimental | admin | Generate a 1099-K tax report for an agent. Summarizes gross payment amounts, transaction counts, and monthly breakdowns for a given tax year. |
| `export_gdpr_data` | experimental | admin | Export all personal data for a customer or agent (GDPR Article 20 — data portability). Returns personal data, payments, communications, and disputes. |
| `delete_gdpr_data` | experimental | admin | Delete personal data for GDPR right to erasure (Article 17). Optionally retains anonymized transaction records for legal/accounting requirements. |
| `compliance_summary` | experimental | read | Generate a compliance dashboard summary with transaction volume, dispute rates, policy violations, and top agents for a given period. |
| `soc2_evidence` | experimental | admin | Generate a SOC2 audit evidence package. Gathers structured evidence for requested controls: access_control, change_management, encryption, monitoring, incident_response. |

## catalog

Tier: **experimental** — agent product catalog; state lives in ~/.stateset/a2a.db, not the --db store

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `publish_product_catalog` | experimental | write | Publish a product to the machine-readable agent catalog. Makes products discoverable by AI agents with capability-based matching, trust levels, and machine-readable specs. |
| `query_agent_catalog` | experimental | read | Query the agent catalog for products matching filters. Supports capability, trust level, price, fulfillment chain, and category filtering. |
| `get_product_spec` | experimental | read | Get the full machine-readable spec for a catalog product. Returns capabilities, requirements, pricing, trust level, and a JSON Schema fragment. |
| `match_agent_to_products` | experimental | read | Find catalog products compatible with an agent based on its capabilities and trust level. Returns products sorted by relevance (capability overlap). |
| `match_product_to_agents` | experimental | read | Find agents compatible with a specific product. Filters available agents by the product's required trust level and capabilities. |
| `export_agent_catalog` | experimental | read | Export the agent catalog in JSON or OpenAPI format. Useful for sharing the catalog with other systems or generating API documentation. |

## a2a-automation

Tier: **experimental** — background services not attached on a fresh store (smoke backlog)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `a2a_billing_tick` | experimental | write | Run one billing cycle: process due subscriptions, execute payments, handle past-due, activate trials. |
| `a2a_billing_start` | experimental | admin | Start the automated billing executor loop. |
| `a2a_billing_stop` | experimental | admin | Stop the automated billing executor loop. |
| `a2a_billing_metrics` | experimental | read | Get billing executor metrics: total billed, failed, cancelled, etc. |
| `a2a_dispute_resolver_tick` | experimental | write | Run one dispute resolution cycle: auto-transition deadlines, apply rule-based arbitration. |
| `a2a_dispute_resolver_start` | experimental | admin | Start the automated dispute resolver loop. |
| `a2a_dispute_resolver_metrics` | experimental | read | Get dispute resolver metrics: transitions, resolutions, escalations. |
| `a2a_sla_enforce` | experimental | write | Enforce SLA penalties for a service: detect breaches and apply credits/suspensions/refunds. |
| `a2a_sla_enforce_all` | experimental | write | Run a full SLA enforcement cycle across all services. |
| `a2a_marketplace_auto_award` | experimental | write | Auto-award expired RFQs to the highest-scored response. Expires RFQs with no responses. |
| `a2a_marketplace_maintenance` | experimental | write | Run a full marketplace maintenance tick: auto-award + expiry + cleanup. |
| `a2a_list_failed_notifications` | experimental | read | List failed webhook notifications (dead-letter queue). Shows notifications that exceeded max retry attempts. |
| `a2a_replay_notification` | experimental | write | Manually retry a specific failed notification. |
| `a2a_notification_retry_all` | experimental | write | Trigger retry of all pending webhook notifications. |
| `a2a_webhook_dlq_status` | experimental | read | Get dead-letter queue metrics: pending, failed, delivered counts. |
| `a2a_health_check` | experimental | read | Run a full health check: database, sequencer, subsystems. |
| `a2a_readiness` | experimental | read | Check if the system is ready to accept traffic. |
| `x402_circuit_status` | experimental | read | Get x402 sequencer circuit breaker status: state (closed/open/half_open), failures, queue depth. |
| `a2a_rate_limit_metrics` | experimental | read | Get MCP rate limiter metrics: active buckets, top agents by request count. |
| `a2a_saga_execute` | experimental | write | Execute a multi-step transaction saga (e.g., purchase, subscription, RFQ). Automatically rolls back on failure. |
| `a2a_saga_status` | experimental | read | Get the status of a running or completed saga by ID. |
| `a2a_saga_list` | experimental | read | List sagas with optional status filter. |
| `a2a_saga_cancel` | experimental | write | Cancel a running saga and trigger compensation/rollback. |
| `a2a_cost_summary` | experimental | read | Get spend summary for an agent with optional asset/network filters and per-rail breakdowns. |
| `a2a_cost_counterparty_breakdown` | experimental | read | Get per-counterparty spend/earn breakdown for an agent, with optional asset/network filters and per-rail details. |
| `a2a_cost_operation_breakdown` | experimental | read | Get per-operation cost breakdown for an agent, with optional asset/network filters and per-rail details. |
| `a2a_cost_daily_trend` | experimental | read | Get daily spend and earnings trend for an agent, with optional asset/network filters and per-rail day breakdowns. |
| `a2a_cost_anomalies` | experimental | read | Detect per-rail spending anomalies, with optional asset/network filters to avoid mixed-unit comparisons. |
| `a2a_cost_margin_analysis` | experimental | read | Get margin analysis with optional asset/network filters and per-rail counterparty breakdowns. |
| `a2a_cost_budget_forecast` | experimental | read | Forecast when a budget in the selected asset units will be exhausted, with optional asset/network filters and per-rail spend breakdowns. |
| `a2a_cost_top_spenders` | experimental | read | Get top-spending agents across the system, with optional asset/network filters. |
| `a2a_escrow_process_all` | experimental | write | Process all escrows: auto-release time-locked escrows where conditions are met, expire past-deadline escrows. |

## a2a-observability

Tier: **experimental** — webhook DLQ store methods missing (smoke backlog)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `a2a_get_trace` | experimental | read | Retrieve all spans for a distributed trace ID. Shows the full journey of a transaction across agents. |
| `a2a_tracing_metrics` | experimental | read | Get tracing metrics: p50/p95/p99 latency, error rate, throughput, span count. |
| `a2a_recent_spans` | experimental | read | Get the most recent trace spans for debugging. |
| `a2a_export_traces` | experimental | read | Export all buffered spans in OpenTelemetry-compatible OTLP JSON format. |
| `a2a_agent_dashboard` | experimental | read | Get a full operational dashboard for an agent: runtime status, budget, recent budget alerts, tick metrics, and rail-aware economics. |
| `a2a_agent_decisions` | experimental | read | Get recent strategy decisions for an agent: what was accepted/rejected and why. |
| `a2a_agent_performance` | experimental | read | Get performance report with optional rail-aware economics context: quote accept rate, response time, settlement success rate, dispute rate, filtered payment metrics, and recent budget alert activity. |
| `a2a_agent_tick_metrics` | experimental | read | Get tick loop metrics: avg duration, ticks/min, quotes evaluated, payments executed, errors. |
| `a2a_agent_lifecycle` | experimental | read | Get agent lifecycle history: start/stop/pause/resume events with timestamps and reasons. |
| `a2a_agent_alerts` | experimental | read | List recent budget and settlement alerts for an agent, with optional category, time window, and payment-rail filters. |
| `a2a_settlement_status` | experimental | read | Get settlement finality status: broadcast → unconfirmed → confirming → final. Shows confirmation count vs chain requirement. |
| `a2a_settlement_pending` | experimental | read | List all settlements not yet final — awaiting blockchain confirmations. |
| `a2a_settlement_finality_metrics` | experimental | read | Get settlement metrics: avg confirmation time, finality rate, reorg count. |
| `a2a_handshake` | experimental | read | Initiate capability handshake with another agent. Returns compatibility report: shared networks/assets, feature mismatches, recommended network/asset. |
| `a2a_my_capabilities` | experimental | read | Get this agent's capability manifest for protocol handshake. |

## a2a-platform

Tier: **experimental** — background services not attached on a fresh store (smoke backlog)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `a2a_send_message` | experimental | write | Send a direct message to another agent. Supports text, task delegation, and status queries. |
| `a2a_get_inbox` | experimental | read | Get your message inbox. Filter by unread, type, or limit. |
| `a2a_delegate_task` | experimental | write | Delegate a task to another agent. Specify description, deadline, reward, and priority. |
| `a2a_respond_to_task` | experimental | write | Respond to a delegated task: accept, reject, or mark complete. |
| `a2a_get_thread` | experimental | read | Get all messages in a conversation thread. |
| `a2a_messaging_metrics` | experimental | read | Get messaging metrics: total messages, unread count, avg response time. |
| `a2a_batch_pay` | experimental | write | Execute multiple payments in one call. Each payment is independent — one failure doesn't block others. |
| `a2a_batch_request_quotes` | experimental | write | Request quotes from multiple sellers simultaneously. |
| `a2a_save_checkpoint` | experimental | write | Save agent state checkpoint for recovery after restart. |
| `a2a_load_checkpoint` | experimental | read | Load last saved agent state checkpoint. |
| `a2a_list_checkpoints` | experimental | read | List all saved agent checkpoints. |
| `a2a_export_agent_data` | experimental | read | Export all commerce data for an agent: payments, quotes, escrows, disputes, subscriptions. |
| `a2a_commerce_report` | experimental | read | Generate a commerce report for an agent: per-rail volume, transactions, dispute rate, top counterparties, and margin. |
| `a2a_data_stats` | experimental | read | Get row counts for all A2A data tables. |
| `a2a_verify_webhook` | experimental | read | Verify a received webhook signature. Use this to validate incoming StateSet webhooks. |
| `a2a_tick_metrics` | experimental | read | Get tick loop performance metrics: p50/p95/p99 duration, ticks/min, idle streaks, adaptive interval. |

## a2a-intelligence

Tier: **experimental** — background services not attached on a fresh store (smoke backlog)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `a2a_schedule_action` | experimental | write | Schedule a future action: "pay in 3 days", "check escrow every hour", "remind me to follow up". |
| `a2a_cancel_scheduled` | experimental | write | Cancel a scheduled action by ID. |
| `a2a_list_scheduled` | experimental | read | List scheduled actions. Filter by status or action type. |
| `a2a_scheduler_metrics` | experimental | read | Get scheduler metrics: total scheduled, executed, failed, pending, recurring. |
| `a2a_remember_interaction` | experimental | write | Record an interaction with a counterparty so the agent learns their patterns over time. |
| `a2a_counterparty_profile` | experimental | read | Get learned profile of a counterparty: success rate, reliability, risk level, negotiation patterns. |
| `a2a_should_transact` | experimental | read | Get AI recommendation on whether to transact with a counterparty, based on learned history. |
| `a2a_agent_insights` | experimental | read | Get aggregate insights: total counterparties, avg success rate, top performers, risk alerts. |
| `a2a_top_counterparties` | experimental | read | Get top counterparties ranked by volume, success rate, or reliability. |
| `a2a_add_rule` | experimental | write | Add a programmable guardrail rule. Example: "block transactions > $1000 without escrow". |
| `a2a_evaluate_rules` | experimental | read | Evaluate all active rules against a transaction context. Returns: allowed, matched rules, explanation. |
| `a2a_list_rules` | experimental | read | List all registered rules. Filter by tags or enabled status. |
| `a2a_rule_audit_log` | experimental | read | Get recent rule evaluation audit log — see which rules fired and why. |
| `a2a_scatter` | experimental | write | Broadcast a task to multiple agents in parallel (fan-out). Returns coordination ID for tracking. |
| `a2a_coordination_status` | experimental | read | Get status of a fan-out coordination: responses received, pending, timed out. |
| `a2a_submit_response` | experimental | write | Submit a response to a fan-out coordination (as a target agent). |
| `a2a_join_results` | experimental | read | Wait for and aggregate fan-out results based on the join strategy. |

## quality

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_inspections` | extended | read | List quality inspections, optionally filtered by type, status, reference, inspector or date range. |
| `get_inspection` | extended | read | Get a quality inspection by ID. |
| `create_inspection` | extended | write | Create a quality inspection. |
| `start_inspection` | extended | write | Start a quality inspection. |
| `complete_inspection` | extended | write | Complete a quality inspection. |
| `list_ncrs` | extended | read | List non-conformance reports, optionally filtered by source, severity, status, SKU, lot, assignee or date range. |
| `get_ncr` | extended | read | Get a non-conformance report by ID. |
| `create_ncr` | extended | write | Create a non-conformance report. |
| `close_ncr` | extended | write | Close a non-conformance report. Closing requires a disposition (what was done with the non-conforming material): pass `disposition` (and optionally `dispositionQuantity`) to record it and close in one step. An NCR with no disposition recorded is refused. |
| `list_quality_holds` | extended | read | List quality holds. |
| `get_quality_hold` | extended | read | Get a quality hold by ID. |
| `create_quality_hold` | extended | write | Create a quality hold. |
| `release_quality_hold` | extended | write | Release a quality hold. |
| `list_active_quality_holds` | extended | read | List active quality holds. |
| `count_active_quality_holds` | extended | read | Count active quality holds. |

## lots

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_lots` | extended | read | List lots. |
| `get_lot` | extended | read | Get a lot by ID or lot number. |
| `create_lot` | extended | write | Create a lot. |
| `list_active_lots` | extended | read | List active lots for a SKU. |
| `list_available_lots_for_sku` | extended | read | List available lots for a SKU in FIFO order. |
| `quarantine_lot` | extended | write | Quarantine a lot. |
| `release_lot_quarantine` | extended | write | Release a lot from quarantine. |
| `list_expiring_lots` | extended | read | List lots expiring within a number of days. |
| `list_expired_lots` | extended | read | List expired lots. |
| `list_quarantined_lots` | extended | read | List quarantined lots. |
| `count_lots` | extended | read | Count lots. |

## search-config

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_search_configs` | extended | read | List search configurations. |
| `get_search_config` | extended | read | Get a search configuration by ID. |
| `get_active_search_config` | extended | read | Get the currently active search configuration. |
| `create_search_config` | extended | write | Create a search configuration. |
| `update_search_config` | extended | write | Update a search configuration. Collection fields replace the existing values. |
| `set_active_search_config` | extended | write | Make a search configuration active, deactivating the current one. |
| `delete_search_config` | extended | write | Delete a search configuration. |

## serials

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_serials` | extended | read | List serial numbers. |
| `get_serial` | extended | read | Get a serial by ID or serial string. |
| `create_serial` | extended | write | Create a serial number. |
| `list_available_serials` | extended | read | List available serials for a SKU. |
| `mark_serial_sold` | extended | write | Mark a serial number as sold. |
| `quarantine_serial` | extended | write | Quarantine a serial number. |
| `check_serial_availability` | extended | read | Check whether a serial string is available. |
| `count_serials` | extended | read | Count serial numbers. |

## warehouse

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_warehouses` | extended | read | List warehouses. |
| `get_warehouse` | extended | read | Get a warehouse by ID or code. |
| `create_warehouse` | extended | write | Create a warehouse. |
| `create_location` | extended | write | Create a warehouse location. |
| `get_location` | extended | read | Get a warehouse location by ID. |
| `list_locations` | extended | read | List warehouse locations. |
| `list_pickable_locations` | extended | read | List pickable locations for a SKU in a warehouse. |
| `get_warehouse_sku_available_quantity` | extended | read | Get total available quantity for a SKU in a warehouse. |
| `count_warehouses` | extended | read | Count warehouses. |

## receiving

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_receipts` | extended | read | List receipts. |
| `get_receipt` | extended | read | Get a receipt by ID or receipt number. |
| `create_receipt` | extended | write | Create a receipt. |
| `create_receipt_from_purchase_order` | extended | write | Create a receipt from a purchase order. |
| `start_receiving` | extended | write | Start receiving against a receipt. |
| `complete_receiving` | extended | write | Complete receiving against a receipt. |
| `cancel_receipt` | extended | write | Cancel a receipt. |
| `count_receipts` | extended | read | Count receipts. |

## fulfillment

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_fulfillment_waves` | extended | read | List fulfillment waves. |
| `get_fulfillment_wave` | extended | read | Get a fulfillment wave by ID. |
| `create_fulfillment_wave` | extended | write | Create a fulfillment wave. |
| `release_fulfillment_wave` | extended | write | Release a fulfillment wave for picking. |
| `complete_fulfillment_wave` | extended | write | Complete a fulfillment wave. |
| `cancel_fulfillment_wave` | extended | write | Cancel a fulfillment wave. |
| `list_pick_tasks` | extended | read | List pick tasks. |
| `get_pick_task` | extended | read | Get a pick task by ID. |
| `assign_pick_task` | extended | write | Assign a pick task. |
| `start_pick_task` | extended | write | Start a pick task. |
| `cancel_pick_task` | extended | write | Cancel a pick task. |
| `check_order_ready_to_pack` | extended | read | Check whether an order is ready to pack. |
| `check_order_ready_to_ship` | extended | read | Check whether an order is ready to ship. |
| `count_fulfillment_waves` | extended | read | Count fulfillment waves. |

## accounts-payable

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_bills` | extended | read | List accounts payable bills. |
| `get_bill` | extended | read | Get a bill by ID or bill number. |
| `create_bill` | extended | write | Create an accounts payable bill. |
| `approve_bill` | extended | write | Approve a bill. |
| `cancel_bill` | extended | write | Cancel a bill. |
| `list_overdue_bills` | extended | read | List overdue bills. |
| `list_bills_due_soon` | extended | read | List bills due soon. |
| `get_accounts_payable_aging_summary` | extended | read | Get the accounts payable aging summary. |
| `get_accounts_payable_total_outstanding` | extended | read | Get the total accounts payable outstanding balance. |
| `three_way_match_bill` | extended | read | Run a three-way match (bill vs purchase order vs receipt) for a bill. |
| `count_accounts_payable_bills` | extended | read | Count accounts payable bills. |

## accounts-receivable

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `get_accounts_receivable_aging_summary` | extended | read | Get the accounts receivable aging summary. |
| `get_accounts_receivable_total_outstanding` | extended | read | Get the total accounts receivable outstanding balance. |
| `get_days_sales_outstanding` | extended | read | Get days sales outstanding over a rolling window. |
| `list_credit_memos` | extended | read | List credit memos. |
| `get_credit_memo` | extended | read | Get a credit memo by ID. |
| `create_credit_memo` | extended | write | Create a credit memo. |
| `void_credit_memo` | extended | write | Void a credit memo. |
| `list_unapplied_credits` | extended | read | List unapplied credits for a customer. |

## cost-accounting

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_item_costs` | extended | read | List item costs. |
| `get_item_cost` | extended | read | Get item cost for a SKU. |
| `set_item_cost` | extended | write | Set item cost inputs for a SKU. |
| `update_average_item_cost` | extended | write | Update average cost for a SKU from a quantity and unit cost. |
| `get_total_inventory_value` | extended | read | Get total inventory value. |

## credit

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_credit_accounts` | extended | read | List credit accounts. |
| `get_credit_account` | extended | read | Get a credit account by account ID or customer ID. |
| `create_credit_account` | extended | write | Create a customer credit account. |
| `check_customer_credit` | extended | read | Check customer credit availability for an order amount. |
| `adjust_credit_limit` | extended | write | Adjust a customer credit limit. |
| `suspend_credit_account` | extended | write | Suspend a customer credit account. |
| `reactivate_credit_account` | extended | write | Reactivate a customer credit account. |
| `list_over_limit_credit_accounts` | extended | read | List over-limit credit accounts. |

## backorders

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_backorders` | extended | read | List backorders. |
| `get_backorder` | extended | read | Get a backorder by ID or backorder number. |
| `create_backorder` | extended | write | Create a backorder. |
| `cancel_backorder` | extended | write | Cancel a backorder. |
| `list_backorders_for_order` | extended | read | List backorders for an order. |
| `list_backorders_for_sku` | extended | read | List backorders for a SKU. |
| `list_overdue_backorders` | extended | read | List overdue backorders. |
| `get_backorder_summary` | extended | read | Get the backorder summary. |
| `auto_allocate_inventory` | extended | write | Allocate available stock to a SKU's open backorders, in priority order (critical first, then oldest first). Call after stock arrives. |
| `allocate_backorder` | extended | write | Reserve a specific quantity of stock against one backorder. |
| `get_backorder_allocations` | extended | read | List the allocations recorded against one backorder. |
| `confirm_backorder_allocation` | extended | write | Confirm a reserved allocation, committing the stock to the backorder. |
| `release_backorder_allocation` | extended | write | Release a reserved allocation, returning the stock to available. |
| `expire_backorder_allocations` | extended | write | Expire every allocation whose hold has lapsed, freeing the stock it held. Returns how many were swept. |
| `fulfill_backorder` | extended | write | Record a fulfilment against a backorder, drawing on the named source. |
| `get_backorder_fulfillment_history` | extended | read | The fulfilment history recorded against one backorder. |
| `list_backorders_for_customer` | extended | read | Every backorder raised for one customer. |
| `get_sku_backorder_summary` | extended | read | Open backorder totals for one SKU. |
| `update_backorder` | extended | write | Update a backorder's priority, dates, source location or notes. |
| `count_pending_backorders` | extended | read | Count pending backorders. |

## general-ledger

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_gl_accounts` | extended | read | List general ledger accounts. |
| `get_gl_account` | extended | read | Get a general ledger account by ID or account number. |
| `create_gl_account` | extended | write | Create a general ledger account. |
| `initialize_chart_of_accounts` | extended | write | Initialize the standard chart of accounts. |
| `list_journal_entries` | extended | read | List journal entries. |
| `get_journal_entry` | extended | read | Get a journal entry by ID. |
| `post_journal_entry` | extended | write | Post a journal entry. |
| `void_journal_entry` | extended | write | Void a journal entry. |
| `get_trial_balance` | extended | read | Get the trial balance as of a date. |
| `get_balance_sheet` | extended | read | Get the balance sheet as of a date. |
| `get_income_statement` | extended | read | Get the income statement for a date range. |
| `revalue_gl` | extended | write | Revalue foreign-currency general ledger balances as of a date. |
| `close_month` | extended | write | Close the month: post scheduled depreciation, recognize revenue through period end, revalue foreign-currency balances, then run the period close. Use dryRun to preview per-step counts and amounts without writing. |
| `create_gl_period` | extended | write | Create an accounting period. |
| `list_gl_periods` | extended | read | List accounting periods with optional filtering. |
| `open_gl_period` | extended | write | Open an accounting period so journal entries can be posted to it. |
| `get_gl_account_balance` | extended | read | Get the balance of a general ledger account. |

## agent-receipt

Tier: **experimental** — shells out to the ves-demo stack (AGENT_RECEIPT_DEMO_DIR)

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `agent_receipt_purchase` | experimental | write | Execute a verifiable agent-to-agent purchase end-to-end: buyer agent locks ssUSD in OrderEscrow, sequencer commits VES events, STARK proof attests order_total ≤ policy cap, SetRegistry anchors the commitment + proof on Set Chain L2, buyer marks delivered, seller releases. Returns the signed Agent Receipt JSON with on-chain tx hashes. Requires the local stack (anvil + sequencer + postgres + deployed contracts) to be running, configured by AGENT_RECEIPT_DEMO_DIR. |
| `agent_receipt_status` | experimental | read | Read the on-chain escrow state for an order. Returns buyer, seller, amount, deadlines, delivery receipt hash, and current status (None / Locked / Delivered / Disputed / Released / Refunded). |
| `agent_receipt_dispute` | experimental | write | Buyer raises an on-chain dispute on a Delivered order. Funds freeze in escrow until the operator resolves. The plain-text reason is hashed (keccak256) and stored on-chain as proof of the filing. |
| `agent_receipt_resolve` | experimental | admin | Operator (sequencer / arbiter) resolves a Disputed order. Routes the locked funds either to the seller (in_favor_of_seller=true) or refunds the buyer (false). Emits DisputeResolved + Released/Refunded. |
| `agent_receipt_fx_quote` | experimental | read | Read a fresh FX quote from the on-chain FxOracle and convert an amount between currencies. Pair format: "BASE/QUOTE", e.g. "EUR/ssUSD" or "JPY/ssUSD". Returns the rate, freshness, and the converted amount. Use this BEFORE locking funds so the agent can verify the rate is fresh and within expected bounds. Pre-seeded pairs at deploy time: EUR/ssUSD, GBP/ssUSD, JPY/ssUSD, MXN/ssUSD. |
| `agent_receipt_merchant_statement` | experimental | read | Aggregate every emitted receipt in a directory into a single platform settlement statement: GMV, marketplace fees earned, FX exposure by currency, dispute outcomes, compliance bundle counts, and a sampled on-chain audit pass rate. Optional filters scope the statement to a date range, a specific seller wallet, or a specific buyer wallet — enabling multi-tenant accounting on a single OrderEscrow contract. |
| `agent_receipt_request_payout` | experimental | write | Initiate a fiat payout from the seller's SSDC balance to their bank via the off-ramp bridge. Auto-handles SSDC.approve idempotently, signs a canonical payout-request message with the seller's wallet key, POSTs the signed request to the bridge, and returns a Stripe-Treasury-shaped OutboundPayment intent. Requires bridge running on http://localhost:4243 (or BRIDGE_PAYOUT_URL env). |
| `agent_receipt_audit` | experimental | read | Independently audit a StateSet commerce receipt against the live chain. Re-verifies on-chain claims (escrow status, registry batch commitment, STARK proof metadata) and — for compliance bundles — runs the Winterfell verifier on every policy proof. Returns a structured pass/fail summary the calling agent can act on. The strongest audit primitive in the stack: any agent can verify any receipt without trusting the producer. |
| `agent_receipt_sweep_yield` | experimental | admin | Operator/marketplace sweeps the rebasing yield surplus held by OrderEscrow to a recipient. With the production SSDC stablecoin, this is the T-Bill yield earned by escrowed funds while orders were in flight — a programmable platform revenue stream alongside any BPS fee. Read first via yield_available; positive amount returns the sweep tx, otherwise a no-op. |
| `agent_receipt_refund` | experimental | write | Buyer recovers locked funds after the order's deliveryDeadline has expired. No dispute, no operator, no platform — purely the safety property of the OrderEscrow primitive. Reverts with DeadlineNotReached if the deadline has not yet passed. |
| `agent_receipt_release` | experimental | write | Seller pulls escrowed funds after delivery + confirmation window. Use this when agent_receipt_purchase was called with skip_release=true and there has been no dispute. Routes funds to the seller wallet. |

## fixed-assets

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_fixed_assets` | extended | read | List fixed assets. |
| `get_fixed_asset` | extended | read | Get a fixed asset by ID. |
| `create_fixed_asset` | extended | write | Create a fixed asset. |
| `place_asset_in_service` | extended | write | Place a fixed asset in service. |
| `dispose_fixed_asset` | extended | write | Dispose of a fixed asset. |
| `write_off_fixed_asset` | extended | write | Write off a fixed asset. |
| `generate_depreciation_schedule` | extended | write | Generate the depreciation schedule for a fixed asset. |
| `get_depreciation_schedule` | extended | read | Get the depreciation schedule for a fixed asset. |
| `post_depreciation` | extended | write | Post the next scheduled depreciation entries for a fixed asset (generate the schedule first). |

## maintenance

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `backup_database` | extended | write | Create a consistent, checksum-verified backup of the database plus a sidecar manifest. Safe to run while the store is being written to. |
| `restore_database` | extended | write | Restore a backup to a target path. Verifies the manifest checksum, refuses backups from a newer engine, and refuses to overwrite an existing database unless overwrite is set. |
| `export_full_data` | extended | read | Export the full store to a versioned JSON file (distinct from export_data, which dumps a single entity type for parity testing). Covers the core commerce and finance domains; see the maintenance module docs for exactly what is and is not included. |
| `import_full_data` | extended | write | Import a JSON export into this store. Records are replayed through the normal create paths, so IDs are re-minted and foreign keys remapped. |
| `list_portable_domains` | extended | read | List the domains that data export and import can cover. |

## revenue-recognition

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_revenue_contracts` | extended | read | List revenue recognition contracts. |
| `get_revenue_contract` | extended | read | Get a revenue recognition contract by ID. |
| `create_revenue_contract` | extended | write | Create a revenue recognition (ASC 606) contract with its performance obligations. The obligations' allocated amounts must sum to the transaction price. |
| `generate_revenue_schedule` | extended | write | Generate the revenue recognition schedule for a performance obligation (ids are on the contract's obligations). |
| `get_revenue_schedule` | extended | read | Get the revenue recognition schedule for a performance obligation. |
| `recognize_revenue` | extended | write | Recognize deferred revenue for a performance obligation: every scheduled entry whose period starts on or before `through`. |

## cycle-counts

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_cycle_counts` | extended | read | List cycle counts. |
| `get_cycle_count` | extended | read | Get a cycle count by ID. |
| `create_cycle_count` | extended | write | Create a draft cycle count for a warehouse with the SKUs to count and the quantity the system expects for each. |
| `start_cycle_count` | extended | write | Start a cycle count. |
| `record_cycle_counts` | extended | write | Record counted quantities for a cycle count. |
| `complete_cycle_count` | extended | write | Complete a cycle count. |
| `cancel_cycle_count` | extended | write | Cancel a cycle count. |

## edi-documents

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `list_edi_documents` | extended | read | List EDI documents with optional filtering. |
| `get_edi_document` | extended | read | Get an EDI document by ID. |
| `create_edi_document` | extended | write | Create / ingest an EDI document. |
| `set_edi_document_status` | extended | write | Update the status of an EDI document. |
| `get_edi_summary` | extended | read | Get an aggregate summary of EDI documents (counts by status and type). |

## prepayments

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_prepayments_supported` | extended | read | Check whether the prepayments backend is available on this engine build. |
| `list_prepayments` | extended | read | List supplier prepayments with optional filtering. |
| `get_prepayment` | extended | read | Get a prepayment by ID. |
| `create_prepayment` | extended | write | Create a supplier prepayment. |
| `apply_prepayment` | extended | write | Apply a prepayment against a bill or payment obligation. |
| `list_prepayment_applications` | extended | read | List applications for a prepayment. |
| `reverse_prepayment_application` | extended | write | Reverse a previously-recorded prepayment application. |
| `refund_prepayment` | extended | write | Refund the remaining balance of a prepayment, closing it. |

## activity-logs

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_activity_logs_supported` | extended | read | Check whether the activity-logs backend is available on this engine build. |
| `list_activity_logs` | extended | read | List activity log entries with optional filtering. |
| `get_activity_log` | extended | read | Get an activity log entry by ID. |
| `get_activity_history_for_subject` | extended | read | Get the activity history for a subject (e.g. an order or product). |
| `record_activity` | extended | write | Record an activity log entry for a subject. |

## channels

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_channels_supported` | extended | read | Check whether the channels backend is available on this engine build. |
| `list_channels` | extended | read | List sales channels with optional filtering. |
| `get_channel` | extended | read | Get a sales channel by ID. |
| `create_channel` | extended | write | Create a sales channel. |
| `update_channel` | extended | write | Update a sales channel. |
| `set_channel_lock` | extended | write | Lock or unlock a sales channel for API writes. |
| `list_channel_product_mappings` | extended | read | List product mappings for a sales channel. |
| `delete_channel` | extended | write | Delete a sales channel. |

## companies

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_companies_supported` | extended | read | Check whether the companies backend is available on this engine build. |
| `list_companies` | extended | read | List B2B companies with optional filtering. |
| `get_company` | extended | read | Get a company by ID. |
| `create_company` | extended | write | Create a B2B company. |
| `update_company` | extended | write | Update a B2B company. |
| `list_company_addresses` | extended | read | List shipping addresses for a company. |
| `list_company_contacts` | extended | read | List contacts for a company. |
| `create_company_contact` | extended | write | Create a contact linked to one or more companies. |
| `delete_company` | extended | write | Delete a B2B company. |

## vendor-credits

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_vendor_credits_supported` | extended | read | Check whether the vendor-credits backend is available on this engine build. |
| `list_vendor_credits` | extended | read | List vendor credits with optional filtering. |
| `get_vendor_credit` | extended | read | Get a vendor credit by ID. |
| `create_vendor_credit` | extended | write | Create a vendor credit. |
| `apply_vendor_credit` | extended | write | Apply a vendor credit against a bill or payment obligation. |
| `list_vendor_credit_applications` | extended | read | List applications for a vendor credit. |
| `reverse_vendor_credit_application` | extended | write | Reverse a previously-recorded vendor credit application. |
| `cancel_vendor_credit` | extended | write | Cancel a vendor credit. |

## price-schedules

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_price_schedules_supported` | extended | read | Check whether the price-schedules backend is available on this engine build. |
| `list_price_schedules` | extended | read | List price schedules with optional filtering. |
| `get_price_schedule` | extended | read | Get a price schedule by ID. |
| `create_price_schedule` | extended | write | Create a price schedule. |
| `update_price_schedule` | extended | write | Update a price schedule. |
| `delete_price_schedule` | extended | write | Delete a price schedule and its entries. |
| `set_price_schedule_entry` | extended | write | Upsert a per-product scheduled price on a price schedule. |
| `delete_price_schedule_entry` | extended | write | Remove a per-product entry from a price schedule. |
| `list_price_schedule_entries` | extended | read | List per-product entries for a price schedule. |
| `resolve_scheduled_price` | extended | read | Resolve the effective scheduled price for a product at an instant (defaults to now). |

## price-levels

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_price_levels_supported` | extended | read | Check whether the price-levels backend is available on this engine build. |
| `list_price_levels` | extended | read | List price levels with optional filtering. |
| `get_price_level` | extended | read | Get a price level by ID. |
| `create_price_level` | extended | write | Create a price level. |
| `update_price_level` | extended | write | Update a price level. |
| `delete_price_level` | extended | write | Delete a price level and its entries. |
| `set_price_level_entry` | extended | write | Upsert a per-product fixed price entry on a price level. |
| `delete_price_level_entry` | extended | write | Remove a per-product entry from a price level. |
| `list_price_level_entries` | extended | read | List per-product entries for a price level. |

## transfer-orders

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_transfer_orders_supported` | extended | read | Check whether the transfer-orders backend is available on this engine build. |
| `list_transfer_orders` | extended | read | List transfer orders with optional filtering. |
| `get_transfer_order` | extended | read | Get a transfer order by ID. |
| `create_transfer_order` | extended | write | Create a transfer order between warehouses. |
| `ship_transfer_order` | extended | write | Mark a transfer order as shipped from the source warehouse. |
| `receive_transfer_order_line` | extended | write | Receive a quantity against a transfer order line at the destination. |
| `cancel_transfer_order` | extended | write | Cancel a transfer order. |

## production-batches

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_production_batches_supported` | extended | read | Check whether the production-batches backend is available on this engine build. |
| `list_production_batches` | extended | read | List production batches with optional filtering. |
| `get_production_batch` | extended | read | Get a production batch by ID. |
| `create_production_batch` | extended | write | Create a production batch. |
| `update_production_batch` | extended | write | Update a production batch. |
| `delete_production_batch` | extended | write | Delete a production batch. |
| `add_production_batch_work_orders` | extended | write | Link work orders to a production batch. |
| `remove_production_batch_work_order` | extended | write | Remove a work order from a production batch. |

## supplier-skus

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_supplier_skus_supported` | extended | read | Check whether the supplier-SKUs backend is available on this engine build. |
| `list_supplier_skus` | extended | read | List supplier SKUs with optional filtering. |
| `get_supplier_sku` | extended | read | Get a supplier SKU by ID. |
| `create_supplier_sku` | extended | write | Create a supplier SKU cross-reference. |
| `update_supplier_sku` | extended | write | Update a supplier SKU. |
| `delete_supplier_sku` | extended | write | Delete a supplier SKU. |
| `bulk_upsert_supplier_skus` | extended | write | Bulk upsert supplier SKUs for a supplier, keyed by internal product. |

## inbound-shipments

Tier: **extended**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `check_inbound_shipments_supported` | extended | read | Check whether the inbound-shipments backend is available on this engine build. |
| `list_inbound_shipments` | extended | read | List inbound shipments with optional filtering. |
| `get_inbound_shipment` | extended | read | Get an inbound shipment by ID. |
| `create_inbound_shipment` | extended | write | Create an inbound shipment. |
| `mark_inbound_shipment_in_transit` | extended | write | Mark an inbound shipment as in transit. |
| `mark_inbound_shipment_arrived` | extended | write | Mark an inbound shipment as arrived. |
| `receive_inbound_shipment_line` | extended | write | Receive a quantity against an inbound shipment line. |
| `cancel_inbound_shipment` | extended | write | Cancel an inbound shipment. |

## explain

Tier: **core**

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `explain_order` | core | read | Explain an order to a customer or merchant: one chronological timeline (checkout, payments, shipments, returns, refunds, fraud, activity), the money charged/refunded/net as exact strings, tax recomputed from the address, and flags for inconsistencies (e.g. paid but paymentStatus pending, refunded more than charged). Read-only; lists what it cannot see. |
| `explain_cart_pricing` | core | read | Explain why a cart costs what it costs: lines, subtotal, promotions applied and REFUSED with reason codes, tax by jurisdiction, shipping, and a check that the explained total equals the stored grand total. Pass couponCodes to ask why a code does or does not apply. Read-only: nothing is written to the cart. |

## agentic-runtime

Server-level tools that belong to no domain (planning, simulation, replay, events, discovery).

| Tool | Tier | Permission | Description |
| --- | --- | --- | --- |
| `agentic_runtime_contract` | core | read | Return a deterministic runtime contract for AI agents: capabilities, side effects, and replay metadata. |
| `agentic_tool_catalog` | core | read | Return a machine-readable tool catalog with runtime metadata and optional Machine Payments Protocol pricing info. |
| `agentic_payment_discovery` | extended | read | Discover payable MCP tools with Machine Payments Protocol metadata, pricing, and optional OpenAPI output. |
| `agentic_prepare_payment` | extended | read | Prepare a Machine Payments Protocol challenge and retry template for a priced MCP tool call. |
| `agentic_plan` | core | read | Evaluate a proposed tool sequence for deterministic simulation: policy, permission, and replayability checks. |
| `agentic_simulate_mutation` | core | read | Run deterministic dry-run simulation for a mutating tool with policy, permission, rollback, and replay metadata. |
| `agentic_replay_mutation` | core | read | Replay a previously logged mutating tool call from the deterministic replay log, with dry-run by default. |
| `agentic_replay` | core | read | Read recent deterministic execution events for auditability and replay tooling. |
| `agentic_subscribe_events` | core | read | Subscribe to MCP execution events for a session or global stream. |
| `agentic_unsubscribe_events` | core | read | Unsubscribe from a previously-created MCP event subscription. |
| `agentic_list_event_subscriptions` | core | read | List active MCP event subscriptions. |
| `agentic_get_event_history` | core | read | Fetch recent MCP event history for debugging and replay. |
| `agentic_execute_plan` | core | write | Execute a tool sequence deterministically with optional dry-run and best-effort rollback. |
| `discover_tools` | core | read | Discover relevant MCP tools by intent description. Returns the top matching tools for a given natural language query. |
| `delegate_to_agent` | experimental— needs the autonomous engine, which stateset-mcp does not start | write | Delegate a sub-task to a specialized commerce agent. Available agents: customer-service, checkout, orders, inventory, returns, analytics, promotions, subscriptions, storefront, sync, manufacturing, payments, stablecoin, shipments, suppliers, invoices, warranties, currency, agents, tax. |
