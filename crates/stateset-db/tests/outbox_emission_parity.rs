//! Source lint: every mutating write path must emit its outbox fact inside the
//! same transaction as the mutation.
//!
//! Text/filesystem based (no `sqlite` feature, no live database) so it always
//! runs under a plain `cargo test -p stateset-db --test outbox_emission_parity`,
//! exactly like `backend_transaction_parity.rs` and `money_sql_lint.rs`.
//!
//! # Why
//!
//! A mutation that commits without its event silently desynchronises the local
//! store from the replicated log. That is invisible: the write succeeds, the
//! read-back is correct, and only a peer notices — much later, if ever. The
//! previous mechanism (`cli/src/sync/capture.js`) made a miss cost a
//! `console.warn`, so misses accumulated.
//!
//! # The rule
//!
//! A method is a **mutating write path** when its body contains an
//! `INSERT INTO`, `UPDATE` or `DELETE FROM` against a domain table. Every such
//! method must, in the same `with_immediate_transaction` / `begin_immediate`
//! block, call `append_kernel_event_tx` or `record_outbox_fact`.
//!
//! # Known blind spots (see task-1-report.md for detail)
//!
//! - Splitting is done at top-level `fn` line boundaries. A `fn` nested inside
//!   another function's body (a local closure-like helper, rare in this crate)
//!   would prematurely close the outer method and start a new one, which can
//!   misattribute a violation. None were observed in `src/sqlite/*.rs` at the
//!   time this lint was written, but this is a real limitation, not a
//!   theoretical one, and any future nested `fn` should be checked by hand.
//! - `mutates()` is a substring match on `"INSERT INTO"` / `"UPDATE "` /
//!   `"DELETE FROM"` against comment-filtered code. A string literal
//!   containing one of these phrases would still false-positive as a
//!   mutation — the filter only removes `//`-prefixed lines, not embedded
//!   string contents. None were observed in this crate's `src/sqlite/*.rs`.
//! - `emits()` only checks for the two known emitter call names anywhere in
//!   the method body — it does not verify the call happens inside the same
//!   transaction as the mutating statement (a `with_immediate_transaction`
//!   block above/below a raw connection write, say). A method could call an
//!   emitter incidentally elsewhere in its body and pass this lint without
//!   satisfying the atomicity requirement in the module doc comment above.
//!   Verifying that requires understanding transaction boundaries, which is
//!   out of scope for a text lint; later tasks that wire emission into the
//!   transactional helpers close this gap structurally instead.
//! - `strip_test_module()` assumes a file's only column-0 `#[cfg(test)]` is
//!   its single, trailing test module (see that function's doc comment for
//!   the two counterexamples found and fixed in fix round 1).
//! - The `(file, method)` backlog key is not unique — the same method name
//!   recurs across `impl` blocks or trait impls in one file — and
//!   `SAFE_EXCEPTIONS` still lists `migrations.rs`, which is dead: the
//!   `sqlite_sources()` scan only reads `src/sqlite/*.rs`, and migrations
//!   live elsewhere. Both are deferred, tracked findings, not fixed here.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Methods that mutate but must not emit, with the reason. Adding an entry is
/// a visible diff and needs a justification a reviewer can check.
const SAFE_EXCEPTIONS: &[(&str, &str, &str)] = &[
    ("kernel_outbox.rs", "*", "the outbox itself: emitting would recurse"),
    ("migrations.rs", "*", "schema changes are not domain events"),
    (
        "subscriptions.rs",
        "create_plan_item_with_conn",
        "a plan line is part of the plan's definition, not a fact of its own: \
         the only caller is `create_plan`, in its transaction, and one event \
         per line would multiply one domain action into N",
    ),
    (
        "subscriptions.rs",
        "create_subscription_item_with_conn",
        "the domain fact is `subscription.created`, which `create_subscription` \
         emits through `record_event_with_conn` in the same transaction; the \
         items are that subscription's lines",
    ),
    (
        "subscriptions.rs",
        "activate_if_trial_elapsed_with_tx",
        "the activation fact is `subscription.activated`, emitted by \
         `record_event_with_conn` in this same transaction three lines below \
         the UPDATE; emitting again would duplicate it",
    ),
    (
        "accounts_payable.rs",
        "recalculate_bill_with_conn",
        "bill subtotal/tax/total/amount_paid/amount_due are derived by summing \
         `ap_bill_items` and unvoided allocations; the facts are the line and \
         the payment that moved, which their own write paths emit",
    ),
    (
        "accounts_receivable.rs",
        "recalculate_invoice_with_conn",
        "invoice amount_paid/balance_due/status are derived by summing payment \
         and credit-memo applications; the facts are the application, write-off \
         or credit memo that moved, not the footing",
    ),
    (
        "invoices.rs",
        "recalculate_with_conn",
        "invoice subtotal/total/balance_due are derived by summing \
         `invoice_items` against the stored discount, tax, shipping and \
         amount_paid; the fact is whichever line or payment changed",
    ),
    (
        "general_ledger.rs",
        "update_account_balance_with_conn",
        "an account balance is derived from its postings: the fact is the \
         journal entry that posted, and a balance event per posted line would \
         restate it without adding anything a peer can act on",
    ),
    (
        "credit.rs",
        "recalculate_available_credit_with_conn",
        "`available_credit` is a derived column (limit - balance - holds), not \
         an independent fact: every caller has just emitted the fact for the \
         limit, balance or hold move that made this recompute necessary",
    ),
    (
        "credit.rs",
        "adjust_credit_limit_with_conn",
        "the limit move is already recorded as a `credit_account.transaction_recorded` \
         ledger row (transaction_type `limit_change`, amount = the delta, notes \
         = the reason) by `insert_transaction_with_conn` in this same \
         transaction",
    ),
    (
        "subscriptions.rs",
        "advance_subscription_after_paid_cycle_with_tx",
        "the fact is `subscription.renewed`, emitted by `record_event_with_conn` \
         in this same transaction and carrying the cycle number and the new \
         billing date; moving the clock is the subscription's own bookkeeping",
    ),
];

