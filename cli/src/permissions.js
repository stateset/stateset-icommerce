/**
 * Fine-Grained Permissions & Guardrails for StateSet CLI
 *
 * Provides configurable permission levels, spending limits,
 * rate limiting, and audit logging for agent operations.
 */

import { getAuditStore } from './audit-store.js';
import { z } from 'zod';

// ============================================================================
// Constants
// ============================================================================

const RATE_WINDOW_MS = 60_000;

// ============================================================================
// Permission Levels
// ============================================================================

/**
 * Permission levels from lowest to highest privilege
 */
export const PERMISSION_LEVELS = {
  none: 0, // No operations allowed
  read: 1, // List, get, query operations
  preview: 2, // Read + show what would happen
  write: 3, // Create, update operations
  delete: 4, // Cancel, void, delete operations
  admin: 5, // Bulk operations, settings, full access
};

/**
 * Map tool names to required permission level
 */
export const TOOL_PERMISSIONS = {
  // Customer tools
  list_customers: 'read',
  get_customer: 'read',
  create_customer: 'write',

  // Order tools
  list_orders: 'read',
  get_order: 'read',
  create_order: 'write',
  update_order_status: 'write',
  ship_order: 'write',
  cancel_order: 'delete',

  // Product tools
  list_products: 'read',
  get_product: 'read',
  get_product_variant: 'read',
  create_product: 'write',

  // Inventory tools
  get_stock: 'read',
  create_inventory_item: 'write',
  adjust_inventory: 'write',
  reserve_inventory: 'write',
  confirm_reservation: 'write',
  release_reservation: 'write',

  // Vector search tools
  vector_search_products: 'read',
  vector_search_customers: 'read',
  vector_search_orders: 'read',
  vector_search_inventory: 'read',
  vector_stats: 'read',
  vector_index_product: 'write',
  vector_index_customer: 'write',
  vector_index_order: 'write',
  vector_index_inventory: 'write',
  vector_index_all_products: 'admin',
  vector_index_all_customers: 'admin',
  vector_index_all_orders: 'admin',
  vector_index_all_inventory: 'admin',
  vector_clear: 'admin',
  vector_clear_all: 'admin',
  vector_reindex_all: 'admin',

  // Custom object tools
  list_custom_object_types: 'read',
  get_custom_object_type: 'read',
  get_custom_object_type_by_handle: 'read',
  create_custom_object_type: 'write',
  update_custom_object_type: 'write',
  delete_custom_object_type: 'delete',
  list_custom_objects: 'read',
  get_custom_object: 'read',
  get_custom_object_by_handle: 'read',
  create_custom_object: 'write',
  update_custom_object: 'write',
  delete_custom_object: 'delete',

  // Return tools
  list_returns: 'read',
  get_return: 'read',
  create_return: 'write',
  approve_return: 'write',
  reject_return: 'write',

  // Cart/Checkout tools
  list_carts: 'read',
  get_cart: 'read',
  create_cart: 'write',
  add_cart_item: 'write',
  update_cart_item: 'write',
  remove_cart_item: 'write',
  set_cart_shipping_address: 'write',
  set_cart_payment: 'write',
  apply_cart_discount: 'write',
  get_shipping_rates: 'read',
  complete_checkout: 'write',
  cancel_cart: 'delete',
  abandon_cart: 'write',
  get_abandoned_carts: 'read',

  // Analytics tools (all read-only)
  get_sales_summary: 'read',
  get_top_products: 'read',
  get_customer_metrics: 'read',
  get_top_customers: 'read',
  get_inventory_health: 'read',
  get_low_stock_items: 'read',
  get_demand_forecast: 'read',
  get_revenue_forecast: 'read',
  get_order_status_breakdown: 'read',
  get_return_metrics: 'read',

  // Currency tools
  get_exchange_rate: 'read',
  list_exchange_rates: 'read',
  convert_currency: 'read',
  set_exchange_rate: 'admin',
  get_currency_settings: 'read',
  set_base_currency: 'admin',
  enable_currencies: 'admin',
  format_currency: 'read',

  // Tax tools
  calculate_tax: 'read',
  get_tax_rate: 'read',
  list_tax_jurisdictions: 'read',
  list_tax_rates: 'read',
  get_tax_settings: 'read',
  get_us_state_tax_info: 'read',
  get_customer_tax_exemptions: 'read',
  create_tax_exemption: 'write',
  calculate_cart_tax: 'read',
  list_tax_providers: 'read',
  validate_tax_jurisdiction_compliance: 'read',
  calculate_tax_quote: 'read',
  calculate_tax_quote_with_failover: 'read',
  get_tax_quote: 'read',
  commit_tax_transaction: 'write',
  get_tax_transaction: 'read',
  list_tax_transactions: 'read',
  void_tax_transaction: 'delete',
  ingest_tax_provider_webhook: 'write',

  // Promotions & Coupons
  list_promotions: 'read',
  get_promotion: 'read',
  get_active_promotions: 'read',
  create_promotion: 'write',
  activate_promotion: 'write',
  deactivate_promotion: 'write',
  create_coupon: 'write',
  validate_coupon: 'read',
  list_coupons: 'read',
  apply_cart_promotions: 'write',

  // Subscriptions
  list_subscription_plans: 'read',
  get_subscription_plan: 'read',
  create_subscription_plan: 'write',
  activate_subscription_plan: 'write',
  archive_subscription_plan: 'delete',
  list_subscriptions: 'read',
  get_subscription: 'read',
  create_subscription: 'write',
  pause_subscription: 'write',
  resume_subscription: 'write',
  cancel_subscription: 'delete',
  skip_billing_cycle: 'write',
  list_billing_cycles: 'read',
  get_billing_cycle: 'read',
  get_subscription_events: 'read',

  // Manufacturing / BOM
  list_boms: 'read',
  get_bom: 'read',
  create_bom: 'write',
  add_bom_component: 'write',
  activate_bom: 'write',
  list_work_orders: 'read',
  get_work_order: 'read',
  create_work_order: 'write',
  start_work_order: 'write',
  complete_work_order: 'write',
  cancel_work_order: 'delete',

  // Payments & Refunds
  list_payments: 'read',
  get_payment: 'read',
  create_payment: 'write',
  complete_payment: 'write',
  create_refund: 'write',
  list_payment_providers: 'read',
  create_payment_intent: 'write',
  get_payment_intent: 'read',
  list_payment_intents: 'read',
  list_payment_settlements: 'read',
  list_payment_settlement_batches: 'read',
  reconcile_payment_provider: 'read',
  create_payment_settlement_batch: 'write',
  capture_payment_intent: 'write',
  cancel_payment_intent: 'delete',
  refund_payment_intent: 'write',
  ingest_payment_provider_webhook: 'write',

  // Shipments
  list_shipments: 'read',
  create_shipment: 'write',
  deliver_shipment: 'write',
  list_shipping_providers: 'read',
  quote_shipping_rates: 'read',
  create_shipping_label: 'write',
  void_shipping_label: 'delete',
  track_shipping_label: 'read',
  list_shipping_labels: 'read',
  ingest_shipping_provider_webhook: 'write',
  handle_fulfillment_exception: 'write',

  // Suppliers & Purchase Orders
  list_suppliers: 'read',
  create_supplier: 'write',
  list_purchase_orders: 'read',
  create_purchase_order: 'write',
  approve_purchase_order: 'write',
  send_purchase_order: 'write',

  // Invoices
  list_invoices: 'read',
  create_invoice: 'write',
  send_invoice: 'write',
  record_invoice_payment: 'write',
  get_overdue_invoices: 'read',

  // Warranties
  list_warranties: 'read',
  create_warranty: 'write',
  create_warranty_claim: 'write',
  approve_warranty_claim: 'write',

  // Import / Export
  import_shopify_data: 'write',
  import_shopify_shadow_data: 'write',
  import_woocommerce_data: 'write',
  import_status: 'read',
  list_id_mappings: 'read',
  import_csv: 'write',
  import_json: 'write',
  export_data: 'read',
  configure_stripe_webhooks: 'write',
  configure_woocommerce_webhooks: 'write',

  // Audit
  audit_query: 'read',
  audit_summary: 'read',
  audit_export: 'admin',
  audit_retention: 'admin',

  // Policy Engine
  evaluate_policy: 'read',
  list_policies: 'read',
  register_policy_template: 'write',
  load_policy_file: 'write',
  explain_policy_denial: 'read',

  // WASM Connector Ecosystem
  list_connector_marketplace: 'read',
  publish_wasm_connector: 'admin',
  install_wasm_connector: 'write',
  assess_wasm_connector_safety: 'read',
  certify_wasm_connector: 'admin',
  sign_wasm_connector_attestation: 'admin',
  verify_wasm_connector_attestation: 'read',
  uninstall_wasm_connector: 'delete',
  list_installed_connectors: 'read',
  get_installed_connector: 'read',
  execute_wasm_connector: 'write',

  // Agentic Runtime
  agentic_runtime_contract: 'read',
  agentic_tool_catalog: 'read',
  agentic_payment_discovery: 'read',
  agentic_prepare_payment: 'read',
  agentic_plan: 'read',
  agentic_simulate_mutation: 'read',
  agentic_replay_mutation: 'read',
  agentic_replay: 'read',
  agentic_subscribe_events: 'read',
  agentic_unsubscribe_events: 'read',
  agentic_list_event_subscriptions: 'read',
  agentic_get_event_history: 'read',
  agentic_execute_plan: 'read',

  // Sync / VES
  sync_status: 'read',
  sync_pull: 'write',
  sync_push: 'write',
  sync_outbox: 'read',
  sync_conflicts: 'read',
  sync_entity_history: 'read',
  sync_pulled_events: 'read',
  sync_decrypt_event: 'read',
  sync_full: 'admin',
  sync_rebase: 'admin',
  sync_resolve: 'admin',
  sync_retry_failed: 'admin',
  sync_verify_receipt: 'read',
  sync_verify_inclusion: 'read',
  sync_inspect_commitment: 'read',

  // Proof Tools (VES commerce proofs)
  verify_receipt: 'read',
  generate_inclusion_proof: 'read',
  verify_inclusion_proof: 'read',
  generate_receipt_bundle: 'read',
  inspect_batch: 'read',
  export_compliance_package: 'read',
  verify_chain_anchor: 'read',

  // Circuit Breakers
  agent_get_breaker_state: 'read',
  agent_get_spending_summary: 'read',
  agent_get_all_breaker_states: 'read',
  agent_trip_breaker: 'admin',
  agent_trip_all_breakers: 'admin',
  agent_reset_breaker: 'admin',
  agent_reset_all_breakers: 'admin',
  agent_set_spending_limits: 'admin',

  // Express Checkout & Payment Links
  create_payment_link: 'write',
  resolve_payment_link: 'read',
  express_checkout: 'write',
  agent_instant_checkout: 'write',
  get_payment_link_status: 'read',
  list_payment_links: 'read',
  revoke_payment_link: 'write',
  checkout_with_crypto: 'write',

  // Compliance & Regulatory
  export_audit_trail: 'admin',
  generate_1099k: 'admin',
  export_gdpr_data: 'admin',
  delete_gdpr_data: 'admin',
  compliance_summary: 'read',
  soc2_evidence: 'admin',

  // Agent Catalog
  publish_product_catalog: 'write',
  query_agent_catalog: 'read',
  get_product_spec: 'read',
  match_agent_to_products: 'read',
  match_product_to_agents: 'read',
  export_agent_catalog: 'read',

  // Agent Key Management
  agent_key_generate: 'write',
  agent_key_list: 'read',
  agent_key_info: 'read',
  agent_key_rotate: 'write',
  agent_key_export: 'read',

  // Treasury / Stablecoin Billing
  treasury_balance: 'read',
  treasury_ledger: 'read',
  treasury_list_tokens: 'read',
  treasury_buy: 'write',
  treasury_deposit: 'write',
  treasury_register_token: 'admin',

  // ERC-8004 Identity Registry
  erc8004_get_identity: 'read',
  erc8004_get_by_wallet: 'read',
  erc8004_list_identities: 'read',
  erc8004_register_identity: 'admin',
  erc8004_link_wallet: 'write',

  // Agent Cards / A2A
  discover_agents: 'read',
  get_agent_card: 'read',
  list_agent_cards: 'read',
  register_agent_card: 'write',
  verify_agent: 'admin',

  // A2A Commerce — Payments & Quotes
  a2a_pay: 'write',
  a2a_request_payment: 'write',
  a2a_pay_request: 'write',
  a2a_request_quote: 'write',
  a2a_provide_quote: 'write',
  a2a_accept_quote: 'write',
  a2a_decline_quote: 'write',
  a2a_fulfill_quote: 'write',
  a2a_get_payment: 'read',
  a2a_list_payments: 'read',
  a2a_list_payment_requests: 'read',
  a2a_list_quotes: 'read',
  a2a_get_balance: 'read',
  a2a_discover_agents: 'read',

  // A2A Negotiation
  a2a_counter_quote: 'write',
  a2a_revise_quote: 'write',

  // A2A Escrow
  a2a_create_escrow: 'write',
  a2a_fund_escrow: 'write',
  a2a_release_escrow: 'write',
  a2a_refund_escrow: 'write',
  a2a_dispute_escrow: 'write',
  a2a_get_escrow: 'read',
  a2a_list_escrows: 'read',

  // A2A Disputes
  a2a_file_dispute: 'write',
  a2a_submit_evidence: 'write',
  a2a_resolve_dispute: 'write',
  a2a_get_dispute: 'read',
  a2a_list_disputes: 'read',

  // A2A Reputation
  a2a_rate_agent: 'write',
  a2a_get_reputation: 'read',
  a2a_respond_to_feedback: 'write',

  // A2A Services
  a2a_register_service: 'write',
  a2a_list_services: 'read',
  a2a_get_service: 'read',

  // A2A Notifications
  a2a_send_notification: 'write',
  a2a_list_notification_log: 'read',
  a2a_configure_webhooks: 'write',

  // A2A Agent Subscriptions
  a2a_create_agent_subscription: 'write',
  a2a_pause_agent_subscription: 'write',
  a2a_resume_agent_subscription: 'write',
  a2a_cancel_agent_subscription: 'write',
  a2a_get_agent_subscription: 'read',
  a2a_list_agent_subscriptions: 'read',
  a2a_process_subscription_billing: 'write',

  // A2A Split Payments
  a2a_create_split_payment: 'write',
  a2a_execute_split_payment: 'write',
  a2a_get_split_payment: 'read',
  a2a_list_split_payments: 'read',

  // A2A Conditional Payments
  a2a_create_conditional_payment: 'write',
  a2a_check_payment_conditions: 'read',
  a2a_settle_conditional_payment: 'write',

  // A2A Event Streaming
  a2a_subscribe_events: 'write',
  a2a_list_event_subscriptions: 'read',
  a2a_get_event_history: 'read',

  // Agent Runtime
  agent_create_runtime: 'write',
  agent_set_strategy: 'write',
  agent_get_budget: 'read',
  agent_register_service: 'write',
  agent_discover_services: 'read',
  agent_tick: 'write',
  agent_start_loop: 'write',
  agent_stop_loop: 'write',
  agent_get_status: 'read',
  agent_list_runtimes: 'read',
  agent_destroy_runtime: 'delete',
  agent_create_escrow_deal: 'write',
  agent_subscribe_to_service: 'write',
  agent_rate_counterparty: 'write',
  agent_get_reputation: 'read',
  agent_create_split_deal: 'write',
  agent_get_event_history: 'read',
  agent_enable_settlement: 'write',
  agent_get_chain_balance: 'read',
  agent_broadcast_rfq: 'write',
  agent_collect_rfq_responses: 'read',
  agent_award_rfq: 'write',
  agent_get_marketplace_metrics: 'read',
  agent_attach_sla: 'write',
  agent_check_sla_compliance: 'read',
  agent_create_workflow: 'write',
  agent_execute_workflow: 'write',
  agent_get_workflow_status: 'read',
  agent_set_dynamic_pricing: 'write',

  // A2A Automation — Billing, Disputes, SLA, Marketplace, DLQ, Health
  a2a_billing_tick: 'write',
  a2a_billing_start: 'admin',
  a2a_billing_stop: 'admin',
  a2a_billing_metrics: 'read',
  a2a_dispute_resolver_tick: 'write',
  a2a_dispute_resolver_start: 'admin',
  a2a_dispute_resolver_metrics: 'read',
  a2a_sla_enforce: 'write',
  a2a_sla_enforce_all: 'write',
  a2a_marketplace_auto_award: 'write',
  a2a_marketplace_maintenance: 'write',
  a2a_list_failed_notifications: 'read',
  a2a_replay_notification: 'write',
  a2a_notification_retry_all: 'write',
  a2a_webhook_dlq_status: 'read',
  a2a_list_webhook_dlq: 'admin',
  a2a_quarantine_failed_webhooks: 'admin',
  a2a_replay_dlq_entry: 'admin',
  a2a_purge_dlq: 'admin',
  a2a_dlq_count: 'read',
  a2a_health_check: 'read',
  a2a_readiness: 'read',
  x402_circuit_status: 'read',
  a2a_rate_limit_metrics: 'read',
  a2a_saga_execute: 'write',
  a2a_saga_status: 'read',
  a2a_saga_list: 'read',
  a2a_saga_cancel: 'write',
  a2a_cost_summary: 'read',
  a2a_cost_counterparty_breakdown: 'read',
  a2a_cost_operation_breakdown: 'read',
  a2a_cost_daily_trend: 'read',
  a2a_cost_anomalies: 'read',
  a2a_cost_margin_analysis: 'read',
  a2a_cost_budget_forecast: 'read',
  a2a_cost_top_spenders: 'read',
  a2a_escrow_process_all: 'write',
  a2a_get_trace: 'read',
  a2a_tracing_metrics: 'read',
  a2a_recent_spans: 'read',
  a2a_export_traces: 'read',
  a2a_agent_dashboard: 'read',
  a2a_agent_decisions: 'read',
  a2a_agent_performance: 'read',
  a2a_agent_tick_metrics: 'read',
  a2a_agent_lifecycle: 'read',
  a2a_agent_alerts: 'read',
  a2a_settlement_status: 'read',
  a2a_settlement_pending: 'read',
  a2a_settlement_finality_metrics: 'read',
  a2a_handshake: 'read',
  a2a_my_capabilities: 'read',
  a2a_send_message: 'write',
  a2a_get_inbox: 'read',
  a2a_delegate_task: 'write',
  a2a_respond_to_task: 'write',
  a2a_get_thread: 'read',
  a2a_messaging_metrics: 'read',
  a2a_batch_pay: 'write',
  a2a_batch_request_quotes: 'write',
  a2a_save_checkpoint: 'write',
  a2a_load_checkpoint: 'read',
  a2a_list_checkpoints: 'read',
  a2a_export_agent_data: 'read',
  a2a_commerce_report: 'read',
  a2a_data_stats: 'read',
  a2a_verify_webhook: 'read',
  a2a_tick_metrics: 'read',
  a2a_schedule_action: 'write',
  a2a_cancel_scheduled: 'write',
  a2a_list_scheduled: 'read',
  a2a_scheduler_metrics: 'read',
  a2a_remember_interaction: 'write',
  a2a_counterparty_profile: 'read',
  a2a_should_transact: 'read',
  a2a_agent_insights: 'read',
  a2a_top_counterparties: 'read',
  a2a_add_rule: 'write',
  a2a_evaluate_rules: 'read',
  a2a_list_rules: 'read',
  a2a_rule_audit_log: 'read',
  a2a_scatter: 'write',
  a2a_coordination_status: 'read',
  a2a_submit_response: 'write',
  a2a_join_results: 'read',

  // Stablecoin Payments
  get_agent_wallet: 'read',
  get_wallet_balance: 'read',
  create_stablecoin_payment: 'write',
  list_supported_chains: 'read',

  // x402 Protocol (AI Agent Commerce)
  x402_create_payment_intent: 'write',
  x402_sign_intent: 'write',
  x402_execute_agent_payment: 'write',
  x402_get_intent: 'read',
  x402_list_intents: 'read',
  x402_settle_intent_onchain: 'write',
  x402_record_incoming_settlement: 'write',
  x402_mark_settled: 'write',
  x402_get_next_nonce: 'read',
  x402_credit_balance: 'read',
  x402_credit_debit: 'write',
  x402_credit_deposit: 'write',
  x402_credit_transactions: 'read',
  x402_balance: 'read',
  x402_budget_status: 'read',
  x402_call: 'write',
  x402_history: 'read',
  x402_receipt: 'read',

  // Gift Cards
  create_gift_card: 'write',
  get_gift_card: 'read',
  list_gift_cards: 'read',
  charge_gift_card: 'write',
  refund_to_gift_card: 'write',
  disable_gift_card: 'write',
  check_gift_card_balance: 'read',

  // Store Credits
  create_store_credit: 'write',
  get_store_credit: 'read',
  list_store_credits: 'read',
  adjust_store_credit: 'write',
  apply_store_credit: 'write',

  // Customer Segments
  create_segment: 'write',
  get_segment: 'read',
  list_segments: 'read',
  update_segment: 'write',
  evaluate_segment_membership: 'read',
  rebuild_dynamic_segment: 'write',

  // Shipping Zones & Methods
  create_shipping_zone: 'write',
  get_shipping_zone: 'read',
  list_shipping_zones: 'read',
  update_shipping_zone: 'write',
  create_shipping_method: 'write',
  calculate_shipping_rate: 'read',
  list_shipping_methods: 'read',

  // Product Reviews
  create_review: 'write',
  get_review: 'read',
  list_reviews: 'read',
  approve_review: 'write',
  reject_review: 'write',
  get_review_summary: 'read',
  flag_review: 'write',

  // Wishlists
  create_wishlist: 'write',
  get_wishlist: 'read',
  add_to_wishlist: 'write',
  remove_from_wishlist: 'write',
  list_wishlists: 'read',
  convert_wishlist_to_cart: 'write',

  // Loyalty Programs
  create_loyalty_program: 'admin',
  get_loyalty_program: 'read',
  enroll_customer: 'write',
  get_loyalty_account: 'read',
  earn_points: 'write',
  redeem_points: 'write',
  list_rewards: 'read',
  create_reward: 'admin',

  // Fraud Detection
  assess_order_fraud: 'read',
  get_fraud_assessment: 'read',
  list_fraud_signals: 'read',
  create_fraud_rule: 'admin',
  update_fraud_rule: 'admin',
  review_flagged_order: 'write',

  // Agentic Runtime
  discover_tools: 'read',
  delegate_to_agent: 'write',
};

