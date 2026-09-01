//! SQLite implementation of payment repository

use super::kernel_outbox::append_kernel_event_tx;
use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_row, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
    with_immediate_transaction,
};
use crate::KernelOutboxEvent;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Row, params};
use stateset_core::{
    BatchResult, CommerceError, CreatePayment, CreatePaymentMethod, CreateRefund, CustomerId,
    InvoiceId, OrderId, Payment, PaymentFilter, PaymentId, PaymentMethod, PaymentRepository,
    PaymentTransactionStatus, Refund, RefundStatus, Result, UpdatePayment, Validate,
    generate_payment_number, generate_refund_number, validate_batch_size,
};
use uuid::Uuid;

/// Payment statuses that hold (or are about to hold) a slice of the order's
/// total: in-flight captures count as well as completed ones so two concurrent
/// captures cannot each pass the check against the same stale balance.
fn capturing_statuses() -> String {
    [
        PaymentTransactionStatus::Pending,
        PaymentTransactionStatus::Processing,
        PaymentTransactionStatus::RequiresAction,
        PaymentTransactionStatus::Completed,
        PaymentTransactionStatus::PartiallyRefunded,
        PaymentTransactionStatus::Refunded,
    ]
    .iter()
    .map(|s| format!("'{s}'"))
    .collect::<Vec<_>>()
    .join(", ")
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
/// that settle without an intermediate `Processing` step (the shipped
/// `mark_completed` contract, exercised by the invariant harness and by
/// `payments_test`). Every other answer — in particular every edge OUT of a
/// settled payment (`Completed`/`PartiallyRefunded`/`Refunded` ->
/// `Cancelled`/`Failed`, which would release the slice of the order total that
/// settled money is consuming and let the order be captured twice) — is the
/// enum's own.
pub(crate) fn payment_transition_allowed(
    from: PaymentTransactionStatus,
    to: PaymentTransactionStatus,
) -> bool {
    if from == PaymentTransactionStatus::Pending && to == PaymentTransactionStatus::Completed {
        return true;
    }
    from.can_transition_to(to)
}

/// SQL fragment listing every status a payment may currently be in for a write
/// that sets its status to `target`, for a `status IN (...)` predicate. Keeping
/// the check inside the UPDATE means a concurrent writer cannot slip between the
/// check and the write.
fn statuses_allowing_transition_to(target: PaymentTransactionStatus) -> String {
    ALL_PAYMENT_STATUSES
        .iter()
        .filter(|from| payment_transition_allowed(**from, target))
        .map(|status| format!("'{status}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Conflict error for a refused status write, naming the status the payment is
/// actually in. Worded identically in the Postgres backend.
fn transition_conflict(
    current: PaymentTransactionStatus,
    target: PaymentTransactionStatus,
) -> CommerceError {
    CommerceError::Conflict(format!("Payment is {current} and cannot transition to {target}"))
}

/// Wrap a domain error for propagation out of a `rusqlite` transaction closure;
/// `map_db_error` unwraps it back to its original variant.
fn domain_err(error: CommerceError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

/// Over-capture guard: reject when Σ(in-flight + completed captures for the
/// order, excluding `exclude_payment_id`) + `amount` would exceed
/// `orders.total_amount`. Must run inside the caller's IMMEDIATE transaction
/// so concurrent captures serialize on the write lock. A payment whose
/// `order_id` does not resolve to an order (standalone / external reference)
/// has nothing to cap against and passes.
///
/// `amount` / `total_amount` are TEXT money columns, so the sum is done in
/// `Decimal` in Rust rather than in SQL.
pub(crate) fn check_order_capture_capacity_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
    exclude_payment_id: Option<&str>,
    amount: rust_decimal::Decimal,
) -> std::result::Result<(), rusqlite::Error> {
    let total =
        match tx.query_row("SELECT total_amount FROM orders WHERE id = ?", [order_id], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(total) => parse_decimal_row(&total, "order", "total_amount")?,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
            Err(e) => return Err(e),
        };

    let sql = format!(
        "SELECT id, amount FROM payments WHERE order_id = ? AND status IN ({})",
        capturing_statuses()
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows =
        stmt.query_map([order_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut captured = rust_decimal::Decimal::ZERO;
    for row in rows {
        let (id, raw_amount) = row?;
        if exclude_payment_id == Some(id.as_str()) {
            continue;
        }
        captured += parse_decimal_row(&raw_amount, "payment", "amount")?;
    }

    if captured + amount > total {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            CommerceError::CaptureExceedsOrderTotal {
                order_id: parse_uuid_row(order_id, "order", "id")?,
                order_total: total.to_string(),
                already_captured: captured.to_string(),
                requested: amount.to_string(),
            },
        )));
    }
    Ok(())
}

#[derive(Debug)]
pub struct SqlitePaymentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePaymentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    pub(crate) fn row_to_payment(row: &Row<'_>) -> rusqlite::Result<Payment> {
        Ok(Payment {
            id: PaymentId::from(parse_uuid_row(&row.get::<_, String>("id")?, "payment", "id")?),
            payment_number: row.get("payment_number")?,
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "payment",
                "order_id",
            )?
            .map(OrderId::from),
            invoice_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("invoice_id")?,
                "payment",
                "invoice_id",
            )?,
            customer_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("customer_id")?,
                "payment",
                "customer_id",
            )?
            .map(CustomerId::from),
            status: parse_enum_row(&row.get::<_, String>("status")?, "payment", "status")?,
            payment_method: parse_enum_row(
                &row.get::<_, String>("payment_method")?,
                "payment",
                "payment_method",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "payment", "amount")?,
            currency: row.get("currency")?,
            amount_refunded: parse_decimal_row(
                &row.get::<_, String>("amount_refunded")?,
                "payment",
                "amount_refunded",
            )?,
            external_id: row.get("external_id")?,
            idempotency_key: row.get("idempotency_key")?,
            processor: row.get("processor")?,
            card_brand: match row.get::<_, Option<String>>("card_brand")? {
                Some(value) => Some(parse_enum_row(&value, "payment", "card_brand")?),
                None => None,
            },
            card_last4: row.get("card_last4")?,
            card_exp_month: row.get("card_exp_month")?,
            card_exp_year: row.get("card_exp_year")?,
            // Blockchain/Stablecoin fields
            blockchain_network: match row
                .get::<_, Option<String>>("blockchain_network")
                .ok()
                .flatten()
            {
                Some(value) => Some(parse_enum_row(&value, "payment", "blockchain_network")?),
                None => None,
            },
            stablecoin_type: match row.get::<_, Option<String>>("stablecoin_type").ok().flatten() {
                Some(value) => Some(parse_enum_row(&value, "payment", "stablecoin_type")?),
                None => None,
            },
            from_wallet_address: row.get("from_wallet_address").ok().flatten(),
            to_wallet_address: row.get("to_wallet_address").ok().flatten(),
            tx_hash: row.get("tx_hash").ok().flatten(),
            block_number: row.get("block_number").ok().flatten(),
            confirmations: row.get("confirmations").ok().flatten(),
            token_address: row.get("token_address").ok().flatten(),
            ves_intent_id: row.get("ves_intent_id").ok().flatten(),
            billing_email: row.get("billing_email")?,
            billing_name: row.get("billing_name")?,
            billing_address: row.get("billing_address")?,
            description: row.get("description")?,
            failure_reason: row.get("failure_reason")?,
            failure_code: row.get("failure_code")?,
            metadata: row.get("metadata")?,
            paid_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("paid_at")?,
                "payment",
                "paid_at",
            )?,
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "payment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "payment",
                "updated_at",
            )?,
        })
    }

    pub(crate) fn row_to_refund(row: &Row<'_>) -> rusqlite::Result<Refund> {
        Ok(Refund {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "refund", "id")?,
            refund_number: row.get("refund_number")?,
            payment_id: PaymentId::from(parse_uuid_row(
                &row.get::<_, String>("payment_id")?,
                "refund",
                "payment_id",
            )?),
            status: parse_enum_row(&row.get::<_, String>("status")?, "refund", "status")?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "refund", "amount")?,
            currency: row.get("currency")?,
            reason: row.get("reason")?,
            external_id: row.get("external_id")?,
            idempotency_key: row.get("idempotency_key")?,
            failure_reason: row.get("failure_reason")?,
            notes: row.get("notes")?,
            refunded_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("refunded_at")?,
                "refund",
                "refunded_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "refund",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "refund",
                "updated_at",
            )?,
        })
    }

    fn row_to_payment_method(row: &Row<'_>) -> rusqlite::Result<PaymentMethod> {
        Ok(PaymentMethod {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "payment_method", "id")?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "payment_method",
                "customer_id",
            )?),
            method_type: parse_enum_row(
                &row.get::<_, String>("method_type")?,
                "payment_method",
                "method_type",
            )?,
            is_default: row.get::<_, i32>("is_default")? != 0,
            card_brand: match row.get::<_, Option<String>>("card_brand")? {
                Some(value) => Some(parse_enum_row(&value, "payment_method", "card_brand")?),
                None => None,
            },
            card_last4: row.get("card_last4")?,
            card_exp_month: row.get("card_exp_month")?,
            card_exp_year: row.get("card_exp_year")?,
            cardholder_name: row.get("cardholder_name")?,
            bank_name: row.get("bank_name")?,
            account_last4: row.get("account_last4")?,
            // Blockchain/Wallet fields
            wallet_address: row.get("wallet_address").ok().flatten(),
            blockchain_network: match row
                .get::<_, Option<String>>("blockchain_network")
                .ok()
                .flatten()
            {
                Some(value) => {
                    Some(parse_enum_row(&value, "payment_method", "blockchain_network")?)
                }
                None => None,
            },
            stablecoin_type: match row.get::<_, Option<String>>("stablecoin_type").ok().flatten() {
                Some(value) => Some(parse_enum_row(&value, "payment_method", "stablecoin_type")?),
                None => None,
            },
            external_id: row.get("external_id")?,
            billing_address: row.get("billing_address")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "payment_method",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "payment_method",
                "updated_at",
            )?,
        })
    }

    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM payments WHERE idempotency_key = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([key], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_refund_by_idempotency_key(&self, key: &str) -> Result<Option<Refund>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM refunds WHERE idempotency_key = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([key], Self::row_to_refund);
        match result {
            Ok(refund) => Ok(Some(refund)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }
}

impl PaymentRepository for SqlitePaymentRepository {
    fn create(&self, input: CreatePayment) -> Result<Payment> {
        input.validate()?;
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key(key)? {
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
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

        // The over-capture check and the INSERT share one IMMEDIATE transaction
        // (same pattern as `create_refund`'s over-refund guard).
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(order_id) = input.order_id {
                check_order_capture_capacity_tx(tx, &order_id.to_string(), None, input.amount)?;
            }
            tx.execute(
                "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    payment_number,
                    input.order_id.map(|id| id.to_string()),
                    input.invoice_id.map(|id| id.to_string()),
                    input.customer_id.map(|id| id.to_string()),
                    PaymentTransactionStatus::Pending.to_string(),
                    input.payment_method.to_string(),
                    input.amount.to_string(),
                    input.currency.unwrap_or_default(),
                    "0",
                    input.external_id,
                    input.idempotency_key,
                    input.processor,
                    input.card_brand.map(|b| b.to_string()),
                    input.card_last4,
                    input.card_exp_month,
                    input.card_exp_year,
                    input.billing_email,
                    input.billing_name,
                    input.billing_address,
                    input.description,
                    input.metadata,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;
            append_kernel_event_tx(tx, &outbox_event)?;
            Ok(())
        })?;

        self.get(PaymentId::from(id))?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: PaymentId) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payments WHERE id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM payments WHERE payment_number = ?")
            .map_err(map_db_error)?;
        let result = stmt.query_row([payment_number], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt =
            conn.prepare("SELECT * FROM payments WHERE external_id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([external_id], Self::row_to_payment);
        match result {
            Ok(payment) => Ok(Some(payment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Update a payment.
    ///
    /// The read, the transition check and the write share ONE `IMMEDIATE`
    /// transaction, and the status check is a `status IN (...)` predicate on the
    /// UPDATE itself, so a concurrent writer cannot slip between them.
    /// (Previously the status was written unconditionally on a lock-free
    /// connection, which let `cancel`/`mark_failed` — both of which funnel
    /// through here — flip a `Completed` payment into a status that releases its
    /// slice of the order total, so the same order could be captured twice.)
    ///
    /// A request that does not change the status (`input.status == None`, or the
    /// status it already has) is always a legal self-transition and never
    /// conflicts.
    fn update(&self, id: PaymentId, input: UpdatePayment) -> Result<Payment> {
        let now = chrono::Utc::now();

        with_immediate_transaction(&self.pool, |tx| {
            let payment = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_payment,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => domain_err(CommerceError::NotFound),
                    other => other,
                })?;
            let current = payment.status;
            let target = input.status.unwrap_or(current);

            let sql = format!(
                "UPDATE payments SET status = ?, external_id = ?, failure_reason = ?,
                 failure_code = ?, metadata = ?, updated_at = ? WHERE id = ? AND status IN ({})",
                statuses_allowing_transition_to(target)
            );
            let rows = tx.execute(
                &sql,
                params![
                    target.to_string(),
                    input.external_id.clone().or(payment.external_id),
                    input.failure_reason.clone().or(payment.failure_reason),
                    input.failure_code.clone().or(payment.failure_code),
                    input.metadata.clone().or(payment.metadata),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )?;
            if rows == 0 {
                return Err(domain_err(transition_conflict(current, target)));
            }
            Ok(())
        })?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT * FROM payments WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }
        if let Some(invoice_id) = &filter.invoice_id {
            sql.push_str(" AND invoice_id = ?");
            params_vec.push(Box::new(invoice_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let rows =
            stmt.query_map(params_refs.as_slice(), Self::row_to_payment).map_err(map_db_error)?;

        let mut payments = Vec::new();
        for row in rows {
            payments.push(row.map_err(map_db_error)?);
        }
        Ok(payments)
    }

    fn for_order(&self, order_id: OrderId) -> Result<Vec<Payment>> {
        self.list(PaymentFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn for_invoice(&self, invoice_id: InvoiceId) -> Result<Vec<Payment>> {
        self.list(PaymentFilter { invoice_id: Some(invoice_id.into()), ..Default::default() })
    }

    fn mark_processing(&self, id: PaymentId) -> Result<Payment> {
        self.update(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Processing),
                ..Default::default()
            },
        )
    }

    fn mark_completed(&self, id: PaymentId) -> Result<Payment> {
        let now = chrono::Utc::now();

        let target = PaymentTransactionStatus::Completed;

        with_immediate_transaction(&self.pool, |tx| {
            // Two guards, in this order:
            //   1. the state machine — only a payment that may legally reach
            //      `Completed` may be completed (never a cancelled/failed/
            //      refunded one);
            //   2. the order's capacity, re-checked at completion time: a
            //      payment that was failed/cancelled while still in flight (and
            //      so released its slice of the total) must not be completed on
            //      top of captures made since.
            let (raw_status, order_id, raw_amount): (String, Option<String>, String) = tx
                .query_row(
                    "SELECT status, order_id, amount FROM payments WHERE id = ?",
                    [id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => domain_err(CommerceError::NotFound),
                    other => other,
                })?;
            let current: PaymentTransactionStatus =
                parse_enum_row(&raw_status, "payment", "status")?;
            if !payment_transition_allowed(current, target) {
                return Err(domain_err(transition_conflict(current, target)));
            }

            if let Some(order_id) = order_id {
                let amount = parse_decimal_row(&raw_amount, "payment", "amount")?;
                check_order_capture_capacity_tx(tx, &order_id, Some(&id.to_string()), amount)?;
            }

            let sql = format!(
                "UPDATE payments SET status = ?, paid_at = ?, updated_at = ?
                 WHERE id = ? AND status IN ({})",
                statuses_allowing_transition_to(target)
            );
            let rows = tx.execute(
                &sql,
                params![target.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
            )?;
            if rows == 0 {
                return Err(domain_err(transition_conflict(current, target)));
            }
            Ok(())
        })?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    /// Mark a payment failed.
    ///
    /// Status-guarded, for the same reason as [`Self::update`]: a `Completed`
    /// payment is settled money whose refund ledger points at it, and `failed`
    /// is not a capturing status — flipping it would release the order-total
    /// slice the capture is consuming and let the order be captured again.
    fn mark_failed(&self, id: PaymentId, reason: &str, code: Option<&str>) -> Result<Payment> {
        let now = chrono::Utc::now();
        let target = PaymentTransactionStatus::Failed;

        with_immediate_transaction(&self.pool, |tx| {
            let raw_status: String = tx
                .query_row("SELECT status FROM payments WHERE id = ?", [id.to_string()], |row| {
                    row.get(0)
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => domain_err(CommerceError::NotFound),
                    other => other,
                })?;
            let current: PaymentTransactionStatus =
                parse_enum_row(&raw_status, "payment", "status")?;

            let sql = format!(
                "UPDATE payments SET status = ?, failure_reason = ?, failure_code = ?,
                 updated_at = ? WHERE id = ? AND status IN ({})",
                statuses_allowing_transition_to(target)
            );
            let rows = tx.execute(
                &sql,
                params![target.to_string(), reason, code, now.to_rfc3339(), id.to_string()],
            )?;
            if rows == 0 {
                return Err(domain_err(transition_conflict(current, target)));
            }
            Ok(())
        })?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    /// Cancel a payment. Routes through [`Self::update`], so the state machine
    /// guard applies: a `Completed`/`PartiallyRefunded`/`Refunded` payment
    /// cannot be cancelled.
    fn cancel(&self, id: PaymentId) -> Result<Payment> {
        self.update(
            id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Cancelled),
                ..Default::default()
            },
        )
    }

    fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_refund_by_idempotency_key(key)? {
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let refund_number = generate_refund_number();
        let payment_id = input.payment_id;

        // The read of the payment, the over-refund validation, and the refund
        // INSERT all run inside ONE `IMMEDIATE` transaction. IMMEDIATE acquires
        // the database write lock up front, so a concurrent `create_refund` for
        // the same payment is serialized rather than racing: each caller sees
        // the other's freshly-inserted in-flight refund and cannot both pass the
        // remaining-balance check. (Previously the read+validate happened on a
        // separate, lock-free connection from the INSERT, so two callers could
        // each validate against the same stale balance and together over-refund
        // the payment once both were completed.)
        //
        // Domain failures (`NotFound`, `validate_refund`'s `ValidationError`)
        // are smuggled out of the closure as `ToSqlConversionFailure(CommerceError)`
        // and unwrapped back to their original variants by `map_db_error`.
        with_immediate_transaction(&self.pool, |tx| {
            let mut payment = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [payment_id.to_string()],
                    Self::row_to_payment,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::NotFound))
                    }
                    other => other,
                })?;

            input
                .validate_for_currency(payment.currency)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            // Reserve against in-flight (non-terminal) refunds as well as the
            // already-committed `amount_refunded`. A `Pending`/`Processing`
            // refund has not yet folded its amount into `amount_refunded`, but
            // it WILL once completed, so it must count against the remaining
            // refundable balance to prevent concurrent over-refund. `Failed` /
            // `Cancelled` refunds release their reservation and are excluded.
            //
            // `amount` is a TEXT column, so the amounts are read as text and
            // summed with `rust_decimal::Decimal` in Rust. Doing `SUM(amount)`
            // in SQL would coerce the TEXT values to IEEE-754 floats (the same
            // money-precision defect avoided on the `complete_refund` write
            // path).
            let mut in_flight = rust_decimal::Decimal::ZERO;
            {
                let mut stmt = tx.prepare(
                    "SELECT amount FROM refunds \
                     WHERE payment_id = ? AND status IN ('pending', 'processing')",
                )?;
                let rows = stmt.query_map([payment_id.to_string()], |row| {
                    let amount: String = row.get(0)?;
                    parse_decimal_row(&amount, "refund", "amount")
                })?;
                for row in rows {
                    in_flight += row?;
                }
            }

            // Fold the in-flight reservation into the payment's refunded total so
            // the unmodified `validate_refund` guard sees the true remaining
            // balance. `validate_refund` still owns all of the rules (refundable
            // status, positive amount, not exceeding remaining).
            payment.amount_refunded += in_flight;
            let refund_amount = payment
                .validate_refund(input.amount)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            tx.execute(
                "INSERT INTO refunds (id, refund_number, payment_id, status, amount, currency, reason, external_id, idempotency_key, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    refund_number,
                    payment_id.to_string(),
                    RefundStatus::Pending.to_string(),
                    refund_amount.to_string(),
                    payment.currency,
                    input.reason,
                    input.external_id,
                    input.idempotency_key,
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            let outbox_event = KernelOutboxEvent::domain(
                "payments.refund_created.v1",
                "refund",
                id.to_string(),
                serde_json::json!({
                    "refund_id": id.to_string(),
                    "refund_number": refund_number,
                    "payment_id": payment_id.to_string(),
                    "amount": refund_amount.to_string(),
                    "currency": payment.currency.as_str(),
                    "status": RefundStatus::Pending.to_string(),
                }),
                input.idempotency_key.clone(),
            );
            append_kernel_event_tx(tx, &outbox_event)?;

            Ok(())
        })?;

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM refunds WHERE id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_refund);
        match result {
            Ok(refund) => Ok(Some(refund)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_refunds(&self, payment_id: PaymentId) -> Result<Vec<Refund>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT * FROM refunds WHERE payment_id = ? ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let rows =
            stmt.query_map([payment_id.to_string()], Self::row_to_refund).map_err(map_db_error)?;

        let mut refunds = Vec::new();
        for row in rows {
            refunds.push(row.map_err(map_db_error)?);
        }
        Ok(refunds)
    }

    fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        let refund = self.get_refund(id)?.ok_or(CommerceError::NotFound)?;
        let now = chrono::Utc::now();

        with_immediate_transaction(&self.pool, |tx| {
            // Read the CURRENT status inside the write transaction so concurrent
            // completions serialize on the IMMEDIATE lock and only one of them
            // folds the refund into the payment.
            let current_status: RefundStatus = parse_enum_row(
                &tx.query_row(
                    "SELECT status FROM refunds WHERE id = ?",
                    params![id.to_string()],
                    |row| row.get::<_, String>(0),
                )?,
                "refund",
                "status",
            )?;

            // Idempotent: completing an already-completed refund is a no-op (a
            // duplicated payment-processor webhook or a retry must NOT re-add the
            // amount to the payment's `amount_refunded`).
            if current_status == RefundStatus::Completed {
                return Ok(());
            }
            // A failed/cancelled refund is terminal and cannot be completed.
            if current_status.is_terminal() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "Cannot complete a {current_status} refund"
                    )),
                )));
            }

            tx.execute(
                "UPDATE refunds SET status = ?, refunded_at = ?, updated_at = ? WHERE id = ?",
                params![
                    RefundStatus::Completed.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string()
                ],
            )?;

            // Update payment amount_refunded.
            //
            // `amount`/`amount_refunded` are TEXT columns (migration 006), so
            // doing the addition or comparison in SQL would coerce the values
            // to IEEE-754 floats (e.g. '0.10' + '0.20' = 0.30000000000000004,
            // and the `>= amount` status comparison would be wrong). Instead we
            // read the current values, compute the new balance and status with
            // `rust_decimal::Decimal` in Rust, and write the precomputed TEXT
            // values back as bound parameters.
            let (raw_payment_status, current_refunded, payment_amount): (String, String, String) =
                tx.query_row(
                    "SELECT status, amount_refunded, amount FROM payments WHERE id = ?",
                    params![refund.payment_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            let payment_status: PaymentTransactionStatus =
                parse_enum_row(&raw_payment_status, "payment", "status")?;
            let current_refunded =
                parse_decimal_row(&current_refunded, "payment", "amount_refunded")?;
            let payment_amount = parse_decimal_row(&payment_amount, "payment", "amount")?;

            let new_refunded = current_refunded + refund.amount;
            let new_status = if new_refunded >= payment_amount {
                PaymentTransactionStatus::Refunded
            } else {
                PaymentTransactionStatus::PartiallyRefunded
            };

            // The payment's status write goes through the same state-machine
            // guard as every other one: a refund may only fold itself into a
            // payment that can legally reach `Refunded`/`PartiallyRefunded`.
            let sql = format!(
                "UPDATE payments SET amount_refunded = ?, status = ?, updated_at = ?
                 WHERE id = ? AND status IN ({})",
                statuses_allowing_transition_to(new_status)
            );
            let rows = tx.execute(
                &sql,
                params![
                    new_refunded.to_string(),
                    new_status.to_string(),
                    now.to_rfc3339(),
                    refund.payment_id.to_string()
                ],
            )?;
            if rows == 0 {
                return Err(domain_err(transition_conflict(payment_status, new_status)));
            }

            Ok(())
        })?;

        self.get_refund(id)?.ok_or(CommerceError::NotFound)
    }

    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        let now = chrono::Utc::now();

        // Only an in-flight refund can fail. A `Completed` refund has already
        // been folded into `payments.amount_refunded`; flipping it to `failed`
        // would leave that balance inflated while the refund ledger no longer
        // shows the money (Σ completed refunds != amount_refunded). The status
        // guard lives in the UPDATE itself so a concurrent completion cannot
        // slip between a read and the write.
        let rows = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE refunds SET status = ?, failure_reason = ?, updated_at = ? \
                 WHERE id = ? AND status IN ('pending', 'processing')",
                params![RefundStatus::Failed.to_string(), reason, now.to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?
        };

        let refund = self.get_refund(id)?.ok_or(CommerceError::NotFound)?;
        // Idempotent: failing an already-failed refund is a no-op.
        if rows == 0 && refund.status != RefundStatus::Failed {
            return Err(CommerceError::ValidationError(format!(
                "Cannot fail a {} refund",
                refund.status
            )));
        }
        Ok(refund)
    }

    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        with_immediate_transaction(&self.pool, |tx| {
            // If setting as default, clear existing default
            if input.is_default.unwrap_or(false) {
                tx.execute(
                    "UPDATE payment_methods SET is_default = 0 WHERE customer_id = ?",
                    [input.customer_id.to_string()],
                )?;
            }

            tx.execute(
                "INSERT INTO payment_methods (id, customer_id, method_type, is_default, card_brand,
                 card_last4, card_exp_month, card_exp_year, cardholder_name, bank_name, account_last4,
                 external_id, billing_address, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    input.customer_id.to_string(),
                    input.method_type.to_string(),
                    i32::from(input.is_default.unwrap_or(false)),
                    input.card_brand.map(|b| b.to_string()),
                    input.card_last4,
                    input.card_exp_month,
                    input.card_exp_year,
                    input.cardholder_name,
                    input.bank_name,
                    input.account_last4,
                    input.external_id,
                    input.billing_address,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(())
        })?;

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt =
            conn.prepare("SELECT * FROM payment_methods WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_payment_method).map_err(map_db_error)
    }

    fn get_payment_methods(&self, customer_id: CustomerId) -> Result<Vec<PaymentMethod>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM payment_methods WHERE customer_id = ? ORDER BY is_default DESC, created_at DESC").map_err(map_db_error)?;
        let rows = stmt
            .query_map([customer_id.to_string()], Self::row_to_payment_method)
            .map_err(map_db_error)?;

        let mut methods = Vec::new();
        for row in rows {
            methods.push(row.map_err(map_db_error)?);
        }
        Ok(methods)
    }

    fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        conn.execute("DELETE FROM payment_methods WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn set_default_payment_method(&self, customer_id: CustomerId, method_id: Uuid) -> Result<()> {
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE payment_methods SET is_default = 0 WHERE customer_id = ?",
                [customer_id.to_string()],
            )?;

            tx.execute(
                "UPDATE payment_methods SET is_default = 1 WHERE id = ? AND customer_id = ?",
                params![method_id.to_string(), customer_id.to_string()],
            )?;

            Ok(())
        })?;

        Ok(())
    }

    fn count(&self, filter: PaymentFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM payments WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }
        if let Some(invoice_id) = &filter.invoice_id {
            sql.push_str(" AND invoice_id = ?");
            params_vec.push(Box::new(invoice_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreatePayment>) -> Result<Vec<Payment>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            input.validate()?;
            let id = Uuid::new_v4();
            let now = chrono::Utc::now();
            let payment_number = generate_payment_number();

            if let Some(order_id) = input.order_id {
                check_order_capture_capacity_tx(&tx, &order_id.to_string(), None, input.amount)
                    .map_err(map_db_error)?;
            }

            tx.execute(
                "INSERT INTO payments (id, payment_number, order_id, invoice_id, customer_id, status,
                 payment_method, amount, currency, amount_refunded, external_id, idempotency_key, processor,
                 card_brand, card_last4, card_exp_month, card_exp_year, billing_email, billing_name,
                 billing_address, description, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    payment_number.clone(),
                    input.order_id.map(|id| id.to_string()),
                    input.invoice_id.map(|id| id.to_string()),
                    input.customer_id.map(|id| id.to_string()),
                    PaymentTransactionStatus::Pending.to_string(),
                    input.payment_method.to_string(),
                    input.amount.to_string(),
                    input.currency.unwrap_or_default(),
                    "0",
                    input.external_id.clone(),
                    input.idempotency_key.clone(),
                    input.processor.clone(),
                    input.card_brand.map(|b| b.to_string()),
                    input.card_last4.clone(),
                    input.card_exp_month,
                    input.card_exp_year,
                    input.billing_email.clone(),
                    input.billing_name.clone(),
                    input.billing_address.clone(),
                    input.description.clone(),
                    input.metadata.clone(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;

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
            append_kernel_event_tx(&tx, &outbox_event).map_err(map_db_error)?;

            results.push(Payment {
                id: PaymentId::from(id),
                payment_number,
                order_id: input.order_id,
                invoice_id: input.invoice_id,
                customer_id: input.customer_id,
                status: PaymentTransactionStatus::Pending,
                payment_method: input.payment_method,
                amount: input.amount,
                currency: input.currency.unwrap_or_default(),
                amount_refunded: rust_decimal::Decimal::ZERO,
                external_id: input.external_id,
                idempotency_key: input.idempotency_key,
                processor: input.processor,
                card_brand: input.card_brand,
                card_last4: input.card_last4,
                card_exp_month: input.card_exp_month,
                card_exp_year: input.card_exp_year,
                // Blockchain/Stablecoin fields
                blockchain_network: input.blockchain_network,
                stablecoin_type: input.stablecoin_type,
                from_wallet_address: input.from_wallet_address,
                to_wallet_address: input.to_wallet_address,
                tx_hash: None,
                block_number: None,
                confirmations: None,
                token_address: input.token_address,
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

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<BatchResult<Payment>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(payment) => result.record_success(payment),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<Vec<Payment>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = chrono::Utc::now();

            // Get existing payment to merge with updates
            let payment: Payment = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_payment,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                    e => map_db_error(e),
                })?;

            // Same state-machine guard as the single-row `update`, inside the
            // batch's own transaction: one illegal status write aborts the whole
            // atomic batch rather than silently landing.
            let current = payment.status;
            let target = input.status.unwrap_or(current);
            let sql = format!(
                "UPDATE payments SET status = ?, external_id = ?, failure_reason = ?,
                 failure_code = ?, metadata = ?, updated_at = ? WHERE id = ? AND status IN ({})",
                statuses_allowing_transition_to(target)
            );
            let rows = tx
                .execute(
                    &sql,
                    params![
                        target.to_string(),
                        input.external_id.or(payment.external_id),
                        input.failure_reason.or(payment.failure_reason),
                        input.failure_code.or(payment.failure_code),
                        input.metadata.or(payment.metadata),
                        now.to_rfc3339(),
                        id.to_string(),
                    ],
                )
                .map_err(map_db_error)?;
            if rows == 0 {
                return Err(transition_conflict(current, target));
            }

            // Fetch the updated payment
            let updated_payment = tx
                .query_row(
                    "SELECT * FROM payments WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_payment,
                )
                .map_err(map_db_error)?;

            results.push(updated_payment);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<PaymentId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id: Uuid = id.into();
            let conn = self.conn()?;
            match conn.execute("DELETE FROM payments WHERE id = ?", [id.to_string()]) {
                Ok(rows) if rows > 0 => result.record_success(raw_id),
                Ok(_) => {
                    result.record_failure(index, Some(id.to_string()), &CommerceError::NotFound);
                }
                Err(e) => result.record_failure(index, Some(id.to_string()), &map_db_error(e)),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<PaymentId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| (*id).into()).collect();
        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        // Delete refunds associated with these payments first
        let sql = format!("DELETE FROM refunds WHERE payment_id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete payments
        let sql = format!("DELETE FROM payments WHERE id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<PaymentId>) -> Result<Vec<Payment>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| (*id).into()).collect();
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM payments WHERE id IN ({placeholders})");

        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let payments = stmt
            .query_map(params_refs.as_slice(), Self::row_to_payment)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(payments)
    }
}
