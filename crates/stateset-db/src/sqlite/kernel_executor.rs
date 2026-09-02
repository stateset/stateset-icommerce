//! Envelope-aware execution for high-risk SQLite commerce commands.

use super::backorder::cancel_backorders_for_order_in_tx;
use super::carts::SqliteCartRepository;
use super::general_ledger::SqliteGeneralLedgerRepository;
use super::inventory::SqliteInventoryRepository;
use super::kernel_outbox::{
    append_kernel_event_tx, append_kernel_receipt_tx, receipt_by_idempotency_key_tx,
    sealed_audit_entry_tx,
};
use super::orders::{ShipMode, SqliteOrderRepository};
use super::payments::{
    SqlitePaymentRepository, check_order_capture_capacity_tx, open_captures_for_order_conn,
    void_in_flight_payments_for_order_conn,
};
use super::returns::{SqliteReturnRepository, row_to_return_item};
use super::subscriptions::SqliteSubscriptionRepository;
use super::x402_payment_intents::SqliteX402PaymentIntentRepository;
use super::{
    parse_datetime_opt_row, parse_datetime_row, parse_decimal_row, parse_uuid_row,
    with_immediate_transaction,
};
use crate::kernel::plans::PlanOutcome;
use crate::kernel::plans::escrow::{
    ESCROW_UNVERSIONED, create_escrow_guard, escrow_id_guard, escrow_legacy_amount,
    plan_fund_escrow,
};
use crate::kernel::plans::orders::{
    OrderTransitionSnapshot, ShipOrderSnapshot, plan_order_transition, plan_ship_order,
    reservation_expired_during_shipment, ship_order_guard, transition_order_guard,
};
use crate::kernel::plans::payments::{RefundSnapshot, create_payment_guard, plan_refund};
use crate::kernel::receipt::{
    attach_command_context, checkout_error_code, preview_receipt, principal_kind_name,
    receipt_record, rejected_receipt, succeeded_receipt,
};
use crate::kernel::{CommandRun, EnvelopeGuard, Replay, resolve_replay};
use crate::kernel_outbox::semantic_request_hash;
use crate::{KernelOutboxEvent, KernelReceiptRecord};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rusqlite::params;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use stateset_core::{
    A2ADispute, A2ADisputeEvidence, A2ADisputeResolution, A2ADisputeResolutionType,
    A2ADisputeStatus, A2AEscrow, A2AEscrowStatus, BillingCycleStatus, ChargeSubscription,
    CheckoutResult, CommandEnvelope, CommerceError, CommitCheckout, ConfirmInventoryReservation,
    CreateA2AEscrow, CreateInventoryItem, CreatePayment, CreateProduct, CreateRefund,
    DisputeA2AEscrow, ExecutionMode, ExecutionReceipt, ExecutionStatus, FileA2ADispute,
    FundA2AEscrow, InventoryItem, InventoryReservation, JournalEntry, JournalEntryStatus,
    KernelPolicy, Order, OrderStatus, Payment, PaymentTransactionStatus, PostJournalEntry, Product,
    ProductId, ProductStatus, Refund, RefundA2AEscrow, RefundStatus, ReleaseA2AEscrow,
    ReleaseInventoryReservation, ReservationStatus, ReserveInventory, ResolveA2ADispute, Result,
    RetryDisposition, Return, SettleX402Intent, ShipOrderCommand, SubmitA2ADisputeEvidence,
    SubscriptionCharge, SubscriptionStatus, TransitionOrder, TransitionReturn, Validate,
    X402IntentStatus, X402PaymentIntent,
};
use uuid::Uuid;

const CREATE_PAYMENT_COMMAND: &str = "payments.create";
const CREATE_PRODUCT_COMMAND: &str = "products.create";
const CREATE_INVENTORY_ITEM_COMMAND: &str = "inventory.item.create";
const CREATE_REFUND_COMMAND: &str = "payments.create_refund";
const RESERVE_INVENTORY_COMMAND: &str = "inventory.reserve";
const CONFIRM_RESERVATION_COMMAND: &str = "inventory.reservation.confirm";
const RELEASE_RESERVATION_COMMAND: &str = "inventory.reservation.release";
const TRANSITION_ORDER_COMMAND: &str = "orders.transition";
const SHIP_ORDER_COMMAND: &str = "orders.ship";
const TRANSITION_RETURN_COMMAND: &str = "returns.transition";
const POST_LEDGER_COMMAND: &str = "ledger.post";
const SETTLE_X402_COMMAND: &str = "x402.settle";
const COMMIT_CHECKOUT_COMMAND: &str = "checkout.commit";
const CHARGE_SUBSCRIPTION_COMMAND: &str = "subscriptions.charge";
const CREATE_A2A_ESCROW_COMMAND: &str = "a2a.escrow.create";
const DISPUTE_A2A_ESCROW_COMMAND: &str = "a2a.escrow.dispute";
const FUND_A2A_ESCROW_COMMAND: &str = "a2a.escrow.fund";
const RELEASE_A2A_ESCROW_COMMAND: &str = "a2a.escrow.release";
const REFUND_A2A_ESCROW_COMMAND: &str = "a2a.escrow.refund";
const FILE_A2A_DISPUTE_COMMAND: &str = "a2a.dispute.file";
const SUBMIT_A2A_EVIDENCE_COMMAND: &str = "a2a.dispute.evidence.submit";
const RESOLVE_A2A_DISPUTE_COMMAND: &str = "a2a.dispute.resolve";

