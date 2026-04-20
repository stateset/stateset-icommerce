# MCP Tool Inventory

This page is generated from the live MCP server export in `cli/src/mcp-server.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_mcp_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/mcp-tool-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Total tools | 535 |
| Policy domains | 29 |
| Read tools | 274 |
| Write tools | 207 |
| Delete tools | 12 |
| Admin tools | 42 |
| Unknown permission | 0 |

## Policy Domain Counts

| Policy domain | Tools |
| --- | --- |
| a2a | 138 |
| agent_cards | 46 |
| agentic | 15 |
| analytics | 10 |
| carts | 15 |
| commerce | 86 |
| connectors | 11 |
| currency | 8 |
| custom_objects | 12 |
| customers | 4 |
| erc8004 | 5 |
| inventory | 6 |
| invoices | 5 |
| manufacturing | 11 |
| orders | 8 |
| payments | 19 |
| products | 6 |
| promotions | 10 |
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
| x402 | 14 |

## Permission Counts

| Permission | Tools |
| --- | --- |
| admin | 42 |
| delete | 12 |
| read | 274 |
| write | 207 |

## Tool Registry

| Tool | Policy domain | Permission |
| --- | --- | --- |
| `a2a_accept_quote` | `a2a` | `write` |
| `a2a_add_rule` | `a2a` | `write` |
| `a2a_agent_alerts` | `a2a` | `read` |
| `a2a_agent_dashboard` | `a2a` | `read` |
| `a2a_agent_decisions` | `a2a` | `read` |
| `a2a_agent_insights` | `a2a` | `read` |
| `a2a_agent_lifecycle` | `a2a` | `read` |
| `a2a_agent_performance` | `a2a` | `read` |
| `a2a_agent_tick_metrics` | `a2a` | `read` |
| `a2a_batch_pay` | `a2a` | `write` |
| `a2a_batch_request_quotes` | `a2a` | `write` |
| `a2a_billing_metrics` | `a2a` | `read` |
| `a2a_billing_start` | `a2a` | `admin` |
| `a2a_billing_stop` | `a2a` | `admin` |
| `a2a_billing_tick` | `a2a` | `write` |
| `a2a_cancel_agent_subscription` | `a2a` | `write` |
| `a2a_cancel_scheduled` | `a2a` | `write` |
| `a2a_check_payment_conditions` | `a2a` | `read` |
| `a2a_commerce_report` | `a2a` | `read` |
| `a2a_configure_webhooks` | `a2a` | `write` |
| `a2a_coordination_status` | `a2a` | `read` |
| `a2a_cost_anomalies` | `a2a` | `read` |
| `a2a_cost_budget_forecast` | `a2a` | `read` |
| `a2a_cost_counterparty_breakdown` | `a2a` | `read` |
| `a2a_cost_daily_trend` | `a2a` | `read` |
| `a2a_cost_margin_analysis` | `a2a` | `read` |
| `a2a_cost_operation_breakdown` | `a2a` | `read` |
| `a2a_cost_summary` | `a2a` | `read` |
| `a2a_cost_top_spenders` | `a2a` | `read` |
| `a2a_counter_quote` | `a2a` | `write` |
| `a2a_counterparty_profile` | `a2a` | `read` |
| `a2a_create_agent_subscription` | `a2a` | `write` |
| `a2a_create_conditional_payment` | `a2a` | `write` |
| `a2a_create_escrow` | `a2a` | `write` |
| `a2a_create_split_payment` | `a2a` | `write` |
| `a2a_data_stats` | `a2a` | `read` |
| `a2a_decline_quote` | `a2a` | `write` |
| `a2a_delegate_task` | `a2a` | `write` |
| `a2a_discover_agents` | `a2a` | `read` |
| `a2a_dispute_escrow` | `a2a` | `write` |
| `a2a_dispute_resolver_metrics` | `a2a` | `read` |
| `a2a_dispute_resolver_start` | `a2a` | `admin` |
| `a2a_dispute_resolver_tick` | `a2a` | `write` |
| `a2a_dlq_count` | `a2a` | `read` |
| `a2a_escrow_process_all` | `a2a` | `write` |
| `a2a_evaluate_rules` | `a2a` | `read` |
| `a2a_execute_split_payment` | `a2a` | `write` |
| `a2a_export_agent_data` | `a2a` | `read` |
| `a2a_export_traces` | `a2a` | `read` |
| `a2a_file_dispute` | `a2a` | `write` |
| `a2a_fulfill_quote` | `a2a` | `write` |
| `a2a_fund_escrow` | `a2a` | `write` |
| `a2a_get_agent_subscription` | `a2a` | `read` |
| `a2a_get_balance` | `a2a` | `read` |
| `a2a_get_dispute` | `a2a` | `read` |
| `a2a_get_escrow` | `a2a` | `read` |
| `a2a_get_event_history` | `a2a` | `read` |
| `a2a_get_inbox` | `a2a` | `read` |
| `a2a_get_payment` | `a2a` | `read` |
| `a2a_get_reputation` | `a2a` | `read` |
| `a2a_get_service` | `a2a` | `read` |
| `a2a_get_split_payment` | `a2a` | `read` |
| `a2a_get_thread` | `a2a` | `read` |
| `a2a_get_trace` | `a2a` | `read` |
| `a2a_handshake` | `a2a` | `read` |
| `a2a_health_check` | `a2a` | `read` |
| `a2a_join_results` | `a2a` | `read` |
| `a2a_list_agent_subscriptions` | `a2a` | `read` |
| `a2a_list_checkpoints` | `a2a` | `read` |
| `a2a_list_disputes` | `a2a` | `read` |
| `a2a_list_escrows` | `a2a` | `read` |
| `a2a_list_event_subscriptions` | `a2a` | `read` |
| `a2a_list_failed_notifications` | `a2a` | `read` |
| `a2a_list_notification_log` | `a2a` | `read` |
| `a2a_list_payment_requests` | `a2a` | `read` |
| `a2a_list_payments` | `a2a` | `read` |
| `a2a_list_quotes` | `a2a` | `read` |
| `a2a_list_rules` | `a2a` | `read` |
| `a2a_list_scheduled` | `a2a` | `read` |
| `a2a_list_services` | `a2a` | `read` |
| `a2a_list_split_payments` | `a2a` | `read` |
| `a2a_list_webhook_dlq` | `a2a` | `admin` |
| `a2a_load_checkpoint` | `a2a` | `read` |
| `a2a_marketplace_auto_award` | `a2a` | `write` |
| `a2a_marketplace_maintenance` | `a2a` | `write` |
| `a2a_messaging_metrics` | `a2a` | `read` |
| `a2a_my_capabilities` | `a2a` | `read` |
| `a2a_notification_retry_all` | `a2a` | `write` |
| `a2a_pause_agent_subscription` | `a2a` | `write` |
| `a2a_pay` | `a2a` | `write` |
| `a2a_pay_request` | `a2a` | `write` |
| `a2a_process_subscription_billing` | `a2a` | `write` |
| `a2a_provide_quote` | `a2a` | `write` |
| `a2a_purge_dlq` | `a2a` | `admin` |
| `a2a_quarantine_failed_webhooks` | `a2a` | `admin` |
| `a2a_rate_agent` | `a2a` | `write` |
| `a2a_rate_limit_metrics` | `a2a` | `read` |
| `a2a_readiness` | `a2a` | `read` |
| `a2a_recent_spans` | `a2a` | `read` |
| `a2a_refund_escrow` | `a2a` | `write` |
| `a2a_register_service` | `a2a` | `write` |
| `a2a_release_escrow` | `a2a` | `write` |
| `a2a_remember_interaction` | `a2a` | `write` |
| `a2a_replay_dlq_entry` | `a2a` | `admin` |
| `a2a_replay_notification` | `a2a` | `write` |
| `a2a_request_payment` | `a2a` | `write` |
| `a2a_request_quote` | `a2a` | `write` |
| `a2a_resolve_dispute` | `a2a` | `write` |
| `a2a_respond_to_feedback` | `a2a` | `write` |
| `a2a_respond_to_task` | `a2a` | `write` |
| `a2a_resume_agent_subscription` | `a2a` | `write` |
| `a2a_revise_quote` | `a2a` | `write` |
| `a2a_rule_audit_log` | `a2a` | `read` |
| `a2a_saga_cancel` | `a2a` | `write` |
| `a2a_saga_execute` | `a2a` | `write` |
| `a2a_saga_list` | `a2a` | `read` |
| `a2a_saga_status` | `a2a` | `read` |
| `a2a_save_checkpoint` | `a2a` | `write` |
| `a2a_scatter` | `a2a` | `write` |
| `a2a_schedule_action` | `a2a` | `write` |
| `a2a_scheduler_metrics` | `a2a` | `read` |
| `a2a_send_message` | `a2a` | `write` |
| `a2a_send_notification` | `a2a` | `write` |
| `a2a_settle_conditional_payment` | `a2a` | `write` |
| `a2a_settlement_finality_metrics` | `a2a` | `read` |
| `a2a_settlement_pending` | `a2a` | `read` |
| `a2a_settlement_status` | `a2a` | `read` |
| `a2a_should_transact` | `a2a` | `read` |
| `a2a_sla_enforce` | `a2a` | `write` |
| `a2a_sla_enforce_all` | `a2a` | `write` |
| `a2a_submit_evidence` | `a2a` | `write` |
| `a2a_submit_response` | `a2a` | `write` |
| `a2a_subscribe_events` | `a2a` | `write` |
| `a2a_tick_metrics` | `a2a` | `read` |
| `a2a_top_counterparties` | `a2a` | `read` |
| `a2a_tracing_metrics` | `a2a` | `read` |
| `a2a_verify_webhook` | `a2a` | `read` |
| `a2a_webhook_dlq_status` | `a2a` | `read` |
| `abandon_cart` | `carts` | `write` |
| `activate_bom` | `manufacturing` | `write` |
| `activate_promotion` | `promotions` | `write` |
| `activate_subscription_plan` | `subscriptions` | `write` |
| `add_bom_component` | `manufacturing` | `write` |
| `add_cart_item` | `carts` | `write` |
| `add_to_wishlist` | `commerce` | `write` |
| `adjust_inventory` | `inventory` | `write` |
| `adjust_store_credit` | `commerce` | `write` |
| `agent_attach_sla` | `agent_cards` | `write` |
| `agent_award_rfq` | `agent_cards` | `write` |
| `agent_broadcast_rfq` | `agent_cards` | `write` |
| `agent_check_sla_compliance` | `agent_cards` | `read` |
| `agent_collect_rfq_responses` | `agent_cards` | `read` |
| `agent_create_escrow_deal` | `agent_cards` | `write` |
| `agent_create_runtime` | `agent_cards` | `write` |
| `agent_create_split_deal` | `agent_cards` | `write` |
| `agent_create_workflow` | `agent_cards` | `write` |
| `agent_destroy_runtime` | `agent_cards` | `delete` |
| `agent_discover_services` | `agent_cards` | `read` |
| `agent_enable_settlement` | `agent_cards` | `write` |
| `agent_execute_workflow` | `agent_cards` | `write` |
| `agent_get_all_breaker_states` | `agent_cards` | `read` |
| `agent_get_breaker_state` | `agent_cards` | `read` |
| `agent_get_budget` | `agent_cards` | `read` |
| `agent_get_chain_balance` | `agent_cards` | `read` |
| `agent_get_event_history` | `agent_cards` | `read` |
| `agent_get_marketplace_metrics` | `agent_cards` | `read` |
| `agent_get_reputation` | `agent_cards` | `read` |
| `agent_get_spending_summary` | `agent_cards` | `read` |
| `agent_get_status` | `agent_cards` | `read` |
| `agent_get_workflow_status` | `agent_cards` | `read` |
| `agent_instant_checkout` | `agent_cards` | `write` |
| `agent_key_export` | `sync` | `read` |
| `agent_key_generate` | `sync` | `write` |
| `agent_key_info` | `sync` | `read` |
| `agent_key_list` | `sync` | `read` |
| `agent_key_rotate` | `sync` | `write` |
| `agent_list_runtimes` | `agent_cards` | `read` |
| `agent_rate_counterparty` | `agent_cards` | `write` |
| `agent_register_service` | `agent_cards` | `write` |
| `agent_reset_all_breakers` | `agent_cards` | `admin` |
| `agent_reset_breaker` | `agent_cards` | `admin` |
| `agent_set_dynamic_pricing` | `agent_cards` | `write` |
| `agent_set_spending_limits` | `agent_cards` | `admin` |
| `agent_set_strategy` | `agent_cards` | `write` |
| `agent_start_loop` | `agent_cards` | `write` |
| `agent_stop_loop` | `agent_cards` | `write` |
| `agent_subscribe_to_service` | `agent_cards` | `write` |
| `agent_tick` | `agent_cards` | `write` |
| `agent_trip_all_breakers` | `agent_cards` | `admin` |
| `agent_trip_breaker` | `agent_cards` | `admin` |
| `agentic_execute_plan` | `agentic` | `read` |
| `agentic_get_event_history` | `agentic` | `read` |
| `agentic_list_event_subscriptions` | `agentic` | `read` |
| `agentic_payment_discovery` | `agentic` | `read` |
| `agentic_plan` | `agentic` | `read` |
| `agentic_prepare_payment` | `agentic` | `read` |
| `agentic_replay` | `agentic` | `read` |
| `agentic_replay_mutation` | `agentic` | `read` |
| `agentic_runtime_contract` | `agentic` | `read` |
| `agentic_simulate_mutation` | `agentic` | `read` |
| `agentic_subscribe_events` | `agentic` | `read` |
| `agentic_tool_catalog` | `agentic` | `read` |
| `agentic_unsubscribe_events` | `agentic` | `read` |
| `apply_cart_discount` | `carts` | `write` |
| `apply_cart_promotions` | `promotions` | `write` |
| `apply_store_credit` | `commerce` | `write` |
| `approve_purchase_order` | `suppliers` | `write` |
| `approve_return` | `returns` | `write` |
| `approve_review` | `commerce` | `write` |
| `approve_warranty_claim` | `warranties` | `write` |
| `archive_subscription_plan` | `subscriptions` | `delete` |
| `assess_order_fraud` | `orders` | `read` |
| `assess_wasm_connector_safety` | `connectors` | `read` |
| `audit_export` | `commerce` | `admin` |
| `audit_query` | `commerce` | `read` |
| `audit_retention` | `commerce` | `admin` |
| `audit_summary` | `commerce` | `read` |
| `calculate_cart_tax` | `tax` | `read` |
| `calculate_shipping_rate` | `commerce` | `read` |
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
| `charge_gift_card` | `commerce` | `write` |
| `check_gift_card_balance` | `commerce` | `read` |
| `checkout_with_crypto` | `commerce` | `write` |
| `commit_tax_transaction` | `tax` | `write` |
| `complete_checkout` | `carts` | `write` |
| `complete_payment` | `payments` | `write` |
| `complete_work_order` | `manufacturing` | `write` |
| `compliance_summary` | `commerce` | `read` |
| `configure_stripe_webhooks` | `commerce` | `write` |
| `configure_woocommerce_webhooks` | `commerce` | `write` |
| `confirm_reservation` | `inventory` | `write` |
| `convert_currency` | `currency` | `read` |
| `convert_wishlist_to_cart` | `carts` | `write` |
| `create_bom` | `manufacturing` | `write` |
| `create_cart` | `carts` | `write` |
| `create_coupon` | `promotions` | `write` |
| `create_custom_object` | `custom_objects` | `write` |
| `create_custom_object_type` | `custom_objects` | `write` |
| `create_customer` | `customers` | `write` |
| `create_fraud_rule` | `commerce` | `admin` |
| `create_gift_card` | `commerce` | `write` |
| `create_inventory_item` | `inventory` | `write` |
| `create_invoice` | `invoices` | `write` |
| `create_loyalty_program` | `commerce` | `admin` |
| `create_order` | `orders` | `write` |
| `create_payment` | `payments` | `write` |
| `create_payment_intent` | `payments` | `write` |
| `create_payment_link` | `commerce` | `write` |
| `create_payment_settlement_batch` | `payments` | `write` |
| `create_product` | `products` | `write` |
| `create_promotion` | `promotions` | `write` |
| `create_purchase_order` | `suppliers` | `write` |
| `create_refund` | `payments` | `write` |
| `create_return` | `returns` | `write` |
| `create_review` | `commerce` | `write` |
| `create_reward` | `commerce` | `admin` |
| `create_segment` | `commerce` | `write` |
| `create_shipment` | `shipments` | `write` |
| `create_shipping_label` | `shipments` | `write` |
| `create_shipping_method` | `commerce` | `write` |
| `create_shipping_zone` | `commerce` | `write` |
| `create_stablecoin_payment` | `stablecoin` | `write` |
| `create_store_credit` | `commerce` | `write` |
| `create_subscription` | `subscriptions` | `write` |
| `create_subscription_plan` | `subscriptions` | `write` |
| `create_supplier` | `suppliers` | `write` |
| `create_tax_exemption` | `tax` | `write` |
| `create_warranty` | `warranties` | `write` |
| `create_warranty_claim` | `warranties` | `write` |
| `create_wishlist` | `commerce` | `write` |
| `create_work_order` | `manufacturing` | `write` |
| `deactivate_promotion` | `promotions` | `write` |
| `delegate_to_agent` | `agentic` | `write` |
| `delete_custom_object` | `custom_objects` | `delete` |
| `delete_custom_object_type` | `custom_objects` | `delete` |
| `delete_gdpr_data` | `commerce` | `admin` |
| `deliver_shipment` | `shipments` | `write` |
| `disable_gift_card` | `commerce` | `write` |
| `discover_agents` | `agent_cards` | `read` |
| `discover_tools` | `agentic` | `read` |
| `earn_points` | `commerce` | `write` |
| `enable_currencies` | `currency` | `admin` |
| `enroll_customer` | `customers` | `write` |
| `erc8004_get_by_wallet` | `erc8004` | `read` |
| `erc8004_get_identity` | `erc8004` | `read` |
| `erc8004_link_wallet` | `erc8004` | `write` |
| `erc8004_list_identities` | `erc8004` | `read` |
| `erc8004_register_identity` | `erc8004` | `admin` |
| `evaluate_policy` | `commerce` | `read` |
| `evaluate_segment_membership` | `commerce` | `read` |
| `execute_wasm_connector` | `connectors` | `write` |
| `explain_policy_denial` | `commerce` | `read` |
| `export_agent_catalog` | `agent_cards` | `read` |
| `export_audit_trail` | `commerce` | `admin` |
| `export_compliance_package` | `commerce` | `read` |
| `export_data` | `commerce` | `read` |
| `export_gdpr_data` | `commerce` | `admin` |
| `express_checkout` | `commerce` | `write` |
| `flag_review` | `commerce` | `write` |
| `format_currency` | `currency` | `read` |
| `generate_1099k` | `commerce` | `admin` |
| `generate_inclusion_proof` | `commerce` | `read` |
| `generate_receipt_bundle` | `commerce` | `read` |
| `get_abandoned_carts` | `carts` | `read` |
| `get_active_promotions` | `promotions` | `read` |
| `get_agent_card` | `agent_cards` | `read` |
| `get_agent_wallet` | `stablecoin` | `read` |
| `get_billing_cycle` | `subscriptions` | `read` |
| `get_bom` | `manufacturing` | `read` |
| `get_cart` | `carts` | `read` |
| `get_currency_settings` | `currency` | `read` |
| `get_custom_object` | `custom_objects` | `read` |
| `get_custom_object_by_handle` | `custom_objects` | `read` |
| `get_custom_object_type` | `custom_objects` | `read` |
| `get_custom_object_type_by_handle` | `custom_objects` | `read` |
| `get_customer` | `customers` | `read` |
| `get_customer_metrics` | `analytics` | `read` |
| `get_customer_tax_exemptions` | `tax` | `read` |
| `get_demand_forecast` | `analytics` | `read` |
| `get_exchange_rate` | `currency` | `read` |
| `get_fraud_assessment` | `commerce` | `read` |
| `get_gift_card` | `commerce` | `read` |
| `get_installed_connector` | `connectors` | `read` |
| `get_inventory_health` | `analytics` | `read` |
| `get_low_stock_items` | `analytics` | `read` |
| `get_loyalty_account` | `commerce` | `read` |
| `get_loyalty_program` | `commerce` | `read` |
| `get_order` | `orders` | `read` |
| `get_order_status_breakdown` | `analytics` | `read` |
| `get_overdue_invoices` | `invoices` | `read` |
| `get_payment` | `payments` | `read` |
| `get_payment_intent` | `payments` | `read` |
| `get_payment_link_status` | `commerce` | `read` |
| `get_product` | `products` | `read` |
| `get_product_spec` | `commerce` | `read` |
| `get_product_variant` | `products` | `read` |
| `get_promotion` | `promotions` | `read` |
| `get_return` | `returns` | `read` |
| `get_return_metrics` | `analytics` | `read` |
| `get_revenue_forecast` | `analytics` | `read` |
| `get_review` | `commerce` | `read` |
| `get_review_summary` | `commerce` | `read` |
| `get_sales_summary` | `analytics` | `read` |
| `get_segment` | `commerce` | `read` |
| `get_shipping_rates` | `carts` | `read` |
| `get_shipping_zone` | `commerce` | `read` |
| `get_stock` | `inventory` | `read` |
| `get_store_credit` | `commerce` | `read` |
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
| `get_wishlist` | `commerce` | `read` |
| `get_work_order` | `manufacturing` | `read` |
| `handle_fulfillment_exception` | `shipments` | `write` |
| `import_csv` | `commerce` | `write` |
| `import_json` | `commerce` | `write` |
| `import_shopify_data` | `commerce` | `write` |
| `import_shopify_shadow_data` | `commerce` | `write` |
| `import_status` | `commerce` | `read` |
| `import_woocommerce_data` | `commerce` | `write` |
| `ingest_payment_provider_webhook` | `payments` | `write` |
| `ingest_shipping_provider_webhook` | `shipments` | `write` |
| `ingest_tax_provider_webhook` | `tax` | `write` |
| `inspect_batch` | `commerce` | `read` |
| `install_wasm_connector` | `connectors` | `write` |
| `list_agent_cards` | `agent_cards` | `read` |
| `list_billing_cycles` | `subscriptions` | `read` |
| `list_boms` | `manufacturing` | `read` |
| `list_carts` | `carts` | `read` |
| `list_connector_marketplace` | `connectors` | `read` |
| `list_coupons` | `promotions` | `read` |
| `list_custom_object_types` | `custom_objects` | `read` |
| `list_custom_objects` | `custom_objects` | `read` |
| `list_customers` | `customers` | `read` |
| `list_exchange_rates` | `currency` | `read` |
| `list_fraud_signals` | `commerce` | `read` |
| `list_gift_cards` | `commerce` | `read` |
| `list_id_mappings` | `commerce` | `read` |
| `list_installed_connectors` | `connectors` | `read` |
| `list_invoices` | `invoices` | `read` |
| `list_orders` | `orders` | `read` |
| `list_payment_intents` | `payments` | `read` |
| `list_payment_links` | `commerce` | `read` |
| `list_payment_providers` | `payments` | `read` |
| `list_payment_settlement_batches` | `payments` | `read` |
| `list_payment_settlements` | `payments` | `read` |
| `list_payments` | `payments` | `read` |
| `list_policies` | `commerce` | `read` |
| `list_products` | `products` | `read` |
| `list_promotions` | `promotions` | `read` |
| `list_purchase_orders` | `suppliers` | `read` |
| `list_returns` | `returns` | `read` |
| `list_reviews` | `commerce` | `read` |
| `list_rewards` | `commerce` | `read` |
| `list_segments` | `commerce` | `read` |
| `list_shipments` | `shipments` | `read` |
| `list_shipping_labels` | `shipments` | `read` |
| `list_shipping_methods` | `commerce` | `read` |
| `list_shipping_providers` | `shipments` | `read` |
| `list_shipping_zones` | `commerce` | `read` |
| `list_store_credits` | `commerce` | `read` |
| `list_subscription_plans` | `subscriptions` | `read` |
| `list_subscriptions` | `subscriptions` | `read` |
| `list_suppliers` | `suppliers` | `read` |
| `list_supported_chains` | `stablecoin` | `read` |
| `list_tax_jurisdictions` | `tax` | `read` |
| `list_tax_providers` | `tax` | `read` |
| `list_tax_rates` | `tax` | `read` |
| `list_tax_transactions` | `tax` | `read` |
| `list_warranties` | `warranties` | `read` |
| `list_wishlists` | `commerce` | `read` |
| `list_work_orders` | `manufacturing` | `read` |
| `load_policy_file` | `commerce` | `write` |
| `match_agent_to_products` | `agent_cards` | `read` |
| `match_product_to_agents` | `products` | `read` |
| `pause_subscription` | `subscriptions` | `write` |
| `publish_product_catalog` | `products` | `write` |
| `publish_wasm_connector` | `connectors` | `admin` |
| `query_agent_catalog` | `agent_cards` | `read` |
| `quote_shipping_rates` | `shipments` | `read` |
| `rebuild_dynamic_segment` | `commerce` | `write` |
| `reconcile_payment_provider` | `payments` | `read` |
| `record_invoice_payment` | `invoices` | `write` |
| `redeem_points` | `commerce` | `write` |
| `refund_payment_intent` | `payments` | `write` |
| `refund_to_gift_card` | `commerce` | `write` |
| `register_agent_card` | `agent_cards` | `write` |
| `register_policy_template` | `commerce` | `write` |
| `reject_return` | `returns` | `write` |
| `reject_review` | `commerce` | `write` |
| `release_reservation` | `inventory` | `write` |
| `remove_cart_item` | `carts` | `write` |
| `remove_from_wishlist` | `commerce` | `write` |
| `reserve_inventory` | `inventory` | `write` |
| `resolve_payment_link` | `payments` | `read` |
| `resume_subscription` | `subscriptions` | `write` |
| `review_flagged_order` | `orders` | `write` |
| `revoke_payment_link` | `payments` | `write` |
| `send_invoice` | `invoices` | `write` |
| `send_purchase_order` | `suppliers` | `write` |
| `set_base_currency` | `currency` | `admin` |
| `set_cart_payment` | `carts` | `write` |
| `set_cart_shipping_address` | `carts` | `write` |
| `set_exchange_rate` | `currency` | `admin` |
| `ship_order` | `orders` | `write` |
| `sign_wasm_connector_attestation` | `connectors` | `admin` |
| `skip_billing_cycle` | `subscriptions` | `write` |
| `soc2_evidence` | `commerce` | `admin` |
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
| `update_custom_object` | `custom_objects` | `write` |
| `update_custom_object_type` | `custom_objects` | `write` |
| `update_fraud_rule` | `commerce` | `admin` |
| `update_order_status` | `orders` | `write` |
| `update_segment` | `commerce` | `write` |
| `update_shipping_zone` | `commerce` | `write` |
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
| `verify_agent` | `agent_cards` | `write` |
| `verify_chain_anchor` | `commerce` | `read` |
| `verify_inclusion_proof` | `commerce` | `read` |
| `verify_receipt` | `commerce` | `read` |
| `verify_wasm_connector_attestation` | `connectors` | `read` |
| `void_shipping_label` | `shipments` | `delete` |
| `void_tax_transaction` | `tax` | `delete` |
| `x402_circuit_status` | `x402` | `read` |
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
