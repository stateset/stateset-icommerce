# MCP Tool Inventory

This page is generated from the live CLI registry in `cli/src/tools/index.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_mcp_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/mcp-tool-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Total tools | 340 |
| Loaded modules | 32 |
| Read tools | 164 |
| Write tools | 132 |
| Delete tools | 11 |
| Admin tools | 33 |

## Module Counts

| Module | Tools |
| --- | --- |
| a2a | 59 |
| agent-cards | 5 |
| analytics | 10 |
| carts | 14 |
| catalog | 6 |
| checkout | 8 |
| circuit-breaker | 8 |
| compliance | 6 |
| connectors | 11 |
| currency | 8 |
| custom-objects | 12 |
| customers | 3 |
| erc8004 | 5 |
| inventory | 6 |
| invoices | 5 |
| manufacturing | 11 |
| orders | 6 |
| payments | 17 |
| products | 4 |
| promotions | 10 |
| proofs | 7 |
| returns | 5 |
| shipments | 11 |
| stablecoin | 4 |
| subscriptions | 15 |
| suppliers | 6 |
| sync | 20 |
| tax | 19 |
| treasury | 6 |
| vector | 16 |
| warranties | 4 |
| x402 | 13 |

## Permission Counts

| Permission | Tools |
| --- | --- |
| admin | 33 |
| delete | 11 |
| read | 164 |
| write | 132 |

## Tool Registry

| Tool | Module | Permission |
| --- | --- | --- |
| `a2a_accept_quote` | `a2a` | `write` |
| `a2a_cancel_agent_subscription` | `a2a` | `write` |
| `a2a_check_payment_conditions` | `a2a` | `read` |
| `a2a_configure_webhooks` | `a2a` | `write` |
| `a2a_counter_quote` | `a2a` | `write` |
| `a2a_create_agent_subscription` | `a2a` | `write` |
| `a2a_create_conditional_payment` | `a2a` | `write` |
| `a2a_create_escrow` | `a2a` | `write` |
| `a2a_create_split_payment` | `a2a` | `write` |
| `a2a_decline_quote` | `a2a` | `write` |
| `a2a_discover_agents` | `a2a` | `read` |
| `a2a_dispute_escrow` | `a2a` | `write` |
| `a2a_dlq_count` | `a2a` | `read` |
| `a2a_execute_split_payment` | `a2a` | `write` |
| `a2a_file_dispute` | `a2a` | `write` |
| `a2a_fulfill_quote` | `a2a` | `write` |
| `a2a_fund_escrow` | `a2a` | `write` |
| `a2a_get_agent_subscription` | `a2a` | `read` |
| `a2a_get_balance` | `a2a` | `read` |
| `a2a_get_dispute` | `a2a` | `read` |
| `a2a_get_escrow` | `a2a` | `read` |
| `a2a_get_event_history` | `a2a` | `read` |
| `a2a_get_payment` | `a2a` | `read` |
| `a2a_get_reputation` | `a2a` | `read` |
| `a2a_get_service` | `a2a` | `read` |
| `a2a_get_split_payment` | `a2a` | `read` |
| `a2a_list_agent_subscriptions` | `a2a` | `read` |
| `a2a_list_disputes` | `a2a` | `read` |
| `a2a_list_escrows` | `a2a` | `read` |
| `a2a_list_event_subscriptions` | `a2a` | `read` |
| `a2a_list_notification_log` | `a2a` | `read` |
| `a2a_list_payment_requests` | `a2a` | `read` |
| `a2a_list_payments` | `a2a` | `read` |
| `a2a_list_quotes` | `a2a` | `read` |
| `a2a_list_services` | `a2a` | `read` |
| `a2a_list_split_payments` | `a2a` | `read` |
| `a2a_list_webhook_dlq` | `a2a` | `admin` |
| `a2a_pause_agent_subscription` | `a2a` | `write` |
| `a2a_pay` | `a2a` | `write` |
| `a2a_pay_request` | `a2a` | `write` |
| `a2a_process_subscription_billing` | `a2a` | `write` |
| `a2a_provide_quote` | `a2a` | `write` |
| `a2a_purge_dlq` | `a2a` | `admin` |
| `a2a_quarantine_failed_webhooks` | `a2a` | `admin` |
| `a2a_rate_agent` | `a2a` | `write` |
| `a2a_refund_escrow` | `a2a` | `write` |
| `a2a_register_service` | `a2a` | `write` |
| `a2a_release_escrow` | `a2a` | `write` |
| `a2a_replay_dlq_entry` | `a2a` | `admin` |
| `a2a_request_payment` | `a2a` | `write` |
| `a2a_request_quote` | `a2a` | `write` |
| `a2a_resolve_dispute` | `a2a` | `write` |
| `a2a_respond_to_feedback` | `a2a` | `write` |
| `a2a_resume_agent_subscription` | `a2a` | `write` |
| `a2a_revise_quote` | `a2a` | `write` |
| `a2a_send_notification` | `a2a` | `write` |
| `a2a_settle_conditional_payment` | `a2a` | `write` |
| `a2a_submit_evidence` | `a2a` | `write` |
| `a2a_subscribe_events` | `a2a` | `write` |
| `abandon_cart` | `carts` | `write` |
| `activate_bom` | `manufacturing` | `write` |
| `activate_promotion` | `promotions` | `write` |
| `activate_subscription_plan` | `subscriptions` | `write` |
| `add_bom_component` | `manufacturing` | `write` |
| `add_cart_item` | `carts` | `write` |
| `adjust_inventory` | `inventory` | `write` |
| `agent_get_all_breaker_states` | `circuit-breaker` | `read` |
| `agent_get_breaker_state` | `circuit-breaker` | `read` |
| `agent_get_spending_summary` | `circuit-breaker` | `read` |
| `agent_instant_checkout` | `checkout` | `write` |
| `agent_key_export` | `sync` | `read` |
| `agent_key_generate` | `sync` | `write` |
| `agent_key_info` | `sync` | `read` |
| `agent_key_list` | `sync` | `read` |
| `agent_key_rotate` | `sync` | `write` |
| `agent_reset_all_breakers` | `circuit-breaker` | `admin` |
| `agent_reset_breaker` | `circuit-breaker` | `admin` |
| `agent_set_spending_limits` | `circuit-breaker` | `admin` |
| `agent_trip_all_breakers` | `circuit-breaker` | `admin` |
| `agent_trip_breaker` | `circuit-breaker` | `admin` |
| `apply_cart_discount` | `carts` | `write` |
| `apply_cart_promotions` | `promotions` | `write` |
| `approve_purchase_order` | `suppliers` | `write` |
| `approve_return` | `returns` | `write` |
| `approve_warranty_claim` | `warranties` | `write` |
| `archive_subscription_plan` | `subscriptions` | `delete` |
| `assess_wasm_connector_safety` | `connectors` | `read` |
| `calculate_cart_tax` | `tax` | `read` |
| `calculate_tax` | `tax` | `read` |
| `calculate_tax_quote` | `tax` | `read` |
| `calculate_tax_quote_with_failover` | `tax` | `read` |
| `cancel_cart` | `carts` | `delete` |
| `cancel_order` | `orders` | `delete` |
| `cancel_payment_intent` | `payments` | `delete` |
| `cancel_subscription` | `subscriptions` | `delete` |
| `cancel_work_order` | `manufacturing` | `delete` |
| `capture_payment_intent` | `payments` | `write` |
| `certify_wasm_connector` | `connectors` | `admin` |
| `checkout_with_crypto` | `checkout` | `write` |
| `commit_tax_transaction` | `tax` | `write` |
| `complete_checkout` | `carts` | `write` |
| `complete_payment` | `payments` | `write` |
| `complete_work_order` | `manufacturing` | `write` |
| `compliance_summary` | `compliance` | `read` |
| `confirm_reservation` | `inventory` | `write` |
| `convert_currency` | `currency` | `read` |
| `create_bom` | `manufacturing` | `write` |
| `create_cart` | `carts` | `write` |
| `create_coupon` | `promotions` | `write` |
| `create_custom_object` | `custom-objects` | `write` |
| `create_custom_object_type` | `custom-objects` | `write` |
| `create_customer` | `customers` | `write` |
| `create_inventory_item` | `inventory` | `write` |
| `create_invoice` | `invoices` | `write` |
| `create_order` | `orders` | `write` |
| `create_payment` | `payments` | `write` |
| `create_payment_intent` | `payments` | `write` |
| `create_payment_link` | `checkout` | `write` |
| `create_payment_settlement_batch` | `payments` | `write` |
| `create_product` | `products` | `write` |
| `create_promotion` | `promotions` | `write` |
| `create_purchase_order` | `suppliers` | `write` |
| `create_refund` | `payments` | `write` |
| `create_return` | `returns` | `write` |
| `create_shipment` | `shipments` | `write` |
| `create_shipping_label` | `shipments` | `write` |
| `create_stablecoin_payment` | `stablecoin` | `write` |
| `create_subscription` | `subscriptions` | `write` |
| `create_subscription_plan` | `subscriptions` | `write` |
| `create_supplier` | `suppliers` | `write` |
| `create_tax_exemption` | `tax` | `write` |
| `create_warranty` | `warranties` | `write` |
| `create_warranty_claim` | `warranties` | `write` |
| `create_work_order` | `manufacturing` | `write` |
| `deactivate_promotion` | `promotions` | `write` |
| `delete_custom_object` | `custom-objects` | `delete` |
| `delete_custom_object_type` | `custom-objects` | `delete` |
| `delete_gdpr_data` | `compliance` | `admin` |
| `deliver_shipment` | `shipments` | `write` |
| `discover_agents` | `agent-cards` | `read` |
| `enable_currencies` | `currency` | `admin` |
| `erc8004_get_by_wallet` | `erc8004` | `read` |
| `erc8004_get_identity` | `erc8004` | `read` |
| `erc8004_link_wallet` | `erc8004` | `write` |
| `erc8004_list_identities` | `erc8004` | `read` |
| `erc8004_register_identity` | `erc8004` | `admin` |
| `execute_wasm_connector` | `connectors` | `write` |
| `export_agent_catalog` | `catalog` | `read` |
| `export_audit_trail` | `compliance` | `admin` |
| `export_compliance_package` | `proofs` | `read` |
| `export_gdpr_data` | `compliance` | `admin` |
| `express_checkout` | `checkout` | `write` |
| `format_currency` | `currency` | `read` |
| `generate_1099k` | `compliance` | `admin` |
| `generate_inclusion_proof` | `proofs` | `read` |
| `generate_receipt_bundle` | `proofs` | `read` |
| `get_abandoned_carts` | `carts` | `read` |
| `get_active_promotions` | `promotions` | `read` |
| `get_agent_card` | `agent-cards` | `read` |
| `get_agent_wallet` | `stablecoin` | `read` |
| `get_billing_cycle` | `subscriptions` | `read` |
| `get_bom` | `manufacturing` | `read` |
| `get_cart` | `carts` | `read` |
| `get_currency_settings` | `currency` | `read` |
| `get_custom_object` | `custom-objects` | `read` |
| `get_custom_object_by_handle` | `custom-objects` | `read` |
| `get_custom_object_type` | `custom-objects` | `read` |
| `get_custom_object_type_by_handle` | `custom-objects` | `read` |
| `get_customer` | `customers` | `read` |
| `get_customer_metrics` | `analytics` | `read` |
| `get_customer_tax_exemptions` | `tax` | `read` |
| `get_demand_forecast` | `analytics` | `read` |
| `get_exchange_rate` | `currency` | `read` |
| `get_installed_connector` | `connectors` | `read` |
| `get_inventory_health` | `analytics` | `read` |
| `get_low_stock_items` | `analytics` | `read` |
| `get_order` | `orders` | `read` |
| `get_order_status_breakdown` | `analytics` | `read` |
| `get_overdue_invoices` | `invoices` | `read` |
| `get_payment` | `payments` | `read` |
| `get_payment_intent` | `payments` | `read` |
| `get_payment_link_status` | `checkout` | `read` |
| `get_product` | `products` | `read` |
| `get_product_spec` | `catalog` | `read` |
| `get_product_variant` | `products` | `read` |
| `get_promotion` | `promotions` | `read` |
| `get_return` | `returns` | `read` |
| `get_return_metrics` | `analytics` | `read` |
| `get_revenue_forecast` | `analytics` | `read` |
| `get_sales_summary` | `analytics` | `read` |
| `get_shipping_rates` | `carts` | `read` |
| `get_stock` | `inventory` | `read` |
| `get_subscription` | `subscriptions` | `read` |
| `get_subscription_events` | `subscriptions` | `read` |
| `get_subscription_plan` | `subscriptions` | `read` |
| `get_tax_quote` | `tax` | `read` |
| `get_tax_rate` | `tax` | `read` |
| `get_tax_settings` | `tax` | `read` |
| `get_tax_transaction` | `tax` | `read` |
| `get_top_customers` | `analytics` | `read` |
| `get_top_products` | `analytics` | `read` |
| `get_us_state_tax_info` | `tax` | `read` |
| `get_wallet_balance` | `stablecoin` | `read` |
| `get_work_order` | `manufacturing` | `read` |
| `handle_fulfillment_exception` | `shipments` | `write` |
| `ingest_payment_provider_webhook` | `payments` | `write` |
| `ingest_shipping_provider_webhook` | `shipments` | `write` |
| `ingest_tax_provider_webhook` | `tax` | `write` |
| `inspect_batch` | `proofs` | `read` |
| `install_wasm_connector` | `connectors` | `write` |
| `list_agent_cards` | `agent-cards` | `read` |
| `list_billing_cycles` | `subscriptions` | `read` |
| `list_boms` | `manufacturing` | `read` |
| `list_carts` | `carts` | `read` |
| `list_connector_marketplace` | `connectors` | `read` |
| `list_coupons` | `promotions` | `read` |
| `list_custom_object_types` | `custom-objects` | `read` |
| `list_custom_objects` | `custom-objects` | `read` |
| `list_customers` | `customers` | `read` |
| `list_exchange_rates` | `currency` | `read` |
| `list_installed_connectors` | `connectors` | `read` |
| `list_invoices` | `invoices` | `read` |
| `list_orders` | `orders` | `read` |
| `list_payment_intents` | `payments` | `read` |
| `list_payment_links` | `checkout` | `read` |
| `list_payment_providers` | `payments` | `read` |
| `list_payment_settlement_batches` | `payments` | `read` |
| `list_payment_settlements` | `payments` | `read` |
| `list_payments` | `payments` | `read` |
| `list_products` | `products` | `read` |
| `list_promotions` | `promotions` | `read` |
| `list_purchase_orders` | `suppliers` | `read` |
| `list_returns` | `returns` | `read` |
| `list_shipments` | `shipments` | `read` |
| `list_shipping_labels` | `shipments` | `read` |
| `list_shipping_providers` | `shipments` | `read` |
| `list_subscription_plans` | `subscriptions` | `read` |
| `list_subscriptions` | `subscriptions` | `read` |
| `list_suppliers` | `suppliers` | `read` |
| `list_supported_chains` | `stablecoin` | `read` |
| `list_tax_jurisdictions` | `tax` | `read` |
| `list_tax_providers` | `tax` | `read` |
| `list_tax_rates` | `tax` | `read` |
| `list_tax_transactions` | `tax` | `read` |
| `list_warranties` | `warranties` | `read` |
| `list_work_orders` | `manufacturing` | `read` |
| `match_agent_to_products` | `catalog` | `read` |
| `match_product_to_agents` | `catalog` | `read` |
| `pause_subscription` | `subscriptions` | `write` |
| `publish_product_catalog` | `catalog` | `write` |
| `publish_wasm_connector` | `connectors` | `admin` |
| `query_agent_catalog` | `catalog` | `read` |
| `quote_shipping_rates` | `shipments` | `read` |
| `reconcile_payment_provider` | `payments` | `read` |
| `record_invoice_payment` | `invoices` | `write` |
| `refund_payment_intent` | `payments` | `write` |
| `register_agent_card` | `agent-cards` | `write` |
| `reject_return` | `returns` | `write` |
| `release_reservation` | `inventory` | `write` |
| `remove_cart_item` | `carts` | `write` |
| `reserve_inventory` | `inventory` | `write` |
| `resolve_payment_link` | `checkout` | `read` |
| `resume_subscription` | `subscriptions` | `write` |
| `revoke_payment_link` | `checkout` | `write` |
| `send_invoice` | `invoices` | `write` |
| `send_purchase_order` | `suppliers` | `write` |
| `set_base_currency` | `currency` | `admin` |
| `set_cart_payment` | `carts` | `write` |
| `set_cart_shipping_address` | `carts` | `write` |
| `set_exchange_rate` | `currency` | `admin` |
| `ship_order` | `orders` | `write` |
| `sign_wasm_connector_attestation` | `connectors` | `admin` |
| `skip_billing_cycle` | `subscriptions` | `write` |
| `soc2_evidence` | `compliance` | `admin` |
| `start_work_order` | `manufacturing` | `write` |
| `sync_conflicts` | `sync` | `read` |
| `sync_decrypt_event` | `sync` | `read` |
| `sync_entity_history` | `sync` | `read` |
| `sync_full` | `sync` | `admin` |
| `sync_inspect_commitment` | `sync` | `read` |
| `sync_outbox` | `sync` | `read` |
| `sync_pull` | `sync` | `write` |
| `sync_pulled_events` | `sync` | `read` |
| `sync_push` | `sync` | `write` |
| `sync_rebase` | `sync` | `admin` |
| `sync_resolve` | `sync` | `admin` |
| `sync_retry_failed` | `sync` | `admin` |
| `sync_status` | `sync` | `read` |
| `sync_verify_inclusion` | `sync` | `read` |
| `sync_verify_receipt` | `sync` | `read` |
| `track_shipping_label` | `shipments` | `read` |
| `treasury_balance` | `treasury` | `read` |
| `treasury_buy` | `treasury` | `write` |
| `treasury_deposit` | `treasury` | `write` |
| `treasury_ledger` | `treasury` | `read` |
| `treasury_list_tokens` | `treasury` | `read` |
| `treasury_register_token` | `treasury` | `admin` |
| `uninstall_wasm_connector` | `connectors` | `delete` |
| `update_cart_item` | `carts` | `write` |
| `update_custom_object` | `custom-objects` | `write` |
| `update_custom_object_type` | `custom-objects` | `write` |
| `update_order_status` | `orders` | `write` |
| `validate_coupon` | `promotions` | `read` |
| `validate_tax_jurisdiction_compliance` | `tax` | `read` |
| `vector_clear` | `vector` | `admin` |
| `vector_clear_all` | `vector` | `admin` |
| `vector_index_all_customers` | `vector` | `admin` |
| `vector_index_all_inventory` | `vector` | `admin` |
| `vector_index_all_orders` | `vector` | `admin` |
| `vector_index_all_products` | `vector` | `admin` |
| `vector_index_customer` | `vector` | `write` |
| `vector_index_inventory` | `vector` | `write` |
| `vector_index_order` | `vector` | `write` |
| `vector_index_product` | `vector` | `write` |
| `vector_reindex_all` | `vector` | `admin` |
| `vector_search_customers` | `vector` | `read` |
| `vector_search_inventory` | `vector` | `read` |
| `vector_search_orders` | `vector` | `read` |
| `vector_search_products` | `vector` | `read` |
| `vector_stats` | `vector` | `read` |
| `verify_agent` | `agent-cards` | `write` |
| `verify_chain_anchor` | `proofs` | `read` |
| `verify_inclusion_proof` | `proofs` | `read` |
| `verify_receipt` | `proofs` | `read` |
| `verify_wasm_connector_attestation` | `connectors` | `read` |
| `void_shipping_label` | `shipments` | `delete` |
| `void_tax_transaction` | `tax` | `delete` |
| `x402_create_payment_intent` | `x402` | `write` |
| `x402_credit_balance` | `x402` | `read` |
| `x402_credit_debit` | `x402` | `write` |
| `x402_credit_deposit` | `x402` | `write` |
| `x402_credit_transactions` | `x402` | `read` |
| `x402_execute_agent_payment` | `x402` | `write` |
| `x402_get_intent` | `x402` | `read` |
| `x402_get_next_nonce` | `x402` | `read` |
| `x402_list_intents` | `x402` | `read` |
| `x402_mark_settled` | `x402` | `write` |
| `x402_record_incoming_settlement` | `x402` | `write` |
| `x402_settle_intent_onchain` | `x402` | `write` |
| `x402_sign_intent` | `x402` | `write` |