fn to_sql_err(error: CommerceError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

#[derive(Clone, Copy)]
enum InventoryLifecycleAction {
    Confirm(Option<rust_decimal::Decimal>),
    Release,
}

/// Executes kernel commands with durable idempotency and policy receipts.
#[derive(Debug, Clone)]
pub struct SqliteKernelExecutor {
    pool: Pool<SqliteConnectionManager>,
    policy: KernelPolicy,
}

impl SqliteKernelExecutor {
    pub(crate) const fn new(pool: Pool<SqliteConnectionManager>, policy: KernelPolicy) -> Self {
        Self { pool, policy }
    }

    /// Preview or atomically create a SKU master, initial exact quantity,
    /// inventory transaction, outbox fact, and sealed receipt.
    pub fn execute_create_inventory_item(
        &self,
        command: &CommandEnvelope<CreateInventoryItem>,
    ) -> Result<ExecutionReceipt<InventoryItem>> {
        command
            .validate_contract()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let input = command.payload.clone();
        let request_hash = semantic_request_hash(command, &input)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != CREATE_INVENTORY_ITEM_COMMAND {
            Some((
                "kernel.command_type_mismatch",
                "expected inventory.item.create command type".into(),
            ))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "create commands cannot carry an expected aggregate version".into(),
            ))
        } else if let Err(error) = input.validate() {
            Some(("commerce.validation_failed", error.to_string()))
        } else {
            None
        };
        let location_id = input.location_id.unwrap_or(1);
        let initial_quantity = input.initial_quantity.unwrap_or_default();

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "inventory_item")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "inventory_item",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let sku_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM inventory_items WHERE sku = ?)",
                [&input.sku],
                |row| row.get(0),
            )?;
            let location_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM inventory_locations WHERE id = ?)",
                [location_id],
                |row| row.get(0),
            )?;
            if sku_exists || !location_exists {
                let (code, message) = if sku_exists {
                    (
                        "commerce.inventory.sku_conflict",
                        format!("inventory SKU '{}' already exists", input.sku),
                    )
                } else {
                    (
                        "commerce.inventory.location_not_found",
                        format!("inventory location {location_id} does not exist"),
                    )
                };
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    &message,
                    RetryDisposition::Never,
                    "inventory_item",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "inventory_item");
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let now = Utc::now();
            let unit = input.unit_of_measure.clone().unwrap_or_else(|| "EA".into());
            tx.execute(
                "INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, 1, ?, ?)",
                params![&input.sku, &input.name, &input.description, &unit, now.to_rfc3339(), now.to_rfc3339()],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock, version, updated_at) VALUES (?, ?, ?, '0', ?, ?, ?, 1, ?)",
                params![id, location_id, initial_quantity.to_string(), initial_quantity.to_string(), input.reorder_point.map(|v| v.to_string()), input.safety_stock.map(|v| v.to_string()), now.to_rfc3339()],
            )?;
            if initial_quantity > rust_decimal::Decimal::ZERO {
                tx.execute(
                    "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_by, created_at) VALUES (?, ?, 'receipt', ?, 'Initial stock', ?, ?)",
                    params![id, location_id, initial_quantity.to_string(), &command.principal.id, now.to_rfc3339()],
                )?;
            }
            let item = InventoryItem {
                id,
                sku: input.sku.clone(),
                name: input.name.clone(),
                description: input.description.clone(),
                unit_of_measure: unit,
                is_active: true,
                created_at: now,
                updated_at: now,
            };
            let mut event = KernelOutboxEvent::domain(
                "inventory.item.created.v1",
                "inventory_item",
                id.to_string(),
                serde_json::json!({"item_id": id, "sku": &item.sku, "location_id": location_id, "initial_quantity": initial_quantity.to_string()}),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = succeeded_kernel_receipt(
                command,
                policy.clone(),
                item,
                "inventory_item",
                id.to_string(),
                vec![event.id],
                started_at,
            );
            receipt.version_after = Some(1);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically apply `products.create`, including every supplied
    /// exact-decimal variant, its outbox fact, and sealed receipt.
    pub fn execute_create_product(
        &self,
        command: &CommandEnvelope<CreateProduct>,
    ) -> Result<ExecutionReceipt<Product>> {
        command
            .validate_contract()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;

        let input = command.payload.clone();
        let request_hash = semantic_request_hash(command, &input)?;
        let now = Utc::now();
        let policy = self.policy.evaluate(command, now);
        let guard = if command.command_type != CREATE_PRODUCT_COMMAND {
            Some(("kernel.command_type_mismatch", "expected products.create command type".into()))
        } else if command.deadline.is_some_and(|deadline| deadline <= now) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "create commands cannot carry an expected aggregate version".into(),
            ))
        } else if let Err(error) = input.validate() {
            Some(("commerce.validation_failed", error.to_string()))
        } else {
            None
        };
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
        let attributes_json = serde_json::to_string(&input.attributes.clone().unwrap_or_default())
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let seo_json = input
            .seo
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "product")?
                {
                    return Ok(stored);
                }
            }

            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "product",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let slug_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE slug = ?)",
                [&slug],
                |row| row.get(0),
            )?;
            let duplicate_sku = if let Some(variants) = &input.variants {
                let mut duplicate = None;
                for variant in variants {
                    let exists: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM product_variants WHERE sku = ?)",
                        [&variant.sku],
                        |row| row.get(0),
                    )?;
                    if exists {
                        duplicate = Some(variant.sku.clone());
                        break;
                    }
                }
                duplicate
            } else {
                None
            };
            if slug_exists || duplicate_sku.is_some() {
                let (code, message) = if slug_exists {
                    (
                        "commerce.product.slug_conflict",
                        format!("product slug '{slug}' already exists"),
                    )
                } else {
                    let sku = duplicate_sku.as_deref().unwrap_or_default();
                    ("commerce.product.sku_conflict", format!("product SKU '{sku}' already exists"))
                };
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    &message,
                    RetryDisposition::Never,
                    "product",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "product");
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let id = ProductId::new();
            let created_at = Utc::now();
            let description = input.description.clone().unwrap_or_default();
            let product_type = input.product_type.unwrap_or_default();
            tx.execute(
                "INSERT INTO products (
                    id, name, slug, description, status, product_type,
                    attributes, seo, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    &input.name,
                    &slug,
                    &description,
                    ProductStatus::Draft.to_string(),
                    product_type.to_string(),
                    &attributes_json,
                    &seo_json,
                    created_at.to_rfc3339(),
                    created_at.to_rfc3339(),
                ],
            )?;

            if let Some(variants) = &input.variants {
                for (index, variant) in variants.iter().enumerate() {
                    let options =
                        serde_json::to_string(&variant.options.clone().unwrap_or_default())
                            .map_err(|error| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(error))
                            })?;
                    tx.execute(
                        "INSERT INTO product_variants (
                            id, product_id, sku, name, price, compare_at_price, cost,
                            barcode, weight, weight_unit, options, is_default, is_active,
                            created_at, updated_at
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                        params![
                            Uuid::new_v4().to_string(),
                            id.to_string(),
                            &variant.sku,
                            variant.name.as_deref().unwrap_or(&variant.sku),
                            variant.price.to_string(),
                            variant.compare_at_price.map(|value| value.to_string()),
                            variant.cost.map(|value| value.to_string()),
                            &variant.barcode,
                            variant.weight.map(|value| value.to_string()),
                            &variant.weight_unit,
                            options,
                            i32::from(index == 0),
                            created_at.to_rfc3339(),
                            created_at.to_rfc3339(),
                        ],
                    )?;
                }
            }

            let product = Product {
                id,
                name: input.name.clone(),
                slug: slug.clone(),
                description,
                status: ProductStatus::Draft,
                product_type,
                attributes: input.attributes.clone().unwrap_or_default(),
                seo: input.seo.clone(),
                created_at,
                updated_at: created_at,
            };
            let mut event = KernelOutboxEvent::domain(
                "products.created.v1",
                "product",
                id.to_string(),
                serde_json::json!({
                    "product_id": id.to_string(),
                    "name": &product.name,
                    "slug": &product.slug,
                    "status": product.status.to_string(),
                    "variants": input.variants.as_ref().map(|variants| variants.iter().map(|variant| {
                        serde_json::json!({"sku": &variant.sku, "price": variant.price.to_string()})
                    }).collect::<Vec<_>>()).unwrap_or_default(),
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = succeeded_kernel_receipt(
                command,
                policy.clone(),
                product,
                "product",
                id.to_string(),
                vec![event.id],
                now,
            );
            receipt.version_after = Some(1);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically apply `payments.create`.
    pub fn execute_create_payment(
        &self,
        command: &CommandEnvelope<CreatePayment>,
    ) -> Result<ExecutionReceipt<Payment>> {
        let mut input = command.payload.clone();
        if input.idempotency_key.is_none() {
            input.idempotency_key = Some(command.idempotency_key.clone());
        }
        let run = CommandRun::prepare(
            command,
            &input,
            &self.policy,
            EnvelopeGuard::create(CREATE_PAYMENT_COMMAND)
                .with_payload_key(input.idempotency_key.as_deref()),
            "payment",
        )?
        .then_guard(|_| create_payment_guard(&input));
        let request_hash = run.request_hash.clone();
        let started_at = run.started_at;

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)?
                && let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "payment")?
            {
                return Ok(stored);
            }
            if let Some(mut receipt) = run.guard_receipt() {
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if run.is_preview() {
                let mut receipt = run.previewed();
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            if let Some(order_id) = input.order_id {
                check_order_capture_capacity_tx(
                    tx,
                    &order_id.to_string(),
                    None,
                    input.amount,
                    input.currency.unwrap_or_default(),
                )?;
            }

            let id = Uuid::new_v4();
            let created_at = Utc::now();
            let payment_number = stateset_core::generate_payment_number();
            tx.execute(
                "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    payment_number,
                    input.order_id.map(|value| value.to_string()),
                    input.invoice_id.map(|value| value.to_string()),
                    input.customer_id.map(|value| value.to_string()),
                    PaymentTransactionStatus::Pending.to_string(),
                    input.payment_method.to_string(),
                    input.amount.to_string(),
                    input.currency.unwrap_or_default(),
                    "0",
                    input.external_id,
                    input.idempotency_key,
                    input.processor,
                    input.card_brand.map(|value| value.to_string()),
                    input.card_last4,
                    input.card_exp_month,
                    input.card_exp_year,
                    input.billing_email,
                    input.billing_name,
                    input.billing_address,
                    input.description,
                    input.metadata,
                    created_at.to_rfc3339(),
                    created_at.to_rfc3339(),
                ],
            )?;

            let payment = tx.query_row(
                "SELECT * FROM payments WHERE id = ?",
                [id.to_string()],
                SqlitePaymentRepository::row_to_payment,
            )?;
            let event = run.event(
                "payments.created.v1",
                "payment",
                id.to_string(),
                serde_json::json!({
                    "payment_id": id.to_string(),
                    "payment_number": payment.payment_number,
                    "order_id": payment.order_id.map(|value| value.to_string()),
                    "amount": payment.amount.to_string(),
                    "currency": payment.currency.as_str(),
                    "status": payment.status.to_string(),
                }),
            );
            append_kernel_event_tx(tx, &event)?;
            let _ = started_at;
            let mut receipt =
                run.succeeded(payment, Some(id.to_string()), None, Some(1), vec![event.id]);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically apply `payments.create_refund`.
    pub fn execute_create_refund(
        &self,
        command: &CommandEnvelope<CreateRefund>,
    ) -> Result<ExecutionReceipt<Refund>> {
        let mut input = command.payload.clone();
        if input.idempotency_key.is_none() {
            input.idempotency_key = Some(command.idempotency_key.clone());
        }
        let run = CommandRun::prepare(
            command,
            &input,
            &self.policy,
            EnvelopeGuard::create(CREATE_REFUND_COMMAND)
                .with_payload_key(input.idempotency_key.as_deref()),
            "refund",
        )?;
        let request_hash = run.request_hash.clone();
        let payment_id = input.payment_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)?
                && let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "refund")?
            {
                return Ok(stored);
            }
            if let Some(mut receipt) = run.guard_receipt() {
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let snapshot = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [&payment_id],
                    SqlitePaymentRepository::row_to_payment,
                )
                .optional()?
                .map(|payment| {
                    let mut in_flight_refunds = rust_decimal::Decimal::ZERO;
                    let mut statement = tx.prepare(
                        "SELECT amount FROM refunds
                         WHERE payment_id = ? AND status IN ('pending', 'processing')",
                    )?;
                    let rows = statement.query_map([&payment_id], |row| {
                        let amount: String = row.get(0)?;
                        parse_decimal_row(&amount, "refund", "amount")
                    })?;
                    for row in rows {
                        in_flight_refunds += row?;
                    }
                    let open_dispute = open_dispute_for_payment(tx, &payment_id)?;
                    Ok::<_, rusqlite::Error>(RefundSnapshot {
                        payment,
                        in_flight_refunds,
                        open_dispute,
                    })
                })
                .transpose()?;
            let effects = match plan_refund(&input, snapshot.as_ref()) {
                PlanOutcome::Reject { rejection, .. } => {
                    let mut receipt = run.rejected_by(&rejection);
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                PlanOutcome::Proceed(effects) => effects,
            };
            if run.is_preview() {
                let mut receipt = run.previewed();
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let id = Uuid::new_v4();
            let created_at = Utc::now();
            let refund_number = stateset_core::generate_refund_number();
            tx.execute(
                "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency,
                 reason, external_id, idempotency_key, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    refund_number,
                    payment_id,
                    RefundStatus::Pending.to_string(),
                    effects.amount.to_string(),
                    effects.currency,
                    input.reason,
                    input.external_id,
                    input.idempotency_key,
                    input.notes,
                    created_at.to_rfc3339(),
                    created_at.to_rfc3339(),
                ],
            )?;
            let refund = tx.query_row(
                "SELECT * FROM refunds WHERE id = ?",
                [id.to_string()],
                SqlitePaymentRepository::row_to_refund,
            )?;
            let event = run.event(
                "payments.refund_created.v1",
                "refund",
                id.to_string(),
                serde_json::json!({
                    "refund_id": id.to_string(),
                    "refund_number": refund.refund_number,
                    "payment_id": refund.payment_id.to_string(),
                    "amount": refund.amount.to_string(),
                    "currency": refund.currency.as_str(),
                    "status": refund.status.to_string(),
                }),
            );
            append_kernel_event_tx(tx, &event)?;
            let mut receipt =
                run.succeeded(refund, Some(id.to_string()), None, Some(1), vec![event.id]);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically apply `inventory.reserve`.
    pub fn execute_reserve_inventory(
        &self,
        command: &CommandEnvelope<ReserveInventory>,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        command
            .validate_contract()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let input = &command.payload;
        let request_hash = semantic_request_hash(command, input)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let static_guard = if command.command_type != RESERVE_INVENTORY_COMMAND {
            Some((
                "kernel.command_type_mismatch",
                "expected inventory.reserve command type".to_string(),
            ))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some((
                "kernel.deadline_exceeded",
                "command deadline elapsed before execution".to_string(),
            ))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if input.sku.trim().is_empty()
            || input.reference_type.trim().is_empty()
            || input.reference_id.trim().is_empty()
        {
            Some((
                "commerce.inventory_validation_failed",
                "sku, reference_type, and reference_id are required".to_string(),
            ))
        } else if input.expires_in_seconds.is_some_and(|seconds| seconds <= 0) {
            Some((
                "commerce.inventory_validation_failed",
                "expires_in_seconds must be greater than zero".to_string(),
            ))
        } else if let Err(error) = stateset_core::validate_quantity(input.quantity) {
            Some(("commerce.inventory_validation_failed", error.to_string()))
        } else {
            None
        };

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) = replay_or_conflict(
                    tx,
                    command,
                    &request_hash,
                    existing,
                    "inventory_reservation",
                )? {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &static_guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "inventory_reservation",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let item_id = match tx.query_row(
                "SELECT id FROM inventory_items WHERE sku = ?",
                [&input.sku],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(item_id) => item_id,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.inventory_item_not_found",
                        "inventory item does not exist",
                        RetryDisposition::Never,
                        "inventory_reservation",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            let location_id = input.location_id.unwrap_or(1);
            let balance = tx.query_row(
                "SELECT quantity_available, version FROM inventory_balances
                 WHERE item_id = ? AND location_id = ?",
                params![item_id, location_id],
                |row| {
                    let available: String = row.get(0)?;
                    Ok((
                        parse_decimal_row(&available, "inventory_balance", "quantity_available")?,
                        row.get::<_, i32>(1)?,
                    ))
                },
            );
            let (mut effective_available, version_before) = match balance {
                Ok(balance) => balance,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.inventory_balance_not_found",
                        "inventory balance does not exist at the requested location",
                        RetryDisposition::Never,
                        "inventory_reservation",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };

            let mut expired_count = 0_i32;
            let mut statement = tx.prepare(
                "SELECT quantity FROM inventory_reservations
                 WHERE item_id = ? AND location_id = ?
                   AND status IN ('pending', 'confirmed', 'allocated')
                   AND expires_at IS NOT NULL AND expires_at < ?",
            )?;
            let expired = statement.query_map(
                params![item_id, location_id, started_at.to_rfc3339()],
                |row| {
                    let quantity: String = row.get(0)?;
                    parse_decimal_row(&quantity, "inventory_reservation", "quantity")
                },
            )?;
            for quantity in expired {
                effective_available += quantity?;
                expired_count += 1;
            }
            drop(statement);

            if command.expected_version.is_some_and(|expected| expected != version_before) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "kernel.version_conflict",
                    "inventory balance version does not match expected_version",
                    RetryDisposition::AfterConflict,
                    "inventory_reservation",
                );
                receipt.version_before = Some(version_before);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if effective_available < input.quantity {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.insufficient_stock",
                    &format!("requested {}, available {}", input.quantity, effective_available),
                    RetryDisposition::Never,
                    "inventory_reservation",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "inventory_reservation");
                receipt.version_before = Some(version_before);
                receipt.version_after = Some(version_before + expired_count + 1);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let (reservation, event_id) = SqliteInventoryRepository::reserve_in_tx(tx, input)?;
            tx.execute(
                "UPDATE kernel_outbox SET command_id = ?, idempotency_key = ?,
                    principal_type = ?, principal_id = ?, correlation_id = ?, causation_id = ?
                 WHERE id = ?",
                params![
                    command.command_id.to_string(),
                    command.idempotency_key,
                    principal_kind_name(command),
                    command.principal.id,
                    command.correlation_id.map(|id| id.to_string()),
                    command.causation_id.map(|id| id.to_string()),
                    event_id.to_string(),
                ],
            )?;
            let version_after: i32 = tx.query_row(
                "SELECT version FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                params![item_id, location_id],
                |row| row.get(0),
            )?;
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(reservation.clone()),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("inventory_reservation".into()),
                aggregate_id: Some(reservation.id.to_string()),
                version_before: Some(version_before),
                version_after: Some(version_after),
                event_ids: vec![event_id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically confirm all or part of a reservation.
    pub fn execute_confirm_inventory_reservation(
        &self,
        command: &CommandEnvelope<ConfirmInventoryReservation>,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        self.execute_inventory_lifecycle(
            command,
            command.payload.reservation_id,
            CONFIRM_RESERVATION_COMMAND,
            InventoryLifecycleAction::Confirm(command.payload.quantity),
        )
    }

    /// Preview or atomically release a reservation.
    pub fn execute_release_inventory_reservation(
        &self,
        command: &CommandEnvelope<ReleaseInventoryReservation>,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        self.execute_inventory_lifecycle(
            command,
            command.payload.reservation_id,
            RELEASE_RESERVATION_COMMAND,
            InventoryLifecycleAction::Release,
        )
    }

    fn execute_inventory_lifecycle<C: Serialize>(
        &self,
        command: &CommandEnvelope<C>,
        reservation_id: Uuid,
        expected_command_type: &str,
        action: InventoryLifecycleAction,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        command
            .validate_contract()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let static_guard = if command.command_type != expected_command_type {
            Some((
                "kernel.command_type_mismatch",
                format!("expected {expected_command_type} command type"),
            ))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some((
                "kernel.deadline_exceeded",
                "command deadline elapsed before execution".to_string(),
            ))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if reservation_id.is_nil() {
            Some((
                "commerce.inventory_validation_failed",
                "reservation_id must not be nil".to_string(),
            ))
        } else if let InventoryLifecycleAction::Confirm(Some(quantity)) = action {
            if quantity <= rust_decimal::Decimal::ZERO {
                Some((
                    "commerce.inventory_validation_failed",
                    "confirmation quantity must be greater than zero".to_string(),
                ))
            } else {
                None
            }
        } else {
            None
        };

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) = replay_or_conflict(
                    tx,
                    command,
                    &request_hash,
                    existing,
                    "inventory_reservation",
                )? {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &static_guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "inventory_reservation",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let reservation = match load_inventory_reservation_tx(tx, reservation_id)? {
                Some(reservation) => reservation,
                None => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.reservation_not_found",
                        "inventory reservation does not exist",
                        RetryDisposition::Never,
                        "inventory_reservation",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
            };
            let version_before: i32 = tx.query_row(
                "SELECT version FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                params![reservation.item_id, reservation.location_id],
                |row| row.get(0),
            )?;
            if command.expected_version.is_some_and(|expected| expected != version_before) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "kernel.version_conflict",
                    "inventory balance version does not match expected_version",
                    RetryDisposition::AfterConflict,
                    "inventory_reservation",
                );
                receipt.version_before = Some(version_before);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if matches!(action, InventoryLifecycleAction::Confirm(_))
                && matches!(
                    reservation.status,
                    ReservationStatus::Released | ReservationStatus::Cancelled
                )
            {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.reservation_not_confirmable",
                    "released or cancelled reservations cannot be confirmed",
                    RetryDisposition::Never,
                    "inventory_reservation",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if matches!(action, InventoryLifecycleAction::Confirm(Some(_)))
                && reservation.status == ReservationStatus::Confirmed
            {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.reservation_not_confirmable",
                    "an already-confirmed reservation cannot be partially confirmed",
                    RetryDisposition::Never,
                    "inventory_reservation",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let expires_during_apply =
                reservation.expires_at.is_some_and(|expiry| expiry < started_at)
                    && !matches!(
                        reservation.status,
                        ReservationStatus::Released
                            | ReservationStatus::Cancelled
                            | ReservationStatus::Expired
                    );
            let releases_balance = matches!(action, InventoryLifecycleAction::Release)
                && !matches!(
                    reservation.status,
                    ReservationStatus::Released
                        | ReservationStatus::Cancelled
                        | ReservationStatus::Expired
                );
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "inventory_reservation");
                receipt.result = Some(reservation);
                receipt.version_before = Some(version_before);
                receipt.version_after =
                    Some(version_before + i32::from(expires_during_apply || releases_balance));
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            match action {
                InventoryLifecycleAction::Confirm(Some(quantity)) => {
                    SqliteInventoryRepository::confirm_reservation_quantity_in_tx_with_now(
                        tx,
                        reservation_id,
                        quantity,
                        started_at,
                    )?;
                }
                InventoryLifecycleAction::Confirm(None) => {
                    SqliteInventoryRepository::confirm_reservation_in_tx_with_now(
                        tx,
                        reservation_id,
                        started_at,
                    )?;
                }
                InventoryLifecycleAction::Release => {
                    SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
                }
            }

            let event_id =
                find_inventory_lifecycle_event_tx(tx, reservation_id, started_at, action)?;
            if let Some(event_id) = event_id {
                tx.execute(
                    "UPDATE kernel_outbox SET command_id = ?, idempotency_key = ?,
                        principal_type = ?, principal_id = ?, correlation_id = ?, causation_id = ?
                     WHERE id = ?",
                    params![
                        command.command_id.to_string(),
                        command.idempotency_key,
                        principal_kind_name(command),
                        command.principal.id,
                        command.correlation_id.map(|id| id.to_string()),
                        command.causation_id.map(|id| id.to_string()),
                        event_id.to_string(),
                    ],
                )?;
            }
            let result_id = event_id
                .and_then(|event_id| {
                    tx.query_row(
                        "SELECT aggregate_id FROM kernel_outbox WHERE id = ?",
                        [event_id.to_string()],
                        |row| row.get::<_, String>(0),
                    )
                    .ok()
                })
                .and_then(|id| Uuid::parse_str(&id).ok())
                .unwrap_or(reservation_id);
            let result = load_inventory_reservation_tx(tx, result_id)?.ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ReservationNotFound(result_id),
                ))
            })?;
            let version_after: i32 = tx.query_row(
                "SELECT version FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                params![reservation.item_id, reservation.location_id],
                |row| row.get(0),
            )?;
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(result.clone()),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("inventory_reservation".into()),
                aggregate_id: Some(result.id.to_string()),
                version_before: Some(version_before),
                version_after: Some(version_after),
                event_ids: event_id.into_iter().collect(),
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically apply an order state-machine transition.
    ///
    /// Cancellations honour the same money rule as
    /// `OrderRepository::update`: captured money must be refunded (or
    /// `void_payments` set to void in-flight payments) before the order can
    /// be cancelled, and every inventory hold is released atomically.
    pub fn execute_transition_order(
        &self,
        command: &CommandEnvelope<TransitionOrder>,
    ) -> Result<ExecutionReceipt<Order>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::aggregate(TRANSITION_ORDER_COMMAND),
            "order",
        )?
        .then_guard(|_| transition_order_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let started_at = run.started_at;
        let order_id = command.payload.order_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)?
                && let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "order")?
            {
                return Ok(stored);
            }
            if let Some(mut receipt) = run.guard_receipt() {
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let snapshot = tx
                .query_row(
                    "SELECT * FROM orders WHERE id = ?",
                    [&order_id],
                    SqliteOrderRepository::row_to_order,
                )
                .optional()?
                .map(|mut order| {
                    order.items = SqliteOrderRepository::load_order_items_with_conn(tx, order.id)
                        .map_err(to_sql_err)?;
                    let open_captures = if command.payload.status == OrderStatus::Cancelled {
                        open_captures_for_order_conn(tx, &order_id)?
                    } else {
                        Vec::new()
                    };
                    Ok::<_, rusqlite::Error>(OrderTransitionSnapshot { order, open_captures })
                })
                .transpose()?;
            let effects = match plan_order_transition(command, snapshot.as_ref()) {
                PlanOutcome::Reject { rejection, version_before, aggregate_id } => {
                    let mut receipt = run.rejected_by(&rejection);
                    receipt.version_before = version_before;
                    receipt.aggregate_id = aggregate_id;
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                PlanOutcome::Proceed(effects) => effects,
            };
            let Some(OrderTransitionSnapshot { order, .. }) = snapshot else {
                return Err(to_sql_err(CommerceError::Internal(
                    "order transition planned without a loaded order".into(),
                )));
            };
            let version_before = effects.version_before;
            if run.is_preview() {
                let mut receipt = run.previewed();
                receipt.aggregate_id = Some(order_id.clone());
                receipt.result = Some(order);
                receipt.version_before = Some(version_before);
                receipt.version_after = Some(version_before + 1);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let rows = tx.execute(
                "UPDATE orders SET status = ?, payment_status = ?, updated_at = ?,
                        version = version + 1 WHERE id = ? AND version = ?",
                params![
                    effects.next_status.to_string(),
                    effects.next_payment_status.to_string(),
                    started_at.to_rfc3339(),
                    order_id,
                    version_before,
                ],
            )?;
            if rows == 0 {
                return Err(to_sql_err(CommerceError::VersionConflict {
                    entity: "order".into(),
                    id: order_id.clone(),
                    expected_version: version_before,
                }));
            }
            let mut related_event_ids = Vec::new();
            let mut voided_payment_ids = Vec::new();
            if effects.void_in_flight_payments {
                voided_payment_ids =
                    void_in_flight_payments_for_order_conn(tx, &order_id, started_at)?;
            }
            if effects.release_holds {
                let reservation_ids =
                    SqliteInventoryRepository::list_reservation_ids_by_reference_in_tx(
                        tx, "order", &order_id,
                    )?;
                for reservation_id in reservation_ids {
                    SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
                    let event_id = tx.query_row(
                        "SELECT id FROM kernel_outbox
                         WHERE event_type = 'inventory.reservation_released.v1'
                           AND aggregate_id = ? AND created_at >= ?
                         ORDER BY rowid DESC LIMIT 1",
                        params![reservation_id.to_string(), started_at.to_rfc3339()],
                        |row| parse_uuid_row(&row.get::<_, String>(0)?, "kernel_outbox", "id"),
                    );
                    if let Ok(event_id) = event_id {
                        tx.execute(
                            "UPDATE kernel_outbox SET command_id = ?, idempotency_key = ?,
                                principal_type = ?, principal_id = ?, correlation_id = ?, causation_id = ?
                             WHERE id = ?",
                            params![
                                command.command_id.to_string(), command.idempotency_key,
                                principal_kind_name(command), command.principal.id,
                                command.correlation_id.map(|id| id.to_string()),
                                command.causation_id.map(|id| id.to_string()), event_id.to_string(),
                            ],
                        )?;
                        related_event_ids.push(event_id);
                    }
                }
                cancel_backorders_for_order_in_tx(tx, command.payload.order_id.into_uuid())?;
            }
            let outstanding_payment_ids: Vec<String> = effects
                .outstanding_capture_ids
                .iter()
                .filter(|id| !voided_payment_ids.contains(id))
                .map(ToString::to_string)
                .collect();

            let event = run.event(
                "orders.updated.v1",
                "order",
                order_id.clone(),
                serde_json::json!({
                    "order_id": order_id,
                    "status_before": effects.status_before.to_string(),
                    "status_after": effects.next_status.to_string(),
                    "payment_status_before": effects.payment_status_before.to_string(),
                    "payment_status_after": effects.next_payment_status.to_string(),
                    "fulfillment_status_after": order.fulfillment_status.to_string(),
                    "version_before": version_before,
                    "version_after": version_before + 1,
                    "total_amount": order.total_amount.to_string(),
                    "void_payments": command.payload.void_payments,
                    "voided_payment_ids": voided_payment_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "outstanding_payment_ids": outstanding_payment_ids,
                }),
            );
            append_kernel_event_tx(tx, &event)?;
            related_event_ids.push(event.id);

            let mut order = tx.query_row(
                "SELECT * FROM orders WHERE id = ?",
                [&order_id],
                SqliteOrderRepository::row_to_order,
            )?;
            order.items = SqliteOrderRepository::load_order_items_with_conn(tx, order.id)
                .map_err(to_sql_err)?;
            let version_after = order.version;
            let mut receipt = run.succeeded(
                order,
                Some(order_id.clone()),
                Some(version_before),
                Some(version_after),
                related_event_ids,
            );
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically ship all or selected order-line quantities.
    ///
    /// A reservation that expires while it is being confirmed rolls the
    /// shipment back to its savepoint and seals a
    /// `commerce.reservation_expired` rejection instead of failing the call.
    pub fn execute_ship_order(
        &self,
        command: &CommandEnvelope<ShipOrderCommand>,
    ) -> Result<ExecutionReceipt<Order>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::aggregate(SHIP_ORDER_COMMAND),
            "order",
        )?
        .then_guard(|_| ship_order_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let started_at = run.started_at;
        let order_id = command.payload.order_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)?
                && let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "order")?
            {
                return Ok(stored);
            }
            if let Some(mut receipt) = run.guard_receipt() {
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let lines = command.payload.lines.as_deref().unwrap_or_default();
            let mode = if lines.is_empty() { ShipMode::All } else { ShipMode::Lines(lines) };
            let snapshot = tx
                .query_row(
                    "SELECT * FROM orders WHERE id = ?",
                    [&order_id],
                    SqliteOrderRepository::row_to_order,
                )
                .optional()?
                .map(|mut order| {
                    order.items = SqliteOrderRepository::load_order_items_with_conn(tx, order.id)
                        .map_err(to_sql_err)?;
                    let shipment = SqliteOrderRepository::plan_shipment_in_tx(
                        tx,
                        command.payload.order_id,
                        &mode,
                    )
                    .map_err(|error| error.to_string());
                    let expired_reservation = tx
                        .query_row(
                            "SELECT id FROM inventory_reservations
                             WHERE reference_type = 'order' AND reference_id = ?
                               AND status IN ('pending', 'confirmed', 'allocated')
                               AND expires_at IS NOT NULL AND expires_at < ? LIMIT 1",
                            params![order_id, started_at.to_rfc3339()],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()?
                        .is_some();
                    Ok::<_, rusqlite::Error>(ShipOrderSnapshot {
                        order,
                        shipment,
                        expired_reservation,
                    })
                })
                .transpose()?;
            let effects = match plan_ship_order(command, snapshot.as_ref()) {
                PlanOutcome::Reject { rejection, version_before, aggregate_id } => {
                    let mut receipt = run.rejected_by(&rejection);
                    receipt.version_before = version_before;
                    receipt.aggregate_id = aggregate_id;
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                PlanOutcome::Proceed(effects) => effects,
            };
            let Some(ShipOrderSnapshot { order, .. }) = snapshot else {
                return Err(to_sql_err(CommerceError::Internal(
                    "shipment planned without a loaded order".into(),
                )));
            };
            let version_before = effects.version_before;
            if run.is_preview() {
                let mut receipt = run.previewed();
                receipt.aggregate_id = Some(order_id.clone());
                receipt.result = Some(order);
                receipt.version_before = Some(version_before);
                receipt.version_after = Some(version_before + 1);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let reservation_ids =
                SqliteInventoryRepository::list_reservation_ids_by_reference_in_tx(
                    tx, "order", &order_id,
                )?;
            tx.execute_batch("SAVEPOINT kernel_ship")?;
            if SqliteOrderRepository::confirm_shipped_reservations_in_tx(
                tx,
                command.payload.order_id,
                &mode,
                &effects.deltas,
                started_at,
            )?
            .is_some()
            {
                tx.execute_batch("ROLLBACK TO kernel_ship; RELEASE kernel_ship")?;
                let mut receipt = run.rejected_by(&reservation_expired_during_shipment());
                receipt.aggregate_id = Some(order_id.clone());
                receipt.version_before = Some(version_before);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            tx.execute_batch("RELEASE kernel_ship")?;
            for delta in effects.deltas.iter().filter(|d| d.delta > 0) {
                tx.execute(
                    "UPDATE order_items SET shipped_quantity = shipped_quantity + ? WHERE id = ?",
                    params![delta.delta, delta.item_id],
                )?;
            }
            let rows = tx.execute(
                "UPDATE orders SET status = ?, tracking_number = COALESCE(?, tracking_number),
                        updated_at = ?, version = version + 1 WHERE id = ? AND version = ?",
                params![
                    effects.resolved_status.to_string(),
                    command.payload.tracking_number,
                    started_at.to_rfc3339(),
                    order_id,
                    version_before
                ],
            )?;
            if rows == 0 {
                return Err(to_sql_err(CommerceError::VersionConflict {
                    entity: "order".into(),
                    id: order_id.clone(),
                    expected_version: version_before,
                }));
            }
            let mut event_ids = Vec::new();
            for reservation_id in reservation_ids {
                let mut stmt = tx.prepare(
                    "SELECT id FROM kernel_outbox WHERE created_at >= ?
                       AND event_type = 'inventory.reservation_confirmed.v1'
                       AND (aggregate_id = ? OR json_extract(payload, '$.source_reservation_id') = ?)
                     ORDER BY rowid",
                )?;
                let ids = stmt.query_map(
                    params![
                        started_at.to_rfc3339(),
                        reservation_id.to_string(),
                        reservation_id.to_string()
                    ],
                    |row| parse_uuid_row(&row.get::<_, String>(0)?, "kernel_outbox", "id"),
                )?;
                for event_id in ids {
                    let event_id = event_id?;
                    tx.execute("UPDATE kernel_outbox SET command_id = ?, idempotency_key = ?, principal_type = ?, principal_id = ?, correlation_id = ?, causation_id = ? WHERE id = ?",
                        params![command.command_id.to_string(), command.idempotency_key, principal_kind_name(command), command.principal.id,
                            command.correlation_id.map(|id| id.to_string()), command.causation_id.map(|id| id.to_string()), event_id.to_string()])?;
                    if !event_ids.contains(&event_id) {
                        event_ids.push(event_id);
                    }
                }
            }
            let event = run.event(
                "orders.updated.v1",
                "order",
                order_id.clone(),
                serde_json::json!({
                    "order_id": order_id, "status_before": effects.status_before.to_string(),
                    "status_after": effects.resolved_status.to_string(), "payment_status_before": order.payment_status.to_string(),
                    "payment_status_after": order.payment_status.to_string(), "fulfillment_status_after": order.fulfillment_status.to_string(),
                    "version_before": version_before, "version_after": version_before + 1, "total_amount": order.total_amount.to_string(),
                }),
            );
            append_kernel_event_tx(tx, &event)?;
            event_ids.push(event.id);
            let mut order = tx.query_row(
                "SELECT * FROM orders WHERE id = ?",
                [&order_id],
                SqliteOrderRepository::row_to_order,
            )?;
            order.items = SqliteOrderRepository::load_order_items_with_conn(tx, order.id)
                .map_err(to_sql_err)?;
            let version_after = order.version;
            let mut receipt = run.succeeded(
                order,
                Some(order_id.clone()),
                Some(version_before),
                Some(version_after),
                event_ids,
            );
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically apply a return state-machine transition.
    pub fn execute_transition_return(
        &self,
        command: &CommandEnvelope<TransitionReturn>,
    ) -> Result<ExecutionReceipt<Return>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != TRANSITION_RETURN_COMMAND {
            Some((
                "kernel.command_type_mismatch",
                "expected returns.transition command type".to_string(),
            ))
        } else if command.deadline.is_some_and(|d| d <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.payload.return_id.into_uuid().is_nil() {
            Some(("commerce.return_validation_failed", "return_id must not be nil".into()))
        } else {
            None
        };
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "return")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "return",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut returned = match tx.query_row(
                "SELECT * FROM returns WHERE id = ?",
                [command.payload.return_id.to_string()],
                SqliteReturnRepository::row_to_return,
            ) {
                Ok(value) => value,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.return_not_found",
                        "return does not exist",
                        RetryDisposition::Never,
                        "return",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(e) => return Err(e),
            };
            let mut stmt = tx.prepare(
                "SELECT id, return_id, order_item_id, sku, name, quantity, condition,
                        refund_amount, disposition, disposition_at, disposition_by,
                        lot_id, serial_ids
                 FROM return_items WHERE return_id = ? ORDER BY rowid",
            )?;
            returned.items = stmt
                .query_map([command.payload.return_id.to_string()], row_to_return_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);
            let version_before = returned.version;
            if command.expected_version.is_some_and(|v| v != version_before) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "kernel.version_conflict",
                    "return version does not match expected_version",
                    RetryDisposition::AfterConflict,
                    "return",
                );
                receipt.version_before = Some(version_before);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if !returned.status.can_transition_to(command.payload.status) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.invalid_return_status_transition",
                    &format!(
                        "return cannot transition from {} to {}",
                        returned.status, command.payload.status
                    ),
                    RetryDisposition::Never,
                    "return",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "return");
                receipt.result = Some(returned);
                receipt.version_before = Some(version_before);
                receipt.version_after = Some(version_before + 1);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let changed = tx.execute(
                "UPDATE returns SET status = ?, updated_at = ?, version = version + 1
                 WHERE id = ? AND version = ?",
                params![
                    command.payload.status.to_string(),
                    started_at.to_rfc3339(),
                    command.payload.return_id.to_string(),
                    version_before
                ],
            )?;
            if changed == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::VersionConflict {
                        entity: "return".into(),
                        id: command.payload.return_id.to_string(),
                        expected_version: version_before,
                    },
                )));
            }
            let mut event = KernelOutboxEvent::domain(
                "returns.updated.v1",
                "return",
                command.payload.return_id.to_string(),
                serde_json::json!({"return_id": command.payload.return_id.to_string(),
                    "status_before": returned.status.to_string(), "status_after": command.payload.status.to_string(),
                    "version_before": version_before, "version_after": version_before + 1,
                    "refund_amount": returned.refund_amount.map(|amount| amount.to_string())}),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            returned = tx.query_row(
                "SELECT * FROM returns WHERE id = ?",
                [command.payload.return_id.to_string()],
                SqliteReturnRepository::row_to_return,
            )?;
            let mut stmt = tx.prepare(
                "SELECT id, return_id, order_item_id, sku, name, quantity, condition,
                        refund_amount, disposition, disposition_at, disposition_by,
                        lot_id, serial_ids
                 FROM return_items WHERE return_id = ? ORDER BY rowid",
            )?;
            returned.items = stmt
                .query_map([command.payload.return_id.to_string()], row_to_return_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(returned.clone()),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("return".into()),
                aggregate_id: Some(returned.id.to_string()),
                version_before: Some(version_before),
                version_after: Some(returned.version),
                event_ids: vec![event.id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically create an A2A escrow in `created` status.
    pub fn execute_create_a2a_escrow(
        &self,
        command: &CommandEnvelope<CreateA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        let input = &command.payload;
        let run = CommandRun::prepare(
            command,
            input,
            &self.policy,
            EnvelopeGuard::create(CREATE_A2A_ESCROW_COMMAND),
            "a2a_escrow",
        )?
        .then_guard(|run| create_escrow_guard(input, run.started_at));
        let request_hash = run.request_hash.clone();
        let started_at = run.started_at;

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)?
                && let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_escrow")?
            {
                return Ok(stored);
            }
            if let Some(mut receipt) = run.guard_receipt() {
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if run.is_preview() {
                let mut receipt = run.previewed();
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let id = Uuid::new_v4().to_string();
            let created = A2AEscrow {
                id: id.clone(),
                tenant_id: command.principal.tenant_id.clone().expect("policy validated tenant"),
                store_id: command.store_id.clone().expect("policy validated store"),
                status: A2AEscrowStatus::Created,
                quote_id: input.quote_id.clone(),
                payment_id: input.payment_id.clone(),
                buyer_address: input.buyer_address.clone(),
                seller_address: input.seller_address.clone(),
                amount: escrow_legacy_amount(input).expect("validated legacy amount"),
                amount_decimal: input.amount,
                asset: input.asset.to_uppercase(),
                network: input.network.clone(),
                release_conditions: input.release_conditions.clone(),
                funded_at: None,
                released_at: None,
                disputed_at: None,
                dispute_id: None,
                expires_at: input.expires_at,
                auto_release_after: input.auto_release_after,
                metadata: input.metadata.clone(),
                created_at: started_at,
                updated_at: started_at,
            };
            let release_conditions = serde_json::to_string(&created.release_conditions)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let metadata = created
                .metadata
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            tx.execute(
                "INSERT INTO a2a_escrows (
                    id, status, quote_id, payment_id, buyer_address, seller_address,
                    amount, amount_decimal, asset, network, release_conditions,
                    funded_at, released_at, disputed_at, dispute_id, expires_at,
                    auto_release_after, metadata, created_at, updated_at, tenant_id, store_id
                 ) VALUES (?, 'created', ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    &created.id,
                    &created.quote_id,
                    &created.payment_id,
                    &created.buyer_address,
                    &created.seller_address,
                    created.amount,
                    created.amount_decimal.to_string(),
                    &created.asset,
                    &created.network,
                    release_conditions,
                    created.expires_at.to_rfc3339(),
                    created.auto_release_after.map(|value| value.to_rfc3339()),
                    metadata,
                    created.created_at.to_rfc3339(),
                    created.updated_at.to_rfc3339(),
                    &created.tenant_id,
                    &created.store_id,
                ],
            )?;
            let event = run.event(
                "a2a.escrow_created.v1",
                "a2a_escrow",
                id.clone(),
                serde_json::json!({
                    "escrow_id": &created.id,
                    "quote_id": &created.quote_id,
                    "payment_id": &created.payment_id,
                    "buyer_address": &created.buyer_address,
                    "seller_address": &created.seller_address,
                    "amount_decimal": created.amount_decimal.to_string(),
                    "asset": &created.asset,
                    "network": &created.network,
                    "status": "created",
                }),
            );
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = run.succeeded(created, Some(id), None, None, vec![event.id]);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically move a created A2A escrow into active custody.
    pub fn execute_fund_a2a_escrow(
        &self,
        command: &CommandEnvelope<FundA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(FUND_A2A_ESCROW_COMMAND, ESCROW_UNVERSIONED),
            "a2a_escrow",
        )?
        .then_guard(|_| escrow_id_guard(&command.payload.escrow_id));
        let request_hash = run.request_hash.clone();
        let started_at = run.started_at;
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)?
                && let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_escrow")?
            {
                return Ok(stored);
            }
            if let Some(mut receipt) = run.guard_receipt() {
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let loaded = load_a2a_escrow_sqlite(
                tx,
                &command.payload.escrow_id,
                command.principal.tenant_id.as_deref().expect("policy validated tenant"),
                command.store_id.as_deref().expect("policy validated store"),
            )
            .optional()?;
            let escrow = match plan_fund_escrow(loaded, started_at) {
                PlanOutcome::Reject { rejection, aggregate_id, .. } => {
                    let mut receipt = run.rejected_by(&rejection);
                    receipt.aggregate_id = aggregate_id;
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                PlanOutcome::Proceed(escrow) => escrow,
            };
            if run.is_preview() {
                let mut receipt = run.previewed();
                receipt.aggregate_id = Some(escrow.id.clone());
                receipt.result = Some(escrow);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if tx.execute(
                "UPDATE a2a_escrows SET status = 'active', funded_at = ?, updated_at = ?
                 WHERE id = ? AND status = 'created'",
                params![started_at.to_rfc3339(), started_at.to_rfc3339(), &escrow.id],
            )? == 0
            {
                return Err(to_sql_err(CommerceError::Conflict(
                    "A2A escrow was modified concurrently".into(),
                )));
            }
            let mut event =
                a2a_transition_event(command, &escrow, "a2a.escrow_funded.v1", "active", None);
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let aggregate_id = escrow.id.clone();
            let mut receipt = run.succeeded(escrow, Some(aggregate_id), None, None, vec![event.id]);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically freeze an active escrow for dispute resolution.
    pub fn execute_dispute_a2a_escrow(
        &self,
        command: &CommandEnvelope<DisputeA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let mut guard = a2a_transition_guard(
            command,
            &policy,
            started_at,
            DISPUTE_A2A_ESCROW_COMMAND,
            "a2a.escrow.dispute",
            &command.payload.escrow_id,
        );
        if guard.is_none() && command.payload.reason.trim().is_empty() {
            guard = Some((
                "commerce.a2a.escrow.validation_failed",
                "dispute reason is required".into(),
            ));
        }
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_escrow")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut escrow = match load_a2a_escrow_sqlite(
                tx,
                &command.payload.escrow_id,
                command.principal.tenant_id.as_deref().expect("policy validated tenant"),
                command.store_id.as_deref().expect("policy validated store"),
            ) {
                Ok(escrow) => escrow,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.a2a.escrow_not_found",
                        "A2A escrow does not exist",
                        RetryDisposition::Never,
                        "a2a_escrow",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            if !matches!(escrow.status, A2AEscrowStatus::Funded | A2AEscrowStatus::Active) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.escrow_not_disputable",
                    &format!("cannot dispute escrow in {} status", escrow.status),
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            escrow.status = A2AEscrowStatus::Disputed;
            escrow.disputed_at = Some(started_at);
            escrow.updated_at = started_at;
            let mut metadata = escrow
                .metadata
                .take()
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
            metadata.insert(
                "dispute".into(),
                serde_json::json!({
                    "reason": command.payload.reason,
                    "category": command.payload.category,
                    "disputed_at": started_at,
                    "principal_id": command.principal.id,
                }),
            );
            escrow.metadata = Some(serde_json::Value::Object(metadata));
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "a2a_escrow");
                receipt.aggregate_id = Some(escrow.id.clone());
                receipt.result = Some(escrow);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let metadata = serde_json::to_string(&escrow.metadata)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            if tx.execute(
                "UPDATE a2a_escrows
                 SET status = 'disputed', disputed_at = ?, metadata = ?, updated_at = ?
                 WHERE id = ? AND status IN ('funded', 'active')",
                params![started_at.to_rfc3339(), metadata, started_at.to_rfc3339(), &escrow.id],
            )? == 0
            {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("A2A escrow was modified concurrently".into()),
                )));
            }
            let mut event = a2a_transition_event(
                command,
                &escrow,
                "a2a.escrow_disputed.v1",
                "disputed",
                Some(&command.payload.reason),
            );
            event.payload["category"] = serde_json::json!(command.payload.category);
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt =
                succeeded_a2a_receipt(command, policy.clone(), escrow, event.id, started_at);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically file a tenant-scoped dispute and freeze its escrow.
    pub fn execute_file_a2a_dispute(
        &self,
        command: &CommandEnvelope<FileA2ADispute>,
    ) -> Result<ExecutionReceipt<A2ADispute>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let input = &command.payload;
        let mut guard = a2a_transition_guard(
            command,
            &policy,
            started_at,
            FILE_A2A_DISPUTE_COMMAND,
            "a2a.dispute.file",
            &input.escrow_id,
        );
        if guard.is_none()
            && (input.reason.trim().is_empty()
                || input.category.trim().is_empty()
                || input.claimant_address.trim().is_empty())
        {
            guard = Some((
                "commerce.a2a.dispute.validation_failed",
                "claimant_address, reason, and category are required".into(),
            ));
        }
        if guard.is_none()
            && (input.evidence_deadline <= started_at
                || input.review_deadline <= input.evidence_deadline)
        {
            guard = Some((
                "commerce.a2a.dispute.invalid_deadlines",
                "evidence_deadline must be in the future and precede review_deadline".into(),
            ));
        }
        if guard.is_none() && !principal_controls_address(command, &input.claimant_address) {
            guard = Some((
                "kernel.actor_mismatch",
                "principal or delegator must control the claimant address".into(),
            ));
        }

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_dispute")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "a2a_dispute",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let tenant_id =
                command.principal.tenant_id.as_deref().expect("policy validated tenant");
            let store_id = command.store_id.as_deref().expect("policy validated store");
            let escrow = match load_a2a_escrow_sqlite(tx, &input.escrow_id, tenant_id, store_id) {
                Ok(value) => value,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.a2a.escrow_not_found",
                        "A2A escrow does not exist in the command scope",
                        RetryDisposition::Never,
                        "a2a_dispute",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            if !matches!(escrow.status, A2AEscrowStatus::Funded | A2AEscrowStatus::Active) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.escrow_not_disputable",
                    &format!("cannot file dispute for escrow in {} status", escrow.status),
                    RetryDisposition::Never,
                    "a2a_dispute",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let respondent_address = if input.claimant_address == escrow.buyer_address {
                escrow.seller_address.clone()
            } else if input.claimant_address == escrow.seller_address {
                escrow.buyer_address.clone()
            } else {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.dispute.claimant_not_participant",
                    "claimant must be the escrow buyer or seller",
                    RetryDisposition::Never,
                    "a2a_dispute",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            };
            let dispute = A2ADispute {
                id: format!("dsp_{}", &request_hash[..32]),
                tenant_id: tenant_id.into(),
                store_id: store_id.into(),
                status: A2ADisputeStatus::Filed,
                escrow_id: escrow.id.clone(),
                quote_id: escrow.quote_id.clone(),
                claimant_address: input.claimant_address.clone(),
                respondent_address,
                reason: input.reason.trim().into(),
                category: input.category.trim().into(),
                amount: escrow.amount_decimal,
                asset: escrow.asset.clone(),
                resolution_type: None,
                buyer_amount: None,
                seller_amount: None,
                resolution_note: None,
                resolved_by: None,
                evidence_deadline: input.evidence_deadline,
                review_deadline: input.review_deadline,
                metadata: input.metadata.clone(),
                created_at: started_at,
                updated_at: started_at,
                resolved_at: None,
            };
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "a2a_dispute");
                receipt.aggregate_id = Some(dispute.id.clone());
                receipt.result = Some(dispute);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let metadata = dispute
                .metadata
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            tx.execute(
                "INSERT INTO a2a_disputes (
                    id, tenant_id, store_id, status, escrow_id, quote_id,
                    claimant_address, respondent_address, reason, category,
                    amount_decimal, asset, evidence_deadline, review_deadline,
                    metadata, created_at, updated_at
                 ) VALUES (?, ?, ?, 'filed', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    &dispute.id,
                    &dispute.tenant_id,
                    &dispute.store_id,
                    &dispute.escrow_id,
                    &dispute.quote_id,
                    &dispute.claimant_address,
                    &dispute.respondent_address,
                    &dispute.reason,
                    &dispute.category,
                    dispute.amount.to_string(),
                    &dispute.asset,
                    dispute.evidence_deadline.to_rfc3339(),
                    dispute.review_deadline.to_rfc3339(),
                    metadata,
                    dispute.created_at.to_rfc3339(),
                    dispute.updated_at.to_rfc3339(),
                ],
            )?;
            if tx.execute(
                "UPDATE a2a_escrows
                 SET status = 'disputed', disputed_at = ?, dispute_id = ?, updated_at = ?
                 WHERE id = ? AND tenant_id = ? AND store_id = ?
                   AND status IN ('funded', 'active')",
                params![
                    started_at.to_rfc3339(),
                    &dispute.id,
                    started_at.to_rfc3339(),
                    &escrow.id,
                    tenant_id,
                    store_id,
                ],
            )? == 0
            {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("A2A escrow was modified concurrently".into()),
                )));
            }
            let mut event = KernelOutboxEvent::domain(
                "a2a.dispute_filed.v1",
                "a2a_dispute",
                dispute.id.clone(),
                serde_json::json!({
                    "dispute_id": &dispute.id,
                    "escrow_id": &dispute.escrow_id,
                    "claimant_address": &dispute.claimant_address,
                    "respondent_address": &dispute.respondent_address,
                    "category": &dispute.category,
                    "amount_decimal": dispute.amount.to_string(),
                    "asset": &dispute.asset,
                    "status": "filed"
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = succeeded_kernel_receipt(
                command,
                policy.clone(),
                dispute,
                "a2a_dispute",
                event.aggregate_id.clone(),
                vec![event.id],
                started_at,
            );
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically append immutable, content-addressed dispute evidence.
    pub fn execute_submit_a2a_dispute_evidence(
        &self,
        command: &CommandEnvelope<SubmitA2ADisputeEvidence>,
    ) -> Result<ExecutionReceipt<A2ADisputeEvidence>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let input = &command.payload;
        let mut guard = a2a_transition_guard(
            command,
            &policy,
            started_at,
            SUBMIT_A2A_EVIDENCE_COMMAND,
            "a2a.dispute.evidence.submit",
            &input.dispute_id,
        );
        if guard.is_none()
            && (input.submitted_by.trim().is_empty()
                || input.evidence_type.trim().is_empty()
                || input.title.trim().is_empty()
                || input.content.is_empty())
        {
            guard = Some((
                "commerce.a2a.dispute.evidence.validation_failed",
                "submitted_by, evidence_type, title, and content are required".into(),
            ));
        }
        if guard.is_none() && (input.title.len() > 256 || input.content.len() > 1_048_576) {
            guard = Some((
                "commerce.a2a.dispute.evidence.too_large",
                "evidence title is limited to 256 bytes and content to 1 MiB".into(),
            ));
        }
        if guard.is_none() && !principal_controls_address(command, &input.submitted_by) {
            guard = Some((
                "kernel.actor_mismatch",
                "principal or delegator must control the evidence submitter address".into(),
            ));
        }

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) = replay_or_conflict(
                    tx,
                    command,
                    &request_hash,
                    existing,
                    "a2a_dispute_evidence",
                )? {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "a2a_dispute_evidence",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let tenant_id =
                command.principal.tenant_id.as_deref().expect("policy validated tenant");
            let store_id = command.store_id.as_deref().expect("policy validated store");
            let dispute = match load_a2a_dispute_sqlite(tx, &input.dispute_id, tenant_id, store_id)
            {
                Ok(value) => value,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.a2a.dispute_not_found",
                        "A2A dispute does not exist in the command scope",
                        RetryDisposition::Never,
                        "a2a_dispute_evidence",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            if !matches!(
                dispute.status,
                A2ADisputeStatus::Filed
                    | A2ADisputeStatus::EvidencePeriod
                    | A2ADisputeStatus::UnderReview
            ) || started_at > dispute.evidence_deadline
            {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.dispute.evidence_closed",
                    "evidence is closed for this dispute",
                    RetryDisposition::Never,
                    "a2a_dispute_evidence",
                );
                receipt.aggregate_id = Some(dispute.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if input.submitted_by != dispute.claimant_address
                && input.submitted_by != dispute.respondent_address
            {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.dispute.submitter_not_participant",
                    "evidence submitter must be a dispute participant",
                    RetryDisposition::Never,
                    "a2a_dispute_evidence",
                );
                receipt.aggregate_id = Some(dispute.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let evidence = A2ADisputeEvidence {
                id: format!("evd_{}", &request_hash[..32]),
                tenant_id: tenant_id.into(),
                store_id: store_id.into(),
                dispute_id: dispute.id.clone(),
                submitted_by: input.submitted_by.clone(),
                evidence_type: input.evidence_type.trim().into(),
                title: input.title.trim().into(),
                description: input.description.clone(),
                content: input.content.clone(),
                content_hash: format!("sha256:{:x}", Sha256::digest(input.content.as_bytes())),
                created_at: started_at,
            };
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "a2a_dispute_evidence");
                receipt.aggregate_id = Some(evidence.id.clone());
                receipt.result = Some(evidence);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            tx.execute(
                "INSERT INTO a2a_dispute_evidence (
                    id, tenant_id, store_id, dispute_id, submitted_by, evidence_type,
                    title, description, content, content_hash, created_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    &evidence.id,
                    &evidence.tenant_id,
                    &evidence.store_id,
                    &evidence.dispute_id,
                    &evidence.submitted_by,
                    &evidence.evidence_type,
                    &evidence.title,
                    &evidence.description,
                    &evidence.content,
                    &evidence.content_hash,
                    evidence.created_at.to_rfc3339(),
                ],
            )?;
            tx.execute(
                "UPDATE a2a_disputes SET status = CASE WHEN status = 'filed' THEN 'evidence_period' ELSE status END,
                        updated_at = ?
                 WHERE id = ? AND tenant_id = ? AND store_id = ?",
                params![
                    started_at.to_rfc3339(),
                    &dispute.id,
                    tenant_id,
                    store_id,
                ],
            )?;
            let mut event = KernelOutboxEvent::domain(
                "a2a.dispute_evidence_submitted.v1",
                "a2a_dispute",
                dispute.id.clone(),
                serde_json::json!({
                    "dispute_id": &dispute.id,
                    "evidence_id": &evidence.id,
                    "submitted_by": &evidence.submitted_by,
                    "evidence_type": &evidence.evidence_type,
                    "title": &evidence.title,
                    "content_hash": &evidence.content_hash
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = succeeded_kernel_receipt(
                command,
                policy.clone(),
                evidence,
                "a2a_dispute_evidence",
                event.aggregate_id.clone(),
                vec![event.id],
                started_at,
            );
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically resolve a dispute and move its escrow in the same transaction.
    pub fn execute_resolve_a2a_dispute(
        &self,
        command: &CommandEnvelope<ResolveA2ADispute>,
    ) -> Result<ExecutionReceipt<A2ADisputeResolution>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let input = &command.payload;
        let mut guard = a2a_transition_guard(
            command,
            &policy,
            started_at,
            RESOLVE_A2A_DISPUTE_COMMAND,
            "a2a.dispute.resolve",
            &input.dispute_id,
        );
        if guard.is_none() && input.note.as_ref().is_some_and(|note| note.len() > 2_000) {
            guard = Some((
                "commerce.a2a.dispute.resolution_note_too_large",
                "resolution note is limited to 2000 bytes".into(),
            ));
        }
        let is_split = input.resolution_type == A2ADisputeResolutionType::Split;
        if guard.is_none()
            && (is_split != (input.buyer_amount.is_some() && input.seller_amount.is_some()))
        {
            guard = Some((
                "commerce.a2a.dispute.invalid_allocations",
                "split requires both exact allocations; other outcomes forbid allocations".into(),
            ));
        }

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_dispute")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "a2a_dispute",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let tenant_id =
                command.principal.tenant_id.as_deref().expect("policy validated tenant");
            let store_id = command.store_id.as_deref().expect("policy validated store");
            let mut dispute =
                match load_a2a_dispute_sqlite(tx, &input.dispute_id, tenant_id, store_id) {
                    Ok(value) => value,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        let mut receipt = rejected_receipt(
                            command,
                            Some(policy.clone()),
                            "commerce.a2a.dispute_not_found",
                            "A2A dispute does not exist in the command scope",
                            RetryDisposition::Never,
                            "a2a_dispute",
                        );
                        append_receipt(tx, &request_hash, &mut receipt)?;
                        return Ok(receipt);
                    }
                    Err(error) => return Err(error),
                };
            if !matches!(
                dispute.status,
                A2ADisputeStatus::Filed
                    | A2ADisputeStatus::EvidencePeriod
                    | A2ADisputeStatus::UnderReview
                    | A2ADisputeStatus::Escalated
            ) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.dispute_not_resolvable",
                    &format!("cannot resolve dispute in {} status", dispute.status),
                    RetryDisposition::Never,
                    "a2a_dispute",
                );
                receipt.aggregate_id = Some(dispute.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut escrow =
                match load_a2a_escrow_sqlite(tx, &dispute.escrow_id, tenant_id, store_id) {
                    Ok(value) => value,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::DatabaseError(
                                "scoped dispute references a missing escrow".into(),
                            ),
                        )));
                    }
                    Err(error) => return Err(error),
                };
            if escrow.status != A2AEscrowStatus::Disputed
                || escrow.dispute_id.as_deref() != Some(dispute.id.as_str())
            {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.dispute.escrow_state_mismatch",
                    "escrow is not frozen by this dispute",
                    RetryDisposition::Never,
                    "a2a_dispute",
                );
                receipt.aggregate_id = Some(dispute.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let zero = rust_decimal::Decimal::ZERO;
            let (buyer_amount, seller_amount, dispute_status, escrow_status, final_resolution) =
                match input.resolution_type {
                    A2ADisputeResolutionType::FullRefund => (
                        dispute.amount,
                        zero,
                        A2ADisputeStatus::Resolved,
                        A2AEscrowStatus::Refunded,
                        true,
                    ),
                    A2ADisputeResolutionType::ReleaseToSeller => (
                        zero,
                        dispute.amount,
                        A2ADisputeStatus::Resolved,
                        A2AEscrowStatus::Released,
                        true,
                    ),
                    A2ADisputeResolutionType::Split => {
                        let buyer = input.buyer_amount.expect("validated buyer allocation");
                        let seller = input.seller_amount.expect("validated seller allocation");
                        if buyer < zero || seller < zero || buyer + seller != dispute.amount {
                            let mut receipt = rejected_receipt(
                                command,
                                Some(policy.clone()),
                                "commerce.a2a.dispute.allocations_do_not_balance",
                                "buyer and seller allocations must be non-negative and sum exactly to escrow amount",
                                RetryDisposition::Never,
                                "a2a_dispute",
                            );
                            receipt.aggregate_id = Some(dispute.id);
                            append_receipt(tx, &request_hash, &mut receipt)?;
                            return Ok(receipt);
                        }
                        (buyer, seller, A2ADisputeStatus::Resolved, A2AEscrowStatus::Resolved, true)
                    }
                    A2ADisputeResolutionType::Escalated => {
                        (zero, zero, A2ADisputeStatus::Escalated, A2AEscrowStatus::Disputed, false)
                    }
                    _ => {
                        let mut receipt = rejected_receipt(
                            command,
                            Some(policy.clone()),
                            "commerce.a2a.dispute.unsupported_resolution",
                            "resolution type is not supported by this kernel version",
                            RetryDisposition::Never,
                            "a2a_dispute",
                        );
                        receipt.aggregate_id = Some(dispute.id);
                        append_receipt(tx, &request_hash, &mut receipt)?;
                        return Ok(receipt);
                    }
                };
            dispute.status = dispute_status;
            dispute.resolution_type = Some(input.resolution_type);
            dispute.buyer_amount = final_resolution.then_some(buyer_amount);
            dispute.seller_amount = final_resolution.then_some(seller_amount);
            dispute.resolution_note = input.note.clone();
            dispute.resolved_by = Some(command.principal.id.clone());
            dispute.updated_at = started_at;
            dispute.resolved_at = final_resolution.then_some(started_at);
            escrow.status = escrow_status;
            escrow.updated_at = started_at;
            if matches!(escrow_status, A2AEscrowStatus::Released | A2AEscrowStatus::Resolved) {
                escrow.released_at = Some(started_at);
            }
            let result = A2ADisputeResolution { dispute: dispute.clone(), escrow: escrow.clone() };
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "a2a_dispute");
                receipt.aggregate_id = Some(dispute.id);
                receipt.result = Some(result);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            tx.execute(
                "UPDATE a2a_disputes SET status = ?, resolution_type = ?,
                        buyer_amount_decimal = ?, seller_amount_decimal = ?, resolution_note = ?,
                        resolved_by = ?, resolved_at = ?, updated_at = ?
                 WHERE id = ? AND tenant_id = ? AND store_id = ?
                   AND status IN ('filed', 'evidence_period', 'under_review', 'escalated')",
                params![
                    dispute.status.to_string(),
                    dispute.resolution_type.map(|value| value.to_string()),
                    dispute.buyer_amount.map(|value| value.to_string()),
                    dispute.seller_amount.map(|value| value.to_string()),
                    &dispute.resolution_note,
                    &dispute.resolved_by,
                    dispute.resolved_at.map(|value| value.to_rfc3339()),
                    started_at.to_rfc3339(),
                    &dispute.id,
                    tenant_id,
                    store_id,
                ],
            )?;
            tx.execute(
                "UPDATE a2a_escrows SET status = ?, released_at = ?, updated_at = ?
                 WHERE id = ? AND tenant_id = ? AND store_id = ?
                   AND status = 'disputed' AND dispute_id = ?",
                params![
                    escrow.status.to_string(),
                    escrow.released_at.map(|value| value.to_rfc3339()),
                    started_at.to_rfc3339(),
                    &escrow.id,
                    tenant_id,
                    store_id,
                    &dispute.id,
                ],
            )?;
            let mut dispute_event = KernelOutboxEvent::domain(
                if final_resolution {
                    "a2a.dispute_resolved.v1"
                } else {
                    "a2a.dispute_escalated.v1"
                },
                "a2a_dispute",
                dispute.id.clone(),
                serde_json::json!({
                    "dispute_id": &dispute.id,
                    "escrow_id": &escrow.id,
                    "resolution_type": input.resolution_type.to_string(),
                    "buyer_amount_decimal": dispute.buyer_amount.map(|value| value.to_string()),
                    "seller_amount_decimal": dispute.seller_amount.map(|value| value.to_string()),
                    "resolved_by": &dispute.resolved_by,
                    "status": dispute.status.to_string()
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut dispute_event, command);
            append_kernel_event_tx(tx, &dispute_event)?;
            let mut event_ids = vec![dispute_event.id];
            if final_resolution {
                let mut escrow_event = KernelOutboxEvent::domain(
                    "a2a.escrow_dispute_resolved.v1",
                    "a2a_escrow",
                    escrow.id.clone(),
                    serde_json::json!({
                        "escrow_id": &escrow.id,
                        "dispute_id": &dispute.id,
                        "status": escrow.status.to_string(),
                        "amount_decimal": escrow.amount_decimal.to_string(),
                        "buyer_amount_decimal": buyer_amount.to_string(),
                        "seller_amount_decimal": seller_amount.to_string(),
                        "asset": &escrow.asset
                    }),
                    Some(command.idempotency_key.clone()),
                );
                attach_command_context(&mut escrow_event, command);
                append_kernel_event_tx(tx, &escrow_event)?;
                event_ids.push(escrow_event.id);
            }
            let mut receipt = succeeded_kernel_receipt(
                command,
                policy.clone(),
                result,
                "a2a_dispute",
                dispute.id,
                event_ids,
                started_at,
            );
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically return escrowed value to its buyer.
    pub fn execute_refund_a2a_escrow(
        &self,
        command: &CommandEnvelope<RefundA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = a2a_transition_guard(
            command,
            &policy,
            started_at,
            REFUND_A2A_ESCROW_COMMAND,
            "a2a.escrow.refund",
            &command.payload.escrow_id,
        );
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_escrow")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut escrow = match load_a2a_escrow_sqlite(
                tx,
                &command.payload.escrow_id,
                command.principal.tenant_id.as_deref().expect("policy validated tenant"),
                command.store_id.as_deref().expect("policy validated store"),
            ) {
                Ok(escrow) => escrow,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.a2a.escrow_not_found",
                        "A2A escrow does not exist",
                        RetryDisposition::Never,
                        "a2a_escrow",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            if !matches!(
                escrow.status,
                A2AEscrowStatus::Created
                    | A2AEscrowStatus::Funded
                    | A2AEscrowStatus::Active
                    | A2AEscrowStatus::Disputed
            ) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.escrow_not_refundable",
                    &format!("cannot refund escrow in {} status", escrow.status),
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            escrow.status = A2AEscrowStatus::Refunded;
            escrow.updated_at = started_at;
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "a2a_escrow");
                receipt.aggregate_id = Some(escrow.id.clone());
                receipt.result = Some(escrow);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if tx.execute(
                "UPDATE a2a_escrows SET status = 'refunded', updated_at = ?
                 WHERE id = ? AND status IN ('created', 'funded', 'active', 'disputed')",
                params![started_at.to_rfc3339(), &escrow.id],
            )? == 0
            {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("A2A escrow was modified concurrently".into()),
                )));
            }
            let mut event = a2a_transition_event(
                command,
                &escrow,
                "a2a.escrow_refunded.v1",
                "refunded",
                command.payload.reason.as_deref(),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt =
                succeeded_a2a_receipt(command, policy.clone(), escrow, event.id, started_at);
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically release an A2A escrow whose conditions are met.
    pub fn execute_release_a2a_escrow(
        &self,
        command: &CommandEnvelope<ReleaseA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != RELEASE_A2A_ESCROW_COMMAND {
            Some((
                "kernel.command_type_mismatch",
                "expected a2a.escrow.release command type".into(),
            ))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.payload.escrow_id.trim().is_empty() {
            Some(("commerce.a2a.escrow.validation_failed", "escrow_id is required".into()))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "A2A escrows do not expose an aggregate version".into(),
            ))
        } else {
            None
        };

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "a2a_escrow")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let escrow = match load_a2a_escrow_sqlite(
                tx,
                &command.payload.escrow_id,
                command.principal.tenant_id.as_deref().expect("policy validated tenant"),
                command.store_id.as_deref().expect("policy validated store"),
            ) {
                Ok(escrow) => escrow,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.a2a.escrow_not_found",
                        "A2A escrow does not exist",
                        RetryDisposition::Never,
                        "a2a_escrow",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            if !matches!(escrow.status, A2AEscrowStatus::Funded | A2AEscrowStatus::Active) {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.escrow_not_releasable",
                    &format!("cannot release escrow in {} status", escrow.status),
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if escrow.expires_at <= started_at {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.escrow_expired",
                    "escrow has reached its expiry and must be refunded",
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if !a2a_release_conditions_met_sqlite(tx, &escrow, started_at)? {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.a2a.escrow_conditions_unmet",
                    "not all escrow release conditions are met",
                    RetryDisposition::Never,
                    "a2a_escrow",
                );
                receipt.aggregate_id = Some(escrow.id);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut released = escrow;
            released.status = A2AEscrowStatus::Released;
            released.released_at = Some(started_at);
            released.updated_at = started_at;
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "a2a_escrow");
                receipt.aggregate_id = Some(released.id.clone());
                receipt.result = Some(released);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if tx.execute(
                "UPDATE a2a_escrows SET status = 'released', released_at = ?, updated_at = ?
                 WHERE id = ? AND status IN ('funded', 'active')",
                params![started_at.to_rfc3339(), started_at.to_rfc3339(), &released.id],
            )? == 0
            {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("A2A escrow was modified concurrently".into()),
                )));
            }
            let mut event = KernelOutboxEvent::domain(
                "a2a.escrow_released.v1",
                "a2a_escrow",
                released.id.clone(),
                serde_json::json!({
                    "escrow_id": released.id,
                    "quote_id": released.quote_id,
                    "payment_id": released.payment_id,
                    "buyer_address": released.buyer_address,
                    "seller_address": released.seller_address,
                    "amount": released.amount.to_string(),
                    "amount_decimal": released.amount_decimal.to_string(),
                    "asset": released.asset,
                    "network": released.network,
                    "status": "released",
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let aggregate_id = released.id.clone();
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(released),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("a2a_escrow".into()),
                aggregate_id: Some(aggregate_id),
                version_before: None,
                version_after: None,
                event_ids: vec![event.id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically begin collecting a subscription billing cycle.
    pub fn execute_charge_subscription(
        &self,
        command: &CommandEnvelope<ChargeSubscription>,
    ) -> Result<ExecutionReceipt<SubscriptionCharge>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != CHARGE_SUBSCRIPTION_COMMAND {
            Some((
                "kernel.command_type_mismatch",
                "expected subscriptions.charge command type".into(),
            ))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.payload.billing_cycle_id.is_nil() {
            Some(("commerce.subscription.validation_failed", "billing_cycle_id is required".into()))
        } else if command.payload.processor.as_deref().is_some_and(|value| value.trim().is_empty())
        {
            Some(("commerce.subscription.validation_failed", "processor cannot be blank".into()))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "billing cycles do not expose an aggregate version".into(),
            ))
        } else {
            None
        };
        let subscription_repo = SqliteSubscriptionRepository::new(self.pool.clone());

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "billing_cycle")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "billing_cycle",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let cycle = match tx.query_row(
                "SELECT * FROM billing_cycles WHERE id = ?",
                [command.payload.billing_cycle_id.to_string()],
                |row| subscription_repo.row_to_billing_cycle(row),
            ) {
                Ok(cycle) => cycle,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.subscription.billing_cycle_not_found",
                        "billing cycle does not exist",
                        RetryDisposition::Never,
                        "billing_cycle",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            let (subscription_status, customer_id): (
                SubscriptionStatus,
                stateset_core::CustomerId,
            ) = tx.query_row(
                "SELECT status, customer_id FROM subscriptions WHERE id = ?",
                [cycle.subscription_id.to_string()],
                |row| {
                    Ok((
                        super::parse_enum_row(&row.get::<_, String>(0)?, "subscription", "status")?,
                        stateset_core::CustomerId::from(parse_uuid_row(
                            &row.get::<_, String>(1)?,
                            "subscription",
                            "customer_id",
                        )?),
                    ))
                },
            )?;
            let status_allowed =
                matches!(cycle.status, BillingCycleStatus::Scheduled | BillingCycleStatus::Failed);
            let subscription_allowed = matches!(
                subscription_status,
                SubscriptionStatus::Active | SubscriptionStatus::PastDue
            );
            let retry_due =
                cycle.next_retry_at.is_none_or(|next_retry_at| next_retry_at <= started_at);
            if !status_allowed || !subscription_allowed || !retry_due {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.subscription.billing_cycle_not_chargeable",
                    &format!(
                        "billing cycle in {} for {} subscription is not chargeable now",
                        cycle.status, subscription_status
                    ),
                    RetryDisposition::Never,
                    "billing_cycle",
                );
                receipt.aggregate_id = Some(cycle.id.to_string());
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if cycle.total <= rust_decimal::Decimal::ZERO {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.subscription.non_positive_charge",
                    "billing cycle total must be positive before collection",
                    RetryDisposition::Never,
                    "billing_cycle",
                );
                receipt.aggregate_id = Some(cycle.id.to_string());
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let payment_input = CreatePayment {
                customer_id: Some(customer_id),
                payment_method: command.payload.payment_method,
                amount: cycle.total,
                currency: Some(cycle.currency),
                idempotency_key: Some(command.idempotency_key.clone()),
                processor: command.payload.processor.clone(),
                description: Some(format!(
                    "Subscription {} billing cycle {}",
                    cycle.subscription_id, cycle.cycle_number
                )),
                metadata: Some(
                    serde_json::json!({
                        "subscription_id": cycle.subscription_id.to_string(),
                        "billing_cycle_id": cycle.id.to_string(),
                    })
                    .to_string(),
                ),
                ..Default::default()
            };
            if let Err(error) = payment_input.validate() {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    error
                        .invariant_code()
                        .unwrap_or("commerce.subscription.charge_validation_failed"),
                    &error.to_string(),
                    RetryDisposition::Never,
                    "billing_cycle",
                );
                receipt.aggregate_id = Some(cycle.id.to_string());
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "billing_cycle");
                receipt.aggregate_id = Some(cycle.id.to_string());
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            let payment_id = Uuid::new_v4();
            let payment_number = stateset_core::generate_payment_number();
            tx.execute(
                "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key,
                 processor, card_brand, card_last4, card_exp_month, card_exp_year, billing_email,
                 billing_name, billing_address, description, metadata, created_at, updated_at)
                 VALUES (?, ?, NULL, NULL, ?, 'pending', ?, ?, ?, '0', NULL, ?, ?, NULL, NULL,
                         NULL, NULL, NULL, NULL, NULL, ?, ?, ?, ?)",
                params![
                    payment_id.to_string(),
                    payment_number,
                    customer_id.to_string(),
                    command.payload.payment_method.to_string(),
                    cycle.total.to_string(),
                    cycle.currency,
                    &command.idempotency_key,
                    &command.payload.processor,
                    &payment_input.description,
                    &payment_input.metadata,
                    started_at.to_rfc3339(),
                    started_at.to_rfc3339(),
                ],
            )?;
            if tx.execute(
                "UPDATE billing_cycles SET status = 'processing', payment_id = ?,
                 failure_reason = NULL, updated_at = ? WHERE id = ? AND status IN ('scheduled', 'failed')",
                params![
                    payment_id.to_string(),
                    started_at.to_rfc3339(),
                    cycle.id.to_string()
                ],
            )? == 0
            {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("billing cycle was modified concurrently".into()),
                )));
            }
            let payment = tx.query_row(
                "SELECT * FROM payments WHERE id = ?",
                [payment_id.to_string()],
                SqlitePaymentRepository::row_to_payment,
            )?;
            let billing_cycle = tx.query_row(
                "SELECT * FROM billing_cycles WHERE id = ?",
                [cycle.id.to_string()],
                |row| subscription_repo.row_to_billing_cycle(row),
            )?;
            let result = SubscriptionCharge { billing_cycle, payment };
            let mut event = KernelOutboxEvent::domain(
                "subscriptions.charge_requested.v1",
                "billing_cycle",
                cycle.id.to_string(),
                serde_json::json!({
                    "billing_cycle_id": cycle.id.to_string(),
                    "subscription_id": cycle.subscription_id.to_string(),
                    "payment_id": payment_id.to_string(),
                    "amount": cycle.total.to_string(),
                    "currency": cycle.currency.as_str(),
                    "payment_method": command.payload.payment_method.to_string(),
                    "processor": command.payload.processor,
                    "status": "processing",
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(result),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("billing_cycle".into()),
                aggregate_id: Some(cycle.id.to_string()),
                version_before: None,
                version_after: None,
                event_ids: vec![event.id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically commit a checkout-ready cart to an order.
    pub fn execute_commit_checkout(
        &self,
        command: &CommandEnvelope<CommitCheckout>,
    ) -> Result<ExecutionReceipt<CheckoutResult>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != COMMIT_CHECKOUT_COMMAND {
            Some(("kernel.command_type_mismatch", "expected checkout.commit command type".into()))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.payload.cart_id.is_nil() {
            Some(("commerce.checkout.validation_failed", "cart_id is required".into()))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "carts do not expose an aggregate version".into(),
            ))
        } else {
            None
        };
        let cart_repo = SqliteCartRepository::new(self.pool.clone());

        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "checkout")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "checkout",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            if command.mode == ExecutionMode::Preview {
                if let Err(error) = cart_repo.validate_checkout_in_tx(tx, command.payload.cart_id) {
                    if let Some(commerce_error) = sqlite_commerce_error(&error) {
                        let mut receipt = rejected_receipt(
                            command,
                            Some(policy.clone()),
                            checkout_error_code(commerce_error),
                            &commerce_error.to_string(),
                            RetryDisposition::Never,
                            "checkout",
                        );
                        receipt.aggregate_id = Some(command.payload.cart_id.to_string());
                        append_receipt(tx, &request_hash, &mut receipt)?;
                        return Ok(receipt);
                    }
                    return Err(error);
                }
                let mut receipt = preview_receipt(command, policy.clone(), "checkout");
                receipt.aggregate_id = Some(command.payload.cart_id.to_string());
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }

            // A savepoint lets expected business rejections become durable
            // receipts without retaining a partially-created order or stock
            // reservation.
            tx.execute_batch("SAVEPOINT kernel_checkout_apply")?;
            let attempted =
                cart_repo.complete_checkout_in_tx(tx, command.payload.cart_id, false, false);

            let checkout = match attempted {
                Ok(checkout) => checkout,
                Err(error) => {
                    tx.execute_batch(
                        "ROLLBACK TO SAVEPOINT kernel_checkout_apply; RELEASE SAVEPOINT kernel_checkout_apply",
                    )?;
                    if let Some(commerce_error) = sqlite_commerce_error(&error) {
                        let code = checkout_error_code(commerce_error);
                        let mut receipt = rejected_receipt(
                            command,
                            Some(policy.clone()),
                            code,
                            &commerce_error.to_string(),
                            RetryDisposition::Never,
                            "checkout",
                        );
                        receipt.aggregate_id = Some(command.payload.cart_id.to_string());
                        append_receipt(tx, &request_hash, &mut receipt)?;
                        return Ok(receipt);
                    }
                    return Err(error);
                }
            };

            tx.execute_batch("RELEASE SAVEPOINT kernel_checkout_apply")?;

            let mut event = KernelOutboxEvent::domain(
                "checkout.committed.v1",
                "checkout",
                command.payload.cart_id.to_string(),
                serde_json::json!({
                    "cart_id": checkout.cart_id.to_string(),
                    "order_id": checkout.order_id.to_string(),
                    "order_number": checkout.order_number,
                    "total": checkout.total_charged.to_string(),
                    "currency": checkout.currency.as_str(),
                    "payment_status": "pending",
                }),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(checkout),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("checkout".into()),
                aggregate_id: Some(command.payload.cart_id.to_string()),
                version_before: None,
                version_after: None,
                event_ids: vec![event.id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically post a balanced draft journal entry.
    pub fn execute_post_journal_entry(
        &self,
        command: &CommandEnvelope<PostJournalEntry>,
    ) -> Result<ExecutionReceipt<JournalEntry>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != POST_LEDGER_COMMAND {
            Some(("kernel.command_type_mismatch", "expected ledger.post command type".to_string()))
        } else if command.deadline.is_some_and(|d| d <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.payload.journal_entry_id.is_nil()
            || command.payload.posted_by.trim().is_empty()
        {
            Some((
                "commerce.ledger.validation_failed",
                "journal_entry_id and posted_by are required".into(),
            ))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "journal entries do not expose an aggregate version".into(),
            ))
        } else {
            None
        };
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "journal_entry")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "journal_entry",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut entry = match SqliteGeneralLedgerRepository::load_journal_entry_with_conn(
                tx,
                command.payload.journal_entry_id,
            ) {
                Ok(entry) => entry,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.ledger.entry_not_found",
                        "journal entry does not exist",
                        RetryDisposition::Never,
                        "journal_entry",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(e) => return Err(e),
            };
            if let Err(error) = entry.ensure_postable() {
                let code = error.invariant_code().unwrap_or("commerce.ledger.entry_not_postable");
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    &error.to_string(),
                    RetryDisposition::Never,
                    "journal_entry",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            // Same guard as `post_journal_entry`: the entry's period must be
            // open, or posting would mutate a closed/locked period's balances.
            let period_status: String = tx.query_row(
                "SELECT status FROM gl_periods WHERE id = ?1",
                params![entry.period_id.to_string()],
                |row| row.get(0),
            )?;
            if period_status != "open" {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.ledger.period_not_open",
                    &format!("cannot post journal entry: its period is {period_status}, not open"),
                    RetryDisposition::Never,
                    "journal_entry",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if command.mode == ExecutionMode::Preview {
                entry.status = JournalEntryStatus::Posted;
                entry.posted_at = Some(started_at);
                entry.posted_by = Some(command.payload.posted_by.clone());
                entry.updated_at = started_at;
                let mut receipt = preview_receipt(command, policy.clone(), "journal_entry");
                receipt.result = Some(entry);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            for line in &entry.lines {
                SqliteGeneralLedgerRepository::update_account_balance_with_conn(
                    tx,
                    line.account_id,
                    line.debit_amount,
                    line.credit_amount,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }
            if tx.execute(
                "UPDATE gl_journal_entries SET status = 'posted', posted_at = ?, posted_by = ? WHERE id = ? AND status = 'draft'",
                params![started_at.to_rfc3339(), command.payload.posted_by, command.payload.journal_entry_id.to_string()],
            )? == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict("Journal entry was modified concurrently".into()))));
            }
            let mut event = KernelOutboxEvent::domain(
                "ledger.journal_entry_posted.v1",
                "journal_entry",
                command.payload.journal_entry_id.to_string(),
                serde_json::json!({"journal_entry_id": command.payload.journal_entry_id.to_string(), "entry_number": entry.entry_number,
                    "source": entry.source.to_string(), "total_debits": entry.total_debits.to_string(), "total_credits": entry.total_credits.to_string(),
                    "line_count": entry.lines.len(), "posted_by": command.payload.posted_by, "status": JournalEntryStatus::Posted.to_string()}),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            entry.status = JournalEntryStatus::Posted;
            entry.posted_at = Some(started_at);
            entry.posted_by = Some(command.payload.posted_by.clone());
            entry.updated_at = started_at;
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(entry.clone()),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("journal_entry".into()),
                aggregate_id: Some(entry.id.to_string()),
                version_before: None,
                version_after: None,
                event_ids: vec![event.id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }

    /// Preview or atomically record confirmed x402 settlement.
    pub fn execute_settle_x402_intent(
        &self,
        command: &CommandEnvelope<SettleX402Intent>,
    ) -> Result<ExecutionReceipt<X402PaymentIntent>> {
        command.validate_contract().map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let request_hash = semantic_request_hash(command, &command.payload)?;
        let started_at = Utc::now();
        let policy = self.policy.evaluate(command, started_at);
        let guard = if command.command_type != SETTLE_X402_COMMAND {
            Some(("kernel.command_type_mismatch", "expected x402.settle command type".into()))
        } else if command.deadline.is_some_and(|deadline| deadline <= started_at) {
            Some(("kernel.deadline_exceeded", "command deadline elapsed before execution".into()))
        } else if !policy.allowed {
            Some((
                "kernel.policy_denied",
                format!("policy denied command: {}", policy.reason_codes.join(", ")),
            ))
        } else if command.payload.intent_id.is_nil() || command.payload.tx_hash.trim().is_empty() {
            Some(("commerce.x402.validation_failed", "intent_id and tx_hash are required".into()))
        } else if command.expected_version.is_some() {
            Some((
                "kernel.expected_version_not_applicable",
                "x402 payment intents do not expose an aggregate version".into(),
            ))
        } else {
            None
        };
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = receipt_by_idempotency_key_tx(tx, &command.idempotency_key)? {
                if let Replay::Return(stored) =
                    replay_or_conflict(tx, command, &request_hash, existing, "x402_payment_intent")?
                {
                    return Ok(stored);
                }
            }
            if let Some((code, message)) = &guard {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    message,
                    RetryDisposition::Never,
                    "x402_payment_intent",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let mut intent = match tx.query_row(
                "SELECT * FROM x402_payment_intents WHERE id = ?",
                [command.payload.intent_id.to_string()],
                SqliteX402PaymentIntentRepository::row_to_intent,
            ) {
                Ok(intent) => intent,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let mut receipt = rejected_receipt(
                        command,
                        Some(policy.clone()),
                        "commerce.x402.intent_not_found",
                        "x402 payment intent does not exist",
                        RetryDisposition::Never,
                        "x402_payment_intent",
                    );
                    append_receipt(tx, &request_hash, &mut receipt)?;
                    return Ok(receipt);
                }
                Err(error) => return Err(error),
            };
            if intent.status != X402IntentStatus::Sequenced {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    "commerce.x402.intent_not_sequenced",
                    &format!("cannot settle intent in {} status", intent.status),
                    RetryDisposition::Never,
                    "x402_payment_intent",
                );
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            let settled_at = Utc::now();
            intent.status = X402IntentStatus::Settled;
            intent.tx_hash = Some(command.payload.tx_hash.clone());
            intent.block_number = Some(command.payload.block_number);
            intent.settled_at = Some(settled_at);
            intent.updated_at = settled_at;
            if command.mode == ExecutionMode::Preview {
                let mut receipt = preview_receipt(command, policy.clone(), "x402_payment_intent");
                receipt.aggregate_id = Some(intent.id.to_string());
                receipt.result = Some(intent);
                append_receipt(tx, &request_hash, &mut receipt)?;
                return Ok(receipt);
            }
            if tx.execute(
                "UPDATE x402_payment_intents SET status = ?, tx_hash = ?, block_number = ?, settled_at = ?, updated_at = ? WHERE id = ? AND status = ?",
                params![X402IntentStatus::Settled.to_string(), command.payload.tx_hash,
                    i64::try_from(command.payload.block_number).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
                    settled_at.to_rfc3339(), settled_at.to_rfc3339(), command.payload.intent_id.to_string(),
                    X402IntentStatus::Sequenced.to_string()],
            )? == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("x402 intent was modified concurrently".into()),
                )));
            }
            let mut event = KernelOutboxEvent::domain(
                "x402.intent_settled.v1",
                "x402_payment_intent",
                intent.id.to_string(),
                serde_json::json!({"intent_id": intent.id.to_string(), "tx_hash": command.payload.tx_hash,
                    "block_number": command.payload.block_number, "amount": intent.amount.to_string(),
                    "amount_decimal": intent.amount_decimal.to_string(), "asset": intent.asset.to_string(),
                    "network": intent.network.to_string()}),
                Some(command.idempotency_key.clone()),
            );
            attach_command_context(&mut event, command);
            append_kernel_event_tx(tx, &event)?;
            let mut receipt = ExecutionReceipt {
                contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
                receipt_id: Uuid::new_v4(),
                command_id: command.command_id,
                idempotency_key: command.idempotency_key.clone(),
                command_type: command.command_type.clone(),
                status: ExecutionStatus::Succeeded,
                result: Some(intent.clone()),
                error_code: None,
                error_message: None,
                retry: RetryDisposition::SameKey,
                aggregate_type: Some("x402_payment_intent".into()),
                aggregate_id: Some(intent.id.to_string()),
                version_before: None,
                version_after: None,
                event_ids: vec![event.id],
                policy: Some(policy.clone()),
                audit_hash: None,
                started_at,
                completed_at: Utc::now(),
            };
            append_receipt(tx, &request_hash, &mut receipt)?;
            Ok(receipt)
        })
    }
}

