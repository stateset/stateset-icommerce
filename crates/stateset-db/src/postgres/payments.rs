//! PostgreSQL implementation of payment repository

use super::kernel_outbox::append_kernel_event_tx;
use super::map_db_error;
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BatchResult, CommerceError, CreatePayment, CreatePaymentMethod, CreateRefund, CurrencyCode,
    CustomerId, InvoiceId, OrderId, OrderStatus, Payment, PaymentFilter, PaymentId, PaymentMethod,
    PaymentMethodType, PaymentRepository, PaymentTransactionStatus, Refund, RefundStatus, Result,
    UpdatePayment, Validate, generate_payment_number, generate_refund_number, validate_batch_size,
};
use uuid::Uuid;

/// Payment statuses that hold (or are about to hold) a slice of the order's
/// total; in-flight captures count so concurrent captures cannot both pass.
///
/// `Disputed` is included: a chargeback under dispute is contested money, not a
/// settled loss — the capture is still on the books (and the dispute may
/// resolve back to `Completed`), so it must keep consuming its slice of the
/// order total. Mirrors the SQLite backend exactly.
pub(crate) const CAPTURING_STATUSES: [PaymentTransactionStatus; 7] = [
    PaymentTransactionStatus::Pending,
    PaymentTransactionStatus::Processing,
    PaymentTransactionStatus::RequiresAction,
    PaymentTransactionStatus::Completed,
    PaymentTransactionStatus::PartiallyRefunded,
    PaymentTransactionStatus::Refunded,
    PaymentTransactionStatus::Disputed,
];

/// Whether `status` holds a slice of its order's total (see
/// [`CAPTURING_STATUSES`]).
pub(crate) fn is_capturing(status: PaymentTransactionStatus) -> bool {
    CAPTURING_STATUSES.contains(&status)
}

/// [`CAPTURING_STATUSES`] as strings, to bind to a `status = ANY($n)` predicate.
fn capturing_statuses() -> Vec<String> {
    CAPTURING_STATUSES.iter().map(ToString::to_string).collect()
}

/// Every payment status, so the transition guards below can be derived from the
/// domain state machine instead of from hand-written status lists that drift
/// away from it. `PaymentTransactionStatus` is `#[non_exhaustive]` and does not
/// derive `EnumIter`, so the variants are enumerated once, here.
const ALL_PAYMENT_STATUSES: [PaymentTransactionStatus; 9] = [
    PaymentTransactionStatus::Pending,
    PaymentTransactionStatus::Processing,
    PaymentTransactionStatus::RequiresAction,
    PaymentTransactionStatus::Completed,
    PaymentTransactionStatus::Failed,
    PaymentTransactionStatus::Cancelled,
    PaymentTransactionStatus::Refunded,
    PaymentTransactionStatus::PartiallyRefunded,
    PaymentTransactionStatus::Disputed,
];

/// Whether a persisted status write from `from` to `to` is legal.
///
/// The rules live in the domain state machine
/// ([`PaymentTransactionStatus::can_transition_to`]); this wrapper adds exactly
/// ONE documented edge that the persistence layer needs and the enum does not
/// model: `Pending -> Completed`, the single-shot capture used by processors
/// that settle without an intermediate `Processing` step. Every other answer —
/// in particular every edge OUT of a settled payment
/// (`Completed`/`PartiallyRefunded`/`Refunded` -> `Cancelled`/`Failed`, which
/// would release the slice of the order total that settled money is consuming
/// and let the order be captured twice) — is the enum's own. Mirrors the SQLite
/// backend exactly.
pub(crate) fn payment_transition_allowed(
    from: PaymentTransactionStatus,
    to: PaymentTransactionStatus,
) -> bool {
    if from == PaymentTransactionStatus::Pending && to == PaymentTransactionStatus::Completed {
        return true;
    }
    from.can_transition_to(to)
}

/// Every status a payment may currently be in for a write that sets its status
/// to `target`, to bind to a `status = ANY($n)` predicate. Keeping the check
/// inside the UPDATE means a concurrent writer cannot slip between the check and
/// the write.
fn statuses_allowing_transition_to(target: PaymentTransactionStatus) -> Vec<String> {
    ALL_PAYMENT_STATUSES
        .iter()
        .filter(|from| payment_transition_allowed(**from, target))
        .map(ToString::to_string)
        .collect()
}

/// Conflict error for a refused status write, naming the status the payment is
/// actually in. Worded identically in the SQLite backend.
fn transition_conflict(
    current: PaymentTransactionStatus,
    target: PaymentTransactionStatus,
) -> CommerceError {
    CommerceError::Conflict(format!("Payment is {current} and cannot transition to {target}"))
}

/// Order-side guards for a capture against `order_id`, all evaluated on ONE
/// `SELECT ... FOR UPDATE` of the order row (so concurrent captures for the
/// same order serialize on the row lock):
///
/// 1. **order status** — a `Cancelled` or `Refunded` order has no money owed on
///    it; creating or completing a capturing payment against it would orphan
///    the captured money (`ValidationError` naming the order status);
/// 2. **currency** — the payment's currency must equal the order's, otherwise
///    the capacity sum below would add unlike units (JPY 100 against a USD 100
///    order used to pass) (`ValidationError` naming both currencies);
/// 3. **capacity** — reject when Σ(captures for the order in a capturing
///    status, excluding `exclude_payment_id`) + `amount` would exceed
///    `orders.total_amount` (`CaptureExceedsOrderTotal`).
///
/// A payment whose `order_id` does not resolve to an order has nothing to cap
/// against and passes. Mirrors the SQLite backend exactly.
pub(crate) async fn check_order_capture_capacity_pg(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    exclude_payment_id: Option<Uuid>,
    amount: Decimal,
    payment_currency: CurrencyCode,
) -> Result<()> {
    let order: Option<(Decimal, String, CurrencyCode)> = sqlx::query_as(
        "SELECT total_amount, status, currency FROM orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?;
    let Some((total, raw_status, order_currency)) = order else { return Ok(()) };
    let order_status: OrderStatus = raw_status.parse().map_err(|_| {
        CommerceError::DatabaseError(format!("Invalid order.status '{raw_status}'"))
    })?;

    if matches!(order_status, OrderStatus::Cancelled | OrderStatus::Refunded) {
        return Err(CommerceError::ValidationError(format!(
            "Order {order_id} is {order_status}; a payment cannot be captured against it"
        )));
    }
    if payment_currency != order_currency {
        return Err(CommerceError::ValidationError(format!(
            "Payment currency {payment_currency} does not match order {order_id} currency {order_currency}"
        )));
    }

    let (captured,): (Decimal,) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount), 0) FROM payments \
         WHERE order_id = $1 AND status = ANY($2) AND id IS DISTINCT FROM $3",
    )
    .bind(order_id)
    .bind(capturing_statuses())
    .bind(exclude_payment_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(map_db_error)?;

    if captured + amount > total {
        return Err(CommerceError::CaptureExceedsOrderTotal {
            order_id,
            order_total: total.to_string(),
            already_captured: captured.to_string(),
            requested: amount.to_string(),
        });
    }
    Ok(())
}