/// Mutating write paths that do not yet emit. **This is the Phase B work
/// order.** Entries are removed as coverage lands; the list must reach empty.
const OUTBOX_EMISSION_BACKLOG: &[(&str, &str)] = &[
    ("a2a.rs", "create_quote"),
    ("a2a.rs", "update_quote_status"),
    ("a2a.rs", "create_purchase"),
    ("a2a.rs", "update_purchase_status"),
    ("a2a.rs", "link_purchase_to_order"),
    ("a2a.rs", "confirm_delivery"),
    ("a2a_credit_terms.rs", "apply_movement"),
    ("a2a_credit_terms.rs", "create_terms"),
    ("a2a_messaging.rs", "send_message"),
    ("a2a_messaging.rs", "acknowledge_message"),
    ("a2a_messaging.rs", "fail_message"),
    ("accounts_payable.rs", "create_bill"),
    ("accounts_payable.rs", "update_bill"),
    ("accounts_payable.rs", "delete_bill"),
    ("accounts_payable.rs", "approve_bill"),
    ("accounts_payable.rs", "cancel_bill"),
    ("accounts_payable.rs", "dispute_bill"),
    ("accounts_payable.rs", "remove_bill_item"),
    ("accounts_payable.rs", "create_payment"),
    ("accounts_payable.rs", "void_payment"),
    ("accounts_payable.rs", "clear_payment"),
    ("accounts_payable.rs", "create_payment_run"),
    ("accounts_payable.rs", "approve_payment_run"),
    ("accounts_payable.rs", "process_payment_run"),
    ("accounts_payable.rs", "cancel_payment_run"),
    ("accounts_receivable.rs", "log_collection_activity"),
    ("accounts_receivable.rs", "update_collection_status"),
    ("accounts_receivable.rs", "send_dunning_letter"),
    ("accounts_receivable.rs", "create_write_off"),
    ("accounts_receivable.rs", "reverse_write_off"),
    ("accounts_receivable.rs", "create_credit_memo"),
    ("accounts_receivable.rs", "apply_credit_memo"),
    ("accounts_receivable.rs", "void_credit_memo"),
    ("accounts_receivable.rs", "apply_payment_to_invoices"),
    ("accounts_receivable.rs", "unapply_payment"),
    ("activity_logs.rs", "record"),
    ("agent_cards.rs", "create"),
    ("agent_cards.rs", "update"),
    ("agent_cards.rs", "delete"),
    ("agent_cards.rs", "verify"),
    ("agent_cards.rs", "suspend"),
    ("agent_cards.rs", "reactivate"),
    ("agent_cards.rs", "create_batch_atomic"),
    ("agent_identities.rs", "register"),
    ("agent_identities.rs", "update"),
    ("agent_identities.rs", "set_agent_wallet"),
    ("agent_identities.rs", "clear_agent_wallet"),
    ("agent_identities.rs", "set_metadata"),
    ("agent_identities.rs", "delete_metadata"),
    ("agent_reputation.rs", "give_feedback"),
    ("agent_reputation.rs", "revoke_feedback"),
    ("agent_reputation.rs", "append_response"),
    ("agent_validation.rs", "request_validation"),
    ("agent_validation.rs", "respond_validation"),
    ("backorder.rs", "release_open_allocations_in_tx"),
    ("backorder.rs", "allocate_in_tx"),
    ("backorder.rs", "settle_backorder_allocation_status_in_tx"),
    ("backorder.rs", "create_backorder_in_tx"),
    ("backorder.rs", "cancel_backorders_for_order_in_tx"),
    ("backorder.rs", "cancel_backorders_for_order_line_in_tx"),
    ("backorder.rs", "consume_stock_for_fulfilment_in_tx"),
    ("backorder.rs", "create_backorder"),
    ("backorder.rs", "update_backorder"),
    ("backorder.rs", "cancel_backorder"),
    ("backorder.rs", "fulfill_backorder"),
    ("backorder.rs", "release_allocation"),
    ("backorder.rs", "confirm_allocation"),
    ("backorder.rs", "expire_allocations"),
    ("bins.rs", "apply_bin_delta_tx"),
    ("bins.rs", "apply_warehouse_delta_tx"),
    ("bins.rs", "insert_bin_movement_tx"),
    ("bins.rs", "create_bin"),
    ("bins.rs", "update_bin"),
    ("bins.rs", "delete_bin"),
    ("bom.rs", "create"),
    ("bom.rs", "update"),
    ("bom.rs", "delete"),
    ("bom.rs", "add_component"),
    ("bom.rs", "update_component"),
    ("bom.rs", "remove_component"),
    ("bom.rs", "create_batch_atomic"),
    ("bom.rs", "update_batch_atomic"),
    ("bom.rs", "delete_batch_atomic"),
    ("carts.rs", "update_cart_totals"),
    ("carts.rs", "create"),
    ("carts.rs", "update"),
    ("carts.rs", "delete"),
    ("carts.rs", "update_item"),
    ("carts.rs", "remove_item"),
    ("carts.rs", "clear_items"),
    ("carts.rs", "set_shipping_address"),
    ("carts.rs", "set_billing_address"),
    ("carts.rs", "set_shipping"),
    ("carts.rs", "set_payment"),
    ("carts.rs", "set_x402_payment"),
    ("carts.rs", "apply_discount"),
    ("carts.rs", "remove_discount"),
    ("carts.rs", "mark_ready_for_payment"),
    ("carts.rs", "begin_checkout"),
    ("carts.rs", "cancel"),
    ("carts.rs", "abandon"),
    ("carts.rs", "expire"),
    ("carts.rs", "reserve_inventory"),
    ("carts.rs", "release_inventory"),
    ("carts.rs", "set_tax"),
    ("carts.rs", "get_expired"),
    ("carts.rs", "create_batch_atomic"),
    ("carts.rs", "update_batch_atomic"),
    ("carts.rs", "delete_batch_atomic"),
    ("carts.rs", "add_item_internal"),
    ("channels.rs", "create"),
    ("channels.rs", "update"),
    ("channels.rs", "delete"),
    ("channels.rs", "set_lock"),
    ("channels.rs", "sync_products"),
    ("companies.rs", "create"),
    ("companies.rs", "update"),
    ("companies.rs", "delete"),
    ("companies.rs", "create_contact"),
    ("cost_accounting.rs", "set_item_cost_with_conn"),
    ("cost_accounting.rs", "record_cost_transaction_with_conn"),
    ("cost_accounting.rs", "update_average_cost"),
    ("cost_accounting.rs", "update_last_cost"),
    ("cost_accounting.rs", "create_cost_layer"),
    ("cost_accounting.rs", "issue_fifo"),
    ("cost_accounting.rs", "issue_lifo"),
    ("cost_accounting.rs", "record_variance"),
    ("cost_accounting.rs", "create_adjustment"),
    ("cost_accounting.rs", "approve_adjustment"),
    ("cost_accounting.rs", "apply_adjustment"),
    ("cost_accounting.rs", "reject_adjustment"),
    ("cost_accounting.rs", "calculate_rollup"),
    ("credit.rs", "update_credit_account"),
    ("credit.rs", "suspend_credit_account"),
    ("credit.rs", "reactivate_credit_account"),
    ("credit.rs", "reserve_credit"),
    ("credit.rs", "charge_credit"),
    ("credit.rs", "place_hold"),
    ("credit.rs", "release_hold"),
    ("credit.rs", "submit_application"),
    ("credit.rs", "review_application"),
    ("credit.rs", "withdraw_application"),
    ("credit.rs", "apply_payment"),
    ("currency.rs", "set_rate"),
    ("currency.rs", "delete_rate"),
    ("currency.rs", "update_settings"),
    ("currency.rs", "set_rates_atomic"),
    ("currency.rs", "delete_rates_atomic"),
    ("custom_objects.rs", "create_type"),
    ("custom_objects.rs", "update_type"),
    ("custom_objects.rs", "delete_type"),
    ("custom_objects.rs", "create_object"),
    ("custom_objects.rs", "update_object"),
    ("custom_objects.rs", "delete_object"),
    ("customers.rs", "update_customer_tx"),
    ("customers.rs", "delete_customer_tx"),
    ("customers.rs", "sync_default_flags"),
    ("customers.rs", "set_default_pointer_tx"),
    ("customers.rs", "clear_pointers_to_tx"),
    ("customers.rs", "anonymize"),
    ("customers.rs", "add_address"),
    ("customers.rs", "update_address"),
    ("customers.rs", "delete_address"),
    ("edi_documents.rs", "create"),
    ("edi_documents.rs", "set_status"),
    ("fixed_assets.rs", "transition"),
    ("fixed_assets.rs", "create"),
    ("fixed_assets.rs", "update"),
    ("fixed_assets.rs", "generate_schedule"),
    ("fixed_assets.rs", "post_depreciation"),
    ("fraud.rs", "create_assessment"),
    ("fraud.rs", "review_assessment"),
    ("fraud.rs", "create_rule"),
    ("fraud.rs", "update_rule"),
    ("fraud.rs", "delete_rule"),
    ("fulfillment.rs", "insert_pick_tx"),
    ("fulfillment.rs", "create_wave"),
    ("fulfillment.rs", "release_wave"),
    ("fulfillment.rs", "complete_wave"),
    ("fulfillment.rs", "cancel_wave"),
    ("fulfillment.rs", "assign_pick"),
    ("fulfillment.rs", "start_pick"),
    ("fulfillment.rs", "complete_pick"),
    ("fulfillment.rs", "report_short"),
    ("fulfillment.rs", "cancel_pick"),
    ("fulfillment.rs", "create_pack"),
    ("fulfillment.rs", "assign_pack"),
    ("fulfillment.rs", "start_pack"),
    ("fulfillment.rs", "complete_pack"),
    ("fulfillment.rs", "add_carton"),
    ("fulfillment.rs", "add_carton_item"),
    ("fulfillment.rs", "mark_label_printed"),
    ("fulfillment.rs", "cancel_pack"),
    ("fulfillment.rs", "create_ship"),
    ("fulfillment.rs", "assign_ship"),
    ("fulfillment.rs", "print_label"),
    ("fulfillment.rs", "complete_ship"),
    ("fulfillment.rs", "cancel_ship"),
    ("general_ledger.rs", "create_account"),
    ("general_ledger.rs", "update_account"),
    ("general_ledger.rs", "delete_account"),
    ("general_ledger.rs", "create_period"),
    ("general_ledger.rs", "open_period"),
    ("general_ledger.rs", "close_period"),
    ("general_ledger.rs", "lock_period"),
    ("general_ledger.rs", "reopen_period"),
    ("general_ledger.rs", "create_journal_entry"),
    ("general_ledger.rs", "void_journal_entry"),
    ("general_ledger.rs", "reverse_journal_entry"),
    ("general_ledger.rs", "set_auto_posting_config"),
    ("gift_cards.rs", "create"),
    ("gift_cards.rs", "update"),
    ("gift_cards.rs", "charge"),
    ("gift_cards.rs", "refund"),
    ("gift_cards.rs", "disable"),
    ("http_idempotency.rs", "get"),
    ("http_idempotency.rs", "put"),
    ("http_idempotency.rs", "purge_expired"),
    ("inbound_shipments.rs", "advance_status"),
    ("inbound_shipments.rs", "apply_cancel_in_tx"),
    ("inbound_shipments.rs", "create"),
    ("inbound_shipments.rs", "receive_line"),
    ("integration_field_mappings.rs", "insert"),
    ("integration_field_mappings.rs", "update"),
    ("integration_field_mappings.rs", "delete"),
    ("integration_field_mappings.rs", "bulk_delete"),
    ("integration_mappings.rs", "create"),
    ("integration_mappings.rs", "update"),
    ("integration_mappings.rs", "delete"),
    ("integration_mappings.rs", "bulk_upsert"),
    ("inventory.rs", "apply_allocation_delta_in_tx"),
    ("inventory.rs", "expire_reservation_in_tx"),
    ("inventory.rs", "consume_available_in_tx"),
    ("inventory.rs", "create_item"),
    ("inventory.rs", "adjust"),
    ("inventory.rs", "record_transaction"),
    ("inventory.rs", "create_item_batch_atomic"),
    ("inventory.rs", "adjust_batch_atomic"),
    ("invoices.rs", "guarded_status_change"),
    ("invoices.rs", "create"),
    ("invoices.rs", "update"),
    ("invoices.rs", "delete"),
    ("invoices.rs", "mark_viewed"),
    ("invoices.rs", "record_payment"),
    ("invoices.rs", "add_item"),
    ("invoices.rs", "update_item"),
    ("invoices.rs", "remove_item"),
    ("invoices.rs", "create_batch_atomic"),
    ("invoices.rs", "update_batch_atomic"),
    ("invoices.rs", "delete_batch_atomic"),
    ("kernel_executor.rs", "enforce_budget_tx"),
    ("kernel_executor.rs", "provision_economic_budget"),
    ("kernel_executor.rs", "execute_reserve_inventory"),
    ("kernel_executor.rs", "execute_inventory_lifecycle"),
    ("lots.rs", "record_transaction"),
    ("lots.rs", "move_placements_on"),
    ("lots.rs", "record_genealogy_on"),
    ("lots.rs", "apply_inventory_delta_on"),
    ("lots.rs", "quarantine_lot_on"),
    ("lots.rs", "release_quarantine_on"),
    ("lots.rs", "release_reservation_on"),
    ("lots.rs", "create"),
    ("lots.rs", "update"),
    ("lots.rs", "delete"),
    ("lots.rs", "adjust"),
    ("lots.rs", "consume"),
    ("lots.rs", "reserve"),
    ("lots.rs", "confirm_reservation"),
    ("lots.rs", "transfer"),
    ("lots.rs", "split"),
    ("lots.rs", "merge"),
    ("lots.rs", "add_certificate"),
    ("lots.rs", "delete_certificate"),
    ("lots.rs", "expire_lots"),
    ("loyalty.rs", "create"),
    ("loyalty.rs", "enroll"),
    ("loyalty.rs", "adjust_points"),
    ("orders.rs", "create_internal_in_tx"),
    ("orders.rs", "delete_in_tx"),
    ("orders.rs", "add_item"),
    ("orders.rs", "remove_item_with"),
    ("orders.rs", "update_order_total"),
    ("payment_obligations.rs", "create"),
    ("payment_obligations.rs", "record_payment"),
    ("payment_obligations.rs", "set_status"),
    ("payment_obligations.rs", "link_bill"),
    ("payments.rs", "void_in_flight_payments_for_order_conn"),
    ("payments.rs", "update"),
    ("payments.rs", "mark_completed"),
    ("payments.rs", "mark_failed"),
    ("payments.rs", "complete_refund"),
    ("payments.rs", "fail_refund"),
    ("payments.rs", "create_payment_method"),
    ("payments.rs", "delete_payment_method"),
    ("payments.rs", "set_default_payment_method"),
    ("payments.rs", "update_batch_atomic"),
    ("payments.rs", "delete_batch"),
    ("payments.rs", "delete_batch_atomic"),
    ("prepayments.rs", "create"),
    ("prepayments.rs", "apply"),
    ("prepayments.rs", "reverse_application"),
    ("prepayments.rs", "refund"),
    ("price_levels.rs", "create"),
    ("price_levels.rs", "update"),
    ("price_levels.rs", "delete"),
    ("price_levels.rs", "set_entry"),
    ("price_levels.rs", "delete_entry"),
    ("price_schedules.rs", "create"),
    ("price_schedules.rs", "update"),
    ("price_schedules.rs", "delete"),
    ("price_schedules.rs", "set_entry"),
    ("price_schedules.rs", "delete_entry"),
    ("print_stations.rs", "pair"),
    ("print_stations.rs", "revoke_station"),
    ("print_stations.rs", "enqueue_job"),
    ("print_stations.rs", "next_job"),
    ("print_stations.rs", "complete_job"),
    ("production_batches.rs", "create"),
    ("production_batches.rs", "update"),
    ("production_batches.rs", "delete"),
    ("production_batches.rs", "add_work_orders"),
    ("production_batches.rs", "remove_work_order"),
    ("products.rs", "insert_product_tx"),
    ("products.rs", "update_product_tx"),
    ("products.rs", "archive_product_tx"),
    ("products.rs", "add_variant"),
    ("products.rs", "update_variant"),
    ("products.rs", "delete_variant"),
    ("promotions.rs", "create"),
    ("promotions.rs", "update"),
    ("promotions.rs", "delete"),
    ("promotions.rs", "create_condition"),
    ("promotions.rs", "create_coupon"),
    ("promotions.rs", "consume_cart_promotions_in_tx"),
    ("promotions.rs", "record_usage_in_tx"),
    ("promotions.rs", "consume_cart_coupon_in_tx"),
    ("promotions.rs", "set_coupon_status"),
    ("purchase_orders.rs", "transition"),
    ("purchase_orders.rs", "recalculate_totals_with_conn"),
    ("purchase_orders.rs", "create_supplier"),
    ("purchase_orders.rs", "update_supplier"),
    ("purchase_orders.rs", "delete_supplier"),
    ("purchase_orders.rs", "create"),
    ("purchase_orders.rs", "update"),
    ("purchase_orders.rs", "delete"),
    ("purchase_orders.rs", "receive"),
    ("purchase_orders.rs", "add_item"),
    ("purchase_orders.rs", "update_item"),
    ("purchase_orders.rs", "remove_item"),
    ("purchase_orders.rs", "create_batch_atomic"),
    ("purchase_orders.rs", "update_batch_atomic"),
    ("purchase_orders.rs", "delete_batch_atomic"),
    ("purgatory.rs", "ingest"),
    ("purgatory.rs", "map_line"),
    ("purgatory.rs", "post"),
    ("purgatory.rs", "delete"),
    ("quality.rs", "finish_ncr"),
    ("quality.rs", "create_inspection"),
    ("quality.rs", "update_inspection"),
    ("quality.rs", "delete_inspection"),
    ("quality.rs", "start_inspection"),
    ("quality.rs", "complete_inspection"),
    ("quality.rs", "record_inspection_result"),
    ("quality.rs", "create_ncr"),
    ("quality.rs", "update_ncr"),
    ("quality.rs", "create_hold"),
    ("quality.rs", "release_hold"),
    ("quality.rs", "create_defect_code"),
    ("quality.rs", "deactivate_defect_code"),
    ("receiving.rs", "update_receipt_totals_tx"),
    ("receiving.rs", "create_receipt"),
    ("receiving.rs", "update_receipt"),
    ("receiving.rs", "delete_receipt"),
    ("receiving.rs", "start_receiving"),
    ("receiving.rs", "receive_items"),
    ("receiving.rs", "complete_receiving"),
    ("receiving.rs", "cancel_receipt"),
    ("receiving.rs", "create_put_away"),
    ("receiving.rs", "assign_put_away"),
    ("receiving.rs", "start_put_away"),
    ("receiving.rs", "complete_put_away"),
    ("receiving.rs", "cancel_put_away"),
    ("returns.rs", "delete_return_tx"),
    ("returns.rs", "serial_hop"),
    ("returns.rs", "apply_lot_restore_tx"),
    ("returns.rs", "set_item_disposition"),
    ("revenue_recognition.rs", "create_contract"),
    ("revenue_recognition.rs", "update_contract"),
    ("revenue_recognition.rs", "generate_schedule"),
    ("revenue_recognition.rs", "recognize_period"),
    ("reviews.rs", "create"),
    ("reviews.rs", "update"),
    ("reviews.rs", "delete"),
    ("reviews.rs", "mark_helpful"),
    ("reviews.rs", "mark_reported"),
    ("rewards.rs", "create"),
    ("rewards.rs", "delete"),
    ("search_configs.rs", "create"),
    ("search_configs.rs", "update"),
    ("search_configs.rs", "delete"),
    ("search_configs.rs", "set_active"),
    ("segments.rs", "create"),
    ("segments.rs", "update"),
    ("segments.rs", "delete"),
    ("segments.rs", "add_member"),
    ("segments.rs", "remove_member"),
    ("serials.rs", "record_history"),
    ("serials.rs", "close_open_reservations"),
    ("serials.rs", "write_transition"),
    ("serials.rs", "create"),
    ("serials.rs", "create_bulk"),
    ("serials.rs", "update"),
    ("serials.rs", "delete"),
    ("serials.rs", "reserve"),
    ("serials.rs", "release_reservation"),
    ("serials.rs", "confirm_reservation"),
    ("serials.rs", "release_expired_reservations"),
    ("serials.rs", "move_serial"),
    ("serials.rs", "activate"),
    ("shipments.rs", "update_status"),
    ("shipments.rs", "create"),
    ("shipments.rs", "update"),
    ("shipments.rs", "delete"),
    ("shipments.rs", "ship"),
    ("shipments.rs", "mark_delivered"),
    ("shipments.rs", "add_item"),
    ("shipments.rs", "remove_item"),
    ("shipments.rs", "add_event"),
    ("shipments.rs", "create_batch_atomic"),
    ("shipments.rs", "update_batch_atomic"),
    ("shipments.rs", "delete_batch_atomic"),
    ("shipping_zones.rs", "create"),
    ("shipping_zones.rs", "update"),
    ("shipping_zones.rs", "delete"),
    ("stock_snapshots.rs", "capture"),
    ("stock_snapshots.rs", "delete"),
    ("store_credits.rs", "create"),
    ("store_credits.rs", "adjust"),
    ("store_credits.rs", "apply"),
    ("subscriptions.rs", "create_plan"),
    ("subscriptions.rs", "update_plan"),
    ("subscriptions.rs", "create_subscription"),
    ("subscriptions.rs", "claim_due_for_billing"),
    ("subscriptions.rs", "release_billing_claim"),
    ("subscriptions.rs", "update_subscription"),
    ("subscriptions.rs", "pause_subscription"),
    ("subscriptions.rs", "resume_subscription"),
    ("subscriptions.rs", "cancel_subscription"),
    ("subscriptions.rs", "skip_billing_cycle"),
    ("supplier_skus.rs", "create"),
    ("supplier_skus.rs", "update"),
    ("supplier_skus.rs", "delete"),
    ("supplier_skus.rs", "bulk_upsert"),
    ("tax.rs", "create_jurisdiction"),
    ("tax.rs", "create_rate"),
    ("tax.rs", "verify_exemption"),
    ("tax.rs", "create_exemption"),
    ("tax.rs", "update_settings"),
    ("tax.rs", "save_calculation"),
    ("topology_snapshots.rs", "capture"),
    ("topology_snapshots.rs", "delete"),
    ("transfer_orders.rs", "create"),
    ("transfer_orders.rs", "ship"),
    ("transfer_orders.rs", "receive_line"),
    ("transfer_orders.rs", "cancel"),
    ("units_of_measure.rs", "create_class"),
    ("units_of_measure.rs", "delete_class"),
    ("units_of_measure.rs", "create_uom"),
    ("units_of_measure.rs", "set_base_uom"),
    ("units_of_measure.rs", "delete_uom"),
    ("units_of_measure.rs", "create_rule"),
    ("units_of_measure.rs", "delete_rule"),
    ("vector.rs", "delete_embedding"),
    ("vector.rs", "clear_embeddings"),
    ("vendor_credits.rs", "create"),
    ("vendor_credits.rs", "apply"),
    ("vendor_credits.rs", "reverse_application"),
    ("vendor_credits.rs", "cancel"),
    ("vendor_returns.rs", "create"),
    ("vendor_returns.rs", "submit"),
    ("vendor_returns.rs", "process"),
    ("vendor_returns.rs", "cancel"),
    ("warehouse.rs", "apply_location_delta_tx"),
    ("warehouse.rs", "insert_wms_movement_tx"),
    ("warehouse.rs", "transition_cycle_count"),
    ("warehouse.rs", "create_warehouse"),
    ("warehouse.rs", "update_warehouse"),
    ("warehouse.rs", "delete_warehouse"),
    ("warehouse.rs", "create_zone"),
    ("warehouse.rs", "update_zone"),
    ("warehouse.rs", "delete_zone"),
    ("warehouse.rs", "create_location"),
    ("warehouse.rs", "update_location"),
    ("warehouse.rs", "delete_location"),
    ("warehouse.rs", "adjust_inventory"),
    ("warehouse.rs", "move_inventory"),
    ("warehouse.rs", "create_cycle_count"),
    ("warehouse.rs", "record_cycle_counts"),
    ("warehouse.rs", "complete_cycle_count"),
    ("warranties.rs", "create"),
    ("warranties.rs", "update"),
    ("warranties.rs", "transfer"),
    ("warranties.rs", "create_claim"),
    ("warranties.rs", "update_claim"),
    ("warranties.rs", "approve_claim"),
    ("warranties.rs", "deny_claim"),
    ("warranties.rs", "complete_claim"),
    ("warranties.rs", "cancel_claim"),
    ("warranties.rs", "create_batch_atomic"),
    ("warranties.rs", "update_batch_atomic"),
    ("warranties.rs", "delete_batch_atomic"),
    ("wishlists.rs", "create"),
    ("wishlists.rs", "update"),
    ("wishlists.rs", "delete"),
    ("wishlists.rs", "add_item"),
    ("wishlists.rs", "remove_item"),
    ("work_orders.rs", "create"),
    ("work_orders.rs", "update"),
    ("work_orders.rs", "delete"),
    ("work_orders.rs", "start"),
    ("work_orders.rs", "complete"),
    ("work_orders.rs", "add_task"),
    ("work_orders.rs", "update_task"),
    ("work_orders.rs", "remove_task"),
    ("work_orders.rs", "start_task"),
    ("work_orders.rs", "complete_task"),
    ("work_orders.rs", "add_material"),
    ("work_orders.rs", "consume_material"),
    ("work_orders.rs", "create_batch_atomic"),
    ("work_orders.rs", "update_batch_atomic"),
    ("work_orders.rs", "delete_batch_atomic"),
    ("x402_credits.rs", "insert_account_if_missing"),
    ("x402_credits.rs", "adjust_balance"),
    ("x402_payment_intents.rs", "insert_new_intent"),
    ("x402_payment_intents.rs", "sign"),
    ("x402_payment_intents.rs", "mark_sequenced"),
    ("x402_payment_intents.rs", "mark_batched"),
    ("x402_payment_intents.rs", "mark_settled"),
    ("x402_payment_intents.rs", "mark_failed"),
    ("x402_payment_intents.rs", "mark_expired"),
    ("x402_payment_intents.rs", "cancel"),
    ("x402_payment_intents.rs", "expire_stale_intents"),
    ("zone_shipping_methods.rs", "create"),
    ("zone_shipping_methods.rs", "delete"),
];