fn load_inventory_reservation_tx(
    tx: &rusqlite::Transaction<'_>,
    reservation_id: Uuid,
) -> rusqlite::Result<Option<InventoryReservation>> {
    let result = tx.query_row(
        "SELECT id, item_id, location_id, quantity, status, reference_type, reference_id,
                expires_at, created_at
         FROM inventory_reservations WHERE id = ?",
        [reservation_id.to_string()],
        |row| {
            let status: String = row.get("status")?;
            Ok(InventoryReservation {
                id: parse_uuid_row(&row.get::<_, String>("id")?, "inventory_reservation", "id")?,
                item_id: row.get("item_id")?,
                location_id: row.get("location_id")?,
                quantity: parse_decimal_row(
                    &row.get::<_, String>("quantity")?,
                    "inventory_reservation",
                    "quantity",
                )?,
                status: status.parse().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(CommerceError::DatabaseError(format!(
                            "invalid inventory reservation status '{status}': {error}"
                        ))),
                    )
                })?,
                reference_type: row.get("reference_type")?,
                reference_id: row.get("reference_id")?,
                expires_at: parse_datetime_opt_row(
                    row.get("expires_at")?,
                    "inventory_reservation",
                    "expires_at",
                )?,
                created_at: parse_datetime_row(
                    &row.get::<_, String>("created_at")?,
                    "inventory_reservation",
                    "created_at",
                )?,
            })
        },
    );
    match result {
        Ok(reservation) => Ok(Some(reservation)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error),
    }
}