// ============================================================================
// Default Guardrails
// ============================================================================

/**
 * Zod schema for guardrail configuration — validates at construction time.
 */
export const GuardrailsSchema = z
  .object({
    maxOrderValue: z.number().nonnegative().default(10000),
    maxDailyOrderTotal: z.number().nonnegative().default(100000),
    maxDailyOrderCount: z.number().int().nonnegative().default(500),
    maxInventoryAdjustment: z.number().int().nonnegative().default(1000),
    maxToolCallsPerMinute: z.number().int().positive().default(120),
    maxWriteOpsPerMinute: z.number().int().positive().default(30),
    confirmOrdersAbove: z.number().nonnegative().default(1000),
    confirmBulkOperations: z.boolean().default(true),
    blockedTools: z.array(z.string()).default([]),
    requireApprovalFor: z.array(z.string()).default([]),
  })
  .passthrough();

/**
 * Default guardrail configuration
 */
export const DEFAULT_GUARDRAILS = {
  // Spending limits
  maxOrderValue: 10000, // Maximum single order value
  maxDailyOrderTotal: 100000, // Maximum daily order total
  maxDailyOrderCount: 500, // Maximum orders per day

  // Inventory limits
  maxInventoryAdjustment: 1000, // Maximum single adjustment quantity

  // Rate limits (per minute)
  maxToolCallsPerMinute: 120,
  maxWriteOpsPerMinute: 30,

  // Confirmation thresholds
  confirmOrdersAbove: 1000, // Ask for confirmation on orders > $1000
  confirmBulkOperations: true, // Confirm bulk operations

  // Blocked operations
  blockedTools: [], // Tools to completely disable

  // Require explicit approval for these tools
  requireApprovalFor: [
    'cancel_order',
    'complete_checkout',
    'cancel_subscription',
    'create_refund',
    'create_payment_intent',
    'create_payment_settlement_batch',
    'capture_payment_intent',
    'cancel_payment_intent',
    'refund_payment_intent',
    'create_shipping_label',
    'void_shipping_label',
    'handle_fulfillment_exception',
    'commit_tax_transaction',
    'void_tax_transaction',
    'cancel_work_order',
    'sync_full',
    'sync_rebase',
    'sync_resolve',
    'treasury_buy',
    'create_stablecoin_payment',
    'x402_sign_intent',
    'x402_execute_agent_payment',
    'x402_settle_intent_onchain',
    'x402_record_incoming_settlement',
    'x402_mark_settled',
    'x402_credit_debit',
    'erc8004_register_identity',
    'a2a_fund_escrow',
    'a2a_release_escrow',
    'a2a_refund_escrow',
    'a2a_resolve_dispute',
    'a2a_settle_conditional_payment',
    'a2a_process_subscription_billing',
    'a2a_execute_split_payment',
    'agent_create_escrow_deal',
    'agent_subscribe_to_service',
    'agent_create_split_deal',
    'agent_enable_settlement',
    'agent_broadcast_rfq',
    'agent_award_rfq',
    'agent_execute_workflow',
    'vector_index_all_products',
    'vector_index_all_customers',
    'vector_index_all_orders',
    'vector_index_all_inventory',
    'vector_clear',
    'vector_clear_all',
    'publish_wasm_connector',
    'install_wasm_connector',
    'certify_wasm_connector',
    'sign_wasm_connector_attestation',
    'uninstall_wasm_connector',
    'execute_wasm_connector',
    'a2a_billing_start',
    'a2a_billing_stop',
    'a2a_billing_tick',
    'a2a_dispute_resolver_start',
    'a2a_dispute_resolver_tick',
    'a2a_sla_enforce',
    'a2a_sla_enforce_all',
    'a2a_marketplace_auto_award',
    'a2a_marketplace_maintenance',
    'a2a_replay_notification',
    'a2a_saga_execute',
    'a2a_saga_cancel',
    'a2a_escrow_process_all',
    'a2a_batch_pay',
    'a2a_delegate_task',
    'a2a_scatter',
  ],
};