/// Drop everything from the trailing `#[cfg(test)]` module onward.
///
/// Every `src/sqlite/*.rs` file in this crate puts its `#[cfg(test)] mod
/// tests { ... }` block as a single trailing module at column 0. Without
/// this cut, the `fn`-boundary splitter below treats test helpers
/// (`seed_inventory`, `seed_customer`, ...) and `#[test]` functions that
/// build fixtures with raw `INSERT INTO` calls as if they were production
/// write paths, which would flood the backlog with fixture code that was
/// never meant to emit an outbox fact.
///
/// The split is anchored to `"\n#[cfg(test)]"` (column 0), matching
/// `backend_transaction_parity.rs`'s `methods()`, precisely so an
/// *indented* `#[cfg(test)]` on a test-only helper that lives ahead of the
/// real trailing module — e.g. `carts.rs`'s `checkout_money_in_tx` at
/// column >0 — does not truncate early. An unanchored `source.find(...)`
/// match here previously discarded 759 lines of production code in
/// `carts.rs` (14 methods, including `complete_checkout_with_policy_in_tx`,
/// which mutates `orders`/`carts` without emitting) and 427 lines in
/// `mod.rs` (a mid-file `mod unique_constraint_mapping_tests`). If a file
/// ever grows a second, non-trailing, column-0 `#[cfg(test)]` module, this
/// truncates too much and silently drops real production code from the
/// scan — there is no structural guard against that here, only the
/// convention that trailing test modules are the only column-0 occurrence.
fn strip_test_module(source: &str) -> &str {
    source.split_once("\n#[cfg(test)]").map_or(source, |(head, _)| head)
}