fn find_inventory_lifecycle_event_tx(
    tx: &rusqlite::Transaction<'_>,
    reservation_id: Uuid,
    started_at: chrono::DateTime<Utc>,
    action: InventoryLifecycleAction,
) -> rusqlite::Result<Option<Uuid>> {
    let event_types = match action {
        InventoryLifecycleAction::Confirm(_) => {
            "('inventory.reservation_confirmed.v1', 'inventory.reservation_expired.v1')"
        }
        InventoryLifecycleAction::Release => {
            "('inventory.reservation_released.v1', 'inventory.reservation_expired.v1')"
        }
    };
    let sql = format!(
        "SELECT id FROM kernel_outbox
         WHERE created_at >= ? AND event_type IN {event_types}
           AND (aggregate_id = ? OR json_extract(payload, '$.source_reservation_id') = ?)
         ORDER BY rowid DESC LIMIT 1"
    );
    let result = tx.query_row(
        &sql,
        params![started_at.to_rfc3339(), reservation_id.to_string(), reservation_id.to_string()],
        |row| parse_uuid_row(&row.get::<_, String>(0)?, "kernel_outbox", "id"),
    );
    match result {
        Ok(event_id) => Ok(Some(event_id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error),
    }
}

fn a2a_transition_guard<T: Serialize>(
    command: &CommandEnvelope<T>,
    policy: &stateset_core::PolicyDecisionEvidence,
    now: chrono::DateTime<Utc>,
    expected_command: &str,
    _expected_name: &str,
    escrow_id: &str,
) -> Option<(&'static str, String)> {
    EnvelopeGuard::unversioned(expected_command, ESCROW_UNVERSIONED)
        .evaluate(command, policy, now)
        .or_else(|| escrow_id_guard(escrow_id))
        .map(|rejection| (rejection.code, rejection.message))
}

fn a2a_transition_event<T>(
    command: &CommandEnvelope<T>,
    escrow: &A2AEscrow,
    event_type: &str,
    status: &str,
    reason: Option<&str>,
) -> KernelOutboxEvent {
    KernelOutboxEvent::domain(
        event_type,
        "a2a_escrow",
        escrow.id.clone(),
        serde_json::json!({
            "escrow_id": &escrow.id,
            "quote_id": &escrow.quote_id,
            "payment_id": &escrow.payment_id,
            "buyer_address": &escrow.buyer_address,
            "seller_address": &escrow.seller_address,
            "amount_decimal": escrow.amount_decimal.to_string(),
            "asset": &escrow.asset,
            "network": &escrow.network,
            "status": status,
            "reason": reason,
        }),
        Some(command.idempotency_key.clone()),
    )
}

fn succeeded_a2a_receipt<C>(
    command: &CommandEnvelope<C>,
    policy: stateset_core::PolicyDecisionEvidence,
    escrow: A2AEscrow,
    event_id: Uuid,
    started_at: chrono::DateTime<Utc>,
) -> ExecutionReceipt<A2AEscrow> {
    let aggregate_id = escrow.id.clone();
    succeeded_receipt(
        command,
        policy,
        escrow,
        "a2a_escrow",
        Some(aggregate_id),
        None,
        None,
        vec![event_id],
        started_at,
    )
}

fn succeeded_kernel_receipt<C, T>(
    command: &CommandEnvelope<C>,
    policy: stateset_core::PolicyDecisionEvidence,
    result: T,
    aggregate_type: &str,
    aggregate_id: String,
    event_ids: Vec<Uuid>,
    started_at: chrono::DateTime<Utc>,
) -> ExecutionReceipt<T> {
    succeeded_receipt(
        command,
        policy,
        result,
        aggregate_type,
        Some(aggregate_id),
        None,
        None,
        event_ids,
        started_at,
    )
}

fn principal_controls_address<C>(command: &CommandEnvelope<C>, address: &str) -> bool {
    command.principal.id == address || command.principal.delegated_by.as_deref() == Some(address)
}

fn load_a2a_escrow_sqlite(
    tx: &rusqlite::Transaction<'_>,
    escrow_id: &str,
    tenant_id: &str,
    store_id: &str,
) -> rusqlite::Result<A2AEscrow> {
    tx.query_row(
        "SELECT id, status, quote_id, payment_id, buyer_address, seller_address, amount,
                CAST(amount_decimal AS TEXT), asset, network, release_conditions, funded_at,
                released_at, disputed_at, dispute_id, expires_at, auto_release_after,
                metadata, created_at, updated_at, tenant_id, store_id
                FROM a2a_escrows WHERE id = ? AND tenant_id = ? AND store_id = ?",
        params![escrow_id, tenant_id, store_id],
        |row| {
            let status_raw: String = row.get(1)?;
            let status = status_raw.parse::<A2AEscrowStatus>().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            let conditions_raw: String = row.get(10)?;
            let release_conditions = serde_json::from_str(&conditions_raw).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            let metadata_raw: Option<String> = row.get(17)?;
            let metadata = metadata_raw.map(|raw| serde_json::from_str(&raw)).transpose().map_err(
                |error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        17,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                },
            )?;
            Ok(A2AEscrow {
                id: row.get(0)?,
                tenant_id: row.get(20)?,
                store_id: row.get(21)?,
                status,
                quote_id: row.get(2)?,
                payment_id: row.get(3)?,
                buyer_address: row.get(4)?,
                seller_address: row.get(5)?,
                amount: row.get(6)?,
                amount_decimal: parse_decimal_row(
                    &row.get::<_, String>(7)?,
                    "a2a_escrow",
                    "amount_decimal",
                )?,
                asset: row.get(8)?,
                network: row.get(9)?,
                release_conditions,
                funded_at: parse_datetime_opt_row(row.get(11)?, "a2a_escrow", "funded_at")?,
                released_at: parse_datetime_opt_row(row.get(12)?, "a2a_escrow", "released_at")?,
                disputed_at: parse_datetime_opt_row(row.get(13)?, "a2a_escrow", "disputed_at")?,
                dispute_id: row.get(14)?,
                expires_at: parse_datetime_row(
                    &row.get::<_, String>(15)?,
                    "a2a_escrow",
                    "expires_at",
                )?,
                auto_release_after: parse_datetime_opt_row(
                    row.get(16)?,
                    "a2a_escrow",
                    "auto_release_after",
                )?,
                metadata,
                created_at: parse_datetime_row(
                    &row.get::<_, String>(18)?,
                    "a2a_escrow",
                    "created_at",
                )?,
                updated_at: parse_datetime_row(
                    &row.get::<_, String>(19)?,
                    "a2a_escrow",
                    "updated_at",
                )?,
            })
        },
    )
}

