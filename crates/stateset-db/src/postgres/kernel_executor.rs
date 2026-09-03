//! Envelope-aware execution for high-risk PostgreSQL commerce commands.

use super::backorder::PgBackorderRepository;
use super::carts::PgCartRepository;
use super::general_ledger::{JournalEntryLineRow, JournalEntryRow, PgGeneralLedgerRepository};
use super::inventory::{PgInventoryRepository, ReservationConfirmOutcome, ReservationRow};
use super::kernel_outbox::{
    append_kernel_event_tx, append_kernel_receipt_tx, receipt_by_idempotency_key_tx,
    sealed_audit_entry_tx,
};
use super::orders::{OrderItemRow, OrderRow, PgOrderRepository, ShipMode};
use super::payments::{
    PaymentRow, PgPaymentRepository, RefundRow, check_order_capture_capacity_pg,
    open_captures_for_order_pg, void_in_flight_payments_for_order_pg,
};
use super::returns::{PgReturnRepository, ReturnItemRow, ReturnRow};
use super::subscriptions::{BillingCycleRow, PgSubscriptionRepository};
use super::x402_payment_intents::{IntentRow, PgX402PaymentIntentRepository};
use crate::kernel::plans::PlanOutcome;
use crate::kernel::plans::catalog::{create_inventory_item_guard, create_product_guard};
use crate::kernel::plans::escrow::{
    ESCROW_UNVERSIONED, create_escrow_guard, dispute_escrow_guard, escrow_id_guard,
    escrow_legacy_amount, escrow_settlement_guard, file_dispute_guard, plan_fund_escrow,
    resolve_dispute_guard, submit_evidence_guard,
};
use crate::kernel::plans::finance::{
    BILLING_CYCLE_UNVERSIONED, CART_UNVERSIONED, JOURNAL_ENTRY_UNVERSIONED,
    X402_INTENT_UNVERSIONED, charge_subscription_guard, commit_checkout_guard,
    post_journal_entry_guard, settle_x402_guard,
};
use crate::kernel::plans::inventory::{reservation_lifecycle_guard, reserve_inventory_guard};
use crate::kernel::plans::orders::{
    OrderTransitionSnapshot, ShipOrderSnapshot, plan_order_transition, plan_ship_order,
    reservation_expired_during_shipment, ship_order_guard, transition_order_guard,
};
use crate::kernel::plans::payments::{RefundSnapshot, create_payment_guard, plan_refund};
use crate::kernel::plans::returns::transition_return_guard;
use crate::kernel::receipt::{
    attach_command_context, checkout_error_code, preview_receipt, principal_kind_name,
    receipt_record, rejected_receipt, succeeded_receipt,
};
use crate::kernel::{CommandRun, EnvelopeGuard, Replay, resolve_replay};
use crate::{KernelOutboxEvent, KernelReceiptRecord};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
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

#[derive(Clone, Copy)]
enum InventoryLifecycleAction {
    Confirm(Option<rust_decimal::Decimal>),
    Release,
}

#[derive(sqlx::FromRow)]
struct A2AEscrowRow {
    id: String,
    tenant_id: String,
    store_id: String,
    status: String,
    quote_id: Option<String>,
    payment_id: Option<String>,
    buyer_address: String,
    seller_address: String,
    amount: i64,
    amount_decimal: rust_decimal::Decimal,
    asset: String,
    network: String,
    release_conditions: serde_json::Value,
    funded_at: Option<chrono::DateTime<Utc>>,
    released_at: Option<chrono::DateTime<Utc>>,
    disputed_at: Option<chrono::DateTime<Utc>>,
    dispute_id: Option<String>,
    expires_at: chrono::DateTime<Utc>,
    auto_release_after: Option<chrono::DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct A2ADisputeRow {
    id: String,
    tenant_id: String,
    store_id: String,
    status: String,
    escrow_id: String,
    quote_id: Option<String>,
    claimant_address: String,
    respondent_address: String,
    reason: String,
    category: String,
    amount_decimal: rust_decimal::Decimal,
    asset: String,
    resolution_type: Option<String>,
    buyer_amount_decimal: Option<rust_decimal::Decimal>,
    seller_amount_decimal: Option<rust_decimal::Decimal>,
    resolution_note: Option<String>,
    resolved_by: Option<String>,
    evidence_deadline: chrono::DateTime<Utc>,
    review_deadline: chrono::DateTime<Utc>,
    metadata: Option<serde_json::Value>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    resolved_at: Option<chrono::DateTime<Utc>>,
}

/// Async kernel executor with transactionally durable receipts.
#[derive(Debug, Clone)]
pub struct PgKernelExecutor {
    pool: PgPool,
    policy: KernelPolicy,
}

impl PgKernelExecutor {
    pub(crate) const fn new(pool: PgPool, policy: KernelPolicy) -> Self {
        Self { pool, policy }
    }