fn sqlite_sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sqlite");
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).expect("read src/sqlite") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).expect("file name").to_string();
        let full = fs::read_to_string(&path).expect("read source");
        let production = strip_test_module(&full).to_string();
        out.push((name, production));
    }
    out.sort();
    out
}

/// The function name declared on this line, if it declares one.
///
/// Strips any combination of visibility/qualifier prefixes before `fn `,
/// the same generic approach `backend_transaction_parity.rs::declared_fn`
/// uses — not a fixed list of 5 forms. A fixed list
/// (`pub fn `/`fn `/`pub(crate) fn `/`pub async fn `/`async fn `) misses
/// `pub const fn`, `pub(crate) const fn`, `const fn` and `pub(super) fn`,
/// all of which occur in `src/sqlite/*.rs`; a method declared with one of
/// those forms would never open a new method here, so its body — and any
/// mutation or emission in it — silently merges into whichever method
/// happened to precede it in the file.
fn declared_fn(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    loop {
        let stripped = ["pub(crate) ", "pub(super) ", "pub ", "async ", "const ", "unsafe "]
            .iter()
            .find_map(|prefix| rest.strip_prefix(prefix));
        match stripped {
            Some(next) => rest = next,
            None => break,
        }
    }
    let rest = rest.strip_prefix("fn ")?;
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    (end > 0).then(|| &rest[..end])
}