fn load_a2a_dispute_sqlite(
    tx: &rusqlite::Transaction<'_>,
    dispute_id: &str,
    tenant_id: &str,
    store_id: &str,
) -> rusqlite::Result<A2ADispute> {
    tx.query_row(
        "SELECT id, tenant_id, store_id, status, escrow_id, quote_id,
                claimant_address, respondent_address, reason, category,
                CAST(amount_decimal AS TEXT), asset, resolution_type,
                CAST(buyer_amount_decimal AS TEXT), CAST(seller_amount_decimal AS TEXT),
                resolution_note, resolved_by, evidence_deadline, review_deadline,
                metadata, created_at, updated_at, resolved_at
         FROM a2a_disputes
         WHERE id = ? AND tenant_id = ? AND store_id = ?",
        params![dispute_id, tenant_id, store_id],
        |row| {
            let status_raw: String = row.get(3)?;
            let status = status_raw.parse::<A2ADisputeStatus>().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            let resolution_type_raw: Option<String> = row.get(12)?;
            let resolution_type = resolution_type_raw
                .map(|value| value.parse::<A2ADisputeResolutionType>())
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let buyer_raw: Option<String> = row.get(13)?;
            let buyer_amount = buyer_raw
                .map(|value| parse_decimal_row(&value, "a2a_dispute", "buyer_amount_decimal"))
                .transpose()?;
            let seller_raw: Option<String> = row.get(14)?;
            let seller_amount = seller_raw
                .map(|value| parse_decimal_row(&value, "a2a_dispute", "seller_amount_decimal"))
                .transpose()?;
            let metadata_raw: Option<String> = row.get(19)?;
            let metadata = metadata_raw
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        19,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            Ok(A2ADispute {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                store_id: row.get(2)?,
                status,
                escrow_id: row.get(4)?,
                quote_id: row.get(5)?,
                claimant_address: row.get(6)?,
                respondent_address: row.get(7)?,
                reason: row.get(8)?,
                category: row.get(9)?,
                amount: parse_decimal_row(
                    &row.get::<_, String>(10)?,
                    "a2a_dispute",
                    "amount_decimal",
                )?,
                asset: row.get(11)?,
                resolution_type,
                buyer_amount,
                seller_amount,
                resolution_note: row.get(15)?,
                resolved_by: row.get(16)?,
                evidence_deadline: parse_datetime_row(
                    &row.get::<_, String>(17)?,
                    "a2a_dispute",
                    "evidence_deadline",
                )?,
                review_deadline: parse_datetime_row(
                    &row.get::<_, String>(18)?,
                    "a2a_dispute",
                    "review_deadline",
                )?,
                metadata,
                created_at: parse_datetime_row(
                    &row.get::<_, String>(20)?,
                    "a2a_dispute",
                    "created_at",
                )?,
                updated_at: parse_datetime_row(
                    &row.get::<_, String>(21)?,
                    "a2a_dispute",
                    "updated_at",
                )?,
                resolved_at: parse_datetime_opt_row(row.get(22)?, "a2a_dispute", "resolved_at")?,
            })
        },
    )
}