// ============================================================================
// Permission Gate
// ============================================================================

/**
 * PermissionGate - Enforces permission levels and guardrails
 *
 * Usage:
 *   const gate = new PermissionGate({ level: 'write', guardrails: { maxOrderValue: 5000 } });
 *   const check = await gate.checkPermission('create_order', { totalAmount: 100 });
 *   if (check.allowed) { ... }
 */
export class PermissionGate {
  constructor(options = {}) {
    // Permission level
    const levelName = options.level || 'preview';
    this.level = PERMISSION_LEVELS[levelName] ?? PERMISSION_LEVELS.preview;
    this.levelName = levelName;

    // Guardrails — validate with schema
    const merged = { ...DEFAULT_GUARDRAILS, ...options.guardrails };
    const parsed = GuardrailsSchema.safeParse(merged);
    if (!parsed.success) {
      const issues = parsed.error.issues.map((i) => `${i.path.join('.')}: ${i.message}`).join('; ');
      console.warn(`[PermissionGate] Invalid guardrails config (${issues}), using defaults`);
      this.guardrails = { ...DEFAULT_GUARDRAILS };
    } else {
      this.guardrails = parsed.data;
    }

    // Callbacks
    this.onConfirmRequired = options.onConfirmRequired || null;
    this.onPermissionDenied = options.onPermissionDenied || null;

    // Audit log — persistent SQLite store + in-memory buffer
    this.auditLog = [];
    try {
      this.auditStore = getAuditStore();
    } catch (err) {
      console.warn('[permissions] Audit store unavailable, using in-memory:', err.message);
      this.auditStore = null;
    }

    // Rate limiting state
    this.rateLimitState = {
      toolCalls: [],
      writeOps: [],
      dailyOrders: { count: 0, total: 0, date: this._getDateKey() },
    };

    // Aggregate session safety limits
    const maxMutations = parseInt(process.env.STATESET_MAX_MUTATIONS, 10);
    const maxMonetary = parseFloat(process.env.STATESET_MAX_MONETARY);
    this.sessionLimits = {
      maxMutationsPerSession: Number.isFinite(maxMutations) ? maxMutations : 50,
      maxMonetaryTotal: Number.isFinite(maxMonetary) ? maxMonetary : 10000,
      mutationCount: 0,
      monetaryTotal: 0,
    };
  }