    /// Preview or atomically create a SKU master and its exact initial stock.
    pub async fn execute_create_inventory_item_async(
        &self,
        command: &CommandEnvelope<CreateInventoryItem>,
    ) -> Result<ExecutionReceipt<InventoryItem>> {
        let input = command.payload.clone();
        let run = CommandRun::prepare(
            command,
            &input,
            &self.policy,
            EnvelopeGuard::create(CREATE_INVENTORY_ITEM_COMMAND),
            "inventory_item",
        )?
        .then_guard(|_| create_inventory_item_guard(&input));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let location_id = input.location_id.unwrap_or(1);
        let initial_quantity = input.initial_quantity.unwrap_or_default();
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        advisory_lock_pg(tx.as_mut(), LOCK_NS_INVENTORY_SKU, &input.sku).await?;

        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "inventory_item")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let sku_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inventory_items WHERE sku = $1)")
                .bind(&input.sku)
                .fetch_one(tx.as_mut())
                .await
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let location_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inventory_locations WHERE id = $1)")
                .bind(location_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
                Some(policy),
                code,
                &message,
                RetryDisposition::Never,
                "inventory_item",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "inventory_item");
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }

        let now = Utc::now();
        let unit = input.unit_of_measure.clone().unwrap_or_else(|| "EA".into());
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, TRUE, $5, $5) RETURNING id",
        ).bind(&input.sku).bind(&input.name).bind(&input.description).bind(&unit).bind(now)
            .fetch_one(tx.as_mut()).await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        sqlx::query(
            "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock, version, updated_at) VALUES ($1, $2, $3, 0, $3, $4, $5, 1, $6)",
        ).bind(id).bind(location_id).bind(initial_quantity).bind(input.reorder_point).bind(input.safety_stock).bind(now)
            .execute(tx.as_mut()).await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if initial_quantity > rust_decimal::Decimal::ZERO {
            sqlx::query(
                "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_by, created_at) VALUES ($1, $2, 'receipt', $3, 'Initial stock', $4, $5)",
            ).bind(id).bind(location_id).bind(initial_quantity).bind(&command.principal.id).bind(now)
                .execute(tx.as_mut()).await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = succeeded_kernel_receipt_pg(
            command,
            policy,
            item,
            "inventory_item",
            id.to_string(),
            vec![event.id],
            started_at,
        );
        receipt.version_after = Some(1);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically apply `products.create` with exact-decimal variants.
    pub async fn execute_create_product_async(
        &self,
        command: &CommandEnvelope<CreateProduct>,
    ) -> Result<ExecutionReceipt<Product>> {
        let input = command.payload.clone();
        let run = CommandRun::prepare(
            command,
            &input,
            &self.policy,
            EnvelopeGuard::create(CREATE_PRODUCT_COMMAND),
            "product",
        )?
        .then_guard(|_| create_product_guard(&input));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        // Serialize semantic uniqueness keys so two distinct command keys cannot
        // race through the preview/check window and surface an unsealed SQL error.
        advisory_lock_pg(tx.as_mut(), LOCK_NS_PRODUCT_SLUG, &slug).await?;
        let mut skus = input
            .variants
            .as_ref()
            .map(|variants| variants.iter().map(|variant| variant.sku.clone()).collect::<Vec<_>>())
            .unwrap_or_default();
        skus.sort();
        skus.dedup();
        for sku in &skus {
            advisory_lock_pg(tx.as_mut(), LOCK_NS_PRODUCT_SKU, sku).await?;
        }

        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "product").await?
            {
                tx.commit()
                    .await
                    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
                return Ok(stored);
            }
        }

        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let slug_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE slug = $1)")
                .bind(&slug)
                .fetch_one(tx.as_mut())
                .await
                .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let duplicate_sku: Option<String> = if skus.is_empty() {
            None
        } else {
            sqlx::query_scalar(
                "SELECT sku FROM product_variants WHERE sku = ANY($1) ORDER BY sku LIMIT 1",
            )
            .bind(&skus)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|error| CommerceError::DatabaseError(error.to_string()))?
        };
        if slug_exists || duplicate_sku.is_some() {
            let (code, message) = if slug_exists {
                ("commerce.product.slug_conflict", format!("product slug '{slug}' already exists"))
            } else {
                let sku = duplicate_sku.as_deref().unwrap_or_default();
                ("commerce.product.sku_conflict", format!("product SKU '{sku}' already exists"))
            };
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                code,
                &message,
                RetryDisposition::Never,
                "product",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "product");
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let id = ProductId::new();
        let created_at = Utc::now();
        let description = input.description.clone().unwrap_or_default();
        let product_type = input.product_type.unwrap_or_default();
        let attributes = input.attributes.clone().unwrap_or_default();
        let attributes_json = serde_json::to_value(&attributes)
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let seo_json = input
            .seo
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        sqlx::query(
            "INSERT INTO products (
                id, name, slug, description, status, product_type,
                attributes, seo, created_at, updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)",
        )
        .bind(id.into_uuid())
        .bind(&input.name)
        .bind(&slug)
        .bind(&description)
        .bind(ProductStatus::Draft.to_string())
        .bind(product_type.to_string())
        .bind(&attributes_json)
        .bind(&seo_json)
        .bind(created_at)
        .execute(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;

        if let Some(variants) = &input.variants {
            for (index, variant) in variants.iter().enumerate() {
                let options = serde_json::to_value(variant.options.clone().unwrap_or_default())
                    .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
                sqlx::query(
                    "INSERT INTO product_variants (
                        id, product_id, sku, name, price, compare_at_price, cost,
                        barcode, weight, weight_unit, options, is_default, is_active,
                        created_at, updated_at
                     ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, TRUE, $13, $13)",
                )
                .bind(Uuid::new_v4())
                .bind(id.into_uuid())
                .bind(&variant.sku)
                .bind(variant.name.as_deref().unwrap_or(&variant.sku))
                .bind(variant.price)
                .bind(variant.compare_at_price)
                .bind(variant.cost)
                .bind(&variant.barcode)
                .bind(variant.weight)
                .bind(&variant.weight_unit)
                .bind(options)
                .bind(index == 0)
                .bind(created_at)
                .execute(tx.as_mut())
                .await
                .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            }
        }

        let product = Product {
            id,
            name: input.name.clone(),
            slug: slug.clone(),
            description,
            status: ProductStatus::Draft,
            product_type,
            attributes,
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = succeeded_kernel_receipt_pg(
            command,
            policy,
            product,
            "product",
            id.to_string(),
            vec![event.id],
            started_at,
        );
        receipt.version_after = Some(1);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically apply `payments.create`.
    pub async fn execute_create_payment_async(
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

        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
            && let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "payment").await?
        {
            tx.commit().await.map_err(pg_err)?;
            return Ok(stored);
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }
        // Every check apply performs must run before the preview answer is
        // sealed, or a preview would promise a capture apply refuses.
        if let Some(order_id) = input.order_id {
            check_order_capture_capacity_pg(
                tx.as_mut(),
                order_id.into_uuid(),
                None,
                input.amount,
                input.currency.unwrap_or_default(),
            )
            .await?;
        }
        if run.is_preview() {
            let mut receipt = run.previewed();
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }

        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let payment_number = stateset_core::generate_payment_number();
        sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
             payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
             card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
             billing_address, description, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     $17, $18, $19, $20, $21, $22, $23, $24)",
        )
        .bind(id)
        .bind(&payment_number)
        .bind(input.order_id.map(|value| value.into_uuid()))
        .bind(input.invoice_id)
        .bind(input.customer_id.map(|value| value.into_uuid()))
        .bind(PaymentTransactionStatus::Pending.to_string())
        .bind(input.payment_method.to_string())
        .bind(input.amount)
        .bind(input.currency.unwrap_or_default())
        .bind(rust_decimal::Decimal::ZERO)
        .bind(&input.external_id)
        .bind(&input.idempotency_key)
        .bind(&input.processor)
        .bind(input.card_brand.map(|value| value.to_string()))
        .bind(&input.card_last4)
        .bind(input.card_exp_month)
        .bind(input.card_exp_year)
        .bind(&input.billing_email)
        .bind(&input.billing_name)
        .bind(&input.billing_address)
        .bind(&input.description)
        .bind(&input.metadata)
        .bind(created_at)
        .bind(created_at)
        .execute(tx.as_mut())
        .await
        .map_err(pg_err)?;

        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                    amount, currency, amount_refunded, external_id, idempotency_key, processor,
                    card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                    billing_address, description, failure_reason, failure_code, metadata, paid_at,
                    version, created_at, updated_at
             FROM payments WHERE id = $1",
        )
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(pg_err)?;
        let payment = PgPaymentRepository::row_to_payment(row)?;

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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt =
            run.succeeded(payment, Some(id.to_string()), None, Some(1), vec![event.id]);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(pg_err)?;
        Ok(receipt)
    }

    /// Preview or atomically apply `payments.create_refund`.
    pub async fn execute_create_refund_async(
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
        let payment_id = input.payment_id.into_uuid();

        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
            && let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "refund").await?
        {
            tx.commit().await.map_err(pg_err)?;
            return Ok(stored);
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }

        let payment_row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                    amount, currency, amount_refunded, external_id, idempotency_key, processor,
                    card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                    billing_address, description, failure_reason, failure_code, metadata, paid_at,
                    version, created_at, updated_at
             FROM payments WHERE id = $1 FOR UPDATE",
        )
        .bind(payment_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(pg_err)?;
        let snapshot = match payment_row {
            Some(row) => {
                let payment = PgPaymentRepository::row_to_payment(row)?;
                let open_dispute: Option<(String, Option<String>)> = sqlx::query_as(
                    "SELECT e.id,
                            COALESCE(e.dispute_id, (
                                SELECT d.id FROM a2a_disputes d
                                WHERE d.escrow_id = e.id
                                  AND d.status IN ('filed', 'evidence_period', 'under_review', 'escalated')
                                LIMIT 1
                            ))
                     FROM a2a_escrows e
                     WHERE e.payment_id = $1
                       AND (
                            e.status = 'disputed'
                            OR EXISTS (
                                SELECT 1 FROM a2a_disputes d
                                WHERE d.escrow_id = e.id
                                  AND d.status IN ('filed', 'evidence_period', 'under_review', 'escalated')
                            )
                       )
                     LIMIT 1",
                )
                .bind(payment_id.to_string())
                .fetch_optional(tx.as_mut())
                .await
                .map_err(pg_err)?;
                let in_flight_refunds: rust_decimal::Decimal = sqlx::query_scalar(
                    "SELECT COALESCE(SUM(amount), 0) FROM refunds
                     WHERE payment_id = $1 AND status IN ($2, $3)",
                )
                .bind(payment_id)
                .bind(RefundStatus::Pending.to_string())
                .bind(RefundStatus::Processing.to_string())
                .fetch_one(tx.as_mut())
                .await
                .map_err(pg_err)?;
                Some(RefundSnapshot { payment, in_flight_refunds, open_dispute })
            }
            None => None,
        };
        let effects = match plan_refund(&input, snapshot.as_ref()) {
            PlanOutcome::Reject { rejection, .. } => {
                let mut receipt = run.rejected_by(&rejection);
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(pg_err)?;
                return Ok(receipt);
            }
            PlanOutcome::Proceed(effects) => effects,
        };
        if run.is_preview() {
            let mut receipt = run.previewed();
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }

        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let refund_number = stateset_core::generate_refund_number();
        sqlx::query(
            "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency,
             reason, external_id, idempotency_key, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(id)
        .bind(&refund_number)
        .bind(payment_id)
        .bind(RefundStatus::Pending.to_string())
        .bind(effects.amount)
        .bind(effects.currency)
        .bind(&input.reason)
        .bind(&input.external_id)
        .bind(&input.idempotency_key)
        .bind(&input.notes)
        .bind(created_at)
        .bind(created_at)
        .execute(tx.as_mut())
        .await
        .map_err(pg_err)?;
        let row = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id,
                    idempotency_key, failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE id = $1",
        )
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(pg_err)?;
        let refund = PgPaymentRepository::row_to_refund(row)?;
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt =
            run.succeeded(refund, Some(id.to_string()), None, Some(1), vec![event.id]);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(pg_err)?;
        Ok(receipt)
    }

    /// Preview or atomically apply `inventory.reserve`.
    pub async fn execute_reserve_inventory_async(
        &self,
        command: &CommandEnvelope<ReserveInventory>,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        let input = &command.payload;
        let run = CommandRun::prepare(
            command,
            input,
            &self.policy,
            EnvelopeGuard::aggregate(RESERVE_INVENTORY_COMMAND),
            "inventory_reservation",
        )?
        .then_guard(|_| reserve_inventory_guard(input));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) = replay_or_conflict(
                tx.as_mut(),
                command,
                &request_hash,
                existing,
                "inventory_reservation",
            )
            .await?
            {
                tx.commit()
                    .await
                    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let item_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM inventory_items WHERE sku = $1")
                .bind(&input.sku)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let Some(item_id) = item_id else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.inventory_item_not_found",
                "inventory item does not exist",
                RetryDisposition::Never,
                "inventory_reservation",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        };
        let location_id = input.location_id.unwrap_or(1);
        let balance: Option<(rust_decimal::Decimal, i32)> = sqlx::query_as(
            "SELECT quantity_available, version FROM inventory_balances
             WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let Some((mut effective_available, version_before)) = balance else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.inventory_balance_not_found",
                "inventory balance does not exist at the requested location",
                RetryDisposition::Never,
                "inventory_reservation",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        };
        let (expired_quantity, expired_count): (rust_decimal::Decimal, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(quantity), 0), COUNT(*) FROM inventory_reservations
             WHERE item_id = $1 AND location_id = $2
               AND status IN ('pending', 'confirmed', 'allocated')
               AND expires_at IS NOT NULL AND expires_at < $3",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(started_at)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        effective_available += expired_quantity;

        if command.expected_version.is_some_and(|expected| expected != version_before) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "kernel.version_conflict",
                "inventory balance version does not match expected_version",
                RetryDisposition::AfterConflict,
                "inventory_reservation",
            );
            receipt.version_before = Some(version_before);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }
        if effective_available < input.quantity {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.insufficient_stock",
                &format!("requested {}, available {}", input.quantity, effective_available),
                RetryDisposition::Never,
                "inventory_reservation",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "inventory_reservation");
            receipt.version_before = Some(version_before);
            let expired_count = i32::try_from(expired_count).map_err(|error| {
                CommerceError::DatabaseError(format!("expired reservation count overflow: {error}"))
            })?;
            receipt.version_after = Some(version_before + expired_count + 1);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let inventory = PgInventoryRepository::new(self.pool.clone());
        let (reservation, event_id) = inventory.reserve_in_tx(&mut tx, input).await?;
        sqlx::query(
            "UPDATE kernel_outbox SET command_id = $1, idempotency_key = $2,
                    principal_type = $3, principal_id = $4, correlation_id = $5, causation_id = $6
             WHERE id = $7",
        )
        .bind(command.command_id)
        .bind(&command.idempotency_key)
        .bind(principal_kind_name(command))
        .bind(&command.principal.id)
        .bind(command.correlation_id)
        .bind(command.causation_id)
        .bind(event_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let version_after: i32 = sqlx::query_scalar(
            "SELECT version FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
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
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically confirm all or part of a reservation.
    pub async fn execute_confirm_inventory_reservation_async(
        &self,
        command: &CommandEnvelope<ConfirmInventoryReservation>,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        self.execute_inventory_lifecycle_async(
            command,
            command.payload.reservation_id,
            CONFIRM_RESERVATION_COMMAND,
            InventoryLifecycleAction::Confirm(command.payload.quantity),
        )
        .await
    }

    /// Preview or atomically release a reservation.
    pub async fn execute_release_inventory_reservation_async(
        &self,
        command: &CommandEnvelope<ReleaseInventoryReservation>,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        self.execute_inventory_lifecycle_async(
            command,
            command.payload.reservation_id,
            RELEASE_RESERVATION_COMMAND,
            InventoryLifecycleAction::Release,
        )
        .await
    }

    async fn execute_inventory_lifecycle_async<C: Serialize>(
        &self,
        command: &CommandEnvelope<C>,
        reservation_id: Uuid,
        expected_command_type: &str,
        action: InventoryLifecycleAction,
    ) -> Result<ExecutionReceipt<InventoryReservation>> {
        let confirm_quantity = match action {
            InventoryLifecycleAction::Confirm(quantity) => quantity,
            InventoryLifecycleAction::Release => None,
        };
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::aggregate(expected_command_type),
            "inventory_reservation",
        )?
        .then_guard(|_| reservation_lifecycle_guard(reservation_id, confirm_quantity));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) = replay_or_conflict(
                tx.as_mut(),
                command,
                &request_hash,
                existing,
                "inventory_reservation",
            )
            .await?
            {
                tx.commit()
                    .await
                    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let row = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let Some(row) = row else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.reservation_not_found",
                "inventory reservation does not exist",
                RetryDisposition::Never,
                "inventory_reservation",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        };
        let reservation = PgInventoryRepository::row_to_reservation(row)?;
        let version_before: i32 = sqlx::query_scalar(
            "SELECT version FROM inventory_balances
             WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(reservation.item_id)
        .bind(reservation.location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        if command.expected_version.is_some_and(|expected| expected != version_before) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "kernel.version_conflict",
                "inventory balance version does not match expected_version",
                RetryDisposition::AfterConflict,
                "inventory_reservation",
            );
            receipt.version_before = Some(version_before);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
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
                Some(policy),
                "commerce.reservation_not_confirmable",
                "released or cancelled reservations cannot be confirmed",
                RetryDisposition::Never,
                "inventory_reservation",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }
        if matches!(action, InventoryLifecycleAction::Confirm(Some(_)))
            && reservation.status == ReservationStatus::Confirmed
        {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.reservation_not_confirmable",
                "an already-confirmed reservation cannot be partially confirmed",
                RetryDisposition::Never,
                "inventory_reservation",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let expires_during_apply = reservation.expires_at.is_some_and(|expiry| expiry < started_at)
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
            let mut receipt = preview_receipt(command, policy, "inventory_reservation");
            receipt.result = Some(reservation);
            receipt.version_before = Some(version_before);
            receipt.version_after =
                Some(version_before + i32::from(expires_during_apply || releases_balance));
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            return Ok(receipt);
        }

        let inventory = PgInventoryRepository::new(self.pool.clone());
        match action {
            InventoryLifecycleAction::Confirm(Some(quantity)) => {
                inventory
                    .confirm_reservation_quantity_in_tx_with_now(
                        &mut tx,
                        reservation_id,
                        quantity,
                        started_at,
                    )
                    .await?;
            }
            InventoryLifecycleAction::Confirm(None) => {
                inventory
                    .confirm_reservation_in_tx_with_now(&mut tx, reservation_id, started_at)
                    .await?;
            }
            InventoryLifecycleAction::Release => {
                inventory.release_reservation_in_tx(&mut tx, reservation_id).await?;
            }
        }

        let event_types: Vec<&str> = match action {
            InventoryLifecycleAction::Confirm(_) => {
                vec!["inventory.reservation_confirmed.v1", "inventory.reservation_expired.v1"]
            }
            InventoryLifecycleAction::Release => {
                vec!["inventory.reservation_released.v1", "inventory.reservation_expired.v1"]
            }
        };
        let event: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, aggregate_id FROM kernel_outbox
             WHERE created_at >= $1 AND event_type = ANY($2)
               AND (aggregate_id = $3 OR payload->>'source_reservation_id' = $3)
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(started_at)
        .bind(&event_types)
        .bind(reservation_id.to_string())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        if let Some((event_id, _)) = &event {
            sqlx::query(
                "UPDATE kernel_outbox SET command_id = $1, idempotency_key = $2,
                        principal_type = $3, principal_id = $4, correlation_id = $5, causation_id = $6
                 WHERE id = $7",
            )
            .bind(command.command_id)
            .bind(&command.idempotency_key)
            .bind(principal_kind_name(command))
            .bind(&command.principal.id)
            .bind(command.correlation_id)
            .bind(command.causation_id)
            .bind(event_id)
            .execute(tx.as_mut())
            .await
            .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        }
        let result_id = event
            .as_ref()
            .and_then(|(_, aggregate_id)| Uuid::parse_str(aggregate_id).ok())
            .unwrap_or(reservation_id);
        let result_row = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1",
        )
        .bind(result_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let result = PgInventoryRepository::row_to_reservation(result_row)?;
        let version_after: i32 = sqlx::query_scalar(
            "SELECT version FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(reservation.item_id)
        .bind(reservation.location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
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
            event_ids: event.as_ref().map(|(id, _)| *id).into_iter().collect(),
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically apply an order state-machine transition.
    ///
    /// Cancellations honour the same money rule as
    /// `OrderRepository::update`: captured money must be refunded (or
    /// `void_payments` set to void in-flight payments) before the order can
    /// be cancelled, and every inventory hold is released atomically.
    pub async fn execute_transition_order_async(
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
        let order_uuid = command.payload.order_id.into_uuid();
        let order_id = order_uuid.to_string();

        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
            && let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "order").await?
        {
            tx.commit().await.map_err(pg_err)?;
            return Ok(stored);
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }

        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1 FOR UPDATE")
            .bind(order_uuid)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(pg_err)?;
        let snapshot = match row {
            Some(row) => {
                let order = load_pg_order(tx.as_mut(), order_uuid, row).await?;
                let open_captures = if command.payload.status == OrderStatus::Cancelled {
                    open_captures_for_order_pg(tx.as_mut(), order_uuid).await?
                } else {
                    Vec::new()
                };
                Some(OrderTransitionSnapshot { order, open_captures })
            }
            None => None,
        };
        let effects = match plan_order_transition(command, snapshot.as_ref()) {
            PlanOutcome::Reject { rejection, version_before, aggregate_id } => {
                let mut receipt = run.rejected_by(&rejection);
                receipt.version_before = version_before;
                receipt.aggregate_id = aggregate_id;
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(pg_err)?;
                return Ok(receipt);
            }
            PlanOutcome::Proceed(effects) => effects,
        };
        let Some(OrderTransitionSnapshot { order, .. }) = snapshot else {
            return Err(CommerceError::Internal(
                "order transition planned without a loaded order".into(),
            ));
        };
        let version_before = effects.version_before;
        if run.is_preview() {
            let mut receipt = run.previewed();
            receipt.aggregate_id = Some(order_id.clone());
            receipt.result = Some(order);
            receipt.version_before = Some(version_before);
            receipt.version_after = Some(version_before + 1);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }

        let updated = sqlx::query("UPDATE orders SET status = $1, payment_status = $2, updated_at = $3, version = version + 1 WHERE id = $4 AND version = $5")
            .bind(effects.next_status.to_string()).bind(effects.next_payment_status.to_string()).bind(started_at)
            .bind(order_uuid).bind(version_before)
            .execute(tx.as_mut()).await.map_err(pg_err)?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".into(),
                id: order_id,
                expected_version: version_before,
            });
        }
        let mut related_event_ids = Vec::new();
        let mut voided_payment_ids = Vec::new();
        if effects.void_in_flight_payments {
            voided_payment_ids =
                void_in_flight_payments_for_order_pg(tx.as_mut(), order_uuid, started_at).await?;
        }
        if effects.release_holds {
            let inventory = PgInventoryRepository::new(self.pool.clone());
            let reservations = inventory
                .list_reservation_ids_by_reference_in_tx(&mut tx, "order", &order_id)
                .await?;
            for reservation_id in reservations {
                inventory.release_reservation_in_tx(&mut tx, reservation_id).await?;
                let event_id: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM kernel_outbox
                     WHERE event_type = 'inventory.reservation_released.v1'
                       AND aggregate_id = $1 AND created_at >= $2
                     ORDER BY created_at DESC, id DESC LIMIT 1",
                )
                .bind(reservation_id.to_string())
                .bind(started_at)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(pg_err)?;
                if let Some(event_id) = event_id {
                    sqlx::query(
                        "UPDATE kernel_outbox SET command_id = $1, idempotency_key = $2,
                                principal_type = $3, principal_id = $4, correlation_id = $5, causation_id = $6
                         WHERE id = $7",
                    )
                    .bind(command.command_id)
                    .bind(&command.idempotency_key)
                    .bind(principal_kind_name(command))
                    .bind(&command.principal.id)
                    .bind(command.correlation_id)
                    .bind(command.causation_id)
                    .bind(event_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(pg_err)?;
                    related_event_ids.push(event_id);
                }
            }
            PgBackorderRepository::new(self.pool.clone())
                .cancel_backorders_for_order_in_tx(&mut tx, order_uuid)
                .await?;
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        related_event_ids.push(event.id);
        let result_row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1")
            .bind(order_uuid)
            .fetch_one(tx.as_mut())
            .await
            .map_err(pg_err)?;
        let result = load_pg_order(tx.as_mut(), order_uuid, result_row).await?;
        let version_after = result.version;
        let mut receipt = run.succeeded(
            result,
            Some(order_id),
            Some(version_before),
            Some(version_after),
            related_event_ids,
        );
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(pg_err)?;
        Ok(receipt)
    }

    /// Preview or atomically ship all or selected order-line quantities.
    ///
    /// A reservation that expires while it is being confirmed rolls the
    /// shipment back to its savepoint and seals a
    /// `commerce.reservation_expired` rejection instead of failing the call.
    pub async fn execute_ship_order_async(
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
        let order_uuid = command.payload.order_id.into_uuid();
        let order_id = order_uuid.to_string();

        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
            && let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "order").await?
        {
            tx.commit().await.map_err(pg_err)?;
            return Ok(stored);
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }
        let lines = command.payload.lines.as_deref().unwrap_or_default();
        let mode = if lines.is_empty() { ShipMode::All } else { ShipMode::Lines(lines) };
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1 FOR UPDATE")
            .bind(order_uuid)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(pg_err)?;
        let snapshot = match row {
            Some(row) => {
                let order = load_pg_order(tx.as_mut(), order_uuid, row).await?;
                let shipment = PgOrderRepository::plan_shipment_in_tx(&mut tx, order_uuid, mode)
                    .await
                    .map_err(|error| error.to_string());
                let expired: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM inventory_reservations WHERE reference_type = 'order' AND reference_id = $1
                       AND status IN ('pending', 'confirmed', 'allocated') AND expires_at IS NOT NULL AND expires_at < $2 LIMIT 1")
                    .bind(&order_id).bind(started_at).fetch_optional(tx.as_mut()).await
                    .map_err(pg_err)?;
                Some(ShipOrderSnapshot { order, shipment, expired_reservation: expired.is_some() })
            }
            None => None,
        };
        let effects = match plan_ship_order(command, snapshot.as_ref()) {
            PlanOutcome::Reject { rejection, version_before, aggregate_id } => {
                let mut receipt = run.rejected_by(&rejection);
                receipt.version_before = version_before;
                receipt.aggregate_id = aggregate_id;
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(pg_err)?;
                return Ok(receipt);
            }
            PlanOutcome::Proceed(effects) => effects,
        };
        let Some(ShipOrderSnapshot { order, .. }) = snapshot else {
            return Err(CommerceError::Internal("shipment planned without a loaded order".into()));
        };
        let version_before = effects.version_before;
        if run.is_preview() {
            let mut receipt = run.previewed();
            receipt.aggregate_id = Some(order_id.clone());
            receipt.result = Some(order);
            receipt.version_before = Some(version_before);
            receipt.version_after = Some(version_before + 1);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }

        let inventory = PgInventoryRepository::new(self.pool.clone());
        let reservation_ids =
            inventory.list_reservation_ids_by_reference_in_tx(&mut tx, "order", &order_id).await?;
        sqlx::query("SAVEPOINT kernel_ship").execute(tx.as_mut()).await.map_err(pg_err)?;
        let mut expired_during_shipment = false;
        if lines.is_empty() {
            for reservation_id in &reservation_ids {
                if inventory
                    .confirm_reservation_in_tx_with_now(&mut tx, *reservation_id, started_at)
                    .await?
                    == ReservationConfirmOutcome::Expired
                {
                    expired_during_shipment = true;
                    break;
                }
            }
        } else {
            'deltas: for delta in effects.deltas.iter().filter(|d| d.delta > 0) {
                let mut remaining = rust_decimal::Decimal::from(delta.delta);
                let open = inventory
                    .list_open_reservations_for_sku_in_tx(&mut tx, "order", &order_id, &delta.sku)
                    .await?;
                for (reservation_id, reserved) in open {
                    if remaining <= rust_decimal::Decimal::ZERO {
                        continue 'deltas;
                    }
                    let take = remaining.min(reserved);
                    if inventory
                        .confirm_reservation_quantity_in_tx_with_now(
                            &mut tx,
                            reservation_id,
                            take,
                            started_at,
                        )
                        .await?
                        == ReservationConfirmOutcome::Expired
                    {
                        expired_during_shipment = true;
                        break 'deltas;
                    }
                    remaining -= take;
                }
            }
        }
        if expired_during_shipment {
            sqlx::query("ROLLBACK TO SAVEPOINT kernel_ship")
                .execute(tx.as_mut())
                .await
                .map_err(pg_err)?;
            sqlx::query("RELEASE SAVEPOINT kernel_ship")
                .execute(tx.as_mut())
                .await
                .map_err(pg_err)?;
            let mut receipt = run.rejected_by(&reservation_expired_during_shipment());
            receipt.aggregate_id = Some(order_id.clone());
            receipt.version_before = Some(version_before);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }
        sqlx::query("RELEASE SAVEPOINT kernel_ship").execute(tx.as_mut()).await.map_err(pg_err)?;
        for delta in effects.deltas.iter().filter(|d| d.delta > 0) {
            sqlx::query(
                "UPDATE order_items SET shipped_quantity = shipped_quantity + $1 WHERE id = $2",
            )
            .bind(delta.delta)
            .bind(delta.item_id)
            .execute(tx.as_mut())
            .await
            .map_err(pg_err)?;
        }
        let updated = sqlx::query("UPDATE orders SET status = $1, tracking_number = COALESCE($2, tracking_number), updated_at = $3, version = version + 1 WHERE id = $4 AND version = $5")
            .bind(effects.resolved_status.to_string()).bind(&command.payload.tracking_number).bind(started_at)
            .bind(order_uuid).bind(version_before).execute(tx.as_mut()).await
            .map_err(pg_err)?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".into(),
                id: order_id,
                expected_version: version_before,
            });
        }
        let mut event_ids = Vec::new();
        for reservation_id in reservation_ids {
            let ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM kernel_outbox WHERE created_at >= $1 AND event_type = 'inventory.reservation_confirmed.v1'
                   AND (aggregate_id = $2 OR payload->>'source_reservation_id' = $2) ORDER BY created_at, id")
                .bind(started_at).bind(reservation_id.to_string()).fetch_all(tx.as_mut()).await
                .map_err(pg_err)?;
            for event_id in ids {
                sqlx::query("UPDATE kernel_outbox SET command_id = $1, idempotency_key = $2, principal_type = $3, principal_id = $4, correlation_id = $5, causation_id = $6 WHERE id = $7")
                    .bind(command.command_id).bind(&command.idempotency_key).bind(principal_kind_name(command)).bind(&command.principal.id)
                    .bind(command.correlation_id).bind(command.causation_id).bind(event_id).execute(tx.as_mut()).await
                    .map_err(pg_err)?;
                if !event_ids.contains(&event_id) {
                    event_ids.push(event_id);
                }
            }
        }
        let event = run.event(
            "orders.updated.v1",
            "order",
            order_id.clone(),
            serde_json::json!({"order_id": order_id, "status_before": effects.status_before.to_string(),
                "status_after": effects.resolved_status.to_string(), "payment_status_before": order.payment_status.to_string(),
                "payment_status_after": order.payment_status.to_string(), "fulfillment_status_after": order.fulfillment_status.to_string(),
                "version_before": version_before, "version_after": version_before + 1, "total_amount": order.total_amount.to_string()}),
        );
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        event_ids.push(event.id);
        let result_row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1")
            .bind(order_uuid)
            .fetch_one(tx.as_mut())
            .await
            .map_err(pg_err)?;
        let result = load_pg_order(tx.as_mut(), order_uuid, result_row).await?;
        let version_after = result.version;
        let mut receipt = run.succeeded(
            result,
            Some(order_id),
            Some(version_before),
            Some(version_after),
            event_ids,
        );
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(pg_err)?;
        Ok(receipt)
    }

    /// Preview or atomically apply a return state-machine transition.
    pub async fn execute_transition_return_async(
        &self,
        command: &CommandEnvelope<TransitionReturn>,
    ) -> Result<ExecutionReceipt<Return>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::aggregate(TRANSITION_RETURN_COMMAND),
            "return",
        )?
        .then_guard(|_| transition_return_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "return").await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1 FOR UPDATE")
            .bind(command.payload.return_id.into_uuid())
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let Some(row) = row else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.return_not_found",
                "return does not exist",
                RetryDisposition::Never,
                "return",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let items = sqlx::query_as::<_, ReturnItemRow>(
            "SELECT * FROM return_items WHERE return_id = $1 ORDER BY id",
        )
        .bind(command.payload.return_id.into_uuid())
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?
        .into_iter()
        .map(PgReturnRepository::row_to_item)
        .collect::<Result<Vec<_>>>()?;
        let returned = PgReturnRepository::row_to_return(row, items)?;
        let version_before = returned.version;
        if command.expected_version.is_some_and(|v| v != version_before) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "kernel.version_conflict",
                "return version does not match expected_version",
                RetryDisposition::AfterConflict,
                "return",
            );
            receipt.version_before = Some(version_before);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if !returned.status.can_transition_to(command.payload.status) {
            let message = format!(
                "return cannot transition from {} to {}",
                returned.status, command.payload.status
            );
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.invalid_return_status_transition",
                &message,
                RetryDisposition::Never,
                "return",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "return");
            receipt.result = Some(returned);
            receipt.version_before = Some(version_before);
            receipt.version_after = Some(version_before + 1);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let row = sqlx::query_as::<_, ReturnRow>(
            "UPDATE returns SET status = $1, updated_at = $2, version = version + 1
             WHERE id = $3 AND version = $4 RETURNING *",
        )
        .bind(command.payload.status.to_string())
        .bind(started_at)
        .bind(command.payload.return_id.into_uuid())
        .bind(version_before)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?
        .ok_or_else(|| CommerceError::VersionConflict {
            entity: "return".into(),
            id: command.payload.return_id.to_string(),
            expected_version: version_before,
        })?;
        let mut event = KernelOutboxEvent::domain(
            "returns.updated.v1",
            "return",
            command.payload.return_id.to_string(),
            serde_json::json!({"return_id": command.payload.return_id.to_string(), "status_before": returned.status.to_string(),
                "status_after": command.payload.status.to_string(), "version_before": version_before, "version_after": version_before + 1,
                "refund_amount": returned.refund_amount.map(|amount| amount.to_string())}),
            Some(command.idempotency_key.clone()),
        );
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let items = sqlx::query_as::<_, ReturnItemRow>(
            "SELECT * FROM return_items WHERE return_id = $1 ORDER BY id",
        )
        .bind(command.payload.return_id.into_uuid())
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?
        .into_iter()
        .map(PgReturnRepository::row_to_item)
        .collect::<Result<Vec<_>>>()?;
        let result = PgReturnRepository::row_to_return(row, items)?;
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
            aggregate_type: Some("return".into()),
            aggregate_id: Some(result.id.to_string()),
            version_before: Some(version_before),
            version_after: Some(result.version),
            event_ids: vec![event.id],
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically create an A2A escrow in `created` status.
    pub async fn execute_create_a2a_escrow_async(
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
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
            && let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_escrow")
                    .await?
        {
            tx.commit().await.map_err(pg_err)?;
            return Ok(stored);
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }
        if run.is_preview() {
            let mut receipt = run.previewed();
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
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
        sqlx::query(
            "INSERT INTO a2a_escrows (
                id, status, quote_id, payment_id, buyer_address, seller_address,
                amount, amount_decimal, asset, network, release_conditions,
                funded_at, released_at, disputed_at, dispute_id, expires_at,
                auto_release_after, metadata, created_at, updated_at, tenant_id, store_id
             ) VALUES ($1, 'created', $2, $3, $4, $5, $6, $7, $8, $9, $10,
                       NULL, NULL, NULL, NULL, $11, $12, $13, $14, $14, $15, $16)",
        )
        .bind(&created.id)
        .bind(&created.quote_id)
        .bind(&created.payment_id)
        .bind(&created.buyer_address)
        .bind(&created.seller_address)
        .bind(created.amount)
        .bind(created.amount_decimal)
        .bind(&created.asset)
        .bind(&created.network)
        .bind(serde_json::Value::Array(created.release_conditions.clone()))
        .bind(created.expires_at)
        .bind(created.auto_release_after)
        .bind(&created.metadata)
        .bind(created.created_at)
        .bind(&created.tenant_id)
        .bind(&created.store_id)
        .execute(tx.as_mut())
        .await
        .map_err(pg_err)?;
        let mut event =
            a2a_transition_event_pg(command, &created, "a2a.escrow_created.v1", "created", None);
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = run.succeeded(created, Some(id), None, None, vec![event.id]);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(pg_err)?;
        Ok(receipt)
    }

    /// Preview or atomically move a created A2A escrow into active custody.
    pub async fn execute_fund_a2a_escrow_async(
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
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
            && let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_escrow")
                    .await?
        {
            tx.commit().await.map_err(pg_err)?;
            return Ok(stored);
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }
        let loaded = load_a2a_escrow_pg(
            tx.as_mut(),
            &command.payload.escrow_id,
            command.principal.tenant_id.as_deref().expect("policy validated tenant"),
            command.store_id.as_deref().expect("policy validated store"),
        )
        .await?
        .map(a2a_escrow_from_pg)
        .transpose()?;
        let escrow = match plan_fund_escrow(loaded, started_at) {
            PlanOutcome::Reject { rejection, aggregate_id, .. } => {
                let mut receipt = run.rejected_by(&rejection);
                receipt.aggregate_id = aggregate_id;
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(pg_err)?;
                return Ok(receipt);
            }
            PlanOutcome::Proceed(escrow) => escrow,
        };
        if run.is_preview() {
            let mut receipt = run.previewed();
            receipt.aggregate_id = Some(escrow.id.clone());
            receipt.result = Some(escrow);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(receipt);
        }
        let updated = sqlx::query(
            "UPDATE a2a_escrows SET status = 'active', funded_at = $1, updated_at = $1
             WHERE id = $2 AND status = 'created'",
        )
        .bind(started_at)
        .bind(&escrow.id)
        .execute(tx.as_mut())
        .await
        .map_err(pg_err)?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("A2A escrow was modified concurrently".into()));
        }
        let mut event =
            a2a_transition_event_pg(command, &escrow, "a2a.escrow_funded.v1", "active", None);
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let aggregate_id = escrow.id.clone();
        let mut receipt = run.succeeded(escrow, Some(aggregate_id), None, None, vec![event.id]);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(pg_err)?;
        Ok(receipt)
    }

    /// Preview or atomically freeze an active escrow for dispute resolution.
    pub async fn execute_dispute_a2a_escrow_async(
        &self,
        command: &CommandEnvelope<DisputeA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(DISPUTE_A2A_ESCROW_COMMAND, ESCROW_UNVERSIONED),
            "a2a_escrow",
        )?
        .then_guard(|_| dispute_escrow_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_escrow")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let Some(row) = load_a2a_escrow_pg(
            tx.as_mut(),
            &command.payload.escrow_id,
            command.principal.tenant_id.as_deref().expect("policy validated tenant"),
            command.store_id.as_deref().expect("policy validated store"),
        )
        .await?
        else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_found",
                "A2A escrow does not exist",
                RetryDisposition::Never,
                "a2a_escrow",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let mut escrow = a2a_escrow_from_pg(row)?;
        if !matches!(escrow.status, A2AEscrowStatus::Funded | A2AEscrowStatus::Active) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_disputable",
                &format!("cannot dispute escrow in {} status", escrow.status),
                RetryDisposition::Never,
                "a2a_escrow",
            );
            receipt.aggregate_id = Some(escrow.id);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        escrow.status = A2AEscrowStatus::Disputed;
        escrow.disputed_at = Some(started_at);
        escrow.updated_at = started_at;
        let mut metadata =
            escrow.metadata.take().and_then(|value| value.as_object().cloned()).unwrap_or_default();
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
            let mut receipt = preview_receipt(command, policy, "a2a_escrow");
            receipt.aggregate_id = Some(escrow.id.clone());
            receipt.result = Some(escrow);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let updated = sqlx::query(
            "UPDATE a2a_escrows
             SET status = 'disputed', disputed_at = $1, metadata = $2, updated_at = $1
             WHERE id = $3 AND status IN ('funded', 'active')",
        )
        .bind(started_at)
        .bind(&escrow.metadata)
        .bind(&escrow.id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("A2A escrow was modified concurrently".into()));
        }
        let mut event = a2a_transition_event_pg(
            command,
            &escrow,
            "a2a.escrow_disputed.v1",
            "disputed",
            Some(&command.payload.reason),
        );
        event.payload["category"] = serde_json::json!(command.payload.category);
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = succeeded_a2a_receipt_pg(command, policy, escrow, event.id, started_at);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically file a tenant-scoped dispute and freeze its escrow.
    pub async fn execute_file_a2a_dispute_async(
        &self,
        command: &CommandEnvelope<FileA2ADispute>,
    ) -> Result<ExecutionReceipt<A2ADispute>> {
        let input = &command.payload;
        let run = CommandRun::prepare(
            command,
            input,
            &self.policy,
            EnvelopeGuard::unversioned(FILE_A2A_DISPUTE_COMMAND, ESCROW_UNVERSIONED),
            "a2a_dispute",
        )?
        .then_guard(|run| {
            file_dispute_guard(
                input,
                run.started_at,
                principal_controls_address_pg(command, &input.claimant_address),
            )
        });
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_dispute")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let tenant_id = command.principal.tenant_id.as_deref().expect("policy validated tenant");
        let store_id = command.store_id.as_deref().expect("policy validated store");
        let Some(row) =
            load_a2a_escrow_pg(tx.as_mut(), &input.escrow_id, tenant_id, store_id).await?
        else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_found",
                "A2A escrow does not exist in the command scope",
                RetryDisposition::Never,
                "a2a_dispute",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let escrow = a2a_escrow_from_pg(row)?;
        if !matches!(escrow.status, A2AEscrowStatus::Funded | A2AEscrowStatus::Active) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_disputable",
                &format!("cannot file dispute for escrow in {} status", escrow.status),
                RetryDisposition::Never,
                "a2a_dispute",
            );
            receipt.aggregate_id = Some(escrow.id);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let respondent_address = if input.claimant_address == escrow.buyer_address {
            escrow.seller_address.clone()
        } else if input.claimant_address == escrow.seller_address {
            escrow.buyer_address.clone()
        } else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.dispute.claimant_not_participant",
                "claimant must be the escrow buyer or seller",
                RetryDisposition::Never,
                "a2a_dispute",
            );
            receipt.aggregate_id = Some(escrow.id);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
            let mut receipt = preview_receipt(command, policy, "a2a_dispute");
            receipt.aggregate_id = Some(dispute.id.clone());
            receipt.result = Some(dispute);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        sqlx::query(
            "INSERT INTO a2a_disputes (
                id, tenant_id, store_id, status, escrow_id, quote_id,
                claimant_address, respondent_address, reason, category,
                amount_decimal, asset, evidence_deadline, review_deadline,
                metadata, created_at, updated_at
             ) VALUES ($1, $2, $3, 'filed', $4, $5, $6, $7, $8, $9,
                       $10, $11, $12, $13, $14, $15, $15)",
        )
        .bind(&dispute.id)
        .bind(&dispute.tenant_id)
        .bind(&dispute.store_id)
        .bind(&dispute.escrow_id)
        .bind(&dispute.quote_id)
        .bind(&dispute.claimant_address)
        .bind(&dispute.respondent_address)
        .bind(&dispute.reason)
        .bind(&dispute.category)
        .bind(dispute.amount)
        .bind(&dispute.asset)
        .bind(dispute.evidence_deadline)
        .bind(dispute.review_deadline)
        .bind(&dispute.metadata)
        .bind(dispute.created_at)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let updated = sqlx::query(
            "UPDATE a2a_escrows
             SET status = 'disputed', disputed_at = $1, dispute_id = $2, updated_at = $1
             WHERE id = $3 AND tenant_id = $4 AND store_id = $5
               AND status IN ('funded', 'active')",
        )
        .bind(started_at)
        .bind(&dispute.id)
        .bind(&escrow.id)
        .bind(tenant_id)
        .bind(store_id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("A2A escrow was modified concurrently".into()));
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let aggregate_id = dispute.id.clone();
        let mut receipt = succeeded_kernel_receipt_pg(
            command,
            policy,
            dispute,
            "a2a_dispute",
            aggregate_id,
            vec![event.id],
            started_at,
        );
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically append immutable, content-addressed dispute evidence.
    pub async fn execute_submit_a2a_dispute_evidence_async(
        &self,
        command: &CommandEnvelope<SubmitA2ADisputeEvidence>,
    ) -> Result<ExecutionReceipt<A2ADisputeEvidence>> {
        let input = &command.payload;
        let run = CommandRun::prepare(
            command,
            input,
            &self.policy,
            EnvelopeGuard::unversioned(SUBMIT_A2A_EVIDENCE_COMMAND, ESCROW_UNVERSIONED),
            "a2a_dispute_evidence",
        )?
        .then_guard(|_| {
            submit_evidence_guard(
                input,
                principal_controls_address_pg(command, &input.submitted_by),
            )
        });
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) = replay_or_conflict(
                tx.as_mut(),
                command,
                &request_hash,
                existing,
                "a2a_dispute_evidence",
            )
            .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let tenant_id = command.principal.tenant_id.as_deref().expect("policy validated tenant");
        let store_id = command.store_id.as_deref().expect("policy validated store");
        let Some(row) =
            load_a2a_dispute_pg(tx.as_mut(), &input.dispute_id, tenant_id, store_id).await?
        else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.dispute_not_found",
                "A2A dispute does not exist in the command scope",
                RetryDisposition::Never,
                "a2a_dispute_evidence",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let dispute = a2a_dispute_from_pg(row)?;
        if !matches!(
            dispute.status,
            A2ADisputeStatus::Filed
                | A2ADisputeStatus::EvidencePeriod
                | A2ADisputeStatus::UnderReview
        ) || started_at > dispute.evidence_deadline
        {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.dispute.evidence_closed",
                "evidence is closed for this dispute",
                RetryDisposition::Never,
                "a2a_dispute_evidence",
            );
            receipt.aggregate_id = Some(dispute.id);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if input.submitted_by != dispute.claimant_address
            && input.submitted_by != dispute.respondent_address
        {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.dispute.submitter_not_participant",
                "evidence submitter must be a dispute participant",
                RetryDisposition::Never,
                "a2a_dispute_evidence",
            );
            receipt.aggregate_id = Some(dispute.id);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
            let mut receipt = preview_receipt(command, policy, "a2a_dispute_evidence");
            receipt.aggregate_id = Some(evidence.id.clone());
            receipt.result = Some(evidence);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        sqlx::query(
            "INSERT INTO a2a_dispute_evidence (
                id, tenant_id, store_id, dispute_id, submitted_by, evidence_type,
                title, description, content, content_hash, created_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(&evidence.id)
        .bind(&evidence.tenant_id)
        .bind(&evidence.store_id)
        .bind(&evidence.dispute_id)
        .bind(&evidence.submitted_by)
        .bind(&evidence.evidence_type)
        .bind(&evidence.title)
        .bind(&evidence.description)
        .bind(&evidence.content)
        .bind(&evidence.content_hash)
        .bind(evidence.created_at)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        sqlx::query(
            "UPDATE a2a_disputes
             SET status = CASE WHEN status = 'filed' THEN 'evidence_period' ELSE status END,
                 updated_at = $1
             WHERE id = $2 AND tenant_id = $3 AND store_id = $4",
        )
        .bind(started_at)
        .bind(&dispute.id)
        .bind(tenant_id)
        .bind(store_id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let aggregate_id = dispute.id;
        let mut receipt = succeeded_kernel_receipt_pg(
            command,
            policy,
            evidence,
            "a2a_dispute_evidence",
            aggregate_id,
            vec![event.id],
            started_at,
        );
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically resolve a dispute and move its escrow in the same transaction.
    pub async fn execute_resolve_a2a_dispute_async(
        &self,
        command: &CommandEnvelope<ResolveA2ADispute>,
    ) -> Result<ExecutionReceipt<A2ADisputeResolution>> {
        let input = &command.payload;
        let run = CommandRun::prepare(
            command,
            input,
            &self.policy,
            EnvelopeGuard::unversioned(RESOLVE_A2A_DISPUTE_COMMAND, ESCROW_UNVERSIONED),
            "a2a_dispute",
        )?
        .then_guard(|_| resolve_dispute_guard(input));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_dispute")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let tenant_id = command.principal.tenant_id.as_deref().expect("policy validated tenant");
        let store_id = command.store_id.as_deref().expect("policy validated store");
        let Some(row) =
            load_a2a_dispute_pg(tx.as_mut(), &input.dispute_id, tenant_id, store_id).await?
        else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy.clone()),
                "commerce.a2a.dispute_not_found",
                "A2A dispute does not exist in the command scope",
                RetryDisposition::Never,
                "a2a_dispute",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let mut dispute = a2a_dispute_from_pg(row)?;
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
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let Some(escrow_row) =
            load_a2a_escrow_pg(tx.as_mut(), &dispute.escrow_id, tenant_id, store_id).await?
        else {
            return Err(CommerceError::DatabaseError(
                "scoped dispute references a missing escrow".into(),
            ));
        };
        let mut escrow = a2a_escrow_from_pg(escrow_row)?;
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
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
                        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                        tx.commit()
                            .await
                            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
                    append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                    tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
            let mut receipt = preview_receipt(command, policy, "a2a_dispute");
            receipt.aggregate_id = Some(dispute.id.clone());
            receipt.result = Some(result);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        sqlx::query(
            "UPDATE a2a_disputes SET status = $1, resolution_type = $2,
                    buyer_amount_decimal = $3, seller_amount_decimal = $4,
                    resolution_note = $5, resolved_by = $6, resolved_at = $7, updated_at = $8
             WHERE id = $9 AND tenant_id = $10 AND store_id = $11
               AND status IN ('filed', 'evidence_period', 'under_review', 'escalated')",
        )
        .bind(dispute.status.to_string())
        .bind(dispute.resolution_type.map(|value| value.to_string()))
        .bind(dispute.buyer_amount)
        .bind(dispute.seller_amount)
        .bind(&dispute.resolution_note)
        .bind(&dispute.resolved_by)
        .bind(dispute.resolved_at)
        .bind(started_at)
        .bind(&dispute.id)
        .bind(tenant_id)
        .bind(store_id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        sqlx::query(
            "UPDATE a2a_escrows SET status = $1, released_at = $2, updated_at = $3
             WHERE id = $4 AND tenant_id = $5 AND store_id = $6
               AND status = 'disputed' AND dispute_id = $7",
        )
        .bind(escrow.status.to_string())
        .bind(escrow.released_at)
        .bind(started_at)
        .bind(&escrow.id)
        .bind(tenant_id)
        .bind(store_id)
        .bind(&dispute.id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut dispute_event = KernelOutboxEvent::domain(
            if final_resolution { "a2a.dispute_resolved.v1" } else { "a2a.dispute_escalated.v1" },
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
        append_kernel_event_tx(tx.as_mut(), &dispute_event).await?;
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
            append_kernel_event_tx(tx.as_mut(), &escrow_event).await?;
            event_ids.push(escrow_event.id);
        }
        let aggregate_id = dispute.id.clone();
        let mut receipt = succeeded_kernel_receipt_pg(
            command,
            policy,
            result,
            "a2a_dispute",
            aggregate_id,
            event_ids,
            started_at,
        );
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically return escrowed value to its buyer.
    pub async fn execute_refund_a2a_escrow_async(
        &self,
        command: &CommandEnvelope<RefundA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(REFUND_A2A_ESCROW_COMMAND, ESCROW_UNVERSIONED),
            "a2a_escrow",
        )?
        .then_guard(|_| escrow_settlement_guard(&command.payload.escrow_id));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_escrow")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let Some(row) = load_a2a_escrow_pg(
            tx.as_mut(),
            &command.payload.escrow_id,
            command.principal.tenant_id.as_deref().expect("policy validated tenant"),
            command.store_id.as_deref().expect("policy validated store"),
        )
        .await?
        else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_found",
                "A2A escrow does not exist",
                RetryDisposition::Never,
                "a2a_escrow",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let mut escrow = a2a_escrow_from_pg(row)?;
        if !matches!(
            escrow.status,
            A2AEscrowStatus::Created
                | A2AEscrowStatus::Funded
                | A2AEscrowStatus::Active
                | A2AEscrowStatus::Disputed
        ) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_refundable",
                &format!("cannot refund escrow in {} status", escrow.status),
                RetryDisposition::Never,
                "a2a_escrow",
            );
            receipt.aggregate_id = Some(escrow.id);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        escrow.status = A2AEscrowStatus::Refunded;
        escrow.updated_at = started_at;
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "a2a_escrow");
            receipt.aggregate_id = Some(escrow.id.clone());
            receipt.result = Some(escrow);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let updated = sqlx::query(
            "UPDATE a2a_escrows SET status = 'refunded', updated_at = $1
             WHERE id = $2 AND status IN ('created', 'funded', 'active', 'disputed')",
        )
        .bind(started_at)
        .bind(&escrow.id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("A2A escrow was modified concurrently".into()));
        }
        let mut event = a2a_transition_event_pg(
            command,
            &escrow,
            "a2a.escrow_refunded.v1",
            "refunded",
            command.payload.reason.as_deref(),
        );
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = succeeded_a2a_receipt_pg(command, policy, escrow, event.id, started_at);
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically release an A2A escrow whose conditions are met.
    pub async fn execute_release_a2a_escrow_async(
        &self,
        command: &CommandEnvelope<ReleaseA2AEscrow>,
    ) -> Result<ExecutionReceipt<A2AEscrow>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(RELEASE_A2A_ESCROW_COMMAND, ESCROW_UNVERSIONED),
            "a2a_escrow",
        )?
        .then_guard(|_| escrow_settlement_guard(&command.payload.escrow_id));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "a2a_escrow")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let row = sqlx::query_as::<_, A2AEscrowRow>(
            "SELECT id, status, quote_id, payment_id, buyer_address, seller_address, amount,
                    amount_decimal, asset, network, release_conditions, funded_at, released_at,
                    disputed_at, dispute_id, expires_at, auto_release_after, metadata,
                    created_at, updated_at, tenant_id, store_id
                    FROM a2a_escrows
                    WHERE id = $1 AND tenant_id = $2 AND store_id = $3 FOR UPDATE",
        )
        .bind(&command.payload.escrow_id)
        .bind(command.principal.tenant_id.as_deref().expect("policy validated tenant"))
        .bind(command.store_id.as_deref().expect("policy validated store"))
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let Some(row) = row else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_found",
                "A2A escrow does not exist",
                RetryDisposition::Never,
                "a2a_escrow",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let escrow = a2a_escrow_from_pg(row)?;
        if !matches!(escrow.status, A2AEscrowStatus::Funded | A2AEscrowStatus::Active) {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_not_releasable",
                &format!("cannot release escrow in {} status", escrow.status),
                RetryDisposition::Never,
                "a2a_escrow",
            );
            receipt.aggregate_id = Some(escrow.id.clone());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if escrow.expires_at <= started_at {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_expired",
                "escrow has reached its expiry and must be refunded",
                RetryDisposition::Never,
                "a2a_escrow",
            );
            receipt.aggregate_id = Some(escrow.id.clone());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if !a2a_release_conditions_met_pg(tx.as_mut(), &escrow, started_at).await? {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.a2a.escrow_conditions_unmet",
                "not all escrow release conditions are met",
                RetryDisposition::Never,
                "a2a_escrow",
            );
            receipt.aggregate_id = Some(escrow.id.clone());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let mut released = escrow;
        released.status = A2AEscrowStatus::Released;
        released.released_at = Some(started_at);
        released.updated_at = started_at;
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "a2a_escrow");
            receipt.aggregate_id = Some(released.id.clone());
            receipt.result = Some(released);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let updated = sqlx::query(
            "UPDATE a2a_escrows SET status = 'released', released_at = $1, updated_at = $1
             WHERE id = $2 AND status IN ('funded', 'active')",
        )
        .bind(started_at)
        .bind(&released.id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("A2A escrow was modified concurrently".into()));
        }
        let mut event = KernelOutboxEvent::domain(
            "a2a.escrow_released.v1",
            "a2a_escrow",
            released.id.clone(),
            serde_json::json!({"escrow_id": released.id, "quote_id": released.quote_id,
                "payment_id": released.payment_id, "buyer_address": released.buyer_address,
                "seller_address": released.seller_address, "amount": released.amount.to_string(),
                "amount_decimal": released.amount_decimal.to_string(), "asset": released.asset,
                "network": released.network, "status": "released"}),
            Some(command.idempotency_key.clone()),
        );
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = ExecutionReceipt {
            contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
            receipt_id: Uuid::new_v4(),
            command_id: command.command_id,
            idempotency_key: command.idempotency_key.clone(),
            command_type: command.command_type.clone(),
            status: ExecutionStatus::Succeeded,
            result: Some(released.clone()),
            error_code: None,
            error_message: None,
            retry: RetryDisposition::SameKey,
            aggregate_type: Some("a2a_escrow".into()),
            aggregate_id: Some(released.id.clone()),
            version_before: None,
            version_after: None,
            event_ids: vec![event.id],
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically begin collecting a subscription billing cycle.
    pub async fn execute_charge_subscription_async(
        &self,
        command: &CommandEnvelope<ChargeSubscription>,
    ) -> Result<ExecutionReceipt<SubscriptionCharge>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(CHARGE_SUBSCRIPTION_COMMAND, BILLING_CYCLE_UNVERSIONED),
            "billing_cycle",
        )?
        .then_guard(|_| charge_subscription_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;

        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "billing_cycle")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let row = sqlx::query_as::<_, BillingCycleRow>(
            "SELECT id, subscription_id, cycle_number, status, period_start, period_end, billed_at,
                    subtotal, discount, tax, total, currency, payment_id, order_id, invoice_id,
                    failure_reason, retry_count, next_retry_at, created_at, updated_at
             FROM billing_cycles WHERE id = $1 FOR UPDATE",
        )
        .bind(command.payload.billing_cycle_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let Some(row) = row else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.subscription.billing_cycle_not_found",
                "billing cycle does not exist",
                RetryDisposition::Never,
                "billing_cycle",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let cycle = PgSubscriptionRepository::row_to_billing_cycle(row)?;
        let (subscription_status_raw, customer_id): (String, Uuid) =
            sqlx::query_as("SELECT status, customer_id FROM subscriptions WHERE id = $1")
                .bind(cycle.subscription_id.into_uuid())
                .fetch_one(tx.as_mut())
                .await
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let subscription_status: SubscriptionStatus =
            subscription_status_raw.parse().map_err(|error| {
                CommerceError::DatabaseError(format!("invalid subscription status: {error}"))
            })?;
        let status_allowed =
            matches!(cycle.status, BillingCycleStatus::Scheduled | BillingCycleStatus::Failed);
        let subscription_allowed =
            matches!(subscription_status, SubscriptionStatus::Active | SubscriptionStatus::PastDue);
        let retry_due = cycle.next_retry_at.is_none_or(|next_retry_at| next_retry_at <= started_at);
        if !status_allowed || !subscription_allowed || !retry_due {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.subscription.billing_cycle_not_chargeable",
                &format!(
                    "billing cycle in {} for {} subscription is not chargeable now",
                    cycle.status, subscription_status
                ),
                RetryDisposition::Never,
                "billing_cycle",
            );
            receipt.aggregate_id = Some(cycle.id.to_string());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if cycle.total <= rust_decimal::Decimal::ZERO {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.subscription.non_positive_charge",
                "billing cycle total must be positive before collection",
                RetryDisposition::Never,
                "billing_cycle",
            );
            receipt.aggregate_id = Some(cycle.id.to_string());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let payment_input = CreatePayment {
            customer_id: Some(customer_id.into()),
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
                Some(policy),
                error.invariant_code().unwrap_or("commerce.subscription.charge_validation_failed"),
                &error.to_string(),
                RetryDisposition::Never,
                "billing_cycle",
            );
            receipt.aggregate_id = Some(cycle.id.to_string());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "billing_cycle");
            receipt.aggregate_id = Some(cycle.id.to_string());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }

        let payment_id = Uuid::new_v4();
        let payment_number = stateset_core::generate_payment_number();
        sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
             payment_method, amount, currency, amount_refunded, external_id, idempotency_key,
             processor, card_brand, card_last4, card_exp_month, card_exp_year, billing_email,
             billing_name, billing_address, description, metadata, created_at, updated_at)
             VALUES ($1, $2, NULL, NULL, $3, 'pending', $4, $5, $6, 0, NULL, $7, $8, NULL,
                     NULL, NULL, NULL, NULL, NULL, NULL, $9, $10, $11, $11)",
        )
        .bind(payment_id)
        .bind(&payment_number)
        .bind(customer_id)
        .bind(command.payload.payment_method.to_string())
        .bind(cycle.total)
        .bind(cycle.currency)
        .bind(&command.idempotency_key)
        .bind(&command.payload.processor)
        .bind(&payment_input.description)
        .bind(&payment_input.metadata)
        .bind(started_at)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let updated = sqlx::query(
            "UPDATE billing_cycles SET status = 'processing', payment_id = $1,
             failure_reason = NULL, updated_at = $2
             WHERE id = $3 AND status IN ('scheduled', 'failed')",
        )
        .bind(payment_id)
        .bind(started_at)
        .bind(cycle.id)
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("billing cycle was modified concurrently".into()));
        }
        let payment_row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                    amount, currency, amount_refunded, external_id, idempotency_key, processor,
                    card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                    billing_address, description, failure_reason, failure_code, metadata, paid_at,
                    version, created_at, updated_at FROM payments WHERE id = $1",
        )
        .bind(payment_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let payment = PgPaymentRepository::row_to_payment(payment_row)?;
        let cycle_row = sqlx::query_as::<_, BillingCycleRow>(
            "SELECT id, subscription_id, cycle_number, status, period_start, period_end, billed_at,
                    subtotal, discount, tax, total, currency, payment_id, order_id, invoice_id,
                    failure_reason, retry_count, next_retry_at, created_at, updated_at
             FROM billing_cycles WHERE id = $1",
        )
        .bind(cycle.id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = SubscriptionCharge {
            billing_cycle: PgSubscriptionRepository::row_to_billing_cycle(cycle_row)?,
            payment,
        };
        let mut event = KernelOutboxEvent::domain(
            "subscriptions.charge_requested.v1",
            "billing_cycle",
            cycle.id.to_string(),
            serde_json::json!({"billing_cycle_id": cycle.id.to_string(),
                "subscription_id": cycle.subscription_id.to_string(), "payment_id": payment_id.to_string(),
                "amount": cycle.total.to_string(), "currency": cycle.currency.as_str(),
                "payment_method": command.payload.payment_method.to_string(),
                "processor": command.payload.processor, "status": "processing"}),
            Some(command.idempotency_key.clone()),
        );
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
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
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically commit a checkout-ready cart to an order.
    pub async fn execute_commit_checkout_async(
        &self,
        command: &CommandEnvelope<CommitCheckout>,
    ) -> Result<ExecutionReceipt<CheckoutResult>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(COMMIT_CHECKOUT_COMMAND, CART_UNVERSIONED),
            "checkout",
        )?
        .then_guard(|_| commit_checkout_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;

        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "checkout")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }

        if command.mode == ExecutionMode::Preview {
            let cart_repo = PgCartRepository::new(self.pool.clone());
            if let Err(error) = cart_repo
                .validate_checkout_in_tx(&mut tx, command.payload.cart_id.into_uuid())
                .await
            {
                if matches!(error, CommerceError::DatabaseError(_)) {
                    return Err(error);
                }
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    checkout_error_code(&error),
                    &error.to_string(),
                    RetryDisposition::Never,
                    "checkout",
                );
                receipt.aggregate_id = Some(command.payload.cart_id.to_string());
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(receipt);
            }
            let mut receipt = preview_receipt(command, policy, "checkout");
            receipt.aggregate_id = Some(command.payload.cart_id.to_string());
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }

        sqlx::query("SAVEPOINT kernel_checkout_apply")
            .execute(tx.as_mut())
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let cart_repo = PgCartRepository::new(self.pool.clone());
        let attempted = cart_repo
            .complete_checkout_in_tx(&mut tx, command.payload.cart_id.into_uuid(), false, false)
            .await;
        let checkout = match attempted {
            Ok(checkout) => checkout,
            Err(error) => {
                sqlx::query("ROLLBACK TO SAVEPOINT kernel_checkout_apply")
                    .execute(tx.as_mut())
                    .await
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                sqlx::query("RELEASE SAVEPOINT kernel_checkout_apply")
                    .execute(tx.as_mut())
                    .await
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                if matches!(error, CommerceError::DatabaseError(_)) {
                    return Err(error);
                }
                let code = checkout_error_code(&error);
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy.clone()),
                    code,
                    &error.to_string(),
                    RetryDisposition::Never,
                    "checkout",
                );
                receipt.aggregate_id = Some(command.payload.cart_id.to_string());
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(receipt);
            }
        };

        sqlx::query("RELEASE SAVEPOINT kernel_checkout_apply")
            .execute(tx.as_mut())
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut event = KernelOutboxEvent::domain(
            "checkout.committed.v1",
            "checkout",
            command.payload.cart_id.to_string(),
            serde_json::json!({"cart_id": checkout.cart_id.to_string(),
                "order_id": checkout.order_id.to_string(), "order_number": checkout.order_number,
                "total": checkout.total_charged.to_string(), "currency": checkout.currency.as_str(),
                "payment_status": "pending"}),
            Some(command.idempotency_key.clone()),
        );
        attach_command_context(&mut event, command);
        append_kernel_event_tx(tx.as_mut(), &event).await?;
        let mut receipt = ExecutionReceipt {
            contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
            receipt_id: Uuid::new_v4(),
            command_id: command.command_id,
            idempotency_key: command.idempotency_key.clone(),
            command_type: command.command_type.clone(),
            status: ExecutionStatus::Succeeded,
            result: Some(checkout.clone()),
            error_code: None,
            error_message: None,
            retry: RetryDisposition::SameKey,
            aggregate_type: Some("checkout".into()),
            aggregate_id: Some(command.payload.cart_id.to_string()),
            version_before: None,
            version_after: None,
            event_ids: vec![event.id],
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    pub async fn execute_post_journal_entry_async(
        &self,
        command: &CommandEnvelope<PostJournalEntry>,
    ) -> Result<ExecutionReceipt<JournalEntry>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(POST_LEDGER_COMMAND, JOURNAL_ENTRY_UNVERSIONED),
            "journal_entry",
        )?
        .then_guard(|_| post_journal_entry_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) =
                replay_or_conflict(tx.as_mut(), command, &request_hash, existing, "journal_entry")
                    .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let row = sqlx::query_as::<_, JournalEntryRow>(
            "SELECT * FROM gl_journal_entries WHERE id = $1 FOR UPDATE",
        )
        .bind(command.payload.journal_entry_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let Some(row) = row else {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.ledger.entry_not_found",
                "journal entry does not exist",
                RetryDisposition::Never,
                "journal_entry",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        };
        let lines = sqlx::query_as::<_, JournalEntryLineRow>(
            "SELECT * FROM gl_journal_entry_lines WHERE journal_entry_id = $1 ORDER BY line_number",
        )
        .bind(command.payload.journal_entry_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?
        .into_iter()
        .map(PgGeneralLedgerRepository::row_to_journal_entry_line)
        .collect();
        let mut entry = PgGeneralLedgerRepository::row_to_journal_entry(row)?;
        entry.lines = lines;
        if let Err(error) = entry.ensure_postable() {
            let code = error.invariant_code().unwrap_or("commerce.ledger.entry_not_postable");
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                code,
                &error.to_string(),
                RetryDisposition::Never,
                "journal_entry",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        // Same guard as `post_journal_entry_async`: the entry's period must be
        // open, or posting would mutate a closed/locked period's balances.
        let period_status: String =
            sqlx::query_scalar("SELECT status FROM gl_periods WHERE id = $1")
                .bind(entry.period_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if period_status != "open" {
            let mut receipt = rejected_receipt(
                command,
                Some(policy.clone()),
                "commerce.ledger.period_not_open",
                &format!("cannot post journal entry: its period is {period_status}, not open"),
                RetryDisposition::Never,
                "journal_entry",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        if command.mode == ExecutionMode::Preview {
            entry.status = JournalEntryStatus::Posted;
            entry.posted_at = Some(started_at);
            entry.posted_by = Some(command.payload.posted_by.clone());
            entry.updated_at = started_at;
            let mut receipt = preview_receipt(command, policy, "journal_entry");
            receipt.result = Some(entry);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let updated = sqlx::query("UPDATE gl_journal_entries SET status = 'posted', posted_at = $1, posted_by = $2 WHERE id = $3 AND status = 'draft'")
            .bind(started_at).bind(&command.payload.posted_by).bind(command.payload.journal_entry_id)
            .execute(tx.as_mut()).await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("Journal entry was modified concurrently".into()));
        }
        let ledger = PgGeneralLedgerRepository::new(self.pool.clone());
        for line in &entry.lines {
            ledger
                .update_account_balance_tx(
                    &mut tx,
                    line.account_id,
                    line.debit_amount,
                    line.credit_amount,
                )
                .await?;
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
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
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }

    /// Preview or atomically record confirmed x402 settlement.
    pub async fn execute_settle_x402_intent_async(
        &self,
        command: &CommandEnvelope<SettleX402Intent>,
    ) -> Result<ExecutionReceipt<X402PaymentIntent>> {
        let run = CommandRun::prepare(
            command,
            &command.payload,
            &self.policy,
            EnvelopeGuard::unversioned(SETTLE_X402_COMMAND, X402_INTENT_UNVERSIONED),
            "x402_payment_intent",
        )?
        .then_guard(|_| settle_x402_guard(&command.payload));
        let request_hash = run.request_hash.clone();
        let policy = run.policy.clone();
        let started_at = run.started_at;
        let mut tx =
            self.pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        lock_kernel_idempotency_pg(tx.as_mut(), &command.idempotency_key).await?;
        if let Some(existing) =
            receipt_by_idempotency_key_tx(tx.as_mut(), &command.idempotency_key).await?
        {
            if let Replay::Return(stored) = replay_or_conflict(
                tx.as_mut(),
                command,
                &request_hash,
                existing,
                "x402_payment_intent",
            )
            .await?
            {
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(stored);
            }
        }
        if let Some(mut receipt) = run.guard_receipt() {
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = $1 FOR UPDATE",
        )
        .bind(command.payload.intent_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut intent = match row {
            Some(row) => PgX402PaymentIntentRepository::row_to_intent(row)?,
            None => {
                let mut receipt = rejected_receipt(
                    command,
                    Some(policy),
                    "commerce.x402.intent_not_found",
                    "x402 payment intent does not exist",
                    RetryDisposition::Never,
                    "x402_payment_intent",
                );
                append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
                tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(receipt);
            }
        };
        if intent.status != X402IntentStatus::Sequenced {
            let mut receipt = rejected_receipt(
                command,
                Some(policy),
                "commerce.x402.intent_not_sequenced",
                &format!("cannot settle intent in {} status", intent.status),
                RetryDisposition::Never,
                "x402_payment_intent",
            );
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let settled_at = Utc::now();
        intent.status = X402IntentStatus::Settled;
        intent.tx_hash = Some(command.payload.tx_hash.clone());
        intent.block_number = Some(command.payload.block_number);
        intent.settled_at = Some(settled_at);
        intent.updated_at = settled_at;
        if command.mode == ExecutionMode::Preview {
            let mut receipt = preview_receipt(command, policy, "x402_payment_intent");
            receipt.aggregate_id = Some(intent.id.to_string());
            receipt.result = Some(intent);
            append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            return Ok(receipt);
        }
        let block_number = i64::try_from(command.payload.block_number)
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let updated = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, tx_hash = $2, block_number = $3,
             settled_at = $4, updated_at = $4 WHERE id = $5 AND status = $6",
        )
        .bind(X402IntentStatus::Settled.to_string())
        .bind(&command.payload.tx_hash)
        .bind(block_number)
        .bind(settled_at)
        .bind(command.payload.intent_id)
        .bind(X402IntentStatus::Sequenced.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict("x402 intent was modified concurrently".into()));
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
        append_kernel_event_tx(tx.as_mut(), &event).await?;
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
            policy: Some(policy),
            audit_hash: None,
            started_at,
            completed_at: Utc::now(),
        };
        append_receipt(tx.as_mut(), &request_hash, &mut receipt).await?;
        tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(receipt)
    }
}

/// Advisory-lock namespaces. Each key family hashes with its own seed so an
/// idempotency key can never collide with a catalog uniqueness key.
const LOCK_NS_IDEMPOTENCY: i64 = 0x5353_4B49_4445_4D50; // "SSKIDEMP"
const LOCK_NS_PRODUCT_SLUG: i64 = 0x5353_4B53_4C55_4700; // "SSKSLUG"
const LOCK_NS_PRODUCT_SKU: i64 = 0x5353_4B53_4B55_0000; // "SSKSKU"
const LOCK_NS_INVENTORY_SKU: i64 = 0x5353_4B49_4E56_534B; // "SSKINVSK"

fn pg_err(error: sqlx::Error) -> CommerceError {
    CommerceError::DatabaseError(error.to_string())
}

async fn load_pg_order(
    tx: &mut sqlx::PgConnection,
    order_id: Uuid,
    row: OrderRow,
) -> Result<Order> {
    let items = sqlx::query_as::<_, OrderItemRow>(
        "SELECT * FROM order_items WHERE order_id = $1 ORDER BY id",
    )
    .bind(order_id)
    .fetch_all(tx)
    .await
    .map_err(pg_err)?
    .into_iter()
    .map(PgOrderRepository::row_to_item)
    .collect();
    PgOrderRepository::row_to_order(row, items)
}

/// Transaction-scoped advisory lock on `key` within `namespace`.
async fn advisory_lock_pg(tx: &mut sqlx::PgConnection, namespace: i64, key: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
        .bind(key)
        .bind(namespace)
        .execute(tx)
        .await
        .map_err(pg_err)?;
    Ok(())
}

async fn lock_kernel_idempotency_pg(
    tx: &mut sqlx::PgConnection,
    idempotency_key: &str,
) -> Result<()> {
    advisory_lock_pg(tx, LOCK_NS_IDEMPOTENCY, idempotency_key).await
}

async fn load_a2a_escrow_pg(
    tx: &mut sqlx::PgConnection,
    escrow_id: &str,
    tenant_id: &str,
    store_id: &str,
) -> Result<Option<A2AEscrowRow>> {
    sqlx::query_as::<_, A2AEscrowRow>(
        "SELECT id, status, quote_id, payment_id, buyer_address, seller_address, amount,
                amount_decimal, asset, network, release_conditions, funded_at, released_at,
                disputed_at, dispute_id, expires_at, auto_release_after, metadata,
                created_at, updated_at, tenant_id, store_id
                FROM a2a_escrows
                WHERE id = $1 AND tenant_id = $2 AND store_id = $3 FOR UPDATE",
    )
    .bind(escrow_id)
    .bind(tenant_id)
    .bind(store_id)
    .fetch_optional(tx)
    .await
    .map_err(|e| CommerceError::DatabaseError(e.to_string()))
}

fn a2a_transition_event_pg<T>(
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

fn succeeded_a2a_receipt_pg<C>(
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

fn succeeded_kernel_receipt_pg<C, T>(
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

fn principal_controls_address_pg<C>(command: &CommandEnvelope<C>, address: &str) -> bool {
    command.principal.id == address || command.principal.delegated_by.as_deref() == Some(address)
}

fn a2a_dispute_from_pg(row: A2ADisputeRow) -> Result<A2ADispute> {
    let status = row.status.parse::<A2ADisputeStatus>().map_err(|error| {
        CommerceError::DatabaseError(format!("invalid A2A dispute status: {error}"))
    })?;
    let resolution_type = row
        .resolution_type
        .map(|value| value.parse::<A2ADisputeResolutionType>())
        .transpose()
        .map_err(|error| {
            CommerceError::DatabaseError(format!("invalid A2A resolution type: {error}"))
        })?;
    Ok(A2ADispute {
        id: row.id,
        tenant_id: row.tenant_id,
        store_id: row.store_id,
        status,
        escrow_id: row.escrow_id,
        quote_id: row.quote_id,
        claimant_address: row.claimant_address,
        respondent_address: row.respondent_address,
        reason: row.reason,
        category: row.category,
        amount: row.amount_decimal,
        asset: row.asset,
        resolution_type,
        buyer_amount: row.buyer_amount_decimal,
        seller_amount: row.seller_amount_decimal,
        resolution_note: row.resolution_note,
        resolved_by: row.resolved_by,
        evidence_deadline: row.evidence_deadline,
        review_deadline: row.review_deadline,
        metadata: row.metadata,
        created_at: row.created_at,
        updated_at: row.updated_at,
        resolved_at: row.resolved_at,
    })
}

async fn load_a2a_dispute_pg(
    tx: &mut sqlx::PgConnection,
    dispute_id: &str,
    tenant_id: &str,
    store_id: &str,
) -> Result<Option<A2ADisputeRow>> {
    sqlx::query_as::<_, A2ADisputeRow>(
        "SELECT id, tenant_id, store_id, status, escrow_id, quote_id,
                claimant_address, respondent_address, reason, category,
                amount_decimal, asset, resolution_type, buyer_amount_decimal,
                seller_amount_decimal, resolution_note, resolved_by, evidence_deadline,
                review_deadline, metadata, created_at, updated_at, resolved_at
         FROM a2a_disputes
         WHERE id = $1 AND tenant_id = $2 AND store_id = $3 FOR UPDATE",
    )
    .bind(dispute_id)
    .bind(tenant_id)
    .bind(store_id)
    .fetch_optional(tx)
    .await
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))
}

fn a2a_escrow_from_pg(row: A2AEscrowRow) -> Result<A2AEscrow> {
    let status = row.status.parse::<A2AEscrowStatus>().map_err(|error| {
        CommerceError::DatabaseError(format!("invalid A2A escrow status: {error}"))
    })?;
    let release_conditions = serde_json::from_value(row.release_conditions)
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    Ok(A2AEscrow {
        id: row.id,
        tenant_id: row.tenant_id,
        store_id: row.store_id,
        status,
        quote_id: row.quote_id,
        payment_id: row.payment_id,
        buyer_address: row.buyer_address,
        seller_address: row.seller_address,
        amount: row.amount,
        amount_decimal: row.amount_decimal,
        asset: row.asset,
        network: row.network,
        release_conditions,
        funded_at: row.funded_at,
        released_at: row.released_at,
        disputed_at: row.disputed_at,
        dispute_id: row.dispute_id,
        expires_at: row.expires_at,
        auto_release_after: row.auto_release_after,
        metadata: row.metadata,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn a2a_release_conditions_met_pg(
    tx: &mut sqlx::PgConnection,
    escrow: &A2AEscrow,
    now: chrono::DateTime<Utc>,
) -> Result<bool> {
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
                    sqlx::query_scalar::<_, bool>(
                        "SELECT status = 'fulfilled' FROM a2a_quotes WHERE id::text = $1",
                    )
                    .bind(quote_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?
                    .unwrap_or(false)
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

async fn replay_or_conflict<C, T: DeserializeOwned>(
    tx: &mut sqlx::PgConnection,
    command: &CommandEnvelope<C>,
    request_hash: &str,
    existing: KernelReceiptRecord,
    aggregate_type: &str,
) -> Result<Replay<T>> {
    let audit = sealed_audit_entry_tx(tx, &existing).await?;
    resolve_replay(command, request_hash, existing, audit.as_ref(), aggregate_type)
}

async fn append_receipt<T: Serialize>(
    tx: &mut sqlx::PgConnection,
    request_hash: &str,
    receipt: &mut ExecutionReceipt<T>,
) -> Result<()> {
    let record = receipt_record(request_hash, receipt)?;
    receipt.audit_hash = Some(append_kernel_receipt_tx(tx, &record).await?);
    Ok(())
}