/// Split a source file into `(method_name, code)` pairs at `fn` boundaries.
///
/// `code` excludes lines whose trimmed start is `//`, matching
/// `backend_transaction_parity.rs::methods()`'s comment filter. Without it,
/// `methods()` runs each method's body to the line *before* the next `fn`,
/// which includes the next method's leading doc comment; `mutates()` then
/// matches English prose like "the UPDATE is additionally guarded" as if it
/// were a SQL statement.
fn methods(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current: Option<(String, String)> = None;

    for line in source.lines() {
        if let Some(name) = declared_fn(line) {
            if let Some(finished) = current.take() {
                out.push(finished);
            }
            current = Some((name.to_string(), String::new()));
        } else if let Some((_, code)) = current.as_mut() {
            if !line.trim_start().starts_with("//") {
                code.push_str(line);
                code.push('\n');
            }
        }
    }
    if let Some(finished) = current {
        out.push(finished);
    }
    out
}

fn mutates(body: &str) -> bool {
    body.contains("INSERT INTO") || body.contains("UPDATE ") || body.contains("DELETE FROM")
}

fn emits(body: &str) -> bool {
    body.contains("append_kernel_event_tx") || body.contains("record_outbox_fact")
}

fn exempt(file: &str, method: &str) -> bool {
    SAFE_EXCEPTIONS.iter().any(|(f, m, _)| *f == file && (*m == "*" || *m == method))
}