  // --------------------------------------------------------------------------
  // Permission Checking
  // --------------------------------------------------------------------------

  /**
   * Check if an operation is allowed
   *
   * @param {string} toolName - Tool name (without mcp__ prefix)
   * @param {object} params - Tool parameters
   * @returns {object} { allowed, reason?, preview? }
   */
  async checkPermission(toolName, params = {}) {
    // Normalize tool name
    const normalizedName = toolName.replace('mcp__stateset-commerce__', '');

    // Check blocked tools
    if (this.guardrails.blockedTools.includes(normalizedName)) {
      return this._deny(`Tool '${normalizedName}' is blocked by policy`, normalizedName);
    }

    // Check permission level
    const requiredLevel = TOOL_PERMISSIONS[normalizedName];
    if (!requiredLevel) {
      return this._deny(
        `Tool '${normalizedName}' has no registered permission policy`,
        normalizedName,
      );
    }
    const requiredValue = PERMISSION_LEVELS[requiredLevel] || 0;

    if (requiredValue > this.level) {
      // Special case: preview mode shows what would happen
      if (this.level === PERMISSION_LEVELS.preview && requiredValue <= PERMISSION_LEVELS.write) {
        return {
          allowed: false,
          preview: true,
          reason: `Preview mode: would execute '${normalizedName}' if --apply flag is set`,
          wouldDo: { tool: normalizedName, params },
        };
      }

      return this._deny(
        `Operation requires '${requiredLevel}' permission (current: '${this.levelName}')`,
        normalizedName,
      );
    }

    // Check aggregate session limits for write operations
    if (requiredLevel !== 'read') {
      const aggregateCheck = this._checkAggregateLimits(normalizedName, params);
      if (!aggregateCheck.allowed) {
        return aggregateCheck;
      }
    }

    // Check rate limits
    const rateLimitCheck = this._checkRateLimits(normalizedName);
    if (!rateLimitCheck.allowed) {
      return rateLimitCheck;
    }

    // Check domain-specific guardrails
    const guardrailCheck = await this._checkGuardrails(normalizedName, params);
    if (!guardrailCheck.allowed) {
      return guardrailCheck;
    }

    // Check if confirmation is required
    if (this.guardrails.requireApprovalFor.includes(normalizedName)) {
      if (this.onConfirmRequired) {
        const confirmed = await this.onConfirmRequired({
          tool: normalizedName,
          params,
          message: `Confirm execution of '${normalizedName}'?`,
        });
        if (!confirmed) {
          return this._deny('User declined confirmation', normalizedName);
        }
      }
    }

    // Track aggregate session mutations for write operations
    if (requiredLevel !== 'read') {
      this.sessionLimits.mutationCount++;
      const amount = parseFloat(params?.amount || params?.totalAmount || params?.total || 0);
      if (Number.isFinite(amount) && amount > 0) {
        this.sessionLimits.monetaryTotal += amount;
      }
    }

    // Log the allowed operation
    this._logAudit(normalizedName, params, 'allowed');

    return { allowed: true };
  }

