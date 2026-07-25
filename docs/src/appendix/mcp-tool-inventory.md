# MCP Tool Inventory

This page is generated from the live MCP server registry in `cli/src/mcp-server-registry.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_mcp_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/mcp-tool-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Total tools | 956 |
| MCP servers | 3 |
| Policy domains | 89 |
| Read tools | 474 |
| Write tools | 414 |
| Delete tools | 21 |
| Admin tools | 47 |
| Unknown permission | 0 |

## MCP Server Counts

| MCP server | Tools | Source |
| --- | --- | --- |
| stateset-commerce | 938 | `cli/src/mcp-server.js` |
| stateset-scaffold | 13 | `cli/src/scaffold-server.js` |
| stateset-x402 | 5 | `cli/src/x402-mcp-server.js` |

## Policy Domain Counts

| Policy domain | Tools |
| --- | --- |
| a2a | 59 |
| a2a_automation | 32 |
| a2a_intelligence | 17 |
| a2a_observability | 15 |
| a2a_platform | 16 |
| accounts_payable | 11 |
| accounts_receivable | 8 |
| activity_logs | 5 |
| agent_cards | 5 |
| agent_receipt | 11 |
| agent_runtime | 29 |
| agentic | 15 |
| analytics | 14 |
| audit | 4 |
| backorders | 9 |
| carts | 30 |
| catalog | 6 |
| channels | 8 |
| checkout | 8 |
| circuit_breaker | 8 |
| companies | 9 |
| compliance | 6 |
| connectors | 11 |
| cost_accounting | 5 |
| credit | 8 |
| currency | 12 |
| custom_objects | 12 |
| customers | 11 |
| cycle_counts | 7 |
| edi_documents | 5 |
| erc8004 | 5 |
| fixed_assets | 9 |
| fraud | 6 |
| fulfillment | 14 |
| general_ledger | 17 |
| gift_cards | 7 |
| import | 10 |
| inbound_shipments | 8 |
| integration-field-mappings | 8 |
| integration-mappings | 7 |
| inventory | 6 |
| invoices | 7 |
| lots | 11 |
| loyalty | 8 |
| maintenance | 5 |
| manufacturing | 11 |
| orders | 6 |
| payment-obligations | 7 |
| payments | 19 |
| policies | 5 |
| prepayments | 8 |
| price_levels | 9 |
| price_schedules | 10 |
| print-stations | 8 |
| production_batches | 8 |
| products | 14 |
| promotions | 15 |
| proofs | 7 |
| purgatory | 6 |
| quality | 15 |
| receiving | 8 |
| returns | 12 |
| revenue_recognition | 6 |
| reviews | 7 |
| scaffold | 13 |
| search-config | 7 |
| segments | 6 |
| serials | 8 |
| shipments | 14 |
| shipping_zones | 7 |
| stablecoin | 4 |
| stock-snapshots | 5 |
| store_credits | 5 |
| subscriptions | 17 |
| supplier_skus | 7 |
| suppliers | 10 |
| sync | 20 |
| tax | 29 |
| topology-snapshots | 5 |
| transfer_orders | 7 |
| treasury | 6 |
| units-of-measure | 10 |
| vector | 16 |
| vendor_credits | 8 |
| vendor-returns | 6 |
| warehouse | 9 |
| warranties | 7 |
| wishlists | 6 |
| x402 | 19 |

## Permission Counts

| Permission | Tools |
| --- | --- |
| admin | 47 |
| delete | 21 |
| read | 474 |
| write | 414 |

## Tool Registry

| Tool | MCP server | Policy domain | Permission |
| --- | --- | --- | --- |
| `a2a_accept_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_add_rule` | `stateset-commerce` | `a2a_intelligence` | `write` |
| `a2a_agent_alerts` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_agent_dashboard` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_agent_decisions` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_agent_insights` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_agent_lifecycle` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_agent_performance` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_agent_tick_metrics` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_batch_pay` | `stateset-commerce` | `a2a_platform` | `write` |
| `a2a_batch_request_quotes` | `stateset-commerce` | `a2a_platform` | `write` |
| `a2a_billing_metrics` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_billing_start` | `stateset-commerce` | `a2a_automation` | `admin` |
| `a2a_billing_stop` | `stateset-commerce` | `a2a_automation` | `admin` |
| `a2a_billing_tick` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_cancel_agent_subscription` | `stateset-commerce` | `a2a` | `write` |
| `a2a_cancel_scheduled` | `stateset-commerce` | `a2a_intelligence` | `write` |
| `a2a_check_payment_conditions` | `stateset-commerce` | `a2a` | `read` |
| `a2a_commerce_report` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_configure_webhooks` | `stateset-commerce` | `a2a` | `write` |
| `a2a_coordination_status` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_cost_anomalies` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_budget_forecast` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_counterparty_breakdown` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_daily_trend` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_margin_analysis` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_operation_breakdown` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_summary` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_cost_top_spenders` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_counter_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_counterparty_profile` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_create_agent_subscription` | `stateset-commerce` | `a2a` | `write` |
| `a2a_create_conditional_payment` | `stateset-commerce` | `a2a` | `write` |
| `a2a_create_escrow` | `stateset-commerce` | `a2a` | `write` |
| `a2a_create_split_payment` | `stateset-commerce` | `a2a` | `write` |
| `a2a_data_stats` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_decline_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_delegate_task` | `stateset-commerce` | `a2a_platform` | `write` |
| `a2a_discover_agents` | `stateset-commerce` | `a2a` | `read` |
| `a2a_dispute_escrow` | `stateset-commerce` | `a2a` | `write` |
| `a2a_dispute_resolver_metrics` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_dispute_resolver_start` | `stateset-commerce` | `a2a_automation` | `admin` |
| `a2a_dispute_resolver_tick` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_dlq_count` | `stateset-commerce` | `a2a` | `read` |
| `a2a_escrow_process_all` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_evaluate_rules` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_execute_split_payment` | `stateset-commerce` | `a2a` | `write` |
| `a2a_export_agent_data` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_export_traces` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_file_dispute` | `stateset-commerce` | `a2a` | `write` |
| `a2a_fulfill_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_fund_escrow` | `stateset-commerce` | `a2a` | `write` |
| `a2a_get_agent_subscription` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_balance` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_dispute` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_escrow` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_event_history` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_inbox` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_get_payment` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_reputation` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_service` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_split_payment` | `stateset-commerce` | `a2a` | `read` |
| `a2a_get_thread` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_get_trace` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_handshake` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_health_check` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_join_results` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_list_agent_subscriptions` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_checkpoints` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_list_disputes` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_escrows` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_event_subscriptions` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_failed_notifications` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_list_notification_log` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_payment_requests` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_payments` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_quotes` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_rules` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_list_scheduled` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_list_services` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_split_payments` | `stateset-commerce` | `a2a` | `read` |
| `a2a_list_webhook_dlq` | `stateset-commerce` | `a2a` | `admin` |
| `a2a_load_checkpoint` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_marketplace_auto_award` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_marketplace_maintenance` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_messaging_metrics` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_my_capabilities` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_notification_retry_all` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_pause_agent_subscription` | `stateset-commerce` | `a2a` | `write` |
| `a2a_pay` | `stateset-commerce` | `a2a` | `write` |
| `a2a_pay_request` | `stateset-commerce` | `a2a` | `write` |
| `a2a_process_subscription_billing` | `stateset-commerce` | `a2a` | `write` |
| `a2a_provide_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_purge_dlq` | `stateset-commerce` | `a2a` | `admin` |
| `a2a_quarantine_failed_webhooks` | `stateset-commerce` | `a2a` | `admin` |
| `a2a_rate_agent` | `stateset-commerce` | `a2a` | `write` |
| `a2a_rate_limit_metrics` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_readiness` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_recent_spans` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_refund_escrow` | `stateset-commerce` | `a2a` | `write` |
| `a2a_register_service` | `stateset-commerce` | `a2a` | `write` |
| `a2a_release_escrow` | `stateset-commerce` | `a2a` | `write` |
| `a2a_remember_interaction` | `stateset-commerce` | `a2a_intelligence` | `write` |
| `a2a_replay_dlq_entry` | `stateset-commerce` | `a2a` | `admin` |
| `a2a_replay_notification` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_request_payment` | `stateset-commerce` | `a2a` | `write` |
| `a2a_request_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_resolve_dispute` | `stateset-commerce` | `a2a` | `write` |
| `a2a_respond_to_feedback` | `stateset-commerce` | `a2a` | `write` |
| `a2a_respond_to_task` | `stateset-commerce` | `a2a_platform` | `write` |
| `a2a_resume_agent_subscription` | `stateset-commerce` | `a2a` | `write` |
| `a2a_revise_quote` | `stateset-commerce` | `a2a` | `write` |
| `a2a_rule_audit_log` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_saga_cancel` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_saga_execute` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_saga_list` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_saga_status` | `stateset-commerce` | `a2a_automation` | `read` |
| `a2a_save_checkpoint` | `stateset-commerce` | `a2a_platform` | `write` |
| `a2a_scatter` | `stateset-commerce` | `a2a_intelligence` | `write` |
| `a2a_schedule_action` | `stateset-commerce` | `a2a_intelligence` | `write` |
| `a2a_scheduler_metrics` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_send_message` | `stateset-commerce` | `a2a_platform` | `write` |
| `a2a_send_notification` | `stateset-commerce` | `a2a` | `write` |
| `a2a_settle_conditional_payment` | `stateset-commerce` | `a2a` | `write` |
| `a2a_settlement_finality_metrics` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_settlement_pending` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_settlement_status` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_should_transact` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_sla_enforce` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_sla_enforce_all` | `stateset-commerce` | `a2a_automation` | `write` |
| `a2a_submit_evidence` | `stateset-commerce` | `a2a` | `write` |
| `a2a_submit_response` | `stateset-commerce` | `a2a_intelligence` | `write` |
| `a2a_subscribe_events` | `stateset-commerce` | `a2a` | `write` |
| `a2a_tick_metrics` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_top_counterparties` | `stateset-commerce` | `a2a_intelligence` | `read` |
| `a2a_tracing_metrics` | `stateset-commerce` | `a2a_observability` | `read` |
| `a2a_verify_webhook` | `stateset-commerce` | `a2a_platform` | `read` |
| `a2a_webhook_dlq_status` | `stateset-commerce` | `a2a_automation` | `read` |
| `abandon_cart` | `stateset-commerce` | `carts` | `write` |
| `activate_bom` | `stateset-commerce` | `manufacturing` | `write` |
| `activate_product` | `stateset-commerce` | `products` | `write` |
| `activate_promotion` | `stateset-commerce` | `promotions` | `write` |
| `activate_subscription_plan` | `stateset-commerce` | `subscriptions` | `write` |
| `add_bom_component` | `stateset-commerce` | `manufacturing` | `write` |
| `add_cart_item` | `stateset-commerce` | `carts` | `write` |
| `add_customer_address` | `stateset-commerce` | `customers` | `write` |
| `add_product_variant` | `stateset-commerce` | `products` | `write` |
| `add_production_batch_work_orders` | `stateset-commerce` | `production_batches` | `write` |
| `add_return_tracking` | `stateset-commerce` | `returns` | `write` |
| `add_to_wishlist` | `stateset-commerce` | `wishlists` | `write` |
| `adjust_credit_limit` | `stateset-commerce` | `credit` | `write` |
| `adjust_inventory` | `stateset-commerce` | `inventory` | `write` |
| `adjust_store_credit` | `stateset-commerce` | `store_credits` | `write` |
| `agent_attach_sla` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_award_rfq` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_broadcast_rfq` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_check_sla_compliance` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_collect_rfq_responses` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_create_escrow_deal` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_create_runtime` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_create_split_deal` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_create_workflow` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_destroy_runtime` | `stateset-commerce` | `agent_runtime` | `delete` |
| `agent_discover_services` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_enable_settlement` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_execute_workflow` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_get_all_breaker_states` | `stateset-commerce` | `circuit_breaker` | `read` |
| `agent_get_breaker_state` | `stateset-commerce` | `circuit_breaker` | `read` |
| `agent_get_budget` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_get_chain_balance` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_get_event_history` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_get_marketplace_metrics` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_get_reputation` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_get_spending_summary` | `stateset-commerce` | `circuit_breaker` | `read` |
| `agent_get_status` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_get_workflow_status` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_instant_checkout` | `stateset-commerce` | `checkout` | `write` |
| `agent_key_export` | `stateset-commerce` | `sync` | `read` |
| `agent_key_generate` | `stateset-commerce` | `sync` | `write` |
| `agent_key_info` | `stateset-commerce` | `sync` | `read` |
| `agent_key_list` | `stateset-commerce` | `sync` | `read` |
| `agent_key_rotate` | `stateset-commerce` | `sync` | `write` |
| `agent_list_runtimes` | `stateset-commerce` | `agent_runtime` | `read` |
| `agent_rate_counterparty` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_receipt_audit` | `stateset-commerce` | `agent_receipt` | `read` |
| `agent_receipt_dispute` | `stateset-commerce` | `agent_receipt` | `write` |
| `agent_receipt_fx_quote` | `stateset-commerce` | `agent_receipt` | `read` |
| `agent_receipt_merchant_statement` | `stateset-commerce` | `agent_receipt` | `read` |
| `agent_receipt_purchase` | `stateset-commerce` | `agent_receipt` | `write` |
| `agent_receipt_refund` | `stateset-commerce` | `agent_receipt` | `write` |
| `agent_receipt_release` | `stateset-commerce` | `agent_receipt` | `write` |
| `agent_receipt_request_payout` | `stateset-commerce` | `agent_receipt` | `write` |
| `agent_receipt_resolve` | `stateset-commerce` | `agent_receipt` | `admin` |
| `agent_receipt_status` | `stateset-commerce` | `agent_receipt` | `read` |
| `agent_receipt_sweep_yield` | `stateset-commerce` | `agent_receipt` | `admin` |
| `agent_register_service` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_reset_all_breakers` | `stateset-commerce` | `circuit_breaker` | `admin` |
| `agent_reset_breaker` | `stateset-commerce` | `circuit_breaker` | `admin` |
| `agent_set_dynamic_pricing` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_set_spending_limits` | `stateset-commerce` | `circuit_breaker` | `admin` |
| `agent_set_strategy` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_start_loop` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_stop_loop` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_subscribe_to_service` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_tick` | `stateset-commerce` | `agent_runtime` | `write` |
| `agent_trip_all_breakers` | `stateset-commerce` | `circuit_breaker` | `admin` |
| `agent_trip_breaker` | `stateset-commerce` | `circuit_breaker` | `admin` |
| `agentic_execute_plan` | `stateset-commerce` | `agentic` | `read` |
| `agentic_get_event_history` | `stateset-commerce` | `agentic` | `read` |
| `agentic_list_event_subscriptions` | `stateset-commerce` | `agentic` | `read` |
| `agentic_payment_discovery` | `stateset-commerce` | `agentic` | `read` |
| `agentic_plan` | `stateset-commerce` | `agentic` | `read` |
| `agentic_prepare_payment` | `stateset-commerce` | `agentic` | `read` |
| `agentic_replay` | `stateset-commerce` | `agentic` | `read` |
| `agentic_replay_mutation` | `stateset-commerce` | `agentic` | `read` |
| `agentic_runtime_contract` | `stateset-commerce` | `agentic` | `read` |
| `agentic_simulate_mutation` | `stateset-commerce` | `agentic` | `read` |
| `agentic_subscribe_events` | `stateset-commerce` | `agentic` | `read` |
| `agentic_tool_catalog` | `stateset-commerce` | `agentic` | `read` |
| `agentic_unsubscribe_events` | `stateset-commerce` | `agentic` | `read` |
| `apply_cart_discount` | `stateset-commerce` | `carts` | `write` |
| `apply_cart_promotions` | `stateset-commerce` | `promotions` | `write` |
| `apply_prepayment` | `stateset-commerce` | `prepayments` | `write` |
| `apply_store_credit` | `stateset-commerce` | `store_credits` | `write` |
| `apply_vendor_credit` | `stateset-commerce` | `vendor_credits` | `write` |
| `approve_bill` | `stateset-commerce` | `accounts_payable` | `write` |
| `approve_purchase_order` | `stateset-commerce` | `suppliers` | `write` |
| `approve_return` | `stateset-commerce` | `returns` | `write` |
| `approve_review` | `stateset-commerce` | `reviews` | `write` |
| `approve_warranty_claim` | `stateset-commerce` | `warranties` | `write` |
| `archive_product` | `stateset-commerce` | `products` | `write` |
| `archive_subscription_plan` | `stateset-commerce` | `subscriptions` | `delete` |
| `assess_order_fraud` | `stateset-commerce` | `fraud` | `read` |
| `assess_wasm_connector_safety` | `stateset-commerce` | `connectors` | `read` |
| `assign_pick_task` | `stateset-commerce` | `fulfillment` | `write` |
| `audit_export` | `stateset-commerce` | `audit` | `admin` |
| `audit_query` | `stateset-commerce` | `audit` | `read` |
| `audit_retention` | `stateset-commerce` | `audit` | `admin` |
| `audit_summary` | `stateset-commerce` | `audit` | `read` |
| `backup_database` | `stateset-commerce` | `maintenance` | `write` |
| `begin_cart_checkout` | `stateset-commerce` | `carts` | `write` |
| `bulk_create_integration_field_mappings` | `stateset-commerce` | `integration-field-mappings` | `write` |
| `bulk_delete_integration_field_mappings` | `stateset-commerce` | `integration-field-mappings` | `write` |
| `bulk_upsert_integration_mappings` | `stateset-commerce` | `integration-mappings` | `write` |
| `bulk_upsert_supplier_skus` | `stateset-commerce` | `supplier_skus` | `write` |
| `calculate_cart_tax` | `stateset-commerce` | `tax` | `read` |
| `calculate_item_tax` | `stateset-commerce` | `tax` | `read` |
| `calculate_shipping_rate` | `stateset-commerce` | `shipping_zones` | `read` |
| `calculate_tax` | `stateset-commerce` | `tax` | `read` |
| `calculate_tax_quote` | `stateset-commerce` | `tax` | `read` |
| `calculate_tax_quote_with_failover` | `stateset-commerce` | `tax` | `read` |
| `cancel_backorder` | `stateset-commerce` | `backorders` | `write` |
| `cancel_bill` | `stateset-commerce` | `accounts_payable` | `write` |
| `cancel_cart` | `stateset-commerce` | `carts` | `delete` |
| `cancel_cycle_count` | `stateset-commerce` | `cycle_counts` | `write` |
| `cancel_fulfillment_wave` | `stateset-commerce` | `fulfillment` | `write` |
| `cancel_inbound_shipment` | `stateset-commerce` | `inbound_shipments` | `write` |
| `cancel_order` | `stateset-commerce` | `orders` | `delete` |
| `cancel_payment` | `stateset-commerce` | `payments` | `delete` |
| `cancel_payment_intent` | `stateset-commerce` | `payments` | `delete` |
| `cancel_pick_task` | `stateset-commerce` | `fulfillment` | `write` |
| `cancel_purchase_order` | `stateset-commerce` | `suppliers` | `write` |
| `cancel_receipt` | `stateset-commerce` | `receiving` | `write` |
| `cancel_return` | `stateset-commerce` | `returns` | `write` |
| `cancel_shipment` | `stateset-commerce` | `shipments` | `delete` |
| `cancel_subscription` | `stateset-commerce` | `subscriptions` | `delete` |
| `cancel_transfer_order` | `stateset-commerce` | `transfer_orders` | `write` |
| `cancel_vendor_credit` | `stateset-commerce` | `vendor_credits` | `write` |
| `cancel_vendor_return` | `stateset-commerce` | `vendor-returns` | `write` |
| `cancel_work_order` | `stateset-commerce` | `manufacturing` | `delete` |
| `capture_payment_intent` | `stateset-commerce` | `payments` | `write` |
| `capture_stock_snapshot` | `stateset-commerce` | `stock-snapshots` | `write` |
| `capture_topology_snapshot` | `stateset-commerce` | `topology-snapshots` | `write` |
| `certify_wasm_connector` | `stateset-commerce` | `connectors` | `admin` |
| `charge_gift_card` | `stateset-commerce` | `gift_cards` | `write` |
| `check_activity_logs_supported` | `stateset-commerce` | `activity_logs` | `read` |
| `check_channels_supported` | `stateset-commerce` | `channels` | `read` |
| `check_companies_supported` | `stateset-commerce` | `companies` | `read` |
| `check_currency_enabled` | `stateset-commerce` | `currency` | `read` |
| `check_customer_credit` | `stateset-commerce` | `credit` | `read` |
| `check_customer_tax_exempt` | `stateset-commerce` | `tax` | `read` |
| `check_gift_card_balance` | `stateset-commerce` | `gift_cards` | `read` |
| `check_inbound_shipments_supported` | `stateset-commerce` | `inbound_shipments` | `read` |
| `check_order_ready_to_pack` | `stateset-commerce` | `fulfillment` | `read` |
| `check_order_ready_to_ship` | `stateset-commerce` | `fulfillment` | `read` |
| `check_prepayments_supported` | `stateset-commerce` | `prepayments` | `read` |
| `check_price_levels_supported` | `stateset-commerce` | `price_levels` | `read` |
| `check_price_schedules_supported` | `stateset-commerce` | `price_schedules` | `read` |
| `check_production_batches_supported` | `stateset-commerce` | `production_batches` | `read` |
| `check_promotion_validity` | `stateset-commerce` | `promotions` | `read` |
| `check_serial_availability` | `stateset-commerce` | `serials` | `read` |
| `check_supplier_skus_supported` | `stateset-commerce` | `supplier_skus` | `read` |
| `check_tax_enabled` | `stateset-commerce` | `tax` | `read` |
| `check_transfer_orders_supported` | `stateset-commerce` | `transfer_orders` | `read` |
| `check_vendor_credits_supported` | `stateset-commerce` | `vendor_credits` | `read` |
| `checkout_with_crypto` | `stateset-commerce` | `checkout` | `write` |
| `clear_cart_items` | `stateset-commerce` | `carts` | `delete` |
| `close_month` | `stateset-commerce` | `general_ledger` | `write` |
| `close_ncr` | `stateset-commerce` | `quality` | `write` |
| `commit_tax_transaction` | `stateset-commerce` | `tax` | `write` |
| `complete_checkout` | `stateset-commerce` | `carts` | `write` |
| `complete_cycle_count` | `stateset-commerce` | `cycle_counts` | `write` |
| `complete_fulfillment_wave` | `stateset-commerce` | `fulfillment` | `write` |
| `complete_inspection` | `stateset-commerce` | `quality` | `write` |
| `complete_payment` | `stateset-commerce` | `payments` | `write` |
| `complete_print_job` | `stateset-commerce` | `print-stations` | `write` |
| `complete_receiving` | `stateset-commerce` | `receiving` | `write` |
| `complete_return` | `stateset-commerce` | `returns` | `write` |
| `complete_warranty_claim` | `stateset-commerce` | `warranties` | `write` |
| `complete_work_order` | `stateset-commerce` | `manufacturing` | `write` |
| `compliance_summary` | `stateset-commerce` | `compliance` | `read` |
| `configure_stripe_webhooks` | `stateset-commerce` | `import` | `write` |
| `configure_woocommerce_webhooks` | `stateset-commerce` | `import` | `write` |
| `confirm_reservation` | `stateset-commerce` | `inventory` | `write` |
| `convert_currency` | `stateset-commerce` | `currency` | `read` |
| `convert_wishlist_to_cart` | `stateset-commerce` | `wishlists` | `write` |
| `count_accounts_payable_bills` | `stateset-commerce` | `accounts_payable` | `read` |
| `count_active_quality_holds` | `stateset-commerce` | `quality` | `read` |
| `count_fulfillment_waves` | `stateset-commerce` | `fulfillment` | `read` |
| `count_lots` | `stateset-commerce` | `lots` | `read` |
| `count_pending_backorders` | `stateset-commerce` | `backorders` | `read` |
| `count_receipts` | `stateset-commerce` | `receiving` | `read` |
| `count_serials` | `stateset-commerce` | `serials` | `read` |
| `count_warehouses` | `stateset-commerce` | `warehouse` | `read` |
| `create_backorder` | `stateset-commerce` | `backorders` | `write` |
| `create_bill` | `stateset-commerce` | `accounts_payable` | `write` |
| `create_bom` | `stateset-commerce` | `manufacturing` | `write` |
| `create_cart` | `stateset-commerce` | `carts` | `write` |
| `create_channel` | `stateset-commerce` | `channels` | `write` |
| `create_company` | `stateset-commerce` | `companies` | `write` |
| `create_company_contact` | `stateset-commerce` | `companies` | `write` |
| `create_coupon` | `stateset-commerce` | `promotions` | `write` |
| `create_credit_account` | `stateset-commerce` | `credit` | `write` |
| `create_credit_memo` | `stateset-commerce` | `accounts_receivable` | `write` |
| `create_custom_object` | `stateset-commerce` | `custom_objects` | `write` |
| `create_custom_object_type` | `stateset-commerce` | `custom_objects` | `write` |
| `create_customer` | `stateset-commerce` | `customers` | `write` |
| `create_cycle_count` | `stateset-commerce` | `cycle_counts` | `write` |
| `create_edi_document` | `stateset-commerce` | `edi_documents` | `write` |
| `create_fixed_asset` | `stateset-commerce` | `fixed_assets` | `write` |
| `create_fraud_rule` | `stateset-commerce` | `fraud` | `admin` |
| `create_fulfillment_wave` | `stateset-commerce` | `fulfillment` | `write` |
| `create_gift_card` | `stateset-commerce` | `gift_cards` | `write` |
| `create_gl_account` | `stateset-commerce` | `general_ledger` | `write` |
| `create_gl_period` | `stateset-commerce` | `general_ledger` | `write` |
| `create_inbound_shipment` | `stateset-commerce` | `inbound_shipments` | `write` |
| `create_inspection` | `stateset-commerce` | `quality` | `write` |
| `create_integration_field_mapping` | `stateset-commerce` | `integration-field-mappings` | `write` |
| `create_integration_mapping` | `stateset-commerce` | `integration-mappings` | `write` |
| `create_inventory_item` | `stateset-commerce` | `inventory` | `write` |
| `create_invoice` | `stateset-commerce` | `invoices` | `write` |
| `create_location` | `stateset-commerce` | `warehouse` | `write` |
| `create_lot` | `stateset-commerce` | `lots` | `write` |
| `create_loyalty_program` | `stateset-commerce` | `loyalty` | `admin` |
| `create_ncr` | `stateset-commerce` | `quality` | `write` |
| `create_order` | `stateset-commerce` | `orders` | `write` |
| `create_payment` | `stateset-commerce` | `payments` | `write` |
| `create_payment_intent` | `stateset-commerce` | `payments` | `write` |
| `create_payment_link` | `stateset-commerce` | `checkout` | `write` |
| `create_payment_obligation` | `stateset-commerce` | `payment-obligations` | `write` |
| `create_payment_settlement_batch` | `stateset-commerce` | `payments` | `write` |
| `create_prepayment` | `stateset-commerce` | `prepayments` | `write` |
| `create_price_level` | `stateset-commerce` | `price_levels` | `write` |
| `create_price_schedule` | `stateset-commerce` | `price_schedules` | `write` |
| `create_product` | `stateset-commerce` | `products` | `write` |
| `create_production_batch` | `stateset-commerce` | `production_batches` | `write` |
| `create_promotion` | `stateset-commerce` | `promotions` | `write` |
| `create_purchase_order` | `stateset-commerce` | `suppliers` | `write` |
| `create_quality_hold` | `stateset-commerce` | `quality` | `write` |
| `create_receipt` | `stateset-commerce` | `receiving` | `write` |
| `create_receipt_from_purchase_order` | `stateset-commerce` | `receiving` | `write` |
| `create_refund` | `stateset-commerce` | `payments` | `write` |
| `create_return` | `stateset-commerce` | `returns` | `write` |
| `create_revenue_contract` | `stateset-commerce` | `revenue_recognition` | `write` |
| `create_review` | `stateset-commerce` | `reviews` | `write` |
| `create_reward` | `stateset-commerce` | `loyalty` | `admin` |
| `create_search_config` | `stateset-commerce` | `search-config` | `write` |
| `create_segment` | `stateset-commerce` | `segments` | `write` |
| `create_serial` | `stateset-commerce` | `serials` | `write` |
| `create_shipment` | `stateset-commerce` | `shipments` | `write` |
| `create_shipping_label` | `stateset-commerce` | `shipments` | `write` |
| `create_shipping_method` | `stateset-commerce` | `shipping_zones` | `write` |
| `create_shipping_zone` | `stateset-commerce` | `shipping_zones` | `write` |
| `create_stablecoin_payment` | `stateset-commerce` | `stablecoin` | `write` |
| `create_store_credit` | `stateset-commerce` | `store_credits` | `write` |
| `create_subscription` | `stateset-commerce` | `subscriptions` | `write` |
| `create_subscription_plan` | `stateset-commerce` | `subscriptions` | `write` |
| `create_supplier` | `stateset-commerce` | `suppliers` | `write` |
| `create_supplier_sku` | `stateset-commerce` | `supplier_skus` | `write` |
| `create_tax_exemption` | `stateset-commerce` | `tax` | `write` |
| `create_tax_jurisdiction` | `stateset-commerce` | `tax` | `write` |
| `create_tax_rate` | `stateset-commerce` | `tax` | `write` |
| `create_transfer_order` | `stateset-commerce` | `transfer_orders` | `write` |
| `create_unit_class` | `stateset-commerce` | `units-of-measure` | `write` |
| `create_unit_conversion_rule` | `stateset-commerce` | `units-of-measure` | `write` |
| `create_unit_of_measure` | `stateset-commerce` | `units-of-measure` | `write` |
| `create_vendor_credit` | `stateset-commerce` | `vendor_credits` | `write` |
| `create_vendor_return` | `stateset-commerce` | `vendor-returns` | `write` |
| `create_warehouse` | `stateset-commerce` | `warehouse` | `write` |
| `create_warranty` | `stateset-commerce` | `warranties` | `write` |
| `create_warranty_claim` | `stateset-commerce` | `warranties` | `write` |
| `create_wishlist` | `stateset-commerce` | `wishlists` | `write` |
| `create_work_order` | `stateset-commerce` | `manufacturing` | `write` |
| `deactivate_promotion` | `stateset-commerce` | `promotions` | `write` |
| `delegate_to_agent` | `stateset-commerce` | `agentic` | `write` |
| `delete_cart` | `stateset-commerce` | `carts` | `delete` |
| `delete_channel` | `stateset-commerce` | `channels` | `write` |
| `delete_company` | `stateset-commerce` | `companies` | `write` |
| `delete_custom_object` | `stateset-commerce` | `custom_objects` | `delete` |
| `delete_custom_object_type` | `stateset-commerce` | `custom_objects` | `delete` |
| `delete_customer` | `stateset-commerce` | `customers` | `write` |
| `delete_customer_address` | `stateset-commerce` | `customers` | `write` |
| `delete_exchange_rate` | `stateset-commerce` | `currency` | `delete` |
| `delete_gdpr_data` | `stateset-commerce` | `compliance` | `admin` |
| `delete_integration_field_mapping` | `stateset-commerce` | `integration-field-mappings` | `write` |
| `delete_integration_mapping` | `stateset-commerce` | `integration-mappings` | `write` |
| `delete_price_level` | `stateset-commerce` | `price_levels` | `write` |
| `delete_price_level_entry` | `stateset-commerce` | `price_levels` | `write` |
| `delete_price_schedule` | `stateset-commerce` | `price_schedules` | `write` |
| `delete_price_schedule_entry` | `stateset-commerce` | `price_schedules` | `write` |
| `delete_product` | `stateset-commerce` | `products` | `write` |
| `delete_product_variant` | `stateset-commerce` | `products` | `write` |
| `delete_production_batch` | `stateset-commerce` | `production_batches` | `write` |
| `delete_promotion` | `stateset-commerce` | `promotions` | `delete` |
| `delete_purgatory_order` | `stateset-commerce` | `purgatory` | `write` |
| `delete_search_config` | `stateset-commerce` | `search-config` | `write` |
| `delete_stock_snapshot` | `stateset-commerce` | `stock-snapshots` | `write` |
| `delete_supplier_sku` | `stateset-commerce` | `supplier_skus` | `write` |
| `delete_topology_snapshot` | `stateset-commerce` | `topology-snapshots` | `write` |
| `delete_unit_class` | `stateset-commerce` | `units-of-measure` | `write` |
| `delete_unit_conversion_rule` | `stateset-commerce` | `units-of-measure` | `write` |
| `delete_unit_of_measure` | `stateset-commerce` | `units-of-measure` | `write` |
| `deliver_shipment` | `stateset-commerce` | `shipments` | `write` |
| `deny_warranty_claim` | `stateset-commerce` | `warranties` | `write` |
| `disable_gift_card` | `stateset-commerce` | `gift_cards` | `write` |
| `discover_agents` | `stateset-commerce` | `agent_cards` | `read` |
| `discover_tools` | `stateset-commerce` | `agentic` | `read` |
| `dispose_fixed_asset` | `stateset-commerce` | `fixed_assets` | `write` |
| `earn_points` | `stateset-commerce` | `loyalty` | `write` |
| `enable_currencies` | `stateset-commerce` | `currency` | `admin` |
| `enqueue_print_job` | `stateset-commerce` | `print-stations` | `write` |
| `enroll_customer` | `stateset-commerce` | `loyalty` | `write` |
| `erc8004_get_by_wallet` | `stateset-commerce` | `erc8004` | `read` |
| `erc8004_get_identity` | `stateset-commerce` | `erc8004` | `read` |
| `erc8004_link_wallet` | `stateset-commerce` | `erc8004` | `write` |
| `erc8004_list_identities` | `stateset-commerce` | `erc8004` | `read` |
| `erc8004_register_identity` | `stateset-commerce` | `erc8004` | `admin` |
| `evaluate_policy` | `stateset-commerce` | `policies` | `read` |
| `evaluate_segment_membership` | `stateset-commerce` | `segments` | `read` |
| `execute_wasm_connector` | `stateset-commerce` | `connectors` | `write` |
| `expire_cart` | `stateset-commerce` | `carts` | `write` |
| `explain_policy_denial` | `stateset-commerce` | `policies` | `read` |
| `export_agent_catalog` | `stateset-commerce` | `catalog` | `read` |
| `export_audit_trail` | `stateset-commerce` | `compliance` | `admin` |
| `export_compliance_package` | `stateset-commerce` | `proofs` | `read` |
| `export_data` | `stateset-commerce` | `import` | `read` |
| `export_full_data` | `stateset-commerce` | `maintenance` | `read` |
| `export_gdpr_data` | `stateset-commerce` | `compliance` | `admin` |
| `express_checkout` | `stateset-commerce` | `checkout` | `write` |
| `find_or_create_customer` | `stateset-commerce` | `customers` | `write` |
| `flag_review` | `stateset-commerce` | `reviews` | `write` |
| `format_currency` | `stateset-commerce` | `currency` | `read` |
| `generate_1099k` | `stateset-commerce` | `compliance` | `admin` |
| `generate_depreciation_schedule` | `stateset-commerce` | `fixed_assets` | `write` |
| `generate_inclusion_proof` | `stateset-commerce` | `proofs` | `read` |
| `generate_receipt_bundle` | `stateset-commerce` | `proofs` | `read` |
| `generate_revenue_schedule` | `stateset-commerce` | `revenue_recognition` | `write` |
| `get_abandoned_carts` | `stateset-commerce` | `carts` | `read` |
| `get_accounts_payable_aging_summary` | `stateset-commerce` | `accounts_payable` | `read` |
| `get_accounts_payable_total_outstanding` | `stateset-commerce` | `accounts_payable` | `read` |
| `get_accounts_receivable_aging_summary` | `stateset-commerce` | `accounts_receivable` | `read` |
| `get_accounts_receivable_total_outstanding` | `stateset-commerce` | `accounts_receivable` | `read` |
| `get_active_promotions` | `stateset-commerce` | `promotions` | `read` |
| `get_active_search_config` | `stateset-commerce` | `search-config` | `read` |
| `get_activity_history_for_subject` | `stateset-commerce` | `activity_logs` | `read` |
| `get_activity_log` | `stateset-commerce` | `activity_logs` | `read` |
| `get_agent_card` | `stateset-commerce` | `agent_cards` | `read` |
| `get_agent_wallet` | `stateset-commerce` | `stablecoin` | `read` |
| `get_backorder` | `stateset-commerce` | `backorders` | `read` |
| `get_backorder_summary` | `stateset-commerce` | `backorders` | `read` |
| `get_balance_sheet` | `stateset-commerce` | `general_ledger` | `read` |
| `get_bill` | `stateset-commerce` | `accounts_payable` | `read` |
| `get_billing_cycle` | `stateset-commerce` | `subscriptions` | `read` |
| `get_bom` | `stateset-commerce` | `manufacturing` | `read` |
| `get_cart` | `stateset-commerce` | `carts` | `read` |
| `get_channel` | `stateset-commerce` | `channels` | `read` |
| `get_company` | `stateset-commerce` | `companies` | `read` |
| `get_coupon` | `stateset-commerce` | `promotions` | `read` |
| `get_credit_account` | `stateset-commerce` | `credit` | `read` |
| `get_credit_memo` | `stateset-commerce` | `accounts_receivable` | `read` |
| `get_currency_settings` | `stateset-commerce` | `currency` | `read` |
| `get_custom_object` | `stateset-commerce` | `custom_objects` | `read` |
| `get_custom_object_by_handle` | `stateset-commerce` | `custom_objects` | `read` |
| `get_custom_object_type` | `stateset-commerce` | `custom_objects` | `read` |
| `get_custom_object_type_by_handle` | `stateset-commerce` | `custom_objects` | `read` |
| `get_customer` | `stateset-commerce` | `customers` | `read` |
| `get_customer_metrics` | `stateset-commerce` | `analytics` | `read` |
| `get_customer_tax_exemptions` | `stateset-commerce` | `tax` | `read` |
| `get_cycle_count` | `stateset-commerce` | `cycle_counts` | `read` |
| `get_days_sales_outstanding` | `stateset-commerce` | `accounts_receivable` | `read` |
| `get_demand_forecast` | `stateset-commerce` | `analytics` | `read` |
| `get_depreciation_schedule` | `stateset-commerce` | `fixed_assets` | `read` |
| `get_edi_document` | `stateset-commerce` | `edi_documents` | `read` |
| `get_edi_summary` | `stateset-commerce` | `edi_documents` | `read` |
| `get_exchange_rate` | `stateset-commerce` | `currency` | `read` |
| `get_expired_carts` | `stateset-commerce` | `carts` | `read` |
| `get_fixed_asset` | `stateset-commerce` | `fixed_assets` | `read` |
| `get_fraud_assessment` | `stateset-commerce` | `fraud` | `read` |
| `get_fulfillment_metrics` | `stateset-commerce` | `analytics` | `read` |
| `get_fulfillment_wave` | `stateset-commerce` | `fulfillment` | `read` |
| `get_gift_card` | `stateset-commerce` | `gift_cards` | `read` |
| `get_gl_account` | `stateset-commerce` | `general_ledger` | `read` |
| `get_gl_account_balance` | `stateset-commerce` | `general_ledger` | `read` |
| `get_inbound_shipment` | `stateset-commerce` | `inbound_shipments` | `read` |
| `get_income_statement` | `stateset-commerce` | `general_ledger` | `read` |
| `get_inspection` | `stateset-commerce` | `quality` | `read` |
| `get_installed_connector` | `stateset-commerce` | `connectors` | `read` |
| `get_integration_field_mapping` | `stateset-commerce` | `integration-field-mappings` | `read` |
| `get_integration_mapping` | `stateset-commerce` | `integration-mappings` | `read` |
| `get_inventory_health` | `stateset-commerce` | `analytics` | `read` |
| `get_inventory_movement` | `stateset-commerce` | `analytics` | `read` |
| `get_invoice` | `stateset-commerce` | `invoices` | `read` |
| `get_item_cost` | `stateset-commerce` | `cost_accounting` | `read` |
| `get_journal_entry` | `stateset-commerce` | `general_ledger` | `read` |
| `get_latest_stock_snapshot` | `stateset-commerce` | `stock-snapshots` | `read` |
| `get_latest_topology_snapshot` | `stateset-commerce` | `topology-snapshots` | `read` |
| `get_location` | `stateset-commerce` | `warehouse` | `read` |
| `get_lot` | `stateset-commerce` | `lots` | `read` |
| `get_low_stock_items` | `stateset-commerce` | `analytics` | `read` |
| `get_loyalty_account` | `stateset-commerce` | `loyalty` | `read` |
| `get_loyalty_program` | `stateset-commerce` | `loyalty` | `read` |
| `get_ncr` | `stateset-commerce` | `quality` | `read` |
| `get_order` | `stateset-commerce` | `orders` | `read` |
| `get_order_status_breakdown` | `stateset-commerce` | `analytics` | `read` |
| `get_overdue_invoices` | `stateset-commerce` | `invoices` | `read` |
| `get_payment` | `stateset-commerce` | `payments` | `read` |
| `get_payment_intent` | `stateset-commerce` | `payments` | `read` |
| `get_payment_link_status` | `stateset-commerce` | `checkout` | `read` |
| `get_payment_obligation` | `stateset-commerce` | `payment-obligations` | `read` |
| `get_payment_obligation_dashboard` | `stateset-commerce` | `payment-obligations` | `read` |
| `get_pick_task` | `stateset-commerce` | `fulfillment` | `read` |
| `get_prepayment` | `stateset-commerce` | `prepayments` | `read` |
| `get_price_level` | `stateset-commerce` | `price_levels` | `read` |
| `get_price_schedule` | `stateset-commerce` | `price_schedules` | `read` |
| `get_print_station` | `stateset-commerce` | `print-stations` | `read` |
| `get_product` | `stateset-commerce` | `products` | `read` |
| `get_product_by_slug` | `stateset-commerce` | `products` | `read` |
| `get_product_performance` | `stateset-commerce` | `analytics` | `read` |
| `get_product_spec` | `stateset-commerce` | `catalog` | `read` |
| `get_product_variant` | `stateset-commerce` | `products` | `read` |
| `get_production_batch` | `stateset-commerce` | `production_batches` | `read` |
| `get_promotion` | `stateset-commerce` | `promotions` | `read` |
| `get_purchase_order` | `stateset-commerce` | `suppliers` | `read` |
| `get_purgatory_order` | `stateset-commerce` | `purgatory` | `read` |
| `get_quality_hold` | `stateset-commerce` | `quality` | `read` |
| `get_receipt` | `stateset-commerce` | `receiving` | `read` |
| `get_return` | `stateset-commerce` | `returns` | `read` |
| `get_return_metrics` | `stateset-commerce` | `analytics` | `read` |
| `get_revenue_by_period` | `stateset-commerce` | `analytics` | `read` |
| `get_revenue_contract` | `stateset-commerce` | `revenue_recognition` | `read` |
| `get_revenue_forecast` | `stateset-commerce` | `analytics` | `read` |
| `get_revenue_schedule` | `stateset-commerce` | `revenue_recognition` | `read` |
| `get_review` | `stateset-commerce` | `reviews` | `read` |
| `get_review_summary` | `stateset-commerce` | `reviews` | `read` |
| `get_sales_summary` | `stateset-commerce` | `analytics` | `read` |
| `get_search_config` | `stateset-commerce` | `search-config` | `read` |
| `get_segment` | `stateset-commerce` | `segments` | `read` |
| `get_serial` | `stateset-commerce` | `serials` | `read` |
| `get_shipment` | `stateset-commerce` | `shipments` | `read` |
| `get_shipping_rates` | `stateset-commerce` | `carts` | `read` |
| `get_shipping_zone` | `stateset-commerce` | `shipping_zones` | `read` |
| `get_stock` | `stateset-commerce` | `inventory` | `read` |
| `get_stock_snapshot` | `stateset-commerce` | `stock-snapshots` | `read` |
| `get_store_credit` | `stateset-commerce` | `store_credits` | `read` |
| `get_subscription` | `stateset-commerce` | `subscriptions` | `read` |
| `get_subscription_events` | `stateset-commerce` | `subscriptions` | `read` |
| `get_subscription_plan` | `stateset-commerce` | `subscriptions` | `read` |
| `get_supplier` | `stateset-commerce` | `suppliers` | `read` |
| `get_supplier_sku` | `stateset-commerce` | `supplier_skus` | `read` |
| `get_tax_exemption` | `stateset-commerce` | `tax` | `read` |
| `get_tax_jurisdiction` | `stateset-commerce` | `tax` | `read` |
| `get_tax_quote` | `stateset-commerce` | `tax` | `read` |
| `get_tax_rate` | `stateset-commerce` | `tax` | `read` |
| `get_tax_rate_record` | `stateset-commerce` | `tax` | `read` |
| `get_tax_settings` | `stateset-commerce` | `tax` | `read` |
| `get_tax_transaction` | `stateset-commerce` | `tax` | `read` |
| `get_top_customers` | `stateset-commerce` | `analytics` | `read` |
| `get_top_products` | `stateset-commerce` | `analytics` | `read` |
| `get_topology_snapshot` | `stateset-commerce` | `topology-snapshots` | `read` |
| `get_total_inventory_value` | `stateset-commerce` | `cost_accounting` | `read` |
| `get_transfer_order` | `stateset-commerce` | `transfer_orders` | `read` |
| `get_trial_balance` | `stateset-commerce` | `general_ledger` | `read` |
| `get_us_state_tax_info` | `stateset-commerce` | `tax` | `read` |
| `get_vendor_credit` | `stateset-commerce` | `vendor_credits` | `read` |
| `get_vendor_return` | `stateset-commerce` | `vendor-returns` | `read` |
| `get_wallet_balance` | `stateset-commerce` | `stablecoin` | `read` |
| `get_warehouse` | `stateset-commerce` | `warehouse` | `read` |
| `get_warehouse_sku_available_quantity` | `stateset-commerce` | `warehouse` | `read` |
| `get_warranty` | `stateset-commerce` | `warranties` | `read` |
| `get_wishlist` | `stateset-commerce` | `wishlists` | `read` |
| `get_work_order` | `stateset-commerce` | `manufacturing` | `read` |
| `handle_fulfillment_exception` | `stateset-commerce` | `shipments` | `write` |
| `import_csv` | `stateset-commerce` | `import` | `write` |
| `import_full_data` | `stateset-commerce` | `maintenance` | `write` |
| `import_json` | `stateset-commerce` | `import` | `write` |
| `import_shopify_data` | `stateset-commerce` | `import` | `write` |
| `import_shopify_shadow_data` | `stateset-commerce` | `import` | `write` |
| `import_status` | `stateset-commerce` | `import` | `read` |
| `import_woocommerce_data` | `stateset-commerce` | `import` | `write` |
| `ingest_payment_provider_webhook` | `stateset-commerce` | `payments` | `write` |
| `ingest_purgatory_order` | `stateset-commerce` | `purgatory` | `write` |
| `ingest_shipping_provider_webhook` | `stateset-commerce` | `shipments` | `write` |
| `ingest_tax_provider_webhook` | `stateset-commerce` | `tax` | `write` |
| `initialize_chart_of_accounts` | `stateset-commerce` | `general_ledger` | `write` |
| `inspect_batch` | `stateset-commerce` | `proofs` | `read` |
| `install_wasm_connector` | `stateset-commerce` | `connectors` | `write` |
| `link_payment_obligation_bill` | `stateset-commerce` | `payment-obligations` | `write` |
| `list_active_lots` | `stateset-commerce` | `lots` | `read` |
| `list_active_quality_holds` | `stateset-commerce` | `quality` | `read` |
| `list_activity_logs` | `stateset-commerce` | `activity_logs` | `read` |
| `list_agent_cards` | `stateset-commerce` | `agent_cards` | `read` |
| `list_available_lots_for_sku` | `stateset-commerce` | `lots` | `read` |
| `list_available_serials` | `stateset-commerce` | `serials` | `read` |
| `list_backorders` | `stateset-commerce` | `backorders` | `read` |
| `list_backorders_for_order` | `stateset-commerce` | `backorders` | `read` |
| `list_backorders_for_sku` | `stateset-commerce` | `backorders` | `read` |
| `list_billing_cycles` | `stateset-commerce` | `subscriptions` | `read` |
| `list_bills` | `stateset-commerce` | `accounts_payable` | `read` |
| `list_bills_due_soon` | `stateset-commerce` | `accounts_payable` | `read` |
| `list_boms` | `stateset-commerce` | `manufacturing` | `read` |
| `list_cart_items` | `stateset-commerce` | `carts` | `read` |
| `list_carts` | `stateset-commerce` | `carts` | `read` |
| `list_channel_product_mappings` | `stateset-commerce` | `channels` | `read` |
| `list_channels` | `stateset-commerce` | `channels` | `read` |
| `list_companies` | `stateset-commerce` | `companies` | `read` |
| `list_company_addresses` | `stateset-commerce` | `companies` | `read` |
| `list_company_contacts` | `stateset-commerce` | `companies` | `read` |
| `list_connector_marketplace` | `stateset-commerce` | `connectors` | `read` |
| `list_coupons` | `stateset-commerce` | `promotions` | `read` |
| `list_credit_accounts` | `stateset-commerce` | `credit` | `read` |
| `list_credit_memos` | `stateset-commerce` | `accounts_receivable` | `read` |
| `list_custom_object_types` | `stateset-commerce` | `custom_objects` | `read` |
| `list_custom_objects` | `stateset-commerce` | `custom_objects` | `read` |
| `list_customer_addresses` | `stateset-commerce` | `customers` | `read` |
| `list_customer_carts` | `stateset-commerce` | `carts` | `read` |
| `list_customers` | `stateset-commerce` | `customers` | `read` |
| `list_cycle_counts` | `stateset-commerce` | `cycle_counts` | `read` |
| `list_edi_documents` | `stateset-commerce` | `edi_documents` | `read` |
| `list_exchange_rates` | `stateset-commerce` | `currency` | `read` |
| `list_expired_lots` | `stateset-commerce` | `lots` | `read` |
| `list_expiring_lots` | `stateset-commerce` | `lots` | `read` |
| `list_fixed_assets` | `stateset-commerce` | `fixed_assets` | `read` |
| `list_fraud_signals` | `stateset-commerce` | `fraud` | `read` |
| `list_fulfillment_waves` | `stateset-commerce` | `fulfillment` | `read` |
| `list_gift_cards` | `stateset-commerce` | `gift_cards` | `read` |
| `list_gl_accounts` | `stateset-commerce` | `general_ledger` | `read` |
| `list_gl_periods` | `stateset-commerce` | `general_ledger` | `read` |
| `list_id_mappings` | `stateset-commerce` | `import` | `read` |
| `list_inbound_shipments` | `stateset-commerce` | `inbound_shipments` | `read` |
| `list_inspections` | `stateset-commerce` | `quality` | `read` |
| `list_installed_connectors` | `stateset-commerce` | `connectors` | `read` |
| `list_integration_field_mappings` | `stateset-commerce` | `integration-field-mappings` | `read` |
| `list_integration_mapping_groups` | `stateset-commerce` | `integration-field-mappings` | `read` |
| `list_integration_mappings` | `stateset-commerce` | `integration-mappings` | `read` |
| `list_invoices` | `stateset-commerce` | `invoices` | `read` |
| `list_item_costs` | `stateset-commerce` | `cost_accounting` | `read` |
| `list_journal_entries` | `stateset-commerce` | `general_ledger` | `read` |
| `list_locations` | `stateset-commerce` | `warehouse` | `read` |
| `list_lots` | `stateset-commerce` | `lots` | `read` |
| `list_ncrs` | `stateset-commerce` | `quality` | `read` |
| `list_orders` | `stateset-commerce` | `orders` | `read` |
| `list_over_limit_credit_accounts` | `stateset-commerce` | `credit` | `read` |
| `list_overdue_backorders` | `stateset-commerce` | `backorders` | `read` |
| `list_overdue_bills` | `stateset-commerce` | `accounts_payable` | `read` |
| `list_payment_intents` | `stateset-commerce` | `payments` | `read` |
| `list_payment_links` | `stateset-commerce` | `checkout` | `read` |
| `list_payment_obligations` | `stateset-commerce` | `payment-obligations` | `read` |
| `list_payment_providers` | `stateset-commerce` | `payments` | `read` |
| `list_payment_settlement_batches` | `stateset-commerce` | `payments` | `read` |
| `list_payment_settlements` | `stateset-commerce` | `payments` | `read` |
| `list_payments` | `stateset-commerce` | `payments` | `read` |
| `list_pending_returns` | `stateset-commerce` | `returns` | `read` |
| `list_pick_tasks` | `stateset-commerce` | `fulfillment` | `read` |
| `list_pickable_locations` | `stateset-commerce` | `warehouse` | `read` |
| `list_policies` | `stateset-commerce` | `policies` | `read` |
| `list_portable_domains` | `stateset-commerce` | `maintenance` | `read` |
| `list_prepayment_applications` | `stateset-commerce` | `prepayments` | `read` |
| `list_prepayments` | `stateset-commerce` | `prepayments` | `read` |
| `list_price_level_entries` | `stateset-commerce` | `price_levels` | `read` |
| `list_price_levels` | `stateset-commerce` | `price_levels` | `read` |
| `list_price_schedule_entries` | `stateset-commerce` | `price_schedules` | `read` |
| `list_price_schedules` | `stateset-commerce` | `price_schedules` | `read` |
| `list_print_jobs` | `stateset-commerce` | `print-stations` | `read` |
| `list_print_stations` | `stateset-commerce` | `print-stations` | `read` |
| `list_product_variants` | `stateset-commerce` | `products` | `read` |
| `list_production_batches` | `stateset-commerce` | `production_batches` | `read` |
| `list_products` | `stateset-commerce` | `products` | `read` |
| `list_promotions` | `stateset-commerce` | `promotions` | `read` |
| `list_purchase_orders` | `stateset-commerce` | `suppliers` | `read` |
| `list_purgatory_orders` | `stateset-commerce` | `purgatory` | `read` |
| `list_quality_holds` | `stateset-commerce` | `quality` | `read` |
| `list_quarantined_lots` | `stateset-commerce` | `lots` | `read` |
| `list_receipts` | `stateset-commerce` | `receiving` | `read` |
| `list_returns` | `stateset-commerce` | `returns` | `read` |
| `list_returns_for_customer` | `stateset-commerce` | `returns` | `read` |
| `list_returns_for_order` | `stateset-commerce` | `returns` | `read` |
| `list_revenue_contracts` | `stateset-commerce` | `revenue_recognition` | `read` |
| `list_reviews` | `stateset-commerce` | `reviews` | `read` |
| `list_rewards` | `stateset-commerce` | `loyalty` | `read` |
| `list_search_configs` | `stateset-commerce` | `search-config` | `read` |
| `list_segments` | `stateset-commerce` | `segments` | `read` |
| `list_serials` | `stateset-commerce` | `serials` | `read` |
| `list_shipments` | `stateset-commerce` | `shipments` | `read` |
| `list_shipping_labels` | `stateset-commerce` | `shipments` | `read` |
| `list_shipping_methods` | `stateset-commerce` | `shipping_zones` | `read` |
| `list_shipping_providers` | `stateset-commerce` | `shipments` | `read` |
| `list_shipping_zones` | `stateset-commerce` | `shipping_zones` | `read` |
| `list_stock_snapshots` | `stateset-commerce` | `stock-snapshots` | `read` |
| `list_store_credits` | `stateset-commerce` | `store_credits` | `read` |
| `list_subscription_plans` | `stateset-commerce` | `subscriptions` | `read` |
| `list_subscriptions` | `stateset-commerce` | `subscriptions` | `read` |
| `list_supplier_skus` | `stateset-commerce` | `supplier_skus` | `read` |
| `list_suppliers` | `stateset-commerce` | `suppliers` | `read` |
| `list_supported_chains` | `stateset-commerce` | `stablecoin` | `read` |
| `list_tax_jurisdictions` | `stateset-commerce` | `tax` | `read` |
| `list_tax_providers` | `stateset-commerce` | `tax` | `read` |
| `list_tax_rates` | `stateset-commerce` | `tax` | `read` |
| `list_tax_transactions` | `stateset-commerce` | `tax` | `read` |
| `list_topology_snapshots` | `stateset-commerce` | `topology-snapshots` | `read` |
| `list_transfer_orders` | `stateset-commerce` | `transfer_orders` | `read` |
| `list_unapplied_credits` | `stateset-commerce` | `accounts_receivable` | `read` |
| `list_unit_classes` | `stateset-commerce` | `units-of-measure` | `read` |
| `list_unit_conversion_rules` | `stateset-commerce` | `units-of-measure` | `read` |
| `list_units_of_measure` | `stateset-commerce` | `units-of-measure` | `read` |
| `list_vendor_credit_applications` | `stateset-commerce` | `vendor_credits` | `read` |
| `list_vendor_credits` | `stateset-commerce` | `vendor_credits` | `read` |
| `list_vendor_returns` | `stateset-commerce` | `vendor-returns` | `read` |
| `list_warehouses` | `stateset-commerce` | `warehouse` | `read` |
| `list_warranties` | `stateset-commerce` | `warranties` | `read` |
| `list_wishlists` | `stateset-commerce` | `wishlists` | `read` |
| `list_work_orders` | `stateset-commerce` | `manufacturing` | `read` |
| `load_policy_file` | `stateset-commerce` | `policies` | `write` |
| `map_purgatory_line` | `stateset-commerce` | `purgatory` | `write` |
| `mark_cart_ready_for_payment` | `stateset-commerce` | `carts` | `write` |
| `mark_failed_payment` | `stateset-commerce` | `payments` | `write` |
| `mark_inbound_shipment_arrived` | `stateset-commerce` | `inbound_shipments` | `write` |
| `mark_inbound_shipment_in_transit` | `stateset-commerce` | `inbound_shipments` | `write` |
| `mark_return_received` | `stateset-commerce` | `returns` | `write` |
| `mark_serial_sold` | `stateset-commerce` | `serials` | `write` |
| `match_agent_to_products` | `stateset-commerce` | `catalog` | `read` |
| `match_product_to_agents` | `stateset-commerce` | `catalog` | `read` |
| `open_gl_period` | `stateset-commerce` | `general_ledger` | `write` |
| `pair_print_station` | `stateset-commerce` | `print-stations` | `write` |
| `pause_subscription` | `stateset-commerce` | `subscriptions` | `write` |
| `pick_up_next_print_job` | `stateset-commerce` | `print-stations` | `write` |
| `place_asset_in_service` | `stateset-commerce` | `fixed_assets` | `write` |
| `post_depreciation` | `stateset-commerce` | `fixed_assets` | `write` |
| `post_journal_entry` | `stateset-commerce` | `general_ledger` | `write` |
| `post_purgatory_order` | `stateset-commerce` | `purgatory` | `write` |
| `process_vendor_return` | `stateset-commerce` | `vendor-returns` | `write` |
| `publish_product_catalog` | `stateset-commerce` | `catalog` | `write` |
| `publish_wasm_connector` | `stateset-commerce` | `connectors` | `admin` |
| `quarantine_lot` | `stateset-commerce` | `lots` | `write` |
| `quarantine_serial` | `stateset-commerce` | `serials` | `write` |
| `query_agent_catalog` | `stateset-commerce` | `catalog` | `read` |
| `quote_shipping_rates` | `stateset-commerce` | `shipments` | `read` |
| `reactivate_credit_account` | `stateset-commerce` | `credit` | `write` |
| `rebuild_dynamic_segment` | `stateset-commerce` | `segments` | `write` |
| `recalculate_cart` | `stateset-commerce` | `carts` | `write` |
| `receive_inbound_shipment_line` | `stateset-commerce` | `inbound_shipments` | `write` |
| `receive_transfer_order_line` | `stateset-commerce` | `transfer_orders` | `write` |
| `recognize_revenue` | `stateset-commerce` | `revenue_recognition` | `write` |
| `reconcile_payment_provider` | `stateset-commerce` | `payments` | `read` |
| `record_activity` | `stateset-commerce` | `activity_logs` | `write` |
| `record_cycle_counts` | `stateset-commerce` | `cycle_counts` | `write` |
| `record_invoice_payment` | `stateset-commerce` | `invoices` | `write` |
| `record_payment_obligation_payment` | `stateset-commerce` | `payment-obligations` | `write` |
| `record_promotion_usage` | `stateset-commerce` | `promotions` | `write` |
| `redeem_points` | `stateset-commerce` | `loyalty` | `write` |
| `refund_payment_intent` | `stateset-commerce` | `payments` | `write` |
| `refund_prepayment` | `stateset-commerce` | `prepayments` | `write` |
| `refund_to_gift_card` | `stateset-commerce` | `gift_cards` | `write` |
| `register_agent_card` | `stateset-commerce` | `agent_cards` | `write` |
| `register_policy_template` | `stateset-commerce` | `policies` | `write` |
| `reject_return` | `stateset-commerce` | `returns` | `write` |
| `reject_review` | `stateset-commerce` | `reviews` | `write` |
| `release_cart_inventory` | `stateset-commerce` | `carts` | `write` |
| `release_fulfillment_wave` | `stateset-commerce` | `fulfillment` | `write` |
| `release_lot_quarantine` | `stateset-commerce` | `lots` | `write` |
| `release_quality_hold` | `stateset-commerce` | `quality` | `write` |
| `release_reservation` | `stateset-commerce` | `inventory` | `write` |
| `remove_cart_discount` | `stateset-commerce` | `carts` | `delete` |
| `remove_cart_item` | `stateset-commerce` | `carts` | `delete` |
| `remove_from_wishlist` | `stateset-commerce` | `wishlists` | `write` |
| `remove_production_batch_work_order` | `stateset-commerce` | `production_batches` | `write` |
| `reserve_cart_inventory` | `stateset-commerce` | `carts` | `write` |
| `reserve_inventory` | `stateset-commerce` | `inventory` | `write` |
| `resolve_integration_mapping` | `stateset-commerce` | `integration-mappings` | `read` |
| `resolve_payment_link` | `stateset-commerce` | `checkout` | `read` |
| `resolve_scheduled_price` | `stateset-commerce` | `price_schedules` | `read` |
| `restore_database` | `stateset-commerce` | `maintenance` | `write` |
| `resume_subscription` | `stateset-commerce` | `subscriptions` | `write` |
| `revalue_gl` | `stateset-commerce` | `general_ledger` | `write` |
| `reverse_prepayment_application` | `stateset-commerce` | `prepayments` | `write` |
| `reverse_vendor_credit_application` | `stateset-commerce` | `vendor_credits` | `write` |
| `review_flagged_order` | `stateset-commerce` | `fraud` | `write` |
| `revoke_payment_link` | `stateset-commerce` | `checkout` | `write` |
| `revoke_print_station` | `stateset-commerce` | `print-stations` | `write` |
| `search_products` | `stateset-commerce` | `products` | `read` |
| `send_invoice` | `stateset-commerce` | `invoices` | `write` |
| `send_purchase_order` | `stateset-commerce` | `suppliers` | `write` |
| `set_active_search_config` | `stateset-commerce` | `search-config` | `write` |
| `set_base_currency` | `stateset-commerce` | `currency` | `admin` |
| `set_base_unit_of_measure` | `stateset-commerce` | `units-of-measure` | `write` |
| `set_cart_billing_address` | `stateset-commerce` | `carts` | `write` |
| `set_cart_payment` | `stateset-commerce` | `carts` | `write` |
| `set_cart_shipping` | `stateset-commerce` | `carts` | `write` |
| `set_cart_shipping_address` | `stateset-commerce` | `carts` | `write` |
| `set_cart_tax` | `stateset-commerce` | `carts` | `write` |
| `set_channel_lock` | `stateset-commerce` | `channels` | `write` |
| `set_default_customer_address` | `stateset-commerce` | `customers` | `write` |
| `set_edi_document_status` | `stateset-commerce` | `edi_documents` | `write` |
| `set_exchange_rate` | `stateset-commerce` | `currency` | `admin` |
| `set_exchange_rates` | `stateset-commerce` | `currency` | `admin` |
| `set_item_cost` | `stateset-commerce` | `cost_accounting` | `write` |
| `set_payment_obligation_status` | `stateset-commerce` | `payment-obligations` | `write` |
| `set_price_level_entry` | `stateset-commerce` | `price_levels` | `write` |
| `set_price_schedule_entry` | `stateset-commerce` | `price_schedules` | `write` |
| `set_tax_enabled` | `stateset-commerce` | `tax` | `write` |
| `ship_order` | `stateset-commerce` | `orders` | `write` |
| `ship_shipment` | `stateset-commerce` | `shipments` | `write` |
| `ship_transfer_order` | `stateset-commerce` | `transfer_orders` | `write` |
| `sign_wasm_connector_attestation` | `stateset-commerce` | `connectors` | `admin` |
| `skip_billing_cycle` | `stateset-commerce` | `subscriptions` | `write` |
| `soc2_evidence` | `stateset-commerce` | `compliance` | `admin` |
| `start_cycle_count` | `stateset-commerce` | `cycle_counts` | `write` |
| `start_inspection` | `stateset-commerce` | `quality` | `write` |
| `start_pick_task` | `stateset-commerce` | `fulfillment` | `write` |
| `start_receiving` | `stateset-commerce` | `receiving` | `write` |
| `start_work_order` | `stateset-commerce` | `manufacturing` | `write` |
| `submit_purchase_order` | `stateset-commerce` | `suppliers` | `write` |
| `submit_vendor_return` | `stateset-commerce` | `vendor-returns` | `write` |
| `suspend_credit_account` | `stateset-commerce` | `credit` | `write` |
| `sync_conflicts` | `stateset-commerce` | `sync` | `read` |
| `sync_decrypt_event` | `stateset-commerce` | `sync` | `read` |
| `sync_entity_history` | `stateset-commerce` | `sync` | `read` |
| `sync_full` | `stateset-commerce` | `sync` | `admin` |
| `sync_inspect_commitment` | `stateset-commerce` | `sync` | `read` |
| `sync_outbox` | `stateset-commerce` | `sync` | `read` |
| `sync_pull` | `stateset-commerce` | `sync` | `write` |
| `sync_pulled_events` | `stateset-commerce` | `sync` | `read` |
| `sync_push` | `stateset-commerce` | `sync` | `write` |
| `sync_rebase` | `stateset-commerce` | `sync` | `admin` |
| `sync_resolve` | `stateset-commerce` | `sync` | `admin` |
| `sync_retry_failed` | `stateset-commerce` | `sync` | `admin` |
| `sync_status` | `stateset-commerce` | `sync` | `read` |
| `sync_verify_inclusion` | `stateset-commerce` | `sync` | `read` |
| `sync_verify_receipt` | `stateset-commerce` | `sync` | `read` |
| `three_way_match_bill` | `stateset-commerce` | `accounts_payable` | `read` |
| `track_shipping_label` | `stateset-commerce` | `shipments` | `read` |
| `treasury_balance` | `stateset-commerce` | `treasury` | `read` |
| `treasury_buy` | `stateset-commerce` | `treasury` | `write` |
| `treasury_deposit` | `stateset-commerce` | `treasury` | `write` |
| `treasury_ledger` | `stateset-commerce` | `treasury` | `read` |
| `treasury_list_tokens` | `stateset-commerce` | `treasury` | `read` |
| `treasury_register_token` | `stateset-commerce` | `treasury` | `admin` |
| `uninstall_wasm_connector` | `stateset-commerce` | `connectors` | `delete` |
| `update_average_item_cost` | `stateset-commerce` | `cost_accounting` | `write` |
| `update_cart` | `stateset-commerce` | `carts` | `write` |
| `update_cart_item` | `stateset-commerce` | `carts` | `write` |
| `update_channel` | `stateset-commerce` | `channels` | `write` |
| `update_company` | `stateset-commerce` | `companies` | `write` |
| `update_currency_settings` | `stateset-commerce` | `currency` | `admin` |
| `update_custom_object` | `stateset-commerce` | `custom_objects` | `write` |
| `update_custom_object_type` | `stateset-commerce` | `custom_objects` | `write` |
| `update_customer` | `stateset-commerce` | `customers` | `write` |
| `update_customer_address` | `stateset-commerce` | `customers` | `write` |
| `update_fraud_rule` | `stateset-commerce` | `fraud` | `admin` |
| `update_integration_field_mapping` | `stateset-commerce` | `integration-field-mappings` | `write` |
| `update_integration_mapping` | `stateset-commerce` | `integration-mappings` | `write` |
| `update_order_status` | `stateset-commerce` | `orders` | `write` |
| `update_price_level` | `stateset-commerce` | `price_levels` | `write` |
| `update_price_schedule` | `stateset-commerce` | `price_schedules` | `write` |
| `update_product` | `stateset-commerce` | `products` | `write` |
| `update_product_variant` | `stateset-commerce` | `products` | `write` |
| `update_production_batch` | `stateset-commerce` | `production_batches` | `write` |
| `update_promotion` | `stateset-commerce` | `promotions` | `write` |
| `update_search_config` | `stateset-commerce` | `search-config` | `write` |
| `update_segment` | `stateset-commerce` | `segments` | `write` |
| `update_shipping_zone` | `stateset-commerce` | `shipping_zones` | `write` |
| `update_subscription` | `stateset-commerce` | `subscriptions` | `write` |
| `update_subscription_plan` | `stateset-commerce` | `subscriptions` | `write` |
| `update_supplier_sku` | `stateset-commerce` | `supplier_skus` | `write` |
| `update_tax_settings` | `stateset-commerce` | `tax` | `write` |
| `validate_coupon` | `stateset-commerce` | `promotions` | `read` |
| `validate_tax_jurisdiction_compliance` | `stateset-commerce` | `tax` | `read` |
| `vector_clear` | `stateset-commerce` | `vector` | `admin` |
| `vector_clear_all` | `stateset-commerce` | `vector` | `admin` |
| `vector_index_all_customers` | `stateset-commerce` | `vector` | `admin` |
| `vector_index_all_inventory` | `stateset-commerce` | `vector` | `admin` |
| `vector_index_all_orders` | `stateset-commerce` | `vector` | `admin` |
| `vector_index_all_products` | `stateset-commerce` | `vector` | `admin` |
| `vector_index_customer` | `stateset-commerce` | `vector` | `write` |
| `vector_index_inventory` | `stateset-commerce` | `vector` | `write` |
| `vector_index_order` | `stateset-commerce` | `vector` | `write` |
| `vector_index_product` | `stateset-commerce` | `vector` | `write` |
| `vector_reindex_all` | `stateset-commerce` | `vector` | `admin` |
| `vector_search_customers` | `stateset-commerce` | `vector` | `read` |
| `vector_search_inventory` | `stateset-commerce` | `vector` | `read` |
| `vector_search_orders` | `stateset-commerce` | `vector` | `read` |
| `vector_search_products` | `stateset-commerce` | `vector` | `read` |
| `vector_stats` | `stateset-commerce` | `vector` | `read` |
| `verify_agent` | `stateset-commerce` | `agent_cards` | `write` |
| `verify_chain_anchor` | `stateset-commerce` | `proofs` | `read` |
| `verify_inclusion_proof` | `stateset-commerce` | `proofs` | `read` |
| `verify_receipt` | `stateset-commerce` | `proofs` | `read` |
| `verify_wasm_connector_attestation` | `stateset-commerce` | `connectors` | `read` |
| `void_credit_memo` | `stateset-commerce` | `accounts_receivable` | `write` |
| `void_invoice` | `stateset-commerce` | `invoices` | `delete` |
| `void_journal_entry` | `stateset-commerce` | `general_ledger` | `write` |
| `void_shipping_label` | `stateset-commerce` | `shipments` | `delete` |
| `void_tax_transaction` | `stateset-commerce` | `tax` | `delete` |
| `write_off_fixed_asset` | `stateset-commerce` | `fixed_assets` | `write` |
| `x402_circuit_status` | `stateset-commerce` | `a2a_automation` | `read` |
| `x402_create_payment_intent` | `stateset-commerce` | `x402` | `write` |
| `x402_credit_balance` | `stateset-commerce` | `x402` | `read` |
| `x402_credit_debit` | `stateset-commerce` | `x402` | `write` |
| `x402_credit_deposit` | `stateset-commerce` | `x402` | `write` |
| `x402_credit_transactions` | `stateset-commerce` | `x402` | `read` |
| `x402_execute_agent_payment` | `stateset-commerce` | `x402` | `write` |
| `x402_get_credit_account` | `stateset-commerce` | `x402` | `read` |
| `x402_get_intent` | `stateset-commerce` | `x402` | `read` |
| `x402_get_next_nonce` | `stateset-commerce` | `x402` | `read` |
| `x402_list_intents` | `stateset-commerce` | `x402` | `read` |
| `x402_mark_settled` | `stateset-commerce` | `x402` | `write` |
| `x402_record_incoming_settlement` | `stateset-commerce` | `x402` | `write` |
| `x402_settle_intent_onchain` | `stateset-commerce` | `x402` | `write` |
| `x402_sign_intent` | `stateset-commerce` | `x402` | `write` |
| `add_api_route` | `stateset-scaffold` | `scaffold` | `write` |
| `add_component` | `stateset-scaffold` | `scaffold` | `write` |
| `add_hook` | `stateset-scaffold` | `scaffold` | `write` |
| `add_page` | `stateset-scaffold` | `scaffold` | `write` |
| `create_project` | `stateset-scaffold` | `scaffold` | `write` |
| `list_component_templates` | `stateset-scaffold` | `scaffold` | `read` |
| `list_files` | `stateset-scaffold` | `scaffold` | `read` |
| `list_page_templates` | `stateset-scaffold` | `scaffold` | `read` |
| `list_templates` | `stateset-scaffold` | `scaffold` | `read` |
| `read_file` | `stateset-scaffold` | `scaffold` | `read` |
| `run_command` | `stateset-scaffold` | `scaffold` | `admin` |
| `seed_database` | `stateset-scaffold` | `scaffold` | `write` |
| `write_file` | `stateset-scaffold` | `scaffold` | `write` |
| `x402_balance` | `stateset-x402` | `x402` | `read` |
| `x402_budget_status` | `stateset-x402` | `x402` | `read` |
| `x402_call` | `stateset-x402` | `x402` | `write` |
| `x402_history` | `stateset-x402` | `x402` | `read` |
| `x402_receipt` | `stateset-x402` | `x402` | `read` |