#[test]
fn every_mutating_write_path_emits_an_outbox_fact() {
    let backlog: BTreeSet<(&str, &str)> = OUTBOX_EMISSION_BACKLOG.iter().copied().collect();
    let mut violations: Vec<(String, String)> = Vec::new();
    let mut stale_backlog: Vec<(&str, &str)> = Vec::new();

    for (file, source) in sqlite_sources() {
        for (method, body) in methods(&source) {
            if exempt(&file, &method) || !mutates(&body) {
                continue;
            }
            let tracked = backlog.contains(&(file.as_str(), method.as_str()));
            match (emits(&body), tracked) {
                (false, false) => violations.push((file.clone(), method.clone())),
                (true, true) => {
                    // Fixed but still listed: the backlog must shrink honestly.
                    stale_backlog.push((
                        OUTBOX_EMISSION_BACKLOG
                            .iter()
                            .find(|(f, m)| *f == file && *m == method)
                            .map(|(f, _)| *f)
                            .unwrap_or_default(),
                        OUTBOX_EMISSION_BACKLOG
                            .iter()
                            .find(|(f, m)| *f == file && *m == method)
                            .map(|(_, m)| *m)
                            .unwrap_or_default(),
                    ));
                }
                _ => {}
            }
        }
    }

    assert!(
        violations.is_empty(),
        "these mutating write paths commit without emitting an outbox fact, and \
         are not tracked in OUTBOX_EMISSION_BACKLOG:\n{}",
        violations
            .iter()
            .map(|(f, m)| format!("    (\"{f}\", \"{m}\"),"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert!(
        stale_backlog.is_empty(),
        "these entries now emit and must be removed from OUTBOX_EMISSION_BACKLOG:\n{stale_backlog:#?}"
    );
}