  // --------------------------------------------------------------------------
  // Aggregate Session Limits
  // --------------------------------------------------------------------------

  /**
   * Check aggregate session-level limits (mutation count and monetary total).
   * @param {string} toolName
   * @param {object} params
   * @returns {{ allowed: boolean, reason?: string }}
   */
  _checkAggregateLimits(toolName, params = {}) {
    const { maxMutationsPerSession, maxMonetaryTotal, mutationCount, monetaryTotal } =
      this.sessionLimits;

    if (mutationCount >= maxMutationsPerSession) {
      return this._deny(
        `Session mutation limit (${maxMutationsPerSession}) reached. ` +
          `Set STATESET_MAX_MUTATIONS env var to increase.`,
        toolName,
      );
    }

    const amount = parseFloat(params?.amount || params?.totalAmount || params?.total || 0);
    if (Number.isFinite(amount) && amount > 0 && monetaryTotal + amount > maxMonetaryTotal) {
      return this._deny(
        `Session monetary limit ($${maxMonetaryTotal}) would be exceeded ` +
          `(current: $${monetaryTotal.toFixed(2)}, requested: $${amount.toFixed(2)}). ` +
          `Set STATESET_MAX_MONETARY env var to increase.`,
        toolName,
      );
    }

    return { allowed: true };
  }

