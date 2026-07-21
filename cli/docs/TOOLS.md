# StateSet CLI Tool Catalog

<!-- GENERATED FILE — do not edit by hand. -->
<!-- Regenerate with: npm run docs:tools (from cli/) -->

Source of truth: `cli/src/tools/domain-registry.js`.

**893 tools** across **86 domains**.

## Domains

| Domain | Tools |
| --- | ---: |
| [customers](#customers) | 3 |
| [orders](#orders) | 6 |
| [products](#products) | 4 |
| [inventory](#inventory) | 6 |
| [custom-objects](#custom-objects) | 12 |
| [returns](#returns) | 5 |
| [carts](#carts) | 30 |
| [analytics](#analytics) | 14 |
| [currency](#currency) | 12 |
| [tax](#tax) | 29 |
| [promotions](#promotions) | 15 |
| [subscriptions](#subscriptions) | 17 |
| [sync](#sync) | 20 |
| [manufacturing](#manufacturing) | 11 |
| [payments](#payments) | 19 |
| [stablecoin](#stablecoin) | 4 |
| [treasury](#treasury) | 6 |
| [erc8004](#erc8004) | 5 |
| [x402](#x402) | 14 |
| [agent-cards](#agent-cards) | 5 |
| [a2a](#a2a) | 59 |
| [agent-runtime](#agent-runtime) | 29 |
| [shipments](#shipments) | 14 |
| [suppliers](#suppliers) | 10 |
| [invoices](#invoices) | 7 |
| [warranties](#warranties) | 7 |
| [import](#import) | 10 |
| [policies](#policies) | 5 |
| [vector](#vector) | 16 |
| [gift-cards](#gift-cards) | 7 |
| [store-credits](#store-credits) | 5 |
| [segments](#segments) | 6 |
| [shipping-zones](#shipping-zones) | 7 |
| [units-of-measure](#units-of-measure) | 10 |
| [stock-snapshots](#stock-snapshots) | 5 |
| [print-stations](#print-stations) | 8 |
| [integration-mappings](#integration-mappings) | 7 |
| [integration-field-mappings](#integration-field-mappings) | 8 |
| [payment-obligations](#payment-obligations) | 7 |
| [purgatory](#purgatory) | 6 |
| [topology-snapshots](#topology-snapshots) | 5 |
| [vendor-returns](#vendor-returns) | 6 |
| [reviews](#reviews) | 7 |
| [wishlists](#wishlists) | 6 |
| [loyalty](#loyalty) | 8 |
| [fraud](#fraud) | 6 |
| [connectors](#connectors) | 11 |
| [audit](#audit) | 4 |
| [proofs](#proofs) | 7 |
| [circuit-breaker](#circuit-breaker) | 8 |
| [checkout](#checkout) | 8 |
| [compliance](#compliance) | 6 |
| [catalog](#catalog) | 6 |
| [a2a-automation](#a2a-automation) | 32 |
| [a2a-observability](#a2a-observability) | 15 |
| [a2a-platform](#a2a-platform) | 16 |
| [a2a-intelligence](#a2a-intelligence) | 17 |
| [quality](#quality) | 15 |
| [lots](#lots) | 11 |
| [search-config](#search-config) | 7 |
| [serials](#serials) | 8 |
| [warehouse](#warehouse) | 9 |
| [receiving](#receiving) | 8 |
| [fulfillment](#fulfillment) | 14 |
| [accounts-payable](#accounts-payable) | 11 |
| [accounts-receivable](#accounts-receivable) | 8 |
| [cost-accounting](#cost-accounting) | 5 |
| [credit](#credit) | 8 |
| [backorders](#backorders) | 9 |
| [general-ledger](#general-ledger) | 17 |
| [agent-receipt](#agent-receipt) | 11 |
| [fixed-assets](#fixed-assets) | 9 |
| [revenue-recognition](#revenue-recognition) | 6 |
| [cycle-counts](#cycle-counts) | 7 |
| [edi-documents](#edi-documents) | 5 |
| [prepayments](#prepayments) | 8 |
| [activity-logs](#activity-logs) | 5 |
| [channels](#channels) | 8 |
| [companies](#companies) | 9 |
| [vendor-credits](#vendor-credits) | 8 |
| [price-schedules](#price-schedules) | 10 |
| [price-levels](#price-levels) | 9 |
| [transfer-orders](#transfer-orders) | 7 |
| [production-batches](#production-batches) | 8 |
| [supplier-skus](#supplier-skus) | 7 |
| [inbound-shipments](#inbound-shipments) | 8 |

## customers

| Tool | Permission | Description |
| --- | --- | --- |
| `list_customers` | read | List all customers in the database. Returns customer details including email, name, and status. |
| `get_customer` | read | Get a specific customer by ID or email address. |
| `create_customer` | write | Create a new customer. Requires email, first name, and last name. |

## orders

| Tool | Permission | Description |
| --- | --- | --- |
| `list_orders` | read | List all orders. Shows order number, status, customer, total amount, and item count. |
| `get_order` | read | Get a specific order by ID or order number. Returns full order details including line items. |
| `create_order` | write | Create a new order for a customer with line items. |
| `update_order_status` | write | Update the status of an order. Valid statuses: pending, confirmed, processing, shipped, delivered, cancelled, refunded. |
| `ship_order` | write | Mark an order as shipped with optional tracking number. |
| `cancel_order` | delete | Cancel an order. Only pending or confirmed orders can be cancelled. |

## products

| Tool | Permission | Description |
| --- | --- | --- |
| `list_products` | read | List all products in the catalog. |
| `get_product` | read | Get a specific product by ID. |
| `get_product_variant` | read | Get a product variant by SKU. |
| `create_product` | write | Create a new product with optional variants. |

## inventory

| Tool | Permission | Description |
| --- | --- | --- |
| `get_stock` | read | Get current stock level for a SKU. Shows on-hand, allocated, and available quantities. |
| `create_inventory_item` | write | Create a new inventory item for a SKU. |
| `adjust_inventory` | write | Adjust inventory quantity for a SKU. Use positive numbers to add stock, negative to remove. |
| `reserve_inventory` | write | Reserve inventory for an order. Reserved stock is allocated but not yet deducted. |
| `confirm_reservation` | write | Confirm an inventory reservation, deducting the reserved quantity from stock. |
| `release_reservation` | write | Release an inventory reservation, returning the reserved quantity to available stock. |

## custom-objects

| Tool | Permission | Description |
| --- | --- | --- |
| `list_custom_object_types` | read | List custom object types (schemas). Custom objects are similar to Shopify metaobjects / Salesforce custom objects. |
| `get_custom_object_type` | read | Get a custom object type (schema) by ID. |
| `get_custom_object_type_by_handle` | read | Get a custom object type (schema) by handle. |
| `create_custom_object_type` | write | Create a custom object type (schema). Fields define allowed keys and types; record values are validated deterministically. |
| `update_custom_object_type` | write | Update a custom object type (schema). Updating fields replaces the full field definition list. |
| `delete_custom_object_type` | delete | Delete a custom object type (schema). Records of this type must be deleted first. |
| `list_custom_objects` | read | List custom object records (entries). |
| `get_custom_object` | read | Get a custom object record by ID. |
| `get_custom_object_by_handle` | read | Get a custom object record by (typeHandle, objectHandle). |
| `create_custom_object` | write | Create a custom object record. Provide `values` (object) or `valuesJson` (string). Values are validated against the type schema. |
| `update_custom_object` | write | Update a custom object record. Provide `values` (object) or `valuesJson` (string) to update record values. |
| `delete_custom_object` | delete | Delete a custom object record by ID. |

## returns

| Tool | Permission | Description |
| --- | --- | --- |
| `list_returns` | read | List all returns. Shows return status, order, and reason. |
| `get_return` | read | Get a specific return by ID. |
| `create_return` | write | Create a return request for an order. |
| `approve_return` | write | Approve a return request. |
| `reject_return` | write | Reject a return request with a reason. |

## carts

| Tool | Permission | Description |
| --- | --- | --- |
| `list_carts` | read | List all shopping carts. Shows cart status, customer, totals, and item count. |
| `get_cart` | read | Get a specific cart by ID or cart number. Returns full cart details including items. |
| `create_cart` | write | Create a new shopping cart. Can be for a guest or authenticated customer. |
| `update_cart` | write | Update cart customer details, shipping method, coupon code, or notes. |
| `list_customer_carts` | read | List carts for a specific customer. |
| `delete_cart` | delete | Delete a cart permanently. |
| `add_cart_item` | write | Add an item to a shopping cart. |
| `update_cart_item` | write | Update the quantity of an item in the cart. |
| `remove_cart_item` | delete | Remove an item from the cart. |
| `list_cart_items` | read | List items currently in a cart. |
| `clear_cart_items` | delete | Remove all items from a cart. |
| `set_cart_shipping_address` | write | Set the shipping address for a cart. |
| `set_cart_shipping` | write | Set shipping address and shipping selection for a cart. |
| `set_cart_billing_address` | write | Set the billing address for a cart. |
| `set_cart_payment` | write | Set the payment method for a cart. |
| `apply_cart_discount` | write | Apply a coupon/discount code to the cart. |
| `remove_cart_discount` | delete | Remove the coupon or discount from a cart. |
| `get_shipping_rates` | read | Get available shipping rates for a cart based on contents and address. |
| `mark_cart_ready_for_payment` | write | Mark a cart as ready for payment processing. |
| `begin_cart_checkout` | write | Begin the checkout process for a cart. |
| `complete_checkout` | write | Complete the checkout process and convert the cart to an order. This is the final step in the checkout flow. |
| `cancel_cart` | delete | Cancel a shopping cart. |
| `abandon_cart` | write | Mark a cart as abandoned (for recovery campaigns). |
| `expire_cart` | write | Mark a cart as expired. |
| `reserve_cart_inventory` | write | Reserve inventory for all cart items. |
| `release_cart_inventory` | write | Release reserved inventory for all cart items. |
| `recalculate_cart` | write | Recalculate cart totals after pricing or address changes. |
| `set_cart_tax` | write | Set the tax amount for a cart explicitly. |
| `get_abandoned_carts` | read | Get all abandoned carts for recovery campaigns. |
| `get_expired_carts` | read | Get all expired carts. |

## analytics

| Tool | Permission | Description |
| --- | --- | --- |
| `get_sales_summary` | read | Get sales summary for a time period. Returns total revenue, order count, average order value, items sold, and unique customers. |
| `get_revenue_by_period` | read | Get revenue broken down by day, week, or month for a selected period. |
| `get_top_products` | read | Get top selling products by revenue or units sold. |
| `get_product_performance` | read | Get product performance metrics for a time period, optionally filtered by SKU. |
| `get_customer_metrics` | read | Get customer metrics including total customers, new customers, returning customers, and average lifetime value. |
| `get_top_customers` | read | Get top customers by total spend. |
| `get_inventory_health` | read | Get inventory health summary showing total SKUs, in-stock, low stock, and out of stock counts. |
| `get_low_stock_items` | read | Get items that are low in stock or approaching reorder point. |
| `get_inventory_movement` | read | Get inventory movement history and net change over a selected period. |
| `get_demand_forecast` | read | Get demand forecast for inventory items based on historical sales. Predicts future demand and days until stockout. |
| `get_revenue_forecast` | read | Get revenue forecast based on historical trends. |
| `get_order_status_breakdown` | read | Get breakdown of orders by status. |
| `get_fulfillment_metrics` | read | Get fulfillment performance metrics for a selected period. |
| `get_return_metrics` | read | Get return metrics including return rate and total refunds. |

## currency

| Tool | Permission | Description |
| --- | --- | --- |
| `get_exchange_rate` | read | Get the exchange rate between two currencies. |
| `list_exchange_rates` | read | List all available exchange rates, optionally filtered by base currency. |
| `convert_currency` | read | Convert an amount from one currency to another using current exchange rates. |
| `set_exchange_rate` | admin | Set or update an exchange rate between two currencies. |
| `set_exchange_rates` | admin | Set multiple exchange rates in a single operation. |
| `delete_exchange_rate` | delete | Delete an exchange rate by ID. |
| `get_currency_settings` | read | Get the store currency settings including base currency and enabled currencies. |
| `update_currency_settings` | admin | Update store currency settings including enabled currencies and rounding behavior. |
| `set_base_currency` | admin | Set the store's base currency. |
| `enable_currencies` | admin | Enable currencies for the store. |
| `check_currency_enabled` | read | Check whether a currency is enabled for the store. |
| `format_currency` | read | Format an amount with currency symbol. |

## tax

| Tool | Permission | Description |
| --- | --- | --- |
| `calculate_tax` | read | Calculate tax for a transaction based on shipping address and line items. Supports US sales tax, EU VAT, and Canadian GST/HST/PST. |
| `get_tax_rate` | read | Get the effective tax rate for a shipping address and product category. |
| `calculate_item_tax` | read | Calculate tax for a single line item and destination address. |
| `get_tax_jurisdiction` | read | Get a tax jurisdiction by ID or code. |
| `list_tax_jurisdictions` | read | List tax jurisdictions with optional filtering by country or level. |
| `create_tax_jurisdiction` | write | Create a tax jurisdiction. Requires --apply flag. |
| `list_tax_rates` | read | List tax rates for a jurisdiction or all active rates. |
| `get_tax_rate_record` | read | Get a tax rate record by ID. |
| `create_tax_rate` | write | Create a tax rate record. Requires --apply flag. |
| `get_tax_settings` | read | Get the store tax calculation settings. |
| `get_us_state_tax_info` | read | Get pre-configured US state sales tax information including rates and rules. |
| `get_customer_tax_exemptions` | read | Get active tax exemptions for a customer. |
| `get_tax_exemption` | read | Get a tax exemption by ID. |
| `create_tax_exemption` | write | Create a tax exemption certificate for a customer. |
| `check_customer_tax_exempt` | read | Check whether a customer is currently tax exempt. |
| `calculate_cart_tax` | read | Calculate and apply tax to a cart based on its shipping address. Must set shipping address first. Returns tax breakdown and updates cart totals. |
| `list_tax_providers` | read | List tax providers and capabilities for quote, commit, and void workflows. |
| `update_tax_settings` | write | Update store tax settings. Requires --apply flag. |
| `set_tax_enabled` | write | Enable or disable tax calculation. Requires --apply flag. |
| `check_tax_enabled` | read | Check whether tax calculation is currently enabled. |
| `validate_tax_jurisdiction_compliance` | read | Validate jurisdiction readiness for tax calculation (country/state/postal requirements and category checks). |
| `calculate_tax_quote` | read | Calculate a provider-backed tax quote with deterministic replay-safe output and optional idempotency key. |
| `calculate_tax_quote_with_failover` | read | Calculate a tax quote with jurisdiction compliance validation and provider failover routing. |
| `get_tax_quote` | read | Get a provider-backed tax quote by ID. |
| `commit_tax_transaction` | write | Commit a previously calculated tax quote into a provider transaction record. |
| `get_tax_transaction` | read | Get a provider-backed tax transaction by ID. |
| `list_tax_transactions` | read | List provider-backed tax transactions with optional filtering. |
| `void_tax_transaction` | delete | Void a committed tax transaction with optional reason. |
| `ingest_tax_provider_webhook` | write | Ingest a tax provider webhook event and reconcile quote/transaction state in shadow or production mode. |

## promotions

| Tool | Permission | Description |
| --- | --- | --- |
| `list_promotions` | read | List all promotions. Shows active, paused, and scheduled promotions with their discount details. |
| `get_promotion` | read | Get a promotion by ID or internal code. |
| `update_promotion` | write | Update an existing promotion. Requires --apply flag. |
| `create_promotion` | write | Create a new promotion. Supports percentage off, fixed amount off, BOGO, free shipping, and tiered discounts. |
| `delete_promotion` | delete | Delete a promotion. Requires --apply flag. |
| `activate_promotion` | write | Activate a promotion to make it available for use. |
| `deactivate_promotion` | write | Pause/deactivate a promotion. |
| `create_coupon` | write | Create a coupon code for a promotion. |
| `get_coupon` | read | Get a coupon by ID or code. |
| `validate_coupon` | read | Check if a coupon code is valid and can be used. |
| `list_coupons` | read | List coupon codes with optional filters. |
| `get_active_promotions` | read | Get all currently active promotions. |
| `check_promotion_validity` | read | Check whether a promotion is currently valid and eligible to apply. |
| `apply_cart_promotions` | write | Calculate and apply all applicable promotions to a cart. Uses coupon codes on the cart and automatic promotions. |
| `record_promotion_usage` | write | Record promotion usage after checkout completion. Requires --apply flag. |

## subscriptions

| Tool | Permission | Description |
| --- | --- | --- |
| `list_subscription_plans` | read | List all subscription plans. Filter by status (draft, active, archived) or billing interval. |
| `get_subscription_plan` | read | Get details for a specific subscription plan. |
| `create_subscription_plan` | write | Create a new subscription plan. Requires --apply flag. |
| `activate_subscription_plan` | write | Activate a subscription plan (make it available for new subscriptions). Requires --apply flag. |
| `update_subscription_plan` | write | Update an existing subscription plan. Requires --apply flag. |
| `archive_subscription_plan` | delete | Archive a subscription plan (no new subscriptions, existing ones continue). Requires --apply flag. |
| `list_subscriptions` | read | List subscriptions. Filter by customer, plan, or status. |
| `get_subscription` | read | Get details for a specific subscription. |
| `create_subscription` | write | Create a new subscription for a customer. Requires --apply flag. |
| `pause_subscription` | write | Pause a subscription (stops billing, can resume later). Requires --apply flag. |
| `update_subscription` | write | Update subscription fields such as payment method or metadata. Requires --apply flag. |
| `resume_subscription` | write | Resume a paused subscription. Requires --apply flag. |
| `cancel_subscription` | delete | Cancel a subscription. By default cancels at end of period. Requires --apply flag. |
| `skip_billing_cycle` | write | Skip the next billing cycle for a subscription. Requires --apply flag. |
| `list_billing_cycles` | read | List billing cycles for a subscription. |
| `get_billing_cycle` | read | Get details for a specific billing cycle. |
| `get_subscription_events` | read | Get event history (audit log) for a subscription. |

## sync

| Tool | Permission | Description |
| --- | --- | --- |
| `sync_status` | read | Get the current sync status between local database and remote sequencer. Shows pending events, sync lag, and connection status. |
| `sync_push` | write | Push pending local events to the remote sequencer. Requires --apply flag for actual push. |
| `sync_pull` | write | Pull events from the remote sequencer and store them locally. |
| `sync_outbox` | read | List events in the local outbox. Shows pending, synced, failed, and rejected events. |
| `sync_pulled_events` | read | List events already pulled from the sequencer and stored locally. Can optionally include plaintext payloads or decrypt encrypted payloads with local keys. |
| `sync_decrypt_event` | read | Decrypt an encrypted sync event from the local outbox or pulled-event store using the local recipient key. Supports legacy X25519 and hybrid X25519 + ML-KEM-768 wraps. |
| `sync_retry_failed` | admin | Reset failed events to pending status so they can be retried. Requires --apply flag. |
| `sync_entity_history` | read | Get the event history for a specific entity from the remote sequencer or the local pulled-event store. |
| `sync_full` | admin | Perform a full sync: push pending events then pull new events. Requires --apply flag for push. |
| `sync_conflicts` | read | List unresolved sync conflicts. Conflicts occur when local and remote events modify the same entity concurrently. |
| `sync_resolve` | admin | Resolve a specific sync conflict using a resolution strategy. Requires --apply flag. |
| `sync_rebase` | admin | Resolve all sync conflicts using a resolution strategy. Requires --apply flag. |
| `sync_verify_receipt` | read | Verify the signature on a VES event receipt. Supports legacy Ed25519 receipts and hybrid Ed25519 + ML-DSA-65 bundles. |
| `sync_verify_inclusion` | read | Verify a Merkle inclusion proof for a VES event. Proves the event is included in a committed batch. |
| `sync_inspect_commitment` | read | Inspect a VES batch commitment from the sequencer. Shows the Merkle root, sequence range, and event count. |
| `agent_key_generate` | write | Generate a new Ed25519 signing or X25519 encryption key pair for an agent. Requires --apply flag. |
| `agent_key_list` | read | List signing and/or encryption keys for an agent. Returns only public metadata — never exposes private keys. |
| `agent_key_info` | read | Get detailed info for a specific agent key. Returns metadata only — no private key. |
| `agent_key_rotate` | write | Rotate an agent key: generate a new key and revoke the current one. Requires --apply flag. |
| `agent_key_export` | read | Export an agent public key for sequencer registration. Returns public key only — never the private key. |

## manufacturing

| Tool | Permission | Description |
| --- | --- | --- |
| `list_boms` | read | List all Bills of Materials (BOMs). BOMs define the components/ingredients needed to manufacture a product. |
| `get_bom` | read | Get a Bill of Materials by ID, including all components/ingredients. |
| `create_bom` | write | Create a new Bill of Materials for a product. Defines what components/ingredients are needed. |
| `add_bom_component` | write | Add a component/ingredient to a Bill of Materials. |
| `activate_bom` | write | Activate a BOM to make it available for work orders. |
| `list_work_orders` | read | List all manufacturing work orders. Work orders track production runs. |
| `get_work_order` | read | Get a work order by ID with full details. |
| `create_work_order` | write | Create a manufacturing work order to produce a quantity of product. |
| `start_work_order` | write | Start a work order (begin production). |
| `complete_work_order` | write | Complete a work order with the quantity produced. |
| `cancel_work_order` | delete | Cancel a work order. |

## payments

| Tool | Permission | Description |
| --- | --- | --- |
| `list_payments` | read | List all payments in the system. |
| `get_payment` | read | Get a payment by ID. |
| `create_payment` | write | Create a payment for an order. |
| `complete_payment` | write | Mark a payment as completed. |
| `mark_failed_payment` | write | Mark a payment as failed with a required reason and optional failure code. |
| `cancel_payment` | delete | Cancel a payment before settlement is finalized. |
| `create_refund` | write | Create a refund for a payment. |
| `list_payment_providers` | read | List available payment providers and capabilities for agentic payment flows. |
| `create_payment_intent` | write | Create a provider-backed payment intent with idempotency support for governed checkout flows. |
| `get_payment_intent` | read | Get a provider-backed payment intent by ID. |
| `list_payment_intents` | read | List provider-backed payment intents with optional filtering. |
| `list_payment_settlements` | read | List settlement records produced by provider payout reconciliation. |
| `list_payment_settlement_batches` | read | List provider payout batches generated from settlement runs. |
| `create_payment_settlement_batch` | write | Create a settlement batch for captured/refunded payment intents to simulate provider payout reconciliation. |
| `reconcile_payment_provider` | read | Reconcile payment intents against settlement records to find pending settlement or over-settlement drift. |
| `capture_payment_intent` | write | Capture all or part of a provider-backed payment intent. |
| `cancel_payment_intent` | delete | Cancel an uncaptured provider-backed payment intent. |
| `refund_payment_intent` | write | Refund all or part of a captured provider-backed payment intent. |
| `ingest_payment_provider_webhook` | write | Ingest a payment provider webhook event and reconcile payment intent state in shadow or production mode. |

## stablecoin

| Tool | Permission | Description |
| --- | --- | --- |
| `get_agent_wallet` | read | Get the agent wallet address for a specific blockchain. Returns the wallet address derived from VES keys. |
| `get_wallet_balance` | read | Check the balance of the agent wallet on a blockchain. |
| `create_stablecoin_payment` | write | Create and execute a blockchain payment to a wallet address. Supports stablecoins plus native BTC and shielded ZEC flows. |
| `list_supported_chains` | read | List all supported blockchain networks for agent payment execution. |

## treasury

| Tool | Permission | Description |
| --- | --- | --- |
| `treasury_balance` | read | Get treasury balances for an agent. |
| `treasury_ledger` | read | List recent treasury transactions for an agent. |
| `treasury_deposit` | write | Record a treasury deposit for an agent (funds received). |
| `treasury_buy` | write | Purchase tokens using treasury stablecoin balances. |
| `treasury_list_tokens` | read | List available tokens from chain config and custom registry. |
| `treasury_register_token` | admin | Add or update a token in the treasury registry. |

## erc8004

| Tool | Permission | Description |
| --- | --- | --- |
| `erc8004_register_identity` | admin | Register or update an ERC-8004 agent identity record. |
| `erc8004_link_wallet` | write | Link a wallet to an existing ERC-8004 identity record. |
| `erc8004_get_identity` | read | Get an ERC-8004 identity by registry + agent id. |
| `erc8004_get_by_wallet` | read | Get an ERC-8004 identity by wallet address. |
| `erc8004_list_identities` | read | List ERC-8004 identities. |

## x402

| Tool | Permission | Description |
| --- | --- | --- |
| `x402_create_payment_intent` | write | Create an x402 payment intent for AI agent commerce. Returns a signing hash that the payer agent must sign with Ed25519. |
| `x402_sign_intent` | write | Sign an x402 payment intent with an Ed25519 signature. Supports manual signature/public key or local agent-key signing. |
| `x402_get_intent` | read | Get details of an x402 payment intent. |
| `x402_list_intents` | read | List x402 payment intents with optional filtering. |
| `x402_settle_intent_onchain` | write | Execute a signed x402 intent on-chain using an agent wallet, then mark the intent as settled. |
| `x402_execute_agent_payment` | write | Execute end-to-end agentic payment: create intent, locally sign with payer agent key, settle on-chain, and optionally record incoming settlement for payee agent. |
| `x402_record_incoming_settlement` | write | Record a settled x402 intent as an incoming treasury deposit for a local payee agent. |
| `x402_mark_settled` | write | Mark an x402 payment intent as settled on-chain. Called after blockchain confirmation. |
| `x402_get_next_nonce` | read | Get the next nonce for a payer address. Used for replay protection. |
| `x402_credit_balance` | read | Get x402 credit balance for a payer (prepaid meter for streaming usage). |
| `x402_get_credit_account` | read | Get the x402 credit account record for a payer address. |
| `x402_credit_deposit` | write | Credit (deposit) x402 balance for metered usage. Requires --apply. |
| `x402_credit_debit` | write | Debit x402 balance for metered usage. Requires --apply. |
| `x402_credit_transactions` | read | List x402 credit ledger transactions. |

## agent-cards

| Tool | Permission | Description |
| --- | --- | --- |
| `register_agent_card` | write | Register an AI agent card for A2A commerce. Advertises capabilities, supported networks, and payment assets. |
| `discover_agents` | read | Discover AI agents with specific commerce capabilities. Find sellers, buyers, or agents supporting specific networks/assets. |
| `get_agent_card` | read | Get details of a registered AI agent card. |
| `verify_agent` | write | Verify an AI agent card (admin operation). Upgrades trust level to Verified. |
| `list_agent_cards` | read | List all registered AI agent cards. |

## a2a

| Tool | Permission | Description |
| --- | --- | --- |
| `a2a_pay` | write | Pay another AI agent directly. Send supported payment assets including USDC, ssUSD, BTC, or shielded ZEC to another agent by identity wallet, native chain address, or agent ID. |
| `a2a_request_payment` | write | Request payment from another agent. Creates a payment request that the other agent can pay. |
| `a2a_pay_request` | write | Pay an existing payment request from another agent. |
| `a2a_request_quote` | write | Request a price quote from another agent for goods or services. |
| `a2a_provide_quote` | write | Respond to a quote request with pricing (for sellers). |
| `a2a_accept_quote` | write | Accept a quote and pay. Automatically sends payment to the seller. |
| `a2a_decline_quote` | write | Decline a quote. |
| `a2a_fulfill_quote` | write | Mark a quote as fulfilled after delivering goods/services (for sellers). |
| `a2a_get_payment` | read | Get a single A2A payment by ID. Optionally refresh native on-chain confirmation state for supported settlement networks including Bitcoin and shielded Zcash. |
| `a2a_list_payments` | read | List A2A payments sent or received by this agent. Can optionally refresh pending native-chain settlement state for payments with on-chain transaction hashes. |
| `a2a_list_payment_requests` | read | List payment requests created by or sent to this agent. |
| `a2a_list_quotes` | read | List quotes where this agent is buyer or seller. |
| `a2a_get_balance` | read | Get an A2A payment summary for this agent, with optional asset/network filters and per-rail breakdowns. |
| `a2a_discover_agents` | read | Discover AI agents that can provide goods or services. Find sellers, buyers, or agents with specific capabilities. |
| `a2a_counter_quote` | write | Counter a quote with a different price (for buyers). Initiates or continues price negotiation with the seller. |
| `a2a_revise_quote` | write | Revise a quote after a buyer counter-offer (for sellers). Adjusts pricing in response to negotiation. |
| `a2a_create_escrow` | write | Create an escrow to hold funds between buyer and seller agents. Supports conditional release, time-based expiry, and dispute escalation. |
| `a2a_fund_escrow` | write | Fund an escrow, moving it to active status so the seller can begin work. |
| `a2a_release_escrow` | write | Release escrow funds to the seller. All release conditions must be met. |
| `a2a_refund_escrow` | write | Refund escrow funds back to the buyer. |
| `a2a_dispute_escrow` | write | Dispute an escrow, escalating it to the dispute resolution system. |
| `a2a_get_escrow` | read | Get details of an escrow by ID. |
| `a2a_list_escrows` | read | List escrows with optional filters. |
| `a2a_file_dispute` | write | File a formal dispute against an escrow. Begins the dispute resolution process with evidence collection and review. |
| `a2a_submit_evidence` | write | Submit evidence for an active dispute. |
| `a2a_resolve_dispute` | write | Resolve a dispute with a resolution type (full refund, partial refund, release to seller, split, or escalate). |
| `a2a_get_dispute` | read | Get details of a dispute by ID, including evidence count. |
| `a2a_list_disputes` | read | List disputes with optional filters. |
| `a2a_rate_agent` | write | Rate an agent after a transaction. Scores 1-5 with optional dimension ratings (reliability, quality, speed, communication). |
| `a2a_get_reputation` | read | Get reputation and trust score for an agent. |
| `a2a_respond_to_feedback` | write | Respond to feedback left on your agent (only the rated agent can respond). |
| `a2a_register_service` | write | Register a service that this agent provides. Other agents can discover and purchase your services. |
| `a2a_list_services` | read | List available agent services with optional filters and search. |
| `a2a_get_service` | read | Get details of a specific agent service. |
| `a2a_send_notification` | write | Send a webhook notification to another agent. Delivers a signed payload to their configured endpoint. |
| `a2a_list_notification_log` | read | View the webhook notification delivery log with optional filters. |
| `a2a_configure_webhooks` | write | Configure webhook settings for an agent. Set the endpoint URL, signing secret, and which event types to receive. |
| `a2a_list_webhook_dlq` | admin | List quarantined webhook notifications that permanently failed delivery. Use to inspect and replay failed deliveries. |
| `a2a_quarantine_failed_webhooks` | admin | Move permanently failed webhook notifications to the dead letter queue. Notifications that exhausted all retry attempts are quarantined for inspection. |
| `a2a_replay_dlq_entry` | admin | Replay a dead letter queue entry by moving it back to the notification log for retry. Resets the attempt counter. |
| `a2a_purge_dlq` | admin | Purge old dead letter queue entries. Removes entries quarantined more than the specified number of days ago. |
| `a2a_dlq_count` | read | Get the count of entries in the webhook dead letter queue. |
| `a2a_create_agent_subscription` | write | Create a recurring payment subscription between two agents. Supports trial periods and configurable billing intervals. |
| `a2a_pause_agent_subscription` | write | Pause an active agent subscription. Billing is suspended until resumed. |
| `a2a_resume_agent_subscription` | write | Resume a paused agent subscription. Recalculates billing dates from now. |
| `a2a_cancel_agent_subscription` | write | Cancel an agent subscription. Can cancel immediately or at the end of the current billing period. |
| `a2a_get_agent_subscription` | read | Get details of an agent-to-agent subscription. |
| `a2a_list_agent_subscriptions` | read | List agent-to-agent subscriptions with optional filters. |
| `a2a_process_subscription_billing` | write | Process all due subscription billing cycles. Bills active subscriptions, handles past-due retries, transitions expired trials, and cancels end-of-period subscriptions. |
| `a2a_create_split_payment` | write | Create a multi-party split payment. Splits a payment across 2+ recipients by percentage or fixed amounts, with optional platform fee. |
| `a2a_execute_split_payment` | write | Execute a pending split payment, sending funds to each recipient. Tracks per-recipient status. |
| `a2a_get_split_payment` | read | Get details of a split payment including all recipient shares and statuses. |
| `a2a_list_split_payments` | read | List split payments with optional filters. |
| `a2a_create_conditional_payment` | write | Create a conditional payment that combines escrow with x402 payment intent. Funds are held in escrow until conditions are met, then automatically settled. |
| `a2a_check_payment_conditions` | read | Check whether all release conditions are met for a conditional payment (escrow). |
| `a2a_settle_conditional_payment` | write | Settle a conditional payment. Checks all conditions, releases escrow funds to the seller, and marks the x402 intent as settled. |
| `a2a_subscribe_events` | write | Subscribe an agent to receive real-time events. Supports wildcard and prefix-based event type filtering. |
| `a2a_list_event_subscriptions` | read | List active event subscriptions for an agent. |
| `a2a_get_event_history` | read | Get historical events for an agent with optional filtering. |

## agent-runtime

| Tool | Permission | Description |
| --- | --- | --- |
| `agent_create_runtime` | write | Create an autonomous AI agent runtime with a wallet, negotiation strategy, and budget. The agent can then register services, discover other agents, negotiate quotes, and make payments autonomously. |
| `agent_destroy_runtime` | delete | Destroy an agent runtime and clean up resources. |
| `agent_list_runtimes` | read | List all active agent runtimes in this session, with optional asset/network budget scope. |
| `agent_get_status` | read | Get detailed status of an agent runtime including budget, strategy, registered services, and optional rail-specific settlement context. |
| `agent_set_strategy` | write | Change an agent's negotiation strategy. Available: always-accept, budget-gated, negotiator, best-of-n, reputation-aware. |
| `agent_get_budget` | read | Get the current budget status of an agent, with optional asset/network scope for multi-rail payment budgets. |
| `agent_tick` | write | Process one autonomous cycle for an agent. The agent will respond to pending quotes, evaluate received offers, and auto-fulfill accepted deals. |
| `agent_start_loop` | write | Start the agent's autonomous polling loop. The agent will continuously process incoming work. |
| `agent_stop_loop` | write | Stop the agent's autonomous polling loop. |
| `agent_register_service` | write | Register a service in the A2A marketplace so other agents can discover and purchase it. |
| `agent_discover_services` | read | Search the A2A marketplace for services by category or capability. |
| `agent_create_escrow_deal` | write | Create an escrow-backed transaction between agents. Funds are held until conditions are met (seller fulfilled, buyer confirmed, time lock, or milestone). |
| `agent_subscribe_to_service` | write | Subscribe an agent to another agent's recurring service (e.g., daily data feed, monthly analytics). |
| `agent_rate_counterparty` | write | Rate another agent after a transaction. Builds reputation in the marketplace. |
| `agent_get_reputation` | read | Get an agent's reputation score, trust tier, and feedback summary. |
| `agent_create_split_deal` | write | Create a multi-party payment split. Revenue from a deal is distributed to multiple agents. |
| `agent_get_event_history` | read | Get an agent's event stream history with optional filters for event type, time window, and payment rail. |
| `agent_enable_settlement` | write | Enable on-chain payment settlement for an agent runtime. The agent will settle payments on the specified blockchain using derived wallets. |
| `agent_get_chain_balance` | read | Get the on-chain payment-token balance for an agent runtime with settlement enabled. |
| `agent_broadcast_rfq` | write | Broadcast a Request for Quotation (RFQ) to multiple sellers in the marketplace. Sellers matching the filter will receive quote requests. |
| `agent_collect_rfq_responses` | read | Collect and score all responses for an RFQ broadcast. |
| `agent_award_rfq` | write | Award an RFQ to the best-scored (or specified) seller. Accepts the winner's quote and declines all others. |
| `agent_get_marketplace_metrics` | read | Get marketplace performance metrics for a registered service (success rate, response time, etc.). |
| `agent_attach_sla` | write | Attach a Service Level Agreement to a registered service. Defines performance thresholds and penalties. |
| `agent_check_sla_compliance` | read | Check if a service is meeting its SLA commitments. |
| `agent_create_workflow` | write | Create a multi-agent workflow with DAG-based step dependencies. Steps execute in topological order. |
| `agent_execute_workflow` | write | Execute a workflow. Steps run in dependency order with parallel fan-out where possible. |
| `agent_get_workflow_status` | read | Get the current status and progress of a workflow. |
| `agent_set_dynamic_pricing` | write | Configure dynamic pricing for an agent. Sets volume breaks, reputation tiers, peak hours, and loyalty tiers. |

## shipments

| Tool | Permission | Description |
| --- | --- | --- |
| `list_shipments` | read | List all shipments. |
| `get_shipment` | read | Get a shipment by ID. |
| `create_shipment` | write | Create a shipment for an order. |
| `ship_shipment` | write | Mark a shipment as shipped with an optional tracking number. |
| `deliver_shipment` | write | Mark a shipment as delivered. |
| `cancel_shipment` | delete | Cancel a shipment before delivery is completed. |
| `list_shipping_providers` | read | List shipping providers and capabilities for quoting, labeling, and tracking. |
| `quote_shipping_rates` | read | Quote carrier rates from provider adapters using structured parcel data and destination address. |
| `create_shipping_label` | write | Create a carrier label from quoted rates or explicit service code. |
| `void_shipping_label` | delete | Void a shipping label before final delivery. |
| `track_shipping_label` | read | Track a shipping label by label ID or tracking number. |
| `list_shipping_labels` | read | List provider-backed shipping labels with optional filtering. |
| `ingest_shipping_provider_webhook` | write | Ingest a shipping provider webhook event and reconcile label/tracking state for shadow mode operations. |
| `handle_fulfillment_exception` | write | Execute governed fulfillment exception workflows for carrier failure, partial shipment, split tender, and returns arbitration. |

## suppliers

| Tool | Permission | Description |
| --- | --- | --- |
| `list_suppliers` | read | List all suppliers. |
| `get_supplier` | read | Get a supplier by ID. |
| `create_supplier` | write | Create a new supplier. |
| `list_purchase_orders` | read | List all purchase orders. |
| `get_purchase_order` | read | Get a purchase order by ID. |
| `create_purchase_order` | write | Create a purchase order to a supplier. |
| `submit_purchase_order` | write | Submit a purchase order for approval. |
| `approve_purchase_order` | write | Approve a purchase order. |
| `send_purchase_order` | write | Send a PO to the supplier. |
| `cancel_purchase_order` | write | Cancel a purchase order. |

## invoices

| Tool | Permission | Description |
| --- | --- | --- |
| `list_invoices` | read | List all invoices. |
| `get_invoice` | read | Get an invoice by ID. |
| `create_invoice` | write | Create an invoice for a customer. |
| `send_invoice` | write | Send an invoice to the customer. |
| `void_invoice` | delete | Void an invoice so it can no longer be paid or collected. |
| `record_invoice_payment` | write | Record payment on an invoice. |
| `get_overdue_invoices` | read | Get all overdue invoices. |

## warranties

| Tool | Permission | Description |
| --- | --- | --- |
| `list_warranties` | read | List all warranties. |
| `get_warranty` | read | Get a warranty by ID. |
| `create_warranty` | write | Create a warranty for a product. |
| `create_warranty_claim` | write | File a warranty claim. |
| `approve_warranty_claim` | write | Approve a warranty claim. |
| `deny_warranty_claim` | write | Deny a warranty claim with a reason. |
| `complete_warranty_claim` | write | Complete a warranty claim with a final resolution. |

## import

| Tool | Permission | Description |
| --- | --- | --- |
| `import_shopify_data` | write | Import data from a Shopify store. Supports API, CSV file, and JSON file sources. Imports customers, products, orders, and inventory in dependency order. |
| `import_shopify_shadow_data` | write | Run Shopify interop in shadow mode for products, inventory, orders, fulfillments, and customers. Produces parity-ready summaries without writes unless explicitly enabled. |
| `import_status` | read | Get the status of the most recent import operation. |
| `list_id_mappings` | read | List external ID to StateSet ID mappings for a platform. Useful for verifying imported data. |
| `import_csv` | write | Import data from a CSV file. Auto-detects Shopify format or uses generic column mapping. |
| `import_json` | write | Import data from a JSON file (Shopify REST API response format or array). |
| `export_data` | read | Export StateSet data to JSON format. Useful for parity testing after imports. |
| `import_woocommerce_data` | write | Import data from a WooCommerce store via REST API. Imports customers, products, orders, and inventory in dependency order. |
| `configure_stripe_webhooks` | write | Configure Stripe webhook endpoint in the webhook server. Sets up the Stripe v1 signature verification and registers the webhook source. |
| `configure_woocommerce_webhooks` | write | Configure WooCommerce webhook endpoint in the webhook server. Sets up HMAC-SHA256 signature verification. |

## policies

| Tool | Permission | Description |
| --- | --- | --- |
| `evaluate_policy` | read | Evaluate a policy domain against a context object. Returns allow/deny decision with full explanation of which rules matched and why. |
| `list_policies` | read | List all registered policy sets. Shows policy set IDs, names, domains, and rule counts. |
| `register_policy_template` | write | Activate one of the built-in policy templates. Available templates: autoApproveReturns, inventoryRestock, orderFraudDetection, promotionEligibility, subscriptionRules. |
| `load_policy_file` | write | Load a YAML or JSON policy file into the engine. The file must define a valid policy set with domain, rules, and actions. |
| `explain_policy_denial` | read | Re-evaluate a policy domain with verbose per-condition breakdown. Shows which conditions matched, which did not, and the expected vs actual values for each. |

## vector

| Tool | Permission | Description |
| --- | --- | --- |
| `vector_search_products` | read | Search products using natural language query with hybrid semantic + BM25 ranking. Returns products sorted by relevance score. |
| `vector_search_customers` | read | Search customers using natural language query with hybrid semantic + BM25 ranking. |
| `vector_search_orders` | read | Search orders using natural language query with hybrid semantic + BM25 ranking. |
| `vector_search_inventory` | read | Search inventory items using natural language query with hybrid semantic + BM25 ranking. |
| `vector_index_product` | write | Index a single product for vector search by its ID. |
| `vector_index_customer` | write | Index a single customer for vector search by their ID. |
| `vector_index_order` | write | Index a single order for vector search by its ID. |
| `vector_index_inventory` | write | Index a single inventory item for vector search by its ID. |
| `vector_index_all_products` | admin | Index all products in the database for vector search. This may take a while for large catalogs. |
| `vector_index_all_customers` | admin | Index all customers in the database for vector search. |
| `vector_index_all_orders` | admin | Index all orders in the database for vector search. |
| `vector_index_all_inventory` | admin | Index all inventory items in the database for vector search. |
| `vector_stats` | read | Get statistics about vector embeddings including counts by entity type. |
| `vector_clear` | admin | Clear all vector embeddings for a specific entity type. |
| `vector_clear_all` | admin | Clear all vector embeddings across all entity types. |
| `vector_reindex_all` | admin | Rebuild all vector embeddings from scratch. Clears existing embeddings then re-indexes all products, customers, orders, and inventory items. Use this after bulk data imports or to fix stale embeddings. |

## gift-cards

| Tool | Permission | Description |
| --- | --- | --- |
| `create_gift_card` | write | Create a new gift card with an initial balance. |
| `get_gift_card` | read | Get a gift card by ID or code. |
| `list_gift_cards` | read | List all gift cards with optional filters. |
| `charge_gift_card` | write | Charge (deduct) an amount from a gift card balance. |
| `refund_to_gift_card` | write | Refund an amount back to a gift card. |
| `disable_gift_card` | write | Disable a gift card so it can no longer be used. |
| `check_gift_card_balance` | read | Check the current balance of a gift card by ID or code. |

## store-credits

| Tool | Permission | Description |
| --- | --- | --- |
| `create_store_credit` | write | Issue store credit to a customer. |
| `get_store_credit` | read | Get store credit details by ID. |
| `list_store_credits` | read | List store credits with optional filters. |
| `adjust_store_credit` | write | Adjust a store credit balance (add or subtract). |
| `apply_store_credit` | write | Apply store credit to an order. |

## segments

| Tool | Permission | Description |
| --- | --- | --- |
| `create_segment` | write | Create a customer segment with filter conditions. |
| `get_segment` | read | Get a segment by ID including its conditions and member count. |
| `list_segments` | read | List all customer segments. |
| `update_segment` | write | Update a segment name, description, or conditions. |
| `evaluate_segment_membership` | read | Check whether a customer belongs to a segment. |
| `rebuild_dynamic_segment` | write | Rebuild a dynamic segment by re-evaluating all customers against its conditions. |

## shipping-zones

| Tool | Permission | Description |
| --- | --- | --- |
| `create_shipping_zone` | write | Create a shipping zone with country/region rules. |
| `get_shipping_zone` | read | Get a shipping zone by ID. |
| `list_shipping_zones` | read | List all shipping zones. |
| `update_shipping_zone` | write | Update a shipping zone name, countries, or regions. |
| `create_shipping_method` | write | Create a shipping method within a zone (e.g., Standard, Express, Overnight). |
| `calculate_shipping_rate` | read | Calculate shipping rate for a destination address and cart items. |
| `list_shipping_methods` | read | List shipping methods for a specific zone. |

## units-of-measure

| Tool | Permission | Description |
| --- | --- | --- |
| `list_unit_classes` | read | List unit classes (e.g. weight, volume). |
| `create_unit_class` | write | Create a unit class. |
| `delete_unit_class` | write | Delete a unit class. |
| `list_units_of_measure` | read | List units of measure, optionally scoped to a unit class. |
| `create_unit_of_measure` | write | Create a unit of measure within a unit class. |
| `set_base_unit_of_measure` | write | Mark a unit of measure as the base unit for its class. |
| `delete_unit_of_measure` | write | Delete a unit of measure. |
| `list_unit_conversion_rules` | read | List unit conversion rules. |
| `create_unit_conversion_rule` | write | Create a unit conversion rule (system-wide or SKU-specific). |
| `delete_unit_conversion_rule` | write | Delete a unit conversion rule. |

## stock-snapshots

| Tool | Permission | Description |
| --- | --- | --- |
| `list_stock_snapshots` | read | List stock snapshots (header level). |
| `get_stock_snapshot` | read | Get a stock snapshot by ID. |
| `get_latest_stock_snapshot` | read | Get the most recent stock snapshot. |
| `capture_stock_snapshot` | write | Capture a stock snapshot; totals are computed from the supplied lines. |
| `delete_stock_snapshot` | write | Delete a stock snapshot. |

## print-stations

| Tool | Permission | Description |
| --- | --- | --- |
| `list_print_stations` | read | List paired print stations. |
| `get_print_station` | read | Get a print station by ID. |
| `pair_print_station` | write | Pair a new print station. Returns a one-time pairing token. |
| `revoke_print_station` | write | Revoke a paired print station. |
| `list_print_jobs` | read | List print jobs for a station. |
| `enqueue_print_job` | write | Enqueue a print job to a station. |
| `pick_up_next_print_job` | write | Pick up the next queued print job for a station. |
| `complete_print_job` | write | Mark a print job printed or failed. |

## integration-mappings

| Tool | Permission | Description |
| --- | --- | --- |
| `list_integration_mappings` | read | List integration value mappings. |
| `get_integration_mapping` | read | Get an integration mapping by ID. |
| `resolve_integration_mapping` | read | Resolve the internal value for an external value. |
| `create_integration_mapping` | write | Create an integration value mapping. |
| `update_integration_mapping` | write | Update an integration mapping. |
| `bulk_upsert_integration_mappings` | write | Bulk upsert integration mappings. |
| `delete_integration_mapping` | write | Delete an integration mapping. |

## integration-field-mappings

| Tool | Permission | Description |
| --- | --- | --- |
| `list_integration_field_mappings` | read | List integration field mappings. |
| `get_integration_field_mapping` | read | Get an integration field mapping by ID. |
| `list_integration_mapping_groups` | read | List the distinct mapping groups for an integration account. |
| `create_integration_field_mapping` | write | Create an integration field mapping. |
| `update_integration_field_mapping` | write | Update an integration field mapping. |
| `bulk_create_integration_field_mappings` | write | Bulk create integration field mappings. |
| `bulk_delete_integration_field_mappings` | write | Bulk delete integration field mappings by ID. |
| `delete_integration_field_mapping` | write | Delete an integration field mapping. |

## payment-obligations

| Tool | Permission | Description |
| --- | --- | --- |
| `list_payment_obligations` | read | List payment obligations. |
| `get_payment_obligation` | read | Get a payment obligation by ID. |
| `get_payment_obligation_dashboard` | read | Aggregate payment obligation dashboard as of a date. |
| `create_payment_obligation` | write | Create a payment obligation. |
| `record_payment_obligation_payment` | write | Record a payment against an obligation. |
| `set_payment_obligation_status` | write | Set the status of a payment obligation. |
| `link_payment_obligation_bill` | write | Link an accounts-payable bill to a payment obligation. |

## purgatory

| Tool | Permission | Description |
| --- | --- | --- |
| `list_purgatory_orders` | read | List staged purgatory orders. |
| `get_purgatory_order` | read | Get a purgatory order by ID. |
| `ingest_purgatory_order` | write | Ingest an external order into purgatory. |
| `map_purgatory_line` | write | Map a staged line to a product and/or toggle its flags. |
| `post_purgatory_order` | write | Post a fully-resolved order out of purgatory. |
| `delete_purgatory_order` | write | Delete a purgatory order. |

## topology-snapshots

| Tool | Permission | Description |
| --- | --- | --- |
| `list_topology_snapshots` | read | List operational topology snapshots. |
| `get_topology_snapshot` | read | Get a topology snapshot by ID. |
| `get_latest_topology_snapshot` | read | Get the most recent topology snapshot. |
| `capture_topology_snapshot` | write | Capture a topology snapshot; health is derived from the supplied metrics. |
| `delete_topology_snapshot` | write | Delete a topology snapshot. |

## vendor-returns

| Tool | Permission | Description |
| --- | --- | --- |
| `list_vendor_returns` | read | List vendor returns. |
| `get_vendor_return` | read | Get a vendor return by ID. |
| `create_vendor_return` | write | Create a draft vendor return. |
| `submit_vendor_return` | write | Submit a draft vendor return to the supplier. |
| `process_vendor_return` | write | Process a vendor return, optionally generating a vendor credit. |
| `cancel_vendor_return` | write | Cancel a vendor return. |

## reviews

| Tool | Permission | Description |
| --- | --- | --- |
| `create_review` | write | Create a product review. |
| `get_review` | read | Get a review by ID. |
| `list_reviews` | read | List reviews with optional filters. |
| `approve_review` | write | Approve a pending review for public display. |
| `reject_review` | write | Reject a review with a reason. |
| `get_review_summary` | read | Get aggregated review summary for a product including average rating and rating distribution. |
| `flag_review` | write | Flag a review for manual moderation. |

## wishlists

| Tool | Permission | Description |
| --- | --- | --- |
| `create_wishlist` | write | Create a new wishlist for a customer. |
| `get_wishlist` | read | Get a wishlist by ID including all items. |
| `add_to_wishlist` | write | Add a product to a wishlist. |
| `remove_from_wishlist` | write | Remove a product from a wishlist. |
| `list_wishlists` | read | List wishlists for a customer. |
| `convert_wishlist_to_cart` | write | Convert all items in a wishlist to a shopping cart. |

## loyalty

| Tool | Permission | Description |
| --- | --- | --- |
| `create_loyalty_program` | admin | Create a loyalty program with tiers and earning rules. |
| `get_loyalty_program` | read | Get loyalty program details including tiers and reward catalog. |
| `enroll_customer` | write | Enroll a customer in a loyalty program. |
| `get_loyalty_account` | read | Get a customer loyalty account including points balance and tier. |
| `earn_points` | write | Award loyalty points to a customer account. |
| `redeem_points` | write | Redeem loyalty points for a reward or discount. |
| `list_rewards` | read | List available rewards in a loyalty program. |
| `create_reward` | admin | Create a redeemable reward in a loyalty program. |

## fraud

| Tool | Permission | Description |
| --- | --- | --- |
| `assess_order_fraud` | read | Run fraud assessment on an order. Returns a risk score and matched signals. |
| `get_fraud_assessment` | read | Get a fraud assessment by ID. |
| `list_fraud_signals` | read | List fraud signals for an order or across all recent orders. |
| `create_fraud_rule` | admin | Create a custom fraud detection rule. |
| `update_fraud_rule` | admin | Update a fraud detection rule. |
| `review_flagged_order` | write | Review a flagged order and mark it as approved or rejected. |

## connectors

| Tool | Permission | Description |
| --- | --- | --- |
| `list_connector_marketplace` | read | List available WASM connectors in the local marketplace catalog. |
| `publish_wasm_connector` | admin | Publish a WASM connector to the local marketplace catalog (app-store style ecosystem index). |
| `install_wasm_connector` | write | Install a connector from marketplace catalog into the local connector runtime. |
| `assess_wasm_connector_safety` | read | Compute connector safety scorecard and risk signals for marketplace governance and installation policy. |
| `certify_wasm_connector` | admin | Issue marketplace certification metadata for a connector version using automated safety score + trust policy. |
| `sign_wasm_connector_attestation` | admin | Sign a marketplace connector attestation using local signing key material for trustable install/execute verification. |
| `verify_wasm_connector_attestation` | read | Verify connector trust attestation in the marketplace catalog before installation or execution. |
| `uninstall_wasm_connector` | delete | Uninstall a connector version from the local connector runtime. |
| `list_installed_connectors` | read | List installed connectors available to agentic runtime execution. |
| `get_installed_connector` | read | Get details for an installed connector and its action contract. |
| `execute_wasm_connector` | write | Execute an installed WASM connector action so agents can orchestrate ecosystem apps through iCommerce. |

## audit

| Tool | Permission | Description |
| --- | --- | --- |
| `audit_query` | read | Query the audit log with optional filters. Returns recent permission checks and tool executions. |
| `audit_summary` | read | Get a summary of audit activity including total entries, breakdown by result type, and most active tools. |
| `audit_export` | admin | Export the full audit log for compliance purposes. Returns all entries with metadata for external archival. |
| `audit_retention` | admin | Run audit log retention cleanup. Removes entries older than the configured retention period (default: 90 days). |

## proofs

| Tool | Permission | Description |
| --- | --- | --- |
| `verify_receipt` | read | Verify a VES commerce receipt — checks signature, hash, and Merkle inclusion proof. |
| `generate_inclusion_proof` | read | Generate a Merkle inclusion proof for a specific event within a batch of events. |
| `verify_inclusion_proof` | read | Verify a Merkle inclusion proof — confirms that a leaf hash is included in a Merkle root. |
| `generate_receipt_bundle` | read | Generate a full verifiable receipt bundle for an event — includes event data, leaf hash, Merkle inclusion proof, and anchor metadata. |
| `inspect_batch` | read | Inspect a batch of events — computes Merkle root, event count, and time range. |
| `export_compliance_package` | read | Generate a compliance package — a complete set of verifiable receipts for all events in a batch, suitable for regulatory export or third-party audit. |
| `verify_chain_anchor` | read | Verify that an event proof matches an expected on-chain anchor transaction hash and Merkle root. Confirms the event was committed to the chain. |

## circuit-breaker

| Tool | Permission | Description |
| --- | --- | --- |
| `agent_get_breaker_state` | read | Get the circuit breaker state for a specific agent, including trip reason and config. |
| `agent_get_spending_summary` | read | Get the spending summary for an agent: today's spend, monthly spend, and remaining limits. |
| `agent_get_all_breaker_states` | read | Get the circuit breaker states for all known agents. |
| `agent_trip_breaker` | admin | Manually trip the circuit breaker for a specific agent. Blocks all transactions until reset. |
| `agent_trip_all_breakers` | admin | Activate the global kill switch — blocks ALL agent transactions immediately. |
| `agent_reset_breaker` | admin | Reset the circuit breaker for a specific agent, allowing transactions again. |
| `agent_reset_all_breakers` | admin | Reset ALL circuit breakers and deactivate the global kill switch. |
| `agent_set_spending_limits` | admin | Update the spending limits for agent circuit breakers: per-transaction, daily, and monthly caps. |

## checkout

| Tool | Permission | Description |
| --- | --- | --- |
| `create_payment_link` | write | Create a shareable payment link for instant checkout. Returns a short URL that buyers or agents can use. |
| `resolve_payment_link` | read | Resolve a payment link by ID or short code. Returns the link details, items, total, and expiry status. |
| `express_checkout` | write | One-call checkout from a payment link. Converts the link into an order and payment. |
| `agent_instant_checkout` | write | Agent-to-agent instant checkout. Creates a payment link and converts it in one step. Returns order and escrow IDs for A2A settlement. |
| `get_payment_link_status` | read | Get the status and metrics (views, conversions) for a payment link. |
| `list_payment_links` | read | List payment links with optional filters by status and customer. |
| `revoke_payment_link` | write | Revoke (cancel) an active payment link. Prevents further checkouts from it. |
| `checkout_with_crypto` | write | Express checkout with a crypto wallet. Similar to express_checkout but takes a wallet address and network for on-chain payment. |

## compliance

| Tool | Permission | Description |
| --- | --- | --- |
| `export_audit_trail` | admin | Export a complete audit trail of agent transactions and events for compliance review. Supports JSON and CSV formats with date range, agent, and event type filters. |
| `generate_1099k` | admin | Generate a 1099-K tax report for an agent. Summarizes gross payment amounts, transaction counts, and monthly breakdowns for a given tax year. |
| `export_gdpr_data` | admin | Export all personal data for a customer or agent (GDPR Article 20 — data portability). Returns personal data, payments, communications, and disputes. |
| `delete_gdpr_data` | admin | Delete personal data for GDPR right to erasure (Article 17). Optionally retains anonymized transaction records for legal/accounting requirements. |
| `compliance_summary` | read | Generate a compliance dashboard summary with transaction volume, dispute rates, policy violations, and top agents for a given period. |
| `soc2_evidence` | admin | Generate a SOC2 audit evidence package. Gathers structured evidence for requested controls: access_control, change_management, encryption, monitoring, incident_response. |

## catalog

| Tool | Permission | Description |
| --- | --- | --- |
| `publish_product_catalog` | write | Publish a product to the machine-readable agent catalog. Makes products discoverable by AI agents with capability-based matching, trust levels, and machine-readable specs. |
| `query_agent_catalog` | read | Query the agent catalog for products matching filters. Supports capability, trust level, price, fulfillment chain, and category filtering. |
| `get_product_spec` | read | Get the full machine-readable spec for a catalog product. Returns capabilities, requirements, pricing, trust level, and a JSON Schema fragment. |
| `match_agent_to_products` | read | Find catalog products compatible with an agent based on its capabilities and trust level. Returns products sorted by relevance (capability overlap). |
| `match_product_to_agents` | read | Find agents compatible with a specific product. Filters available agents by the product's required trust level and capabilities. |
| `export_agent_catalog` | read | Export the agent catalog in JSON or OpenAPI format. Useful for sharing the catalog with other systems or generating API documentation. |

## a2a-automation

| Tool | Permission | Description |
| --- | --- | --- |
| `a2a_billing_tick` | write | Run one billing cycle: process due subscriptions, execute payments, handle past-due, activate trials. |
| `a2a_billing_start` | admin | Start the automated billing executor loop. |
| `a2a_billing_stop` | admin | Stop the automated billing executor loop. |
| `a2a_billing_metrics` | read | Get billing executor metrics: total billed, failed, cancelled, etc. |
| `a2a_dispute_resolver_tick` | write | Run one dispute resolution cycle: auto-transition deadlines, apply rule-based arbitration. |
| `a2a_dispute_resolver_start` | admin | Start the automated dispute resolver loop. |
| `a2a_dispute_resolver_metrics` | read | Get dispute resolver metrics: transitions, resolutions, escalations. |
| `a2a_sla_enforce` | write | Enforce SLA penalties for a service: detect breaches and apply credits/suspensions/refunds. |
| `a2a_sla_enforce_all` | write | Run a full SLA enforcement cycle across all services. |
| `a2a_marketplace_auto_award` | write | Auto-award expired RFQs to the highest-scored response. Expires RFQs with no responses. |
| `a2a_marketplace_maintenance` | write | Run a full marketplace maintenance tick: auto-award + expiry + cleanup. |
| `a2a_list_failed_notifications` | read | List failed webhook notifications (dead-letter queue). Shows notifications that exceeded max retry attempts. |
| `a2a_replay_notification` | write | Manually retry a specific failed notification. |
| `a2a_notification_retry_all` | write | Trigger retry of all pending webhook notifications. |
| `a2a_webhook_dlq_status` | read | Get dead-letter queue metrics: pending, failed, delivered counts. |
| `a2a_health_check` | read | Run a full health check: database, sequencer, subsystems. |
| `a2a_readiness` | read | Check if the system is ready to accept traffic. |
| `x402_circuit_status` | read | Get x402 sequencer circuit breaker status: state (closed/open/half_open), failures, queue depth. |
| `a2a_rate_limit_metrics` | read | Get MCP rate limiter metrics: active buckets, top agents by request count. |
| `a2a_saga_execute` | write | Execute a multi-step transaction saga (e.g., purchase, subscription, RFQ). Automatically rolls back on failure. |
| `a2a_saga_status` | read | Get the status of a running or completed saga by ID. |
| `a2a_saga_list` | read | List sagas with optional status filter. |
| `a2a_saga_cancel` | write | Cancel a running saga and trigger compensation/rollback. |
| `a2a_cost_summary` | read | Get spend summary for an agent with optional asset/network filters and per-rail breakdowns. |
| `a2a_cost_counterparty_breakdown` | read | Get per-counterparty spend/earn breakdown for an agent, with optional asset/network filters and per-rail details. |
| `a2a_cost_operation_breakdown` | read | Get per-operation cost breakdown for an agent, with optional asset/network filters and per-rail details. |
| `a2a_cost_daily_trend` | read | Get daily spend and earnings trend for an agent, with optional asset/network filters and per-rail day breakdowns. |
| `a2a_cost_anomalies` | read | Detect per-rail spending anomalies, with optional asset/network filters to avoid mixed-unit comparisons. |
| `a2a_cost_margin_analysis` | read | Get margin analysis with optional asset/network filters and per-rail counterparty breakdowns. |
| `a2a_cost_budget_forecast` | read | Forecast when a budget in the selected asset units will be exhausted, with optional asset/network filters and per-rail spend breakdowns. |
| `a2a_cost_top_spenders` | read | Get top-spending agents across the system, with optional asset/network filters. |
| `a2a_escrow_process_all` | write | Process all escrows: auto-release time-locked escrows where conditions are met, expire past-deadline escrows. |

## a2a-observability

| Tool | Permission | Description |
| --- | --- | --- |
| `a2a_get_trace` | read | Retrieve all spans for a distributed trace ID. Shows the full journey of a transaction across agents. |
| `a2a_tracing_metrics` | read | Get tracing metrics: p50/p95/p99 latency, error rate, throughput, span count. |
| `a2a_recent_spans` | read | Get the most recent trace spans for debugging. |
| `a2a_export_traces` | read | Export all buffered spans in OpenTelemetry-compatible OTLP JSON format. |
| `a2a_agent_dashboard` | read | Get a full operational dashboard for an agent: runtime status, budget, recent budget alerts, tick metrics, and rail-aware economics. |
| `a2a_agent_decisions` | read | Get recent strategy decisions for an agent: what was accepted/rejected and why. |
| `a2a_agent_performance` | read | Get performance report with optional rail-aware economics context: quote accept rate, response time, settlement success rate, dispute rate, filtered payment metrics, and recent budget alert activity. |
| `a2a_agent_tick_metrics` | read | Get tick loop metrics: avg duration, ticks/min, quotes evaluated, payments executed, errors. |
| `a2a_agent_lifecycle` | read | Get agent lifecycle history: start/stop/pause/resume events with timestamps and reasons. |
| `a2a_agent_alerts` | read | List recent budget and settlement alerts for an agent, with optional category, time window, and payment-rail filters. |
| `a2a_settlement_status` | read | Get settlement finality status: broadcast → unconfirmed → confirming → final. Shows confirmation count vs chain requirement. |
| `a2a_settlement_pending` | read | List all settlements not yet final — awaiting blockchain confirmations. |
| `a2a_settlement_finality_metrics` | read | Get settlement metrics: avg confirmation time, finality rate, reorg count. |
| `a2a_handshake` | read | Initiate capability handshake with another agent. Returns compatibility report: shared networks/assets, feature mismatches, recommended network/asset. |
| `a2a_my_capabilities` | read | Get this agent's capability manifest for protocol handshake. |

## a2a-platform

| Tool | Permission | Description |
| --- | --- | --- |
| `a2a_send_message` | write | Send a direct message to another agent. Supports text, task delegation, and status queries. |
| `a2a_get_inbox` | read | Get your message inbox. Filter by unread, type, or limit. |
| `a2a_delegate_task` | write | Delegate a task to another agent. Specify description, deadline, reward, and priority. |
| `a2a_respond_to_task` | write | Respond to a delegated task: accept, reject, or mark complete. |
| `a2a_get_thread` | read | Get all messages in a conversation thread. |
| `a2a_messaging_metrics` | read | Get messaging metrics: total messages, unread count, avg response time. |
| `a2a_batch_pay` | write | Execute multiple payments in one call. Each payment is independent — one failure doesn't block others. |
| `a2a_batch_request_quotes` | write | Request quotes from multiple sellers simultaneously. |
| `a2a_save_checkpoint` | write | Save agent state checkpoint for recovery after restart. |
| `a2a_load_checkpoint` | read | Load last saved agent state checkpoint. |
| `a2a_list_checkpoints` | read | List all saved agent checkpoints. |
| `a2a_export_agent_data` | read | Export all commerce data for an agent: payments, quotes, escrows, disputes, subscriptions. |
| `a2a_commerce_report` | read | Generate a commerce report for an agent: per-rail volume, transactions, dispute rate, top counterparties, and margin. |
| `a2a_data_stats` | read | Get row counts for all A2A data tables. |
| `a2a_verify_webhook` | read | Verify a received webhook signature. Use this to validate incoming StateSet webhooks. |
| `a2a_tick_metrics` | read | Get tick loop performance metrics: p50/p95/p99 duration, ticks/min, idle streaks, adaptive interval. |

## a2a-intelligence

| Tool | Permission | Description |
| --- | --- | --- |
| `a2a_schedule_action` | write | Schedule a future action: "pay in 3 days", "check escrow every hour", "remind me to follow up". |
| `a2a_cancel_scheduled` | write | Cancel a scheduled action by ID. |
| `a2a_list_scheduled` | read | List scheduled actions. Filter by status or action type. |
| `a2a_scheduler_metrics` | read | Get scheduler metrics: total scheduled, executed, failed, pending, recurring. |
| `a2a_remember_interaction` | write | Record an interaction with a counterparty so the agent learns their patterns over time. |
| `a2a_counterparty_profile` | read | Get learned profile of a counterparty: success rate, reliability, risk level, negotiation patterns. |
| `a2a_should_transact` | read | Get AI recommendation on whether to transact with a counterparty, based on learned history. |
| `a2a_agent_insights` | read | Get aggregate insights: total counterparties, avg success rate, top performers, risk alerts. |
| `a2a_top_counterparties` | read | Get top counterparties ranked by volume, success rate, or reliability. |
| `a2a_add_rule` | write | Add a programmable guardrail rule. Example: "block transactions > $1000 without escrow". |
| `a2a_evaluate_rules` | read | Evaluate all active rules against a transaction context. Returns: allowed, matched rules, explanation. |
| `a2a_list_rules` | read | List all registered rules. Filter by tags or enabled status. |
| `a2a_rule_audit_log` | read | Get recent rule evaluation audit log — see which rules fired and why. |
| `a2a_scatter` | write | Broadcast a task to multiple agents in parallel (fan-out). Returns coordination ID for tracking. |
| `a2a_coordination_status` | read | Get status of a fan-out coordination: responses received, pending, timed out. |
| `a2a_submit_response` | write | Submit a response to a fan-out coordination (as a target agent). |
| `a2a_join_results` | read | Wait for and aggregate fan-out results based on the join strategy. |

## quality

| Tool | Permission | Description |
| --- | --- | --- |
| `list_inspections` | read | List quality inspections. |
| `get_inspection` | read | Get a quality inspection by ID. |
| `create_inspection` | write | Create a quality inspection. |
| `start_inspection` | write | Start a quality inspection. |
| `complete_inspection` | write | Complete a quality inspection. |
| `list_ncrs` | read | List non-conformance reports. |
| `get_ncr` | read | Get a non-conformance report by ID. |
| `create_ncr` | write | Create a non-conformance report. |
| `close_ncr` | write | Close a non-conformance report. |
| `list_quality_holds` | read | List quality holds. |
| `get_quality_hold` | read | Get a quality hold by ID. |
| `create_quality_hold` | write | Create a quality hold. |
| `release_quality_hold` | write | Release a quality hold. |
| `list_active_quality_holds` | read | List active quality holds. |
| `count_active_quality_holds` | read | Count active quality holds. |

## lots

| Tool | Permission | Description |
| --- | --- | --- |
| `list_lots` | read | List lots. |
| `get_lot` | read | Get a lot by ID or lot number. |
| `create_lot` | write | Create a lot. |
| `list_active_lots` | read | List active lots for a SKU. |
| `list_available_lots_for_sku` | read | List available lots for a SKU in FIFO order. |
| `quarantine_lot` | write | Quarantine a lot. |
| `release_lot_quarantine` | write | Release a lot from quarantine. |
| `list_expiring_lots` | read | List lots expiring within a number of days. |
| `list_expired_lots` | read | List expired lots. |
| `list_quarantined_lots` | read | List quarantined lots. |
| `count_lots` | read | Count lots. |

## search-config

| Tool | Permission | Description |
| --- | --- | --- |
| `list_search_configs` | read | List search configurations. |
| `get_search_config` | read | Get a search configuration by ID. |
| `get_active_search_config` | read | Get the currently active search configuration. |
| `create_search_config` | write | Create a search configuration. |
| `update_search_config` | write | Update a search configuration. Collection fields replace the existing values. |
| `set_active_search_config` | write | Make a search configuration active, deactivating the current one. |
| `delete_search_config` | write | Delete a search configuration. |

## serials

| Tool | Permission | Description |
| --- | --- | --- |
| `list_serials` | read | List serial numbers. |
| `get_serial` | read | Get a serial by ID or serial string. |
| `create_serial` | write | Create a serial number. |
| `list_available_serials` | read | List available serials for a SKU. |
| `mark_serial_sold` | write | Mark a serial number as sold. |
| `quarantine_serial` | write | Quarantine a serial number. |
| `check_serial_availability` | read | Check whether a serial string is available. |
| `count_serials` | read | Count serial numbers. |

## warehouse

| Tool | Permission | Description |
| --- | --- | --- |
| `list_warehouses` | read | List warehouses. |
| `get_warehouse` | read | Get a warehouse by ID or code. |
| `create_warehouse` | write | Create a warehouse. |
| `create_location` | write | Create a warehouse location. |
| `get_location` | read | Get a warehouse location by ID. |
| `list_locations` | read | List warehouse locations. |
| `list_pickable_locations` | read | List pickable locations for a SKU in a warehouse. |
| `get_warehouse_sku_available_quantity` | read | Get total available quantity for a SKU in a warehouse. |
| `count_warehouses` | read | Count warehouses. |

## receiving

| Tool | Permission | Description |
| --- | --- | --- |
| `list_receipts` | read | List receipts. |
| `get_receipt` | read | Get a receipt by ID or receipt number. |
| `create_receipt` | write | Create a receipt. |
| `create_receipt_from_purchase_order` | write | Create a receipt from a purchase order. |
| `start_receiving` | write | Start receiving against a receipt. |
| `complete_receiving` | write | Complete receiving against a receipt. |
| `cancel_receipt` | write | Cancel a receipt. |
| `count_receipts` | read | Count receipts. |

## fulfillment

| Tool | Permission | Description |
| --- | --- | --- |
| `list_fulfillment_waves` | read | List fulfillment waves. |
| `get_fulfillment_wave` | read | Get a fulfillment wave by ID. |
| `create_fulfillment_wave` | write | Create a fulfillment wave. |
| `release_fulfillment_wave` | write | Release a fulfillment wave for picking. |
| `complete_fulfillment_wave` | write | Complete a fulfillment wave. |
| `cancel_fulfillment_wave` | write | Cancel a fulfillment wave. |
| `list_pick_tasks` | read | List pick tasks. |
| `get_pick_task` | read | Get a pick task by ID. |
| `assign_pick_task` | write | Assign a pick task. |
| `start_pick_task` | write | Start a pick task. |
| `cancel_pick_task` | write | Cancel a pick task. |
| `check_order_ready_to_pack` | read | Check whether an order is ready to pack. |
| `check_order_ready_to_ship` | read | Check whether an order is ready to ship. |
| `count_fulfillment_waves` | read | Count fulfillment waves. |

## accounts-payable

| Tool | Permission | Description |
| --- | --- | --- |
| `list_bills` | read | List accounts payable bills. |
| `get_bill` | read | Get a bill by ID or bill number. |
| `create_bill` | write | Create an accounts payable bill. |
| `approve_bill` | write | Approve a bill. |
| `cancel_bill` | write | Cancel a bill. |
| `list_overdue_bills` | read | List overdue bills. |
| `list_bills_due_soon` | read | List bills due soon. |
| `get_accounts_payable_aging_summary` | read | Get the accounts payable aging summary. |
| `get_accounts_payable_total_outstanding` | read | Get the total accounts payable outstanding balance. |
| `three_way_match_bill` | read | Run a three-way match (bill vs purchase order vs receipt) for a bill. |
| `count_accounts_payable_bills` | read | Count accounts payable bills. |

## accounts-receivable

| Tool | Permission | Description |
| --- | --- | --- |
| `get_accounts_receivable_aging_summary` | read | Get the accounts receivable aging summary. |
| `get_accounts_receivable_total_outstanding` | read | Get the total accounts receivable outstanding balance. |
| `get_days_sales_outstanding` | read | Get days sales outstanding over a rolling window. |
| `list_credit_memos` | read | List credit memos. |
| `get_credit_memo` | read | Get a credit memo by ID. |
| `create_credit_memo` | write | Create a credit memo. |
| `void_credit_memo` | write | Void a credit memo. |
| `list_unapplied_credits` | read | List unapplied credits for a customer. |

## cost-accounting

| Tool | Permission | Description |
| --- | --- | --- |
| `list_item_costs` | read | List item costs. |
| `get_item_cost` | read | Get item cost for a SKU. |
| `set_item_cost` | write | Set item cost inputs for a SKU. |
| `update_average_item_cost` | write | Update average cost for a SKU from a quantity and unit cost. |
| `get_total_inventory_value` | read | Get total inventory value. |

## credit

| Tool | Permission | Description |
| --- | --- | --- |
| `list_credit_accounts` | read | List credit accounts. |
| `get_credit_account` | read | Get a credit account by account ID or customer ID. |
| `create_credit_account` | write | Create a customer credit account. |
| `check_customer_credit` | read | Check customer credit availability for an order amount. |
| `adjust_credit_limit` | write | Adjust a customer credit limit. |
| `suspend_credit_account` | write | Suspend a customer credit account. |
| `reactivate_credit_account` | write | Reactivate a customer credit account. |
| `list_over_limit_credit_accounts` | read | List over-limit credit accounts. |

## backorders

| Tool | Permission | Description |
| --- | --- | --- |
| `list_backorders` | read | List backorders. |
| `get_backorder` | read | Get a backorder by ID or backorder number. |
| `create_backorder` | write | Create a backorder. |
| `cancel_backorder` | write | Cancel a backorder. |
| `list_backorders_for_order` | read | List backorders for an order. |
| `list_backorders_for_sku` | read | List backorders for a SKU. |
| `list_overdue_backorders` | read | List overdue backorders. |
| `get_backorder_summary` | read | Get the backorder summary. |
| `count_pending_backorders` | read | Count pending backorders. |

## general-ledger

| Tool | Permission | Description |
| --- | --- | --- |
| `list_gl_accounts` | read | List general ledger accounts. |
| `get_gl_account` | read | Get a general ledger account by ID or account number. |
| `create_gl_account` | write | Create a general ledger account. |
| `initialize_chart_of_accounts` | write | Initialize the standard chart of accounts. |
| `list_journal_entries` | read | List journal entries. |
| `get_journal_entry` | read | Get a journal entry by ID. |
| `post_journal_entry` | write | Post a journal entry. |
| `void_journal_entry` | write | Void a journal entry. |
| `get_trial_balance` | read | Get the trial balance as of a date. |
| `get_balance_sheet` | read | Get the balance sheet as of a date. |
| `get_income_statement` | read | Get the income statement for a date range. |
| `revalue_gl` | write | Revalue foreign-currency general ledger balances as of a date. |
| `close_month` | write | Close the month: post scheduled depreciation, recognize revenue through period end, revalue foreign-currency balances, then run the period close. Use dryRun to preview per-step counts and amounts without writing. |
| `create_gl_period` | write | Create an accounting period. |
| `list_gl_periods` | read | List accounting periods with optional filtering. |
| `open_gl_period` | write | Open an accounting period so journal entries can be posted to it. |
| `get_gl_account_balance` | read | Get the balance of a general ledger account. |

## agent-receipt

| Tool | Permission | Description |
| --- | --- | --- |
| `agent_receipt_purchase` | write | Execute a verifiable agent-to-agent purchase end-to-end: buyer agent locks ssUSD in OrderEscrow, sequencer commits VES events, STARK proof attests order_total ≤ policy cap, SetRegistry anchors the commitment + proof on Set Chain L2, buyer marks delivered, seller releases. Returns the signed Agent Receipt JSON with on-chain tx hashes. Requires the local stack (anvil + sequencer + postgres + deployed contracts) to be running — see /home/dom/icommerce-app/setup.sh. |
| `agent_receipt_status` | read | Read the on-chain escrow state for an order. Returns buyer, seller, amount, deadlines, delivery receipt hash, and current status (None / Locked / Delivered / Disputed / Released / Refunded). |
| `agent_receipt_dispute` | write | Buyer raises an on-chain dispute on a Delivered order. Funds freeze in escrow until the operator resolves. The plain-text reason is hashed (keccak256) and stored on-chain as proof of the filing. |
| `agent_receipt_resolve` | admin | Operator (sequencer / arbiter) resolves a Disputed order. Routes the locked funds either to the seller (in_favor_of_seller=true) or refunds the buyer (false). Emits DisputeResolved + Released/Refunded. |
| `agent_receipt_fx_quote` | read | Read a fresh FX quote from the on-chain FxOracle and convert an amount between currencies. Pair format: "BASE/QUOTE", e.g. "EUR/ssUSD" or "JPY/ssUSD". Returns the rate, freshness, and the converted amount. Use this BEFORE locking funds so the agent can verify the rate is fresh and within expected bounds. Pre-seeded pairs at deploy time: EUR/ssUSD, GBP/ssUSD, JPY/ssUSD, MXN/ssUSD. |
| `agent_receipt_merchant_statement` | read | Aggregate every emitted receipt in a directory into a single platform settlement statement: GMV, marketplace fees earned, FX exposure by currency, dispute outcomes, compliance bundle counts, and a sampled on-chain audit pass rate. Optional filters scope the statement to a date range, a specific seller wallet, or a specific buyer wallet — enabling multi-tenant accounting on a single OrderEscrow contract. |
| `agent_receipt_request_payout` | write | Initiate a fiat payout from the seller's SSDC balance to their bank via the off-ramp bridge. Auto-handles SSDC.approve idempotently, signs a canonical payout-request message with the seller's wallet key, POSTs the signed request to the bridge, and returns a Stripe-Treasury-shaped OutboundPayment intent. Requires bridge running on http://localhost:4243 (or BRIDGE_PAYOUT_URL env). |
| `agent_receipt_audit` | read | Independently audit a StateSet commerce receipt against the live chain. Re-verifies on-chain claims (escrow status, registry batch commitment, STARK proof metadata) and — for compliance bundles — runs the Winterfell verifier on every policy proof. Returns a structured pass/fail summary the calling agent can act on. The strongest audit primitive in the stack: any agent can verify any receipt without trusting the producer. |
| `agent_receipt_sweep_yield` | admin | Operator/marketplace sweeps the rebasing yield surplus held by OrderEscrow to a recipient. With the production SSDC stablecoin, this is the T-Bill yield earned by escrowed funds while orders were in flight — a programmable platform revenue stream alongside any BPS fee. Read first via yield_available; positive amount returns the sweep tx, otherwise a no-op. |
| `agent_receipt_refund` | write | Buyer recovers locked funds after the order's deliveryDeadline has expired. No dispute, no operator, no platform — purely the safety property of the OrderEscrow primitive. Reverts with DeadlineNotReached if the deadline has not yet passed. |
| `agent_receipt_release` | write | Seller pulls escrowed funds after delivery + confirmation window. Use this when agent_receipt_purchase was called with skip_release=true and there has been no dispute. Routes funds to the seller wallet. |

## fixed-assets

| Tool | Permission | Description |
| --- | --- | --- |
| `list_fixed_assets` | read | List fixed assets. |
| `get_fixed_asset` | read | Get a fixed asset by ID. |
| `create_fixed_asset` | write | Create a fixed asset. |
| `place_asset_in_service` | write | Place a fixed asset in service. |
| `dispose_fixed_asset` | write | Dispose of a fixed asset. |
| `write_off_fixed_asset` | write | Write off a fixed asset. |
| `generate_depreciation_schedule` | write | Generate the depreciation schedule for a fixed asset. |
| `get_depreciation_schedule` | read | Get the depreciation schedule for a fixed asset. |
| `post_depreciation` | write | Post depreciation for a period. |

## revenue-recognition

| Tool | Permission | Description |
| --- | --- | --- |
| `list_revenue_contracts` | read | List revenue recognition contracts. |
| `get_revenue_contract` | read | Get a revenue recognition contract by ID. |
| `create_revenue_contract` | write | Create a revenue recognition contract. |
| `generate_revenue_schedule` | write | Generate the revenue recognition schedule for a contract. |
| `get_revenue_schedule` | read | Get the revenue recognition schedule for a contract. |
| `recognize_revenue` | write | Recognize revenue for a contract period. |

## cycle-counts

| Tool | Permission | Description |
| --- | --- | --- |
| `list_cycle_counts` | read | List cycle counts. |
| `get_cycle_count` | read | Get a cycle count by ID. |
| `create_cycle_count` | write | Create a cycle count. |
| `start_cycle_count` | write | Start a cycle count. |
| `record_cycle_counts` | write | Record counted quantities for a cycle count. |
| `complete_cycle_count` | write | Complete a cycle count. |
| `cancel_cycle_count` | write | Cancel a cycle count. |

## edi-documents

| Tool | Permission | Description |
| --- | --- | --- |
| `list_edi_documents` | read | List EDI documents with optional filtering. |
| `get_edi_document` | read | Get an EDI document by ID. |
| `create_edi_document` | write | Create / ingest an EDI document. |
| `set_edi_document_status` | write | Update the status of an EDI document. |
| `get_edi_summary` | read | Get an aggregate summary of EDI documents (counts by status and type). |

## prepayments

| Tool | Permission | Description |
| --- | --- | --- |
| `check_prepayments_supported` | read | Check whether the prepayments backend is available on this engine build. |
| `list_prepayments` | read | List supplier prepayments with optional filtering. |
| `get_prepayment` | read | Get a prepayment by ID. |
| `create_prepayment` | write | Create a supplier prepayment. |
| `apply_prepayment` | write | Apply a prepayment against a bill or payment obligation. |
| `list_prepayment_applications` | read | List applications for a prepayment. |
| `reverse_prepayment_application` | write | Reverse a previously-recorded prepayment application. |
| `refund_prepayment` | write | Refund the remaining balance of a prepayment, closing it. |

## activity-logs

| Tool | Permission | Description |
| --- | --- | --- |
| `check_activity_logs_supported` | read | Check whether the activity-logs backend is available on this engine build. |
| `list_activity_logs` | read | List activity log entries with optional filtering. |
| `get_activity_log` | read | Get an activity log entry by ID. |
| `get_activity_history_for_subject` | read | Get the activity history for a subject (e.g. an order or product). |
| `record_activity` | write | Record an activity log entry for a subject. |

## channels

| Tool | Permission | Description |
| --- | --- | --- |
| `check_channels_supported` | read | Check whether the channels backend is available on this engine build. |
| `list_channels` | read | List sales channels with optional filtering. |
| `get_channel` | read | Get a sales channel by ID. |
| `create_channel` | write | Create a sales channel. |
| `update_channel` | write | Update a sales channel. |
| `set_channel_lock` | write | Lock or unlock a sales channel for API writes. |
| `list_channel_product_mappings` | read | List product mappings for a sales channel. |
| `delete_channel` | write | Delete a sales channel. |

## companies

| Tool | Permission | Description |
| --- | --- | --- |
| `check_companies_supported` | read | Check whether the companies backend is available on this engine build. |
| `list_companies` | read | List B2B companies with optional filtering. |
| `get_company` | read | Get a company by ID. |
| `create_company` | write | Create a B2B company. |
| `update_company` | write | Update a B2B company. |
| `list_company_addresses` | read | List shipping addresses for a company. |
| `list_company_contacts` | read | List contacts for a company. |
| `create_company_contact` | write | Create a contact linked to one or more companies. |
| `delete_company` | write | Delete a B2B company. |

## vendor-credits

| Tool | Permission | Description |
| --- | --- | --- |
| `check_vendor_credits_supported` | read | Check whether the vendor-credits backend is available on this engine build. |
| `list_vendor_credits` | read | List vendor credits with optional filtering. |
| `get_vendor_credit` | read | Get a vendor credit by ID. |
| `create_vendor_credit` | write | Create a vendor credit. |
| `apply_vendor_credit` | write | Apply a vendor credit against a bill or payment obligation. |
| `list_vendor_credit_applications` | read | List applications for a vendor credit. |
| `reverse_vendor_credit_application` | write | Reverse a previously-recorded vendor credit application. |
| `cancel_vendor_credit` | write | Cancel a vendor credit. |

## price-schedules

| Tool | Permission | Description |
| --- | --- | --- |
| `check_price_schedules_supported` | read | Check whether the price-schedules backend is available on this engine build. |
| `list_price_schedules` | read | List price schedules with optional filtering. |
| `get_price_schedule` | read | Get a price schedule by ID. |
| `create_price_schedule` | write | Create a price schedule. |
| `update_price_schedule` | write | Update a price schedule. |
| `delete_price_schedule` | write | Delete a price schedule and its entries. |
| `set_price_schedule_entry` | write | Upsert a per-product scheduled price on a price schedule. |
| `delete_price_schedule_entry` | write | Remove a per-product entry from a price schedule. |
| `list_price_schedule_entries` | read | List per-product entries for a price schedule. |
| `resolve_scheduled_price` | read | Resolve the effective scheduled price for a product at an instant (defaults to now). |

## price-levels

| Tool | Permission | Description |
| --- | --- | --- |
| `check_price_levels_supported` | read | Check whether the price-levels backend is available on this engine build. |
| `list_price_levels` | read | List price levels with optional filtering. |
| `get_price_level` | read | Get a price level by ID. |
| `create_price_level` | write | Create a price level. |
| `update_price_level` | write | Update a price level. |
| `delete_price_level` | write | Delete a price level and its entries. |
| `set_price_level_entry` | write | Upsert a per-product fixed price entry on a price level. |
| `delete_price_level_entry` | write | Remove a per-product entry from a price level. |
| `list_price_level_entries` | read | List per-product entries for a price level. |

## transfer-orders

| Tool | Permission | Description |
| --- | --- | --- |
| `check_transfer_orders_supported` | read | Check whether the transfer-orders backend is available on this engine build. |
| `list_transfer_orders` | read | List transfer orders with optional filtering. |
| `get_transfer_order` | read | Get a transfer order by ID. |
| `create_transfer_order` | write | Create a transfer order between warehouses. |
| `ship_transfer_order` | write | Mark a transfer order as shipped from the source warehouse. |
| `receive_transfer_order_line` | write | Receive a quantity against a transfer order line at the destination. |
| `cancel_transfer_order` | write | Cancel a transfer order. |

## production-batches

| Tool | Permission | Description |
| --- | --- | --- |
| `check_production_batches_supported` | read | Check whether the production-batches backend is available on this engine build. |
| `list_production_batches` | read | List production batches with optional filtering. |
| `get_production_batch` | read | Get a production batch by ID. |
| `create_production_batch` | write | Create a production batch. |
| `update_production_batch` | write | Update a production batch. |
| `delete_production_batch` | write | Delete a production batch. |
| `add_production_batch_work_orders` | write | Link work orders to a production batch. |
| `remove_production_batch_work_order` | write | Remove a work order from a production batch. |

## supplier-skus

| Tool | Permission | Description |
| --- | --- | --- |
| `check_supplier_skus_supported` | read | Check whether the supplier-SKUs backend is available on this engine build. |
| `list_supplier_skus` | read | List supplier SKUs with optional filtering. |
| `get_supplier_sku` | read | Get a supplier SKU by ID. |
| `create_supplier_sku` | write | Create a supplier SKU cross-reference. |
| `update_supplier_sku` | write | Update a supplier SKU. |
| `delete_supplier_sku` | write | Delete a supplier SKU. |
| `bulk_upsert_supplier_skus` | write | Bulk upsert supplier SKUs for a supplier, keyed by internal product. |

## inbound-shipments

| Tool | Permission | Description |
| --- | --- | --- |
| `check_inbound_shipments_supported` | read | Check whether the inbound-shipments backend is available on this engine build. |
| `list_inbound_shipments` | read | List inbound shipments with optional filtering. |
| `get_inbound_shipment` | read | Get an inbound shipment by ID. |
| `create_inbound_shipment` | write | Create an inbound shipment. |
| `mark_inbound_shipment_in_transit` | write | Mark an inbound shipment as in transit. |
| `mark_inbound_shipment_arrived` | write | Mark an inbound shipment as arrived. |
| `receive_inbound_shipment_line` | write | Receive a quantity against an inbound shipment line. |
| `cancel_inbound_shipment` | write | Cancel an inbound shipment. |