fn a2a_release_conditions_met_sqlite(
    tx: &rusqlite::Transaction<'_>,
    escrow: &A2AEscrow,
    now: chrono::DateTime<Utc>,
) -> rusqlite::Result<bool> {
    for condition in &escrow.release_conditions {
        let condition_type = condition.get("type").and_then(serde_json::Value::as_str);
        let met = match condition_type {
            Some("seller_fulfilled") => {
                let quote_id = condition
                    .get("quoteId")
                    .or_else(|| condition.get("quote_id"))
                    .and_then(serde_json::Value::as_str)
                    .or(escrow.quote_id.as_deref());
                if let Some(quote_id) = quote_id {
                    let core_quote_met = tx
                        .query_row(
                            "SELECT status = 'fulfilled' FROM a2a_quotes WHERE id = ?",
                            [quote_id],
                            |row| row.get::<_, bool>(0),
                        )
                        .unwrap_or(false);
                    if core_quote_met {
                        true
                    } else {
                        // The JavaScript A2A runtime keeps its richer negotiation
                        // projection under a distinct name so it can coexist with
                        // the native quote schema in one commerce database.
                        tx.query_row(
                            "SELECT status = 'fulfilled' FROM a2a_market_quotes WHERE id = ?",
                            [quote_id],
                            |row| row.get::<_, bool>(0),
                        )
                        .unwrap_or(false)
                    }
                } else {
                    false
                }
            }
            Some("time_lock") => condition
                .get("releaseAfter")
                .or_else(|| condition.get("release_after"))
                .and_then(serde_json::Value::as_str)
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .is_some_and(|release_after| now >= release_after.with_timezone(&Utc)),
            Some("buyer_confirmed") | Some("milestone") | Some(_) | None => {
                condition.get("completed").and_then(serde_json::Value::as_bool) == Some(true)
            }
        };
        if !met {
            return Ok(false);
        }
    }
    Ok(true)
}