  /**
   * Get current session limits status.
   * @returns {{ mutationCount: number, monetaryTotal: number, maxMutations: number, maxMonetary: number }}
   */
  getSessionLimitsStatus() {
    return {
      mutationCount: this.sessionLimits.mutationCount,
      monetaryTotal: this.sessionLimits.monetaryTotal,
      maxMutations: this.sessionLimits.maxMutationsPerSession,
      maxMonetary: this.sessionLimits.maxMonetaryTotal,
    };
  }

  // --------------------------------------------------------------------------
  // Rate Limiting
  // --------------------------------------------------------------------------

  _checkRateLimits(toolName) {
    const now = Date.now();
    const oneMinuteAgo = now - RATE_WINDOW_MS;

    // Clean old entries
    this.rateLimitState.toolCalls = this.rateLimitState.toolCalls.filter((t) => t > oneMinuteAgo);
    this.rateLimitState.writeOps = this.rateLimitState.writeOps.filter((t) => t > oneMinuteAgo);

    // Check total tool calls
    if (this.rateLimitState.toolCalls.length >= this.guardrails.maxToolCallsPerMinute) {
      return this._deny(
        `Rate limit exceeded: ${this.guardrails.maxToolCallsPerMinute} tool calls per minute`,
        toolName,
      );
    }

    // Check write operations
    const isWriteOp = ['write', 'delete', 'admin'].includes(TOOL_PERMISSIONS[toolName]);
    if (isWriteOp && this.rateLimitState.writeOps.length >= this.guardrails.maxWriteOpsPerMinute) {
      return this._deny(
        `Rate limit exceeded: ${this.guardrails.maxWriteOpsPerMinute} write operations per minute`,
        toolName,
      );
    }

    // Record this call
    this.rateLimitState.toolCalls.push(now);
    if (isWriteOp) {
      this.rateLimitState.writeOps.push(now);
    }

    return { allowed: true };
  }