/// Payments for `order_id` still holding captured money: every payment in a
/// capturing status whose `amount` exceeds its `amount_refunded`. Runs on the
/// caller's connection so the orders module can consult it inside its own
/// cancel transaction.
pub(crate) async fn open_captures_for_order_pg(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
) -> Result<Vec<Payment>> {
    let rows = sqlx::query_as::<_, PaymentRow>(
        "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
         amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
         card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
         description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
         FROM payments WHERE order_id = $1 AND status = ANY($2) AND amount > amount_refunded
         ORDER BY created_at",
    )
    .bind(order_id)
    .bind(capturing_statuses())
    .fetch_all(&mut *conn)
    .await
    .map_err(map_db_error)?;
    rows.into_iter().map(PgPaymentRepository::row_to_payment).collect()
}

/// Money still refundable on `payment`, on the caller's connection: the
/// payment's remaining balance minus every in-flight (`pending`/`processing`)
/// refund, exactly as `create_refund_async` reserves it. Zero when the payment
/// is not in a refundable status.
pub(crate) async fn refundable_remaining_pg(
    conn: &mut sqlx::PgConnection,
    payment: &Payment,
) -> Result<Decimal> {
    if !payment.status.is_refundable() {
        return Ok(Decimal::ZERO);
    }
    let in_flight: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM refunds \
         WHERE payment_id = $1 AND status IN ($2, $3)",
    )
    .bind(payment.id.into_uuid())
    .bind(RefundStatus::Pending.to_string())
    .bind(RefundStatus::Processing.to_string())
    .fetch_one(&mut *conn)
    .await
    .map_err(map_db_error)?;
    Ok((payment.refundable_remaining() - in_flight).max(Decimal::ZERO))
}

/// Create a `pending` refund of `amount` against `payment` on the caller's
/// connection/transaction, returning the refund id. The in-transaction twin of
/// [`PgPaymentRepository::create_refund_async`] for callers (the returns
/// module) that must settle a refund in the SAME commit as their own state
/// change. The caller must already hold the payment row lock (`FOR UPDATE`).
/// `idempotency_key` is honoured inside the transaction: an existing refund
/// with the key is returned as-is.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_refund_pg_tx(
    conn: &mut sqlx::PgConnection,
    payment: &Payment,
    amount: Decimal,
    reason: Option<&str>,
    idempotency_key: Option<&str>,
    notes: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Uuid> {
    if let Some(key) = idempotency_key {
        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM refunds WHERE idempotency_key = $1")
                .bind(key)
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?;
        if let Some(id) = existing {
            return Ok(id);
        }
    }
    let mut reserved = payment.clone();
    reserved.amount_refunded +=
        payment.refundable_remaining() - refundable_remaining_pg(conn, payment).await?;
    let refund_amount = reserved.validate_refund(Some(amount))?;

    let id = Uuid::new_v4();
    let refund_number = generate_refund_number();
    sqlx::query(
        "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key, notes, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, $10, $11)",
    )
    .bind(id)
    .bind(&refund_number)
    .bind(payment.id.into_uuid())
    .bind(RefundStatus::Pending.to_string())
    .bind(refund_amount)
    .bind(payment.currency)
    .bind(reason)
    .bind(idempotency_key)
    .bind(notes)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    append_kernel_event_tx(
        conn,
        &KernelOutboxEvent::domain(
            "payments.refund_created.v1",
            "refund",
            id.to_string(),
            serde_json::json!({
                "refund_id": id.to_string(),
                "refund_number": refund_number,
                "payment_id": payment.id.to_string(),
                "amount": refund_amount.to_string(),
                "currency": payment.currency.as_str(),
                "status": RefundStatus::Pending.to_string(),
            }),
            idempotency_key.map(str::to_string),
        ),
    )
    .await?;
    Ok(id)
}

/// Statuses a payment can be voided from when its order is force-cancelled:
/// money that is in flight but not yet captured.
const IN_FLIGHT_STATUSES: [PaymentTransactionStatus; 3] = [
    PaymentTransactionStatus::Pending,
    PaymentTransactionStatus::Processing,
    PaymentTransactionStatus::RequiresAction,
];

/// Void (`cancelled`) every in-flight payment against `order_id`, returning the
/// ids voided. Runs on the caller's connection/transaction so a forced order
/// cancel (`UpdateOrder::void_payments`) voids the holds in the same commit as
/// the status change. Settled payments are NOT touched: captured money leaves
/// through a refund. Mirrors the SQLite implementation.
pub(crate) async fn void_in_flight_payments_for_order_pg(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>> {
    let in_flight: Vec<String> = IN_FLIGHT_STATUSES.iter().map(ToString::to_string).collect();
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "UPDATE payments SET status = $1, updated_at = $2
         WHERE order_id = $3 AND status = ANY($4)
         RETURNING id",
    )
    .bind(PaymentTransactionStatus::Cancelled.to_string())
    .bind(now)
    .bind(order_id)
    .bind(&in_flight)
    .fetch_all(&mut *conn)
    .await
    .map_err(map_db_error)?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Whether any payment row references `order_id`, on the caller's connection.
/// The orders module consults it inside its delete transaction.
pub(crate) async fn order_has_payments_pg(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payments WHERE order_id = $1)")
        .bind(order_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)
}