fn sqlite_commerce_error(error: &rusqlite::Error) -> Option<&CommerceError> {
    match error {
        rusqlite::Error::ToSqlConversionFailure(source) => source.downcast_ref::<CommerceError>(),
        _ => None,
    }
}

/// Escrow (and its dispute id, if one was filed) that holds `payment_id` and
/// is currently frozen: either the escrow itself is `disputed` or a dispute
/// row for it is still open. Returns `None` when the payment is unencumbered.
fn open_dispute_for_payment(
    tx: &rusqlite::Transaction<'_>,
    payment_id: &str,
) -> rusqlite::Result<Option<(String, Option<String>)>> {
    tx.query_row(
        "SELECT e.id,
                COALESCE(e.dispute_id, (
                    SELECT d.id FROM a2a_disputes d
                    WHERE d.escrow_id = e.id
                      AND d.status IN ('filed', 'evidence_period', 'under_review', 'escalated')
                    LIMIT 1
                ))
         FROM a2a_escrows e
         WHERE e.payment_id = ?
           AND (
                e.status = 'disputed'
                OR EXISTS (
                    SELECT 1 FROM a2a_disputes d
                    WHERE d.escrow_id = e.id
                      AND d.status IN ('filed', 'evidence_period', 'under_review', 'escalated')
                )
           )
         LIMIT 1",
        [payment_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

fn replay_or_conflict<C, T: DeserializeOwned>(
    tx: &rusqlite::Transaction<'_>,
    command: &CommandEnvelope<C>,
    request_hash: &str,
    existing: KernelReceiptRecord,
    aggregate_type: &str,
) -> rusqlite::Result<Replay<T>> {
    let audit = sealed_audit_entry_tx(tx, &existing)?;
    resolve_replay(command, request_hash, existing, audit.as_ref(), aggregate_type)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn append_receipt<T: Serialize>(
    tx: &rusqlite::Transaction<'_>,
    request_hash: &str,
    receipt: &mut ExecutionReceipt<T>,
) -> rusqlite::Result<()> {
    let record = receipt_record(request_hash, receipt)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    receipt.audit_hash = Some(append_kernel_receipt_tx(tx, &record)?);
    Ok(())
}