  // --------------------------------------------------------------------------
  // Domain-Specific Guardrails
  // --------------------------------------------------------------------------

  async _checkGuardrails(toolName, params) {
    // Order value limits
    if (toolName === 'create_order') {
      const total = this._calculateOrderTotal(params);
      if (total > this.guardrails.maxOrderValue) {
        return this._deny(
          `Order value $${total} exceeds maximum $${this.guardrails.maxOrderValue}`,
          toolName,
        );
      }

      // Check daily limits
      this._resetDailyCountersIfNeeded();
      if (this.rateLimitState.dailyOrders.count >= this.guardrails.maxDailyOrderCount) {
        return this._deny(
          `Daily order limit (${this.guardrails.maxDailyOrderCount}) exceeded`,
          toolName,
        );
      }
      if (this.rateLimitState.dailyOrders.total + total > this.guardrails.maxDailyOrderTotal) {
        return this._deny(
          `Daily order total would exceed $${this.guardrails.maxDailyOrderTotal}`,
          toolName,
        );
      }

      // Confirmation for large orders
      if (total > this.guardrails.confirmOrdersAbove && this.onConfirmRequired) {
        const confirmed = await this.onConfirmRequired({
          tool: toolName,
          params,
          amount: total,
          message: `Create order for $${total.toFixed(2)}?`,
        });
        if (!confirmed) {
          return this._deny('User declined confirmation for large order', toolName);
        }
      }
    }

    // Checkout value limits
    if (toolName === 'complete_checkout') {
      // Would need to fetch cart to check value
      // For now, always require confirmation (handled in requireApprovalFor)
    }

    // Inventory adjustment limits
    if (toolName === 'adjust_inventory') {
      const qty = Math.abs(params.quantity || 0);
      if (qty > this.guardrails.maxInventoryAdjustment) {
        return this._deny(
          `Adjustment quantity ${qty} exceeds maximum ${this.guardrails.maxInventoryAdjustment}`,
          toolName,
        );
      }
    }

    return { allowed: true };
  }

  _calculateOrderTotal(params) {
    if (params.totalAmount) return params.totalAmount;
    if (params.items) {
      return params.items.reduce((sum, item) => {
        return sum + (item.quantity || 1) * (item.unitPrice || 0);
      }, 0);
    }
    return 0;
  }

  _getDateKey() {
    return new Date().toISOString().split('T')[0];
  }

  _resetDailyCountersIfNeeded() {
    const today = this._getDateKey();
    if (this.rateLimitState.dailyOrders.date !== today) {
      this.rateLimitState.dailyOrders = { count: 0, total: 0, date: today };
    }
  }

  // --------------------------------------------------------------------------
  // Audit Logging
  // --------------------------------------------------------------------------

  _logAudit(toolName, params, result, reason = null) {
    const entry = {
      timestamp: new Date().toISOString(),
      tool: toolName,
      params: this._sanitizeParams(params),
      result,
      reason,
      level: this.levelName,
    };

    // In-memory buffer (keep last 1000 for fast access)
    this.auditLog.push(entry);
    if (this.auditLog.length > 1000) {
      this.auditLog = this.auditLog.slice(-1000);
    }

    // Persist to SQLite
    if (this.auditStore) {
      try {
        this.auditStore.log(entry);
      } catch (err) {
        console.warn('[permissions] Audit log write error:', err.message);
      }
    }
  }