/// Refuse `update` writes that would move a payment INTO `Refunded` /
/// `PartiallyRefunded` as a bare status flip: those are ledger states written
/// only by `complete_refund_async`, which also advances `amount_refunded`.
/// Taking `Completed`/`PartiallyRefunded`/`Disputed -> Refunded` through
/// `update` left `amount_refunded` stale, so `open_captures_for_order` kept
/// reporting the money as outstanding. Same-status writes still pass. Worded
/// identically in the SQLite backend.
pub(crate) fn ensure_not_refund_by_status_flip(
    current: PaymentTransactionStatus,
    target: PaymentTransactionStatus,
) -> Result<()> {
    if current != target
        && matches!(
            target,
            PaymentTransactionStatus::Refunded | PaymentTransactionStatus::PartiallyRefunded
        )
    {
        return Err(CommerceError::ValidationError(format!(
            "Payment status cannot be set to {target} directly (from {current}); \
             refunds are recorded through create_refund + complete_refund so that \
             amount_refunded and the refund ledger stay consistent"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PgPaymentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
pub(crate) struct PaymentRow {
    id: Uuid,
    payment_number: String,
    order_id: Option<Uuid>,
    invoice_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    status: String,
    payment_method: String,
    amount: Decimal,
    currency: CurrencyCode,
    amount_refunded: Decimal,
    external_id: Option<String>,
    idempotency_key: Option<String>,
    processor: Option<String>,
    card_brand: Option<String>,
    card_last4: Option<String>,
    card_exp_month: Option<i32>,
    card_exp_year: Option<i32>,
    billing_email: Option<String>,
    billing_name: Option<String>,
    billing_address: Option<String>,
    description: Option<String>,
    failure_reason: Option<String>,
    failure_code: Option<String>,
    metadata: Option<String>,
    paid_at: Option<DateTime<Utc>>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub(crate) struct RefundRow {
    id: Uuid,
    refund_number: String,
    payment_id: Uuid,
    status: String,
    amount: Decimal,
    currency: CurrencyCode,
    reason: Option<String>,
    external_id: Option<String>,
    idempotency_key: Option<String>,
    failure_reason: Option<String>,
    notes: Option<String>,
    refunded_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PaymentMethodRow {
    id: Uuid,
    customer_id: Uuid,
    method_type: String,
    is_default: bool,
    card_brand: Option<String>,
    card_last4: Option<String>,
    card_exp_month: Option<i32>,
    card_exp_year: Option<i32>,
    cardholder_name: Option<String>,
    bank_name: Option<String>,
    account_last4: Option<String>,
    external_id: Option<String>,
    billing_address: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPaymentRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) fn row_to_payment(row: PaymentRow) -> Result<Payment> {
        let PaymentRow {
            id,
            payment_number,
            order_id,
            invoice_id,
            customer_id,
            status,
            payment_method,
            amount,
            currency,
            amount_refunded,
            external_id,
            idempotency_key,
            processor,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            billing_email,
            billing_name,
            billing_address,
            description,
            failure_reason,
            failure_code,
            metadata,
            paid_at,
            version,
            created_at,
            updated_at,
        } = row;

        let status: PaymentTransactionStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid payment.status '{}': {}", status, e))
        })?;
        let payment_method: PaymentMethodType = payment_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment.payment_method '{}': {}",
                payment_method, e
            ))
        })?;
        let card_brand = match card_brand {
            Some(value) => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid payment.card_brand '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };

        Ok(Payment {
            id: PaymentId::from(id),
            payment_number,
            order_id: order_id.map(OrderId::from),
            invoice_id,
            customer_id: customer_id.map(CustomerId::from),
            status,
            payment_method,
            amount,
            currency,
            amount_refunded,
            external_id,
            idempotency_key,
            processor,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            blockchain_network: None,
            stablecoin_type: None,
            from_wallet_address: None,
            to_wallet_address: None,
            tx_hash: None,
            block_number: None,
            confirmations: None,
            token_address: None,
            ves_intent_id: None,
            billing_email,
            billing_name,
            billing_address,
            description,
            failure_reason,
            failure_code,
            metadata,
            paid_at,
            version,
            created_at,
            updated_at,
        })
    }

    pub(crate) fn row_to_refund(row: RefundRow) -> Result<Refund> {
        let RefundRow {
            id,
            refund_number,
            payment_id,
            status,
            amount,
            currency,
            reason,
            external_id,
            idempotency_key,
            failure_reason,
            notes,
            refunded_at,
            created_at,
            updated_at,
        } = row;

        let status: RefundStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid refund.status '{}': {}", status, e))
        })?;

        Ok(Refund {
            id,
            refund_number,
            payment_id: PaymentId::from(payment_id),
            status,
            amount,
            currency,
            reason,
            external_id,
            idempotency_key,
            failure_reason,
            notes,
            refunded_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_payment_method(row: PaymentMethodRow) -> Result<PaymentMethod> {
        let PaymentMethodRow {
            id,
            customer_id,
            method_type,
            is_default,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            cardholder_name,
            bank_name,
            account_last4,
            external_id,
            billing_address,
            created_at,
            updated_at,
        } = row;

        let method_type: PaymentMethodType = method_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment_method.method_type '{}': {}",
                method_type, e
            ))
        })?;
        let card_brand = match card_brand {
            Some(value) => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid payment_method.card_brand '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };

        Ok(PaymentMethod {
            id,
            customer_id: CustomerId::from(customer_id),
            method_type,
            is_default,
            card_brand,
            card_last4,
            card_exp_month,
            card_exp_year,
            cardholder_name,
            bank_name,
            account_last4,
            wallet_address: None,
            blockchain_network: None,
            stablecoin_type: None,
            external_id,
            billing_address,
            created_at,
            updated_at,
        })
    }

    /// Create payment (async)
    pub async fn create_async(&self, input: CreatePayment) -> Result<Payment> {
        input.validate()?;
        // A duplicate idempotency key is only a replay when it carries the
        // SAME request (`Payment::check_idempotent_replay`); a different
        // amount/order/currency/method under a reused key is a `Conflict`.
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key_async(key).await? {
                existing.check_idempotent_replay(&input)?;
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let payment_number = generate_payment_number();
        let outbox_event = KernelOutboxEvent::domain(
            "payments.created.v1",
            "payment",
            id.to_string(),
            serde_json::json!({
                "payment_id": id.to_string(),
                "payment_number": payment_number,
                "order_id": input.order_id.map(|value| value.to_string()),
                "amount": input.amount.to_string(),
                "currency": input.currency.unwrap_or_default().as_str(),
                "status": PaymentTransactionStatus::Pending.to_string(),
            }),
            input.idempotency_key.clone(),
        );

        // Over-capture check and INSERT share one transaction; the guard locks
        // the order row so concurrent captures serialize.
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        if let Some(order_id) = input.order_id {
            check_order_capture_capacity_pg(
                tx.as_mut(),
                order_id.into_uuid(),
                None,
                input.amount,
                currency,
            )
            .await?;
        }

        let inserted = sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
             payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
             card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
             billing_address, description, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)"
        )
        .bind(id)
        .bind(&payment_number)
        .bind(input.order_id.map(|oid| oid.into_uuid()))
        .bind(input.invoice_id)
        .bind(input.customer_id.map(|cid| cid.into_uuid()))
        .bind(PaymentTransactionStatus::Pending.to_string())
        .bind(input.payment_method.to_string())
        .bind(input.amount)
        .bind(currency)
        .bind(Decimal::ZERO)
        .bind(&input.external_id)
        .bind(&input.idempotency_key)
        .bind(&input.processor)
        .bind(input.card_brand.map(|b| b.to_string()))
        .bind(&input.card_last4)
        .bind(input.card_exp_month)
        .bind(input.card_exp_year)
        .bind(&input.billing_email)
        .bind(&input.billing_name)
        .bind(&input.billing_address)
        .bind(&input.description)
        .bind(&input.metadata)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error);

        // Two callers racing the same idempotency key can both miss the
        // pre-transaction lookup; the loser's INSERT then trips the UNIQUE index
        // on `idempotency_key`. That is the idempotent case, not an error: roll
        // back and return the row the winner wrote.
        if let Err(err) = inserted {
            if let (CommerceError::Conflict(_), Some(key)) =
                (&err, input.idempotency_key.as_deref())
            {
                drop(tx);
                if let Some(existing) = self.get_by_idempotency_key_async(key).await? {
                    existing.check_idempotent_replay(&input)?;
                    return Ok(existing);
                }
            }
            return Err(err);
        }
        append_kernel_event_tx(tx.as_mut(), &outbox_event).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get payment by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    /// Get payment by number (async)
    pub async fn get_by_number_async(&self, payment_number: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE payment_number = $1"
        )
        .bind(payment_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    /// Get payment by external ID (async)
    pub async fn get_by_external_id_async(&self, external_id: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE external_id = $1"
        )
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    async fn get_by_idempotency_key_async(&self, key: &str) -> Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE idempotency_key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    /// Update payment (async)
    ///
    /// The read, the transition check and the write share ONE transaction with
    /// `SELECT ... FOR UPDATE` on the payment row, and the status check is a
    /// `status = ANY(...)` predicate on the UPDATE itself, so a concurrent
    /// writer cannot slip between them. (Previously the status was written
    /// unconditionally on a lock-free pool connection, which let
    /// `cancel`/`mark_failed` — both of which funnel through here — flip a
    /// `Completed` payment into a status that releases its slice of the order
    /// total, so the same order could be captured twice.)
    ///
    /// A request that does not change the status (`input.status == None`, or the
    /// status it already has) is always a legal self-transition and never
    /// conflicts.
    pub async fn update_async(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = $1 FOR UPDATE"
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let payment = Self::row_to_payment(row)?;
        let current = payment.status;
        let target = input.status.unwrap_or(current);
        if !payment_transition_allowed(current, target) {
            return Err(transition_conflict(current, target));
        }
        ensure_not_refund_by_status_flip(current, target)?;

        // A write that moves the payment from a non-capturing status into a
        // capturing one re-acquires a slice of the order total, so it gets the
        // same in-transaction order guards as `mark_completed_async` (capacity,
        // order status, currency). Today no legal edge does this (every
        // non-capturing status is terminal), so this is the guard that keeps
        // `update` honest if the state machine ever grows one.
        if !is_capturing(current) && is_capturing(target) {
            if let Some(order_id) = payment.order_id {
                check_order_capture_capacity_pg(
                    tx.as_mut(),
                    order_id.into_uuid(),
                    Some(id),
                    payment.amount,
                    payment.currency,
                )
                .await?;
            }
        }

        let rows = sqlx::query(
            "UPDATE payments SET status = $1, external_id = $2, failure_reason = $3,
             failure_code = $4, metadata = $5, updated_at = $6
             WHERE id = $7 AND status = ANY($8)",
        )
        .bind(target.to_string())
        .bind(input.external_id.or(payment.external_id))
        .bind(input.failure_reason.or(payment.failure_reason))
        .bind(input.failure_code.or(payment.failure_code))
        .bind(input.metadata.or(payment.metadata))
        .bind(now)
        .bind(id)
        .bind(statuses_allowing_transition_to(target))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(transition_conflict(current, target));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List payments (async)
    pub async fn list_async(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0) as i64;

        let mut query = String::from(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE 1=1"
        );
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.invoice_id.is_some() {
            query.push_str(&format!(" AND invoice_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PaymentRow>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id.into_uuid());
        }
        if let Some(invoice_id) = filter.invoice_id {
            q = q.bind(invoice_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut payments = Vec::with_capacity(rows.len());
        for row in rows {
            payments.push(Self::row_to_payment(row)?);
        }
        Ok(payments)
    }

    /// Get payments for order (async)
    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.list_async(PaymentFilter {
            order_id: Some(OrderId::from(order_id)),
            ..Default::default()
        })
        .await
    }

    /// Get payments for invoice (async)
    pub async fn for_invoice_async(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.list_async(PaymentFilter { invoice_id: Some(invoice_id), ..Default::default() }).await
    }

    /// Payments for `order_id` still holding captured money (async); see
    /// [`PaymentRepository::open_captures_for_order`].
    pub async fn open_captures_for_order_async(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        open_captures_for_order_pg(conn.as_mut(), order_id).await
    }

    /// Mark payment as processing (async)
    pub async fn mark_processing_async(&self, id: Uuid) -> Result<Payment> {
        self.update_async(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Processing),
                ..Default::default()
            },
        )
        .await
    }

    /// Mark payment as completed (async)
    pub async fn mark_completed_async(&self, id: Uuid) -> Result<Payment> {
        let now = Utc::now();
        let target = PaymentTransactionStatus::Completed;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        // Two guards, in this order (same as SQLite):
        //   1. the state machine — only a payment that may legally reach
        //      `Completed` may be completed (never a cancelled/failed/refunded
        //      one);
        //   2. the order's capacity, re-checked at completion time: a payment
        //      that was failed/cancelled while still in flight (and so released
        //      its slice of the total) must not be completed on top of captures
        //      made since.
        let (raw_status, order_id, amount, currency): (
            String,
            Option<Uuid>,
            Decimal,
            CurrencyCode,
        ) = sqlx::query_as(
            "SELECT status, order_id, amount, currency FROM payments WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let current: PaymentTransactionStatus = raw_status.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid payment status '{raw_status}'"))
        })?;
        if !payment_transition_allowed(current, target) {
            return Err(transition_conflict(current, target));
        }

        if let Some(order_id) = order_id {
            check_order_capture_capacity_pg(tx.as_mut(), order_id, Some(id), amount, currency)
                .await?;
        }

        let rows = sqlx::query(
            "UPDATE payments SET status = $1, paid_at = $2, updated_at = $3
             WHERE id = $4 AND status = ANY($5)",
        )
        .bind(target.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .bind(statuses_allowing_transition_to(target))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(transition_conflict(current, target));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Mark payment as failed (async)
    ///
    /// Status-guarded, for the same reason as [`Self::update_async`]: a
    /// `Completed` payment is settled money whose refund ledger points at it,
    /// and `failed` is not a capturing status — flipping it would release the
    /// order-total slice the capture is consuming and let the order be captured
    /// again.
    pub async fn mark_failed_async(
        &self,
        id: Uuid,
        reason: &str,
        code: Option<&str>,
    ) -> Result<Payment> {
        let now = Utc::now();
        let target = PaymentTransactionStatus::Failed;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let (raw_status,): (String,) =
            sqlx::query_as("SELECT status FROM payments WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        let current: PaymentTransactionStatus = raw_status.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid payment status '{raw_status}'"))
        })?;

        let rows = sqlx::query(
            "UPDATE payments SET status = $1, failure_reason = $2, failure_code = $3,
             updated_at = $4 WHERE id = $5 AND status = ANY($6)",
        )
        .bind(target.to_string())
        .bind(reason)
        .bind(code)
        .bind(now)
        .bind(id)
        .bind(statuses_allowing_transition_to(target))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(transition_conflict(current, target));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel payment (async)
    ///
    /// Routes through [`Self::update_async`], so the state machine guard
    /// applies: a `Completed`/`PartiallyRefunded`/`Refunded` payment cannot be
    /// cancelled.
    pub async fn cancel_async(&self, id: Uuid) -> Result<Payment> {
        self.update_async(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Cancelled),
                ..Default::default()
            },
        )
        .await
    }

    /// Create refund (async)
    ///
    /// The payment read, the over-refund validation, and the refund `INSERT` all
    /// run inside ONE transaction, and the payment row is locked with
    /// `SELECT ... FOR UPDATE` up front. The lock serializes concurrent
    /// `create_refund_async` calls for the same payment: each caller sees the
    /// other's freshly-inserted in-flight refund and cannot both pass the
    /// remaining-balance check. (Previously the read+validate happened on a
    /// lock-free pool connection with no transaction, so two callers could each
    /// validate against the same stale balance and together over-refund the
    /// payment once both were completed — the TOCTOU race that the SQLite
    /// backend already closed with its `IMMEDIATE` transaction.)
    pub async fn create_refund_async(&self, input: CreateRefund) -> Result<Refund> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_refund_by_idempotency_key_async(key).await? {
                return Ok(existing);
            }
        }

        let raw_payment_id = input.payment_id.into_uuid();
        let id = Uuid::new_v4();
        let now = Utc::now();
        let refund_number = generate_refund_number();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the payment row for the duration of the transaction so a
        // concurrent refund of the same payment is serialized behind us.
        let row = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = $1 FOR UPDATE"
        )
        .bind(raw_payment_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let mut payment = Self::row_to_payment(row)?;
        input.validate_for_currency(payment.currency)?;

        // Reserve against in-flight (non-terminal) refunds as well as the
        // already-committed `amount_refunded`. A `Pending`/`Processing` refund
        // has not yet folded its amount into `amount_refunded`, but it WILL once
        // completed, so it must count against the remaining refundable balance to
        // prevent concurrent over-refund. `Failed`/`Cancelled` refunds release
        // their reservation and are excluded.
        //
        // The `amount` column is `DECIMAL`, so `SUM` is exact NUMERIC arithmetic;
        // a missing row (`NULL`) coalesces to zero.
        let in_flight: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM refunds \
             WHERE payment_id = $1 AND status IN ($2, $3)",
        )
        .bind(raw_payment_id)
        .bind(RefundStatus::Pending.to_string())
        .bind(RefundStatus::Processing.to_string())
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Fold the in-flight reservation into the payment's refunded total so the
        // unmodified `validate_refund` guard sees the true remaining balance.
        // `validate_refund` still owns all of the rules (refundable status,
        // positive amount, not exceeding remaining), resolving `None` to a full
        // remaining refund.
        payment.amount_refunded += in_flight;
        let refund_amount = payment.validate_refund(input.amount)?;

        sqlx::query(
            "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
        )
        .bind(id)
        .bind(&refund_number)
        .bind(raw_payment_id)
        .bind(RefundStatus::Pending.to_string())
        .bind(refund_amount)
        .bind(payment.currency)
        .bind(&input.reason)
        .bind(&input.external_id)
        .bind(&input.idempotency_key)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let outbox_event = KernelOutboxEvent::domain(
            "payments.refund_created.v1",
            "refund",
            id.to_string(),
            serde_json::json!({
                "refund_id": id.to_string(),
                "refund_number": refund_number,
                "payment_id": raw_payment_id.to_string(),
                "amount": refund_amount.to_string(),
                "currency": payment.currency.as_str(),
                "status": RefundStatus::Pending.to_string(),
            }),
            input.idempotency_key.clone(),
        );
        append_kernel_event_tx(tx.as_mut(), &outbox_event).await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get refund by ID (async)
    pub async fn get_refund_async(&self, id: Uuid) -> Result<Option<Refund>> {
        let row = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_refund).transpose()
    }

    async fn get_refund_by_idempotency_key_async(&self, key: &str) -> Result<Option<Refund>> {
        let row = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE idempotency_key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_refund).transpose()
    }

    /// Get refunds for payment (async)
    pub async fn get_refunds_async(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        let rows = sqlx::query_as::<_, RefundRow>(
            "SELECT id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key,
             failure_reason, notes, refunded_at, created_at, updated_at
             FROM refunds WHERE payment_id = $1 ORDER BY created_at DESC"
        )
        .bind(payment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut refunds = Vec::with_capacity(rows.len());
        for row in rows {
            refunds.push(Self::row_to_refund(row)?);
        }
        Ok(refunds)
    }

    /// Complete refund (async)
    ///
    /// Marks the refund as completed and advances the parent payment's
    /// `amount_refunded` / status in a single transaction. Both writes must
    /// succeed or fail together: a partial failure would otherwise leave the
    /// refund flagged complete while the payment balance lagged behind (or vice
    /// versa). `amount_refunded` is a `DECIMAL` column in Postgres, so the
    /// `+`/`>=` arithmetic is exact NUMERIC arithmetic (unlike the SQLite TEXT
    /// columns, which require Rust-side `Decimal` math).
    pub async fn complete_refund_async(&self, id: Uuid) -> Result<Refund> {
        let refund = self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the refund row and read its CURRENT status so concurrent
        // completions serialize and only one folds the refund into the payment.
        let (current_status,): (String,) =
            sqlx::query_as("SELECT status FROM refunds WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        let current_status: RefundStatus = current_status.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid refund status '{current_status}'"))
        })?;

        // Idempotent: completing an already-completed refund is a no-op (a
        // duplicated payment-processor webhook or a retry must NOT re-add the
        // amount to the payment's `amount_refunded`).
        if current_status == RefundStatus::Completed {
            return self.get_refund_async(id).await?.ok_or(CommerceError::NotFound);
        }
        // A failed/cancelled refund is terminal and cannot be completed.
        if current_status.is_terminal() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot complete a {current_status} refund"
            )));
        }

        sqlx::query(
            "UPDATE refunds SET status = $1, refunded_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(RefundStatus::Completed.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Update payment amount_refunded.
        //
        // The payment row is locked and its new status computed in Rust (rather
        // than with an inline SQL `CASE`) so that the write can carry the same
        // state-machine guard as every other status write, and so that the shape
        // matches the SQLite backend: a refund may only fold itself into a
        // payment that can legally reach `Refunded`/`PartiallyRefunded`.
        let payment_id = refund.payment_id.into_uuid();
        let (raw_payment_status, current_refunded, payment_amount): (String, Decimal, Decimal) =
            sqlx::query_as(
                "SELECT status, amount_refunded, amount FROM payments WHERE id = $1 FOR UPDATE",
            )
            .bind(payment_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let payment_status: PaymentTransactionStatus =
            raw_payment_status.parse().map_err(|_| {
                CommerceError::DatabaseError(format!(
                    "Invalid payment status '{raw_payment_status}'"
                ))
            })?;

        let new_refunded = current_refunded + refund.amount;
        let new_status = if new_refunded >= payment_amount {
            PaymentTransactionStatus::Refunded
        } else {
            PaymentTransactionStatus::PartiallyRefunded
        };

        let rows = sqlx::query(
            "UPDATE payments SET amount_refunded = $1, status = $2, updated_at = $3
             WHERE id = $4 AND status = ANY($5)",
        )
        .bind(new_refunded)
        .bind(new_status.to_string())
        .bind(now)
        .bind(payment_id)
        .bind(statuses_allowing_transition_to(new_status))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(transition_conflict(payment_status, new_status));
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Fail refund (async)
    pub async fn fail_refund_async(&self, id: Uuid, reason: &str) -> Result<Refund> {
        let now = Utc::now();

        // Only an in-flight refund can fail; a `Completed` refund is already
        // folded into `payments.amount_refunded` (see the SQLite backend).
        let rows = sqlx::query(
            "UPDATE refunds SET status = $1, failure_reason = $2, updated_at = $3 \
             WHERE id = $4 AND status IN ('pending', 'processing')",
        )
        .bind(RefundStatus::Failed.to_string())
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?
        .rows_affected();

        let refund = self.get_refund_async(id).await?.ok_or(CommerceError::NotFound)?;
        if rows == 0 && refund.status != RefundStatus::Failed {
            return Err(CommerceError::ValidationError(format!(
                "Cannot fail a {} refund",
                refund.status
            )));
        }
        Ok(refund)
    }

    /// Create payment method (async)
    pub async fn create_payment_method_async(
        &self,
        input: CreatePaymentMethod,
    ) -> Result<PaymentMethod> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Clearing the old default and inserting the new one share ONE
        // transaction (as they already do in SQLite): a failure between the two
        // statements would otherwise leave the customer with no default payment
        // method at all.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // If setting as default, clear existing default
        if input.is_default.unwrap_or(false) {
            sqlx::query("UPDATE payment_methods SET is_default = false WHERE customer_id = $1")
                .bind(input.customer_id.into_uuid())
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }

        sqlx::query(
            "INSERT INTO payment_methods (id, customer_id, method_type, is_default, card_brand,
             card_last4, card_exp_month, card_exp_year, cardholder_name, bank_name, account_last4,
             external_id, billing_address, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(id)
        .bind(input.customer_id.into_uuid())
        .bind(input.method_type.to_string())
        .bind(input.is_default.unwrap_or(false))
        .bind(input.card_brand.map(|b| b.to_string()))
        .bind(&input.card_last4)
        .bind(input.card_exp_month)
        .bind(input.card_exp_year)
        .bind(&input.cardholder_name)
        .bind(&input.bank_name)
        .bind(&input.account_last4)
        .bind(&input.external_id)
        .bind(&input.billing_address)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        let row = sqlx::query_as::<_, PaymentMethodRow>(
            "SELECT id, customer_id, method_type, is_default, card_brand, card_last4, card_exp_month,
             card_exp_year, cardholder_name, bank_name, account_last4, external_id, billing_address,
             created_at, updated_at FROM payment_methods WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_payment_method(row)
    }

    /// Get payment methods for customer (async)
    pub async fn get_payment_methods_async(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        let rows = sqlx::query_as::<_, PaymentMethodRow>(
            "SELECT id, customer_id, method_type, is_default, card_brand, card_last4, card_exp_month,
             card_exp_year, cardholder_name, bank_name, account_last4, external_id, billing_address,
             created_at, updated_at FROM payment_methods WHERE customer_id = $1
             ORDER BY is_default DESC, created_at DESC"
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut methods = Vec::with_capacity(rows.len());
        for row in rows {
            methods.push(Self::row_to_payment_method(row)?);
        }
        Ok(methods)
    }

    /// Delete payment method (async)
    pub async fn delete_payment_method_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM payment_methods WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Set default payment method (async)
    pub async fn set_default_payment_method_async(
        &self,
        customer_id: Uuid,
        method_id: Uuid,
    ) -> Result<()> {
        // Both statements share ONE transaction (as they already do in SQLite):
        // a failure between them would leave the customer with no default.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query("UPDATE payment_methods SET is_default = false WHERE customer_id = $1")
            .bind(customer_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE payment_methods SET is_default = true WHERE id = $1 AND customer_id = $2",
        )
        .bind(method_id)
        .bind(customer_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Count payments (async)
    pub async fn count_async(&self, filter: PaymentFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM payments WHERE 1=1");
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.invoice_id.is_some() {
            query.push_str(&format!(" AND invoice_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id.into_uuid());
        }
        if let Some(invoice_id) = filter.invoice_id {
            q = q.bind(invoice_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    /// Delete payment (async) - hard delete
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        // Both deletes share ONE transaction (as they already do in
        // `delete_batch_atomic_async` and in SQLite): a failure between them
        // would orphan the refund rows from their payment.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete associated refunds first
        sqlx::query("DELETE FROM refunds WHERE payment_id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM payments WHERE id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    // =========================================================================
    // Batch Operations (async)
    // =========================================================================

    /// Create multiple payments - partial success allowed (async)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreatePayment>,
    ) -> Result<BatchResult<Payment>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple payments - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreatePayment>,
    ) -> Result<Vec<Payment>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut payments = Vec::with_capacity(inputs.len());

        for input in inputs {
            input.validate()?;
            let id = Uuid::new_v4();
            let now = Utc::now();
            let payment_number = generate_payment_number();

            if let Some(order_id) = input.order_id {
                check_order_capture_capacity_pg(
                    tx.as_mut(),
                    order_id.into_uuid(),
                    None,
                    input.amount,
                    input.currency.unwrap_or(CurrencyCode::USD),
                )
                .await?;
            }

            sqlx::query(
            "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)"
            )
            .bind(id)
            .bind(&payment_number)
            .bind(input.order_id.map(|oid| oid.into_uuid()))
            .bind(input.invoice_id)
            .bind(input.customer_id.map(|cid| cid.into_uuid()))
            .bind(PaymentTransactionStatus::Pending.to_string())
            .bind(input.payment_method.to_string())
            .bind(input.amount)
            .bind(input.currency.unwrap_or(CurrencyCode::USD))
            .bind(Decimal::ZERO)
            .bind(&input.external_id)
            .bind(&input.idempotency_key)
            .bind(&input.processor)
            .bind(input.card_brand.map(|b| b.to_string()))
            .bind(&input.card_last4)
            .bind(input.card_exp_month)
            .bind(input.card_exp_year)
            .bind(&input.billing_email)
            .bind(&input.billing_name)
            .bind(&input.billing_address)
            .bind(&input.description)
            .bind(&input.metadata)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let outbox_event = KernelOutboxEvent::domain(
                "payments.created.v1",
                "payment",
                id.to_string(),
                serde_json::json!({
                    "payment_id": id.to_string(),
                    "payment_number": payment_number,
                    "order_id": input.order_id.map(|value| value.to_string()),
                    "amount": input.amount.to_string(),
                    "currency": input.currency.unwrap_or_default().as_str(),
                    "status": PaymentTransactionStatus::Pending.to_string(),
                }),
                input.idempotency_key.clone(),
            );
            append_kernel_event_tx(tx.as_mut(), &outbox_event).await?;

            payments.push(Payment {
                id: PaymentId::from(id),
                payment_number,
                order_id: input.order_id,
                invoice_id: input.invoice_id,
                customer_id: input.customer_id,
                status: PaymentTransactionStatus::Pending,
                payment_method: input.payment_method,
                amount: input.amount,
                currency: input.currency.unwrap_or(CurrencyCode::USD),
                amount_refunded: Decimal::ZERO,
                external_id: input.external_id,
                idempotency_key: input.idempotency_key,
                processor: input.processor,
                card_brand: input.card_brand,
                card_last4: input.card_last4,
                card_exp_month: input.card_exp_month,
                card_exp_year: input.card_exp_year,
                blockchain_network: None,
                stablecoin_type: None,
                from_wallet_address: None,
                to_wallet_address: None,
                tx_hash: None,
                block_number: None,
                confirmations: None,
                token_address: None,
                ves_intent_id: None,
                billing_email: input.billing_email,
                billing_name: input.billing_name,
                billing_address: input.billing_address,
                description: input.description,
                failure_reason: None,
                failure_code: None,
                metadata: input.metadata,
                paid_at: None,
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(payments)
    }

    /// Update multiple payments - partial success allowed (async)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<BatchResult<Payment>> {
        validate_batch_size(&updates)?;

        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match self.update_async(raw_id, input).await {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, Some(raw_id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple payments - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<Vec<Payment>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut payments = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let raw_id = id.into_uuid();
            let payment = sqlx::query_as::<_, PaymentRow>(
                "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                 amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
                 card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
                 description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
                 FROM payments WHERE id = $1 FOR UPDATE"
            )
            .bind(raw_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .map(Self::row_to_payment)
            .transpose()?
            .ok_or(CommerceError::NotFound)?;

            let now = Utc::now();

            // Same state-machine guard as the single-row `update_async`, inside
            // the batch's own transaction: one illegal status write aborts the
            // whole atomic batch rather than silently landing.
            let current = payment.status;
            let target = input.status.unwrap_or(current);
            if !payment_transition_allowed(current, target) {
                return Err(transition_conflict(current, target));
            }
            ensure_not_refund_by_status_flip(current, target)?;
            // Same order guards as the single-row `update_async` when the
            // write re-acquires a slice of the order total.
            if !is_capturing(current) && is_capturing(target) {
                if let Some(order_id) = payment.order_id {
                    check_order_capture_capacity_pg(
                        tx.as_mut(),
                        order_id.into_uuid(),
                        Some(raw_id),
                        payment.amount,
                        payment.currency,
                    )
                    .await?;
                }
            }
            let rows = sqlx::query(
                "UPDATE payments SET status = $1, external_id = $2, failure_reason = $3,
                 failure_code = $4, metadata = $5, updated_at = $6
                 WHERE id = $7 AND status = ANY($8)",
            )
            .bind(target.to_string())
            .bind(input.external_id.or(payment.external_id))
            .bind(input.failure_reason.or(payment.failure_reason))
            .bind(input.failure_code.or(payment.failure_code))
            .bind(input.metadata.or(payment.metadata))
            .bind(now)
            .bind(raw_id)
            .bind(statuses_allowing_transition_to(target))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .rows_affected();
            if rows == 0 {
                return Err(transition_conflict(current, target));
            }

            // Fetch the updated payment
            let updated_row = sqlx::query_as::<_, PaymentRow>(
                "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
                 amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
                 card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
                 description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
                 FROM payments WHERE id = $1"
            )
            .bind(raw_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            payments.push(Self::row_to_payment(updated_row)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(payments)
    }

    /// Delete multiple payments - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<PaymentId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;

        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match self.delete_async(raw_id).await {
                Ok(()) => result.record_success(raw_id),
                Err(e) => result.record_failure(index, Some(raw_id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple payments - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<PaymentId>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete associated refunds first (foreign key constraint)
        sqlx::query("DELETE FROM refunds WHERE payment_id = ANY($1)")
            .bind(&raw_ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Delete payments
        sqlx::query("DELETE FROM payments WHERE id = ANY($1)")
            .bind(&raw_ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple payments by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<PaymentId>) -> Result<Vec<Payment>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();

        let rows = sqlx::query_as::<_, PaymentRow>(
            "SELECT id, payment_number, order_id, invoice_id, customer_id, status, payment_method,
             amount, currency, amount_refunded, external_id, idempotency_key, processor, card_brand, card_last4,
             card_exp_month, card_exp_year, billing_email, billing_name, billing_address,
             description, failure_reason, failure_code, metadata, paid_at, version, created_at, updated_at
             FROM payments WHERE id = ANY($1)"
        )
        .bind(&raw_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut payments = Vec::with_capacity(rows.len());
        for row in rows {
            payments.push(Self::row_to_payment(row)?);
        }
        Ok(payments)
    }
}

impl PaymentRepository for PgPaymentRepository {
    fn create(&self, input: CreatePayment) -> Result<Payment> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PaymentId) -> Result<Option<Payment>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        super::block_on(self.get_by_number_async(payment_number))
    }

    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        super::block_on(self.get_by_external_id_async(external_id))
    }

    fn update(&self, id: PaymentId, input: UpdatePayment) -> Result<Payment> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        super::block_on(self.list_async(filter))
    }

    fn for_order(&self, order_id: OrderId) -> Result<Vec<Payment>> {
        super::block_on(self.for_order_async(order_id.into_uuid()))
    }

    fn for_invoice(&self, invoice_id: InvoiceId) -> Result<Vec<Payment>> {
        super::block_on(self.for_invoice_async(invoice_id.into_uuid()))
    }

    fn open_captures_for_order(&self, order_id: OrderId) -> Result<Vec<Payment>> {
        super::block_on(self.open_captures_for_order_async(order_id.into_uuid()))
    }

    fn mark_processing(&self, id: PaymentId) -> Result<Payment> {
        super::block_on(self.mark_processing_async(id.into_uuid()))
    }

    fn mark_completed(&self, id: PaymentId) -> Result<Payment> {
        super::block_on(self.mark_completed_async(id.into_uuid()))
    }

    fn mark_failed(&self, id: PaymentId, reason: &str, code: Option<&str>) -> Result<Payment> {
        super::block_on(self.mark_failed_async(id.into_uuid(), reason, code))
    }

    fn cancel(&self, id: PaymentId) -> Result<Payment> {
        super::block_on(self.cancel_async(id.into_uuid()))
    }

    fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        super::block_on(self.create_refund_async(input))
    }

    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        super::block_on(self.get_refund_async(id))
    }

    fn get_refunds(&self, payment_id: PaymentId) -> Result<Vec<Refund>> {
        super::block_on(self.get_refunds_async(payment_id.into_uuid()))
    }

    fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        super::block_on(self.complete_refund_async(id))
    }

    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        super::block_on(self.fail_refund_async(id, reason))
    }

    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        super::block_on(self.create_payment_method_async(input))
    }

    fn get_payment_methods(&self, customer_id: CustomerId) -> Result<Vec<PaymentMethod>> {
        super::block_on(self.get_payment_methods_async(customer_id.into_uuid()))
    }

    fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_payment_method_async(id))
    }

    fn set_default_payment_method(&self, customer_id: CustomerId, method_id: Uuid) -> Result<()> {
        super::block_on(self.set_default_payment_method_async(customer_id.into_uuid(), method_id))
    }

    fn count(&self, filter: PaymentFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    fn create_batch(&self, inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreatePayment>) -> Result<Vec<Payment>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<BatchResult<Payment>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<Vec<Payment>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<PaymentId>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<PaymentId>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<PaymentId>) -> Result<Vec<Payment>> {
        super::block_on(self.get_batch_async(ids))
    }
}