  _sanitizeParams(params) {
    // Remove sensitive fields for audit log
    const sanitized = { ...params };
    const sensitiveFields = ['password', 'token', 'secret', 'key', 'paymentToken'];
    for (const field of sensitiveFields) {
      if (sanitized[field]) {
        sanitized[field] = '[REDACTED]';
      }
    }
    return sanitized;
  }

  /**
   * Get audit log — queries persistent store when available, falls back to in-memory.
   * @param {object} [options]
   * @param {string} [options.tool] - Filter by tool name
   * @param {string} [options.result] - Filter by result ('allowed', 'denied', 'executed')
   * @param {string} [options.since] - ISO timestamp to filter from
   * @param {number} [options.limit] - Max entries to return (default: 100)
   * @returns {Array<object>}
   */
  getAuditLog(options = {}) {
    // Prefer persistent store
    if (this.auditStore) {
      try {
        return this.auditStore.query({
          tool: options.tool || null,
          result: options.result || null,
          since: options.since || null,
          limit: options.limit || 100,
        });
      } catch (err) {
        console.warn('[permissions] Audit store query error:', err.message);
      }
    }

    // In-memory fallback
    let log = this.auditLog;
    if (options.tool) {
      log = log.filter((e) => e.tool === options.tool);
    }
    if (options.result) {
      log = log.filter((e) => e.result === options.result);
    }
    if (options.since) {
      log = log.filter((e) => new Date(e.timestamp) >= new Date(options.since));
    }
    if (options.limit) {
      log = log.slice(-options.limit);
    }
    return log;
  }

  /**
   * Export audit log for compliance — uses persistent store when available.
   * @param {object} [options]
   * @param {string} [options.since] - ISO timestamp
   * @param {number} [options.limit] - Max entries (default: 10000)
   * @returns {object}
   */
  exportAuditLog(options = {}) {
    if (this.auditStore) {
      try {
        const exported = this.auditStore.export({
          since: options.since || null,
          limit: options.limit || 10000,
        });
        return {
          ...exported,
          permissionLevel: this.levelName,
          guardrails: this.guardrails,
        };
      } catch (err) {
        console.warn('[permissions] Audit store export error:', err.message);
      }
    }

    return {
      exportedAt: new Date().toISOString(),
      permissionLevel: this.levelName,
      guardrails: this.guardrails,
      entries: this.auditLog,
    };
  }

  // --------------------------------------------------------------------------
  // Helpers
  // --------------------------------------------------------------------------

  _deny(reason, toolName = 'unknown') {
    this._logAudit(toolName, {}, 'denied', reason);
    if (this.onPermissionDenied) {
      this.onPermissionDenied({ reason });
    }
    return { allowed: false, reason };
  }

  /**
   * Record a successful operation (for daily limits tracking)
   */
  recordOperation(toolName, params = {}) {
    if (toolName === 'create_order') {
      this._resetDailyCountersIfNeeded();
      this.rateLimitState.dailyOrders.count++;
      this.rateLimitState.dailyOrders.total += this._calculateOrderTotal(params);
    }
    this._logAudit(toolName, params, 'executed');
  }

  /**
   * Get current permission level name
   */
  getLevelName() {
    return this.levelName;
  }

  /**
   * Check if a specific permission level is met
   */
  hasPermission(level) {
    const requiredValue = PERMISSION_LEVELS[level] || 0;
    return this.level >= requiredValue;
  }

  /**
   * Get summary of current state
   */
  getSummary() {
    this._resetDailyCountersIfNeeded();
    const now = Date.now();
    const oneMinuteAgo = now - RATE_WINDOW_MS;

    return {
      level: this.levelName,
      rateLimits: {
        toolCallsLastMinute: this.rateLimitState.toolCalls.filter((t) => t > oneMinuteAgo).length,
        maxToolCallsPerMinute: this.guardrails.maxToolCallsPerMinute,
        writeOpsLastMinute: this.rateLimitState.writeOps.filter((t) => t > oneMinuteAgo).length,
        maxWriteOpsPerMinute: this.guardrails.maxWriteOpsPerMinute,
      },
      dailyLimits: {
        ordersToday: this.rateLimitState.dailyOrders.count,
        maxDailyOrders: this.guardrails.maxDailyOrderCount,
        totalToday: this.rateLimitState.dailyOrders.total,
        maxDailyTotal: this.guardrails.maxDailyOrderTotal,
      },
      auditLogSize: this.auditStore ? this.auditStore.count() : this.auditLog.length,
    };
  }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Create a permission gate from CLI flags
 */
export function createPermissionGate(options = {}) {
  let level = 'preview';

  if (options.apply) {
    level = 'write';
  }
  if (options.admin) {
    level = 'admin';
  }
  if (options.readonly) {
    level = 'read';
  }

  return new PermissionGate({
    level,
    guardrails: options.guardrails,
    onConfirmRequired: options.onConfirmRequired,
    onPermissionDenied: options.onPermissionDenied,
  });
}

/**
 * Get level name from CLI flags
 */
export function getLevelFromFlags(flags) {
  if (flags.admin) return 'admin';
  if (flags.apply) return 'write';
  if (flags.readonly) return 'read';
  return 'preview';
}

export default PermissionGate;
