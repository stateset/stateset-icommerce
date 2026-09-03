//! PostgreSQL implementation of credit repository

use super::{block_on, map_db_error};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CommerceError, CreateCreditAccount, CreditAccount, CreditAccountFilter, CreditAccountStatus,
    CreditAgingBucket, CreditApplication, CreditApplicationFilter, CreditApplicationStatus,
    CreditCheckResult, CreditHold, CreditHoldFilter, CreditHoldStatus, CreditHoldType, CreditId,
    CreditRepository, CreditTransaction, CreditTransactionFilter, CreditTransactionType,
    CurrencyCode, CustomerCreditSummary, CustomerId, OrderId, PlaceCreditHold,
    RecordCreditTransaction, ReleaseCreditHold, Result, ReviewCreditApplication,
    SubmitCreditApplication, UpdateCreditAccount, generate_credit_application_number,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgCreditRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct CreditAccountRow {
    id: Uuid,
    customer_id: Uuid,
    credit_limit: Decimal,
    available_credit: Decimal,
    current_balance: Decimal,
    hold_amount: Decimal,
    currency: CurrencyCode,
    status: String,
    payment_terms: Option<String>,
    risk_rating: Option<String>,
    last_review_date: Option<NaiveDate>,
    next_review_date: Option<NaiveDate>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CreditHoldRow {
    id: Uuid,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    hold_type: String,
    hold_amount: Decimal,
    reason: String,
    status: String,
    placed_by: Option<String>,
    placed_at: DateTime<Utc>,
    released_by: Option<String>,
    released_at: Option<DateTime<Utc>>,
    release_notes: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CreditApplicationRow {
    id: Uuid,
    application_number: String,
    customer_id: Uuid,
    requested_limit: Decimal,
    approved_limit: Option<Decimal>,
    status: String,
    business_name: Option<String>,
    tax_id: Option<String>,
    years_in_business: Option<i32>,
    annual_revenue: Option<Decimal>,
    bank_reference: Option<String>,
    trade_references: Option<String>,
    submitted_at: DateTime<Utc>,
    reviewed_by: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    decision_notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CreditTransactionRow {
    id: Uuid,
    customer_id: Uuid,
    transaction_type: String,
    amount: Decimal,
    running_balance: Decimal,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgCreditRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_credit_account(row: CreditAccountRow) -> Result<CreditAccount> {
        let CreditAccountRow {
            id,
            customer_id,
            credit_limit,
            available_credit,
            current_balance,
            hold_amount,
            currency,
            status,
            payment_terms,
            risk_rating,
            last_review_date,
            next_review_date,
            notes,
            created_at,
            updated_at,
        } = row;

        let status: CreditAccountStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid credit_account.status '{}': {}",
                status, e
            ))
        })?;
        let risk_rating = match risk_rating {
            Some(value) if !value.trim().is_empty() => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid credit_account.risk_rating '{}': {}",
                    value, e
                ))
            })?),
            _ => None,
        };

        Ok(CreditAccount {
            id: id.into(),
            customer_id: customer_id.into(),
            credit_limit,
            available_credit,
            current_balance,
            hold_amount,
            currency,
            status,
            payment_terms,
            risk_rating,
            last_review_date: last_review_date.map(from_date),
            next_review_date: next_review_date.map(from_date),
            notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_credit_hold(row: CreditHoldRow) -> Result<CreditHold> {
        let CreditHoldRow {
            id,
            customer_id,
            order_id,
            hold_type,
            hold_amount,
            reason,
            status,
            placed_by,
            placed_at,
            released_by,
            released_at,
            release_notes,
            created_at,
        } = row;

        let hold_type: CreditHoldType = hold_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid credit_hold.hold_type '{}': {}",
                hold_type, e
            ))
        })?;
        let status: CreditHoldStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid credit_hold.status '{}': {}", status, e))
        })?;

        Ok(CreditHold {
            id,
            customer_id: customer_id.into(),
            order_id: order_id.map(Into::into),
            hold_type,
            hold_amount,
            reason,
            status,
            placed_by,
            placed_at,
            released_by,
            released_at,
            release_notes,
            created_at,
        })
    }

    fn row_to_credit_application(row: CreditApplicationRow) -> Result<CreditApplication> {
        let CreditApplicationRow {
            id,
            application_number,
            customer_id,
            requested_limit,
            approved_limit,
            status,
            business_name,
            tax_id,
            years_in_business,
            annual_revenue,
            bank_reference,
            trade_references,
            submitted_at,
            reviewed_by,
            reviewed_at,
            decision_notes,
            created_at,
            updated_at,
        } = row;

        let status: CreditApplicationStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid credit_application.status '{}': {}",
                status, e
            ))
        })?;

        Ok(CreditApplication {
            id,
            application_number,
            customer_id: customer_id.into(),
            requested_limit,
            approved_limit,
            status,
            business_name,
            tax_id,
            years_in_business,
            annual_revenue,
            bank_reference,
            trade_references,
            submitted_at,
            reviewed_by,
            reviewed_at,
            decision_notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_credit_transaction(row: CreditTransactionRow) -> Result<CreditTransaction> {
        let CreditTransactionRow {
            id,
            customer_id,
            transaction_type,
            amount,
            running_balance,
            reference_type,
            reference_id,
            notes,
            created_at,
        } = row;

        let transaction_type: CreditTransactionType = transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid credit_transaction.transaction_type '{}': {}",
                transaction_type, e
            ))
        })?;

        Ok(CreditTransaction {
            id,
            customer_id: customer_id.into(),
            transaction_type,
            amount,
            running_balance,
            reference_type,
            reference_id,
            notes,
            created_at,
        })
    }

    /// Recompute `available = limit - balance - holds` inside the caller's
    /// transaction, so it commits with whatever moved those numbers.
    ///
    /// A separate statement on the pool would leave a crash window in which
    /// `available_credit` disagreed with the columns it is derived from, and
    /// `check_credit` approves orders against that column.
    async fn recalculate_available_credit_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        customer_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE credit_accounts SET available_credit = credit_limit - current_balance - hold_amount
             WHERE customer_id = $1",
        )
        .bind(customer_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Release this order's active reservation (if any) and decrement the
    /// account hold by its amount, clamped at zero. Returns the hold that
    /// remains, so callers can enforce the credit line against it.
    async fn release_reservation_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        customer_id: Uuid,
        order_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Decimal> {
        let reserved: Decimal = sqlx::query_scalar(
            "UPDATE credit_reservations SET status = 'released', released_at = $1
             WHERE customer_id = $2 AND order_id = $3 AND status = 'active'
             RETURNING amount",
        )
        .bind(now)
        .bind(customer_id)
        .bind(order_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .unwrap_or(Decimal::ZERO);

        let remaining_hold: Decimal = sqlx::query_scalar(
            "UPDATE credit_accounts SET hold_amount = GREATEST(0, hold_amount - $1)
             WHERE customer_id = $2
             RETURNING hold_amount",
        )
        .bind(reserved)
        .bind(customer_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        Ok(remaining_hold)
    }

    /// Append a ledger row inside the caller's transaction.
    ///
    /// `running_balance` is passed in rather than re-derived: callers that have
    /// just moved the balance in this transaction know the post-transaction
    /// figure exactly, and re-reading it would double-count a payment.
    async fn insert_transaction_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        input: &RecordCreditTransaction,
        running_balance: Decimal,
        now: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO credit_transactions (id, customer_id, transaction_type, amount,
                running_balance, reference_type, reference_id, notes, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(input.customer_id)
        .bind(input.transaction_type.to_string())
        .bind(input.amount)
        .bind(running_balance)
        .bind(input.reference_type.clone())
        .bind(input.reference_id)
        .bind(input.notes.clone())
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Insert a credit account inside the caller's transaction.
    async fn create_credit_account_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        input: &CreateCreditAccount,
        now: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO credit_accounts (id, customer_id, credit_limit, available_credit, current_balance,
                hold_amount, currency, status, payment_terms, risk_rating, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(id)
        .bind(input.customer_id)
        .bind(input.credit_limit)
        .bind(input.credit_limit)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(input.currency.unwrap_or(CurrencyCode::USD))
        .bind(CreditAccountStatus::Active.to_string())
        .bind(input.payment_terms.clone())
        .bind(input.risk_rating.map(|r| r.to_string()))
        .bind(input.notes.clone())
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Move an account's credit limit, append the `limit_change` ledger row and
    /// recompute available credit — all inside the caller's transaction, so the
    /// limit and its audit trail can never disagree.
    async fn adjust_credit_limit_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        customer_id: Uuid,
        new_limit: Decimal,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let (old_limit, current_balance): (Decimal, Decimal) = sqlx::query_as(
            "SELECT credit_limit, current_balance FROM credit_accounts WHERE customer_id = $1 FOR UPDATE",
        )
        .bind(customer_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        sqlx::query(
            "UPDATE credit_accounts SET credit_limit = $1, updated_at = $2 WHERE customer_id = $3",
        )
        .bind(new_limit)
        .bind(now)
        .bind(customer_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::insert_transaction_tx(
            tx,
            Uuid::new_v4(),
            &RecordCreditTransaction {
                customer_id: customer_id.into(),
                transaction_type: CreditTransactionType::LimitChange,
                amount: new_limit - old_limit,
                reference_type: None,
                reference_id: None,
                notes: Some(reason.to_string()),
            },
            current_balance,
            now,
        )
        .await?;

        Self::recalculate_available_credit_tx(tx, customer_id).await
    }

    /// Lock the account row and return its parsed status, or `NotFound`.
    async fn locked_account_status_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        customer_id: Uuid,
    ) -> Result<CreditAccountStatus> {
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM credit_accounts WHERE customer_id = $1 FOR UPDATE",
        )
        .bind(customer_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let status = status.ok_or(CommerceError::NotFound)?;
        status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid credit_account.status '{status}': {e}"))
        })
    }

    /// Is a credit application still open to a decision?
    ///
    /// `approved`, `denied` and `withdrawn` are terminal; `pending`,
    /// `under_review` and `more_info_needed` are the working states. Mirrors
    /// the SQLite backend.
    const fn is_reviewable(status: CreditApplicationStatus) -> bool {
        matches!(
            status,
            CreditApplicationStatus::Pending
                | CreditApplicationStatus::UnderReview
                | CreditApplicationStatus::MoreInfoNeeded
        )
    }

    pub async fn create_credit_account_async(
        &self,
        input: CreateCreditAccount,
    ) -> Result<CreditAccount> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::create_credit_account_tx(&mut tx, id, &input, now).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_credit_account_async(&self, id: Uuid) -> Result<Option<CreditAccount>> {
        let row = sqlx::query_as::<_, CreditAccountRow>(
            "SELECT id, customer_id, credit_limit, available_credit, current_balance, hold_amount,
                    currency, status, payment_terms, risk_rating, last_review_date, next_review_date,
                    notes, created_at, updated_at
             FROM credit_accounts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_credit_account).transpose()
    }

    pub async fn get_credit_account_by_customer_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CreditAccount>> {
        let row = sqlx::query_as::<_, CreditAccountRow>(
            "SELECT id, customer_id, credit_limit, available_credit, current_balance, hold_amount,
                    currency, status, payment_terms, risk_rating, last_review_date, next_review_date,
                    notes, created_at, updated_at
             FROM credit_accounts WHERE customer_id = $1",
        )
        .bind(customer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_credit_account).transpose()
    }

    pub async fn update_credit_account_async(
        &self,
        id: Uuid,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount> {
        // The status guard, the write and the available-credit recompute share
        // one transaction: `credit_limit` feeds `available_credit`, so
        // committing one without the other leaves `check_credit` approving
        // orders against a stale line.
        let now = Utc::now();
        let account = self.get_credit_account_async(id).await?.ok_or(CommerceError::NotFound)?;
        let customer_id = account.customer_id.into_uuid();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let current = Self::locked_account_status_tx(&mut tx, customer_id).await?;
        // `closed` is terminal: an account may not be reopened by a blind
        // update, only by opening a new one.
        if current == CreditAccountStatus::Closed
            && input.status.is_some_and(|next| next != CreditAccountStatus::Closed)
        {
            return Err(CommerceError::Conflict(format!(
                "Credit account {id} is '{current}' and cannot be reopened"
            )));
        }

        let result = sqlx::query(
            "UPDATE credit_accounts SET
                credit_limit = COALESCE($1, credit_limit),
                status = COALESCE($2, status),
                payment_terms = COALESCE($3, payment_terms),
                risk_rating = COALESCE($4, risk_rating),
                notes = COALESCE($5, notes),
                updated_at = $6
             WHERE id = $7 AND status = $8",
        )
        .bind(input.credit_limit)
        .bind(input.status.map(|s| s.to_string()))
        .bind(input.payment_terms)
        .bind(input.risk_rating.map(|r| r.to_string()))
        .bind(input.notes)
        .bind(now)
        .bind(id)
        .bind(current.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::Conflict(format!(
                "Credit account {id} changed concurrently; expected status '{current}'"
            )));
        }

        Self::recalculate_available_credit_tx(&mut tx, customer_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_credit_accounts_async(
        &self,
        filter: CreditAccountFilter,
    ) -> Result<Vec<CreditAccount>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, customer_id, credit_limit, available_credit, current_balance, hold_amount,
                    currency, status, payment_terms, risk_rating, last_review_date, next_review_date,
                    notes, created_at, updated_at
             FROM credit_accounts WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(risk_rating) = filter.risk_rating {
            builder.push(" AND risk_rating = ").push_bind(risk_rating.to_string());
        }
        if filter.over_limit == Some(true) {
            builder.push(" AND current_balance > credit_limit");
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CreditAccountRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_credit_account).collect::<Result<Vec<_>>>()
    }

    pub async fn adjust_credit_limit_async(
        &self,
        customer_id: Uuid,
        new_limit: Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        // The limit write, its `limit_change` ledger row and the available-credit
        // recompute are ONE transaction: a limit that moved without an audit row
        // (or vice versa) is unreconcilable.
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::adjust_credit_limit_tx(&mut tx, customer_id, new_limit, reason, now).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn suspend_credit_account_async(
        &self,
        customer_id: Uuid,
        reason: &str,
    ) -> Result<CreditAccount> {
        // `closed` is terminal — suspending a closed account would resurrect a
        // line nobody re-underwrote. The locked status read and the guarded
        // write share one transaction, so a concurrent close cannot slip
        // between them; a zero-row UPDATE means exactly that race.
        let now = Utc::now();
        let note = format!("\nSuspended: {reason}");

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let current = Self::locked_account_status_tx(&mut tx, customer_id).await?;
        if current == CreditAccountStatus::Closed {
            return Err(CommerceError::Conflict(format!(
                "Credit account for customer {customer_id} is '{current}' and cannot be suspended"
            )));
        }

        let result = sqlx::query(
            "UPDATE credit_accounts SET status = $1, notes = COALESCE(notes, '') || $2, updated_at = $3
             WHERE customer_id = $4 AND status = $5",
        )
        .bind(CreditAccountStatus::Suspended.to_string())
        .bind(note)
        .bind(now)
        .bind(customer_id)
        .bind(current.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::Conflict(format!(
                "Credit account for customer {customer_id} changed concurrently; expected status '{current}'"
            )));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn reactivate_credit_account_async(
        &self,
        customer_id: Uuid,
    ) -> Result<CreditAccount> {
        // Mirror of `suspend_credit_account_async`: `closed` is terminal, so a
        // closed account cannot be flipped back to active.
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let current = Self::locked_account_status_tx(&mut tx, customer_id).await?;
        if current == CreditAccountStatus::Closed {
            return Err(CommerceError::Conflict(format!(
                "Credit account for customer {customer_id} is '{current}' and cannot be reactivated"
            )));
        }

        let result = sqlx::query(
            "UPDATE credit_accounts SET status = $1, updated_at = $2
             WHERE customer_id = $3 AND status = $4",
        )
        .bind(CreditAccountStatus::Active.to_string())
        .bind(now)
        .bind(customer_id)
        .bind(current.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::Conflict(format!(
                "Credit account for customer {customer_id} changed concurrently; expected status '{current}'"
            )));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn check_credit_async(
        &self,
        customer_id: Uuid,
        order_amount: Decimal,
    ) -> Result<CreditCheckResult> {
        let now = Utc::now();
        let account = self.get_credit_account_by_customer_async(customer_id).await?;

        match account {
            Some(acc) => {
                let approved = acc.status == CreditAccountStatus::Active
                    && acc.available_credit >= order_amount;
                let reason = if !approved {
                    if acc.status != CreditAccountStatus::Active {
                        Some(format!("Account status: {}", acc.status))
                    } else {
                        Some(format!(
                            "Insufficient credit: available {}, required {}",
                            acc.available_credit, order_amount
                        ))
                    }
                } else {
                    None
                };

                Ok(CreditCheckResult {
                    customer_id: customer_id.into(),
                    order_amount,
                    credit_limit: acc.credit_limit,
                    available_credit: acc.available_credit,
                    current_balance: acc.current_balance,
                    approved,
                    reason,
                    requires_approval: !approved && acc.status == CreditAccountStatus::Active,
                    checked_at: now,
                })
            }
            None => Ok(CreditCheckResult {
                customer_id: customer_id.into(),
                order_amount,
                credit_limit: Decimal::ZERO,
                available_credit: Decimal::ZERO,
                current_balance: Decimal::ZERO,
                approved: false,
                reason: Some("No credit account found".to_string()),
                requires_approval: true,
                checked_at: now,
            }),
        }
    }

    pub async fn reserve_credit_async(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Credit reservation amount must be positive".to_string(),
            ));
        }

        let now = Utc::now();
        let id = Uuid::new_v4();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // The reservation must fit within the customer's remaining credit
        // (limit minus balance minus existing holds) — otherwise a hold can
        // extend credit past the agreed line. Lock the account row so
        // concurrent reservations serialize against the same headroom.
        let (credit_limit, current_balance, hold_amount): (Decimal, Decimal, Decimal) =
            sqlx::query_as(
                "SELECT credit_limit, current_balance, hold_amount FROM credit_accounts
                 WHERE customer_id = $1 FOR UPDATE",
            )
            .bind(customer_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let available = credit_limit - current_balance - hold_amount;
        if amount > available {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient available credit: requested {amount}, available {available}"
            )));
        }

        sqlx::query(
            "INSERT INTO credit_reservations (id, customer_id, order_id, amount, status, created_at)
             VALUES ($1, $2, $3, $4, 'active', $5)",
        )
        .bind(id)
        .bind(customer_id)
        .bind(order_id)
        .bind(amount)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE credit_accounts SET hold_amount = hold_amount + $1 WHERE customer_id = $2",
        )
        .bind(amount)
        .bind(customer_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Recompute inside the same transaction: a hold must never commit
        // without the `available_credit` it invalidates.
        Self::recalculate_available_credit_tx(&mut tx, customer_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn release_credit_reservation_async(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
    ) -> Result<CreditAccount> {
        // Read, release, hold decrement and available-credit recompute are ONE
        // transaction. Run as four separate statements on the pool, a
        // concurrent release could read the same reservation and decrement the
        // hold twice, and a crash could release a reservation whose hold was
        // never given back.
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::release_reservation_tx(&mut tx, customer_id, order_id, now).await?;
        Self::recalculate_available_credit_tx(&mut tx, customer_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn charge_credit_async(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Credit charge amount must be positive".to_string(),
            ));
        }

        // The whole charge is ONE transaction: limit check, balance write,
        // reservation release and ledger row commit or roll back together.
        //
        // Splitting them (balance in one transaction, then the release and the
        // ledger row on the pool) let a crash in between double-consume the
        // customer's credit with no reconciliation path, and left the ledger
        // disagreeing with the balance. Locking the account row FOR UPDATE also
        // serializes concurrent charges, so two of them cannot both pass the
        // limit check against the same stale balance.
        //
        // Limit rule: `new_balance + remaining_hold <= credit_limit`. Charging
        // against *this* order's own reservation converts hold into balance, so
        // the reservation is consumed first and only the holds still
        // outstanding for OTHER orders bind the charge. Checking the balance
        // alone (the previous rule) let a charge for one order spend credit
        // already reserved for another, driving `available_credit` — defined as
        // `limit - balance - holds` — negative.
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let (credit_limit, current_balance): (Decimal, Decimal) = sqlx::query_as(
            "SELECT credit_limit, current_balance FROM credit_accounts WHERE customer_id = $1 FOR UPDATE",
        )
        .bind(customer_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let new_balance = current_balance + amount;

        // Consume this order's reservation before testing the line, so a charge
        // against your own hold is not counted twice. A rejected charge rolls
        // the release back with everything else, preserving the reservation.
        let remaining_hold =
            Self::release_reservation_tx(&mut tx, customer_id, order_id, now).await?;

        if new_balance + remaining_hold > credit_limit {
            return Err(CommerceError::ValidationError(format!(
                "Charge would exceed credit limit: new balance {new_balance}, \
                 holds {remaining_hold}, limit {credit_limit}"
            )));
        }

        sqlx::query(
            "UPDATE credit_accounts SET current_balance = $1, updated_at = $2 WHERE customer_id = $3",
        )
        .bind(new_balance)
        .bind(now)
        .bind(customer_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::insert_transaction_tx(
            &mut tx,
            Uuid::new_v4(),
            &RecordCreditTransaction {
                customer_id: customer_id.into(),
                transaction_type: CreditTransactionType::Charge,
                amount,
                reference_type: Some("order".to_string()),
                reference_id: Some(order_id),
                notes: None,
            },
            new_balance,
            now,
        )
        .await?;

        Self::recalculate_available_credit_tx(&mut tx, customer_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn place_hold_async(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO credit_holds (id, customer_id, order_id, hold_type, hold_amount, reason,
                status, placed_by, placed_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'active', $7, $8, $9)",
        )
        .bind(id)
        .bind(input.customer_id)
        .bind(input.order_id)
        .bind(input.hold_type.to_string())
        .bind(input.hold_amount)
        .bind(&input.reason)
        .bind(input.placed_by.clone())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CreditHold {
            id,
            customer_id: input.customer_id,
            order_id: input.order_id,
            hold_type: input.hold_type,
            hold_amount: input.hold_amount,
            reason: input.reason,
            status: CreditHoldStatus::Active,
            placed_by: input.placed_by,
            placed_at: now,
            released_by: None,
            released_at: None,
            release_notes: None,
            created_at: now,
        })
    }

    pub async fn get_hold_async(&self, id: Uuid) -> Result<Option<CreditHold>> {
        let row = sqlx::query_as::<_, CreditHoldRow>(
            "SELECT id, customer_id, order_id, hold_type, hold_amount, reason, status,
                    placed_by, placed_at, released_by, released_at, release_notes, created_at
             FROM credit_holds WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_credit_hold).transpose()
    }

    pub async fn list_holds_async(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, customer_id, order_id, hold_type, hold_amount, reason, status,
                    placed_by, placed_at, released_by, released_at, release_notes, created_at
             FROM credit_holds WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(hold_type) = filter.hold_type {
            builder.push(" AND hold_type = ").push_bind(hold_type.to_string());
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CreditHoldRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_credit_hold).collect::<Result<Vec<_>>>()
    }

    pub async fn release_hold_async(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        // Only an active hold may be released. Previously unguarded: releasing
        // an already-released (or expired) hold silently succeeded and
        // overwrote the original `released_by`/`released_at`/`release_notes`,
        // destroying the audit trail of who actually cleared it.
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM credit_holds WHERE id = $1 FOR UPDATE")
                .bind(input.hold_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let status = status.ok_or(CommerceError::NotFound)?;
        let current: CreditHoldStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid credit_hold.status '{status}': {e}"))
        })?;
        if current != CreditHoldStatus::Active {
            return Err(CommerceError::Conflict(format!(
                "Credit hold {} is '{current}' and cannot be released",
                input.hold_id
            )));
        }

        let result = sqlx::query(
            "UPDATE credit_holds SET status = $1, released_by = $2, released_at = $3, release_notes = $4
             WHERE id = $5 AND status = $6",
        )
        .bind(CreditHoldStatus::Released.to_string())
        .bind(input.released_by.clone())
        .bind(now)
        .bind(input.release_notes.clone())
        .bind(input.hold_id)
        .bind(CreditHoldStatus::Active.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::Conflict(format!(
                "Credit hold {} was released concurrently",
                input.hold_id
            )));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_hold_async(input.hold_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_active_holds_async(&self, customer_id: Uuid) -> Result<Vec<CreditHold>> {
        self.list_holds_async(CreditHoldFilter {
            customer_id: Some(customer_id.into()),
            status: Some(CreditHoldStatus::Active),
            ..Default::default()
        })
        .await
    }

    pub async fn get_holds_for_order_async(&self, order_id: Uuid) -> Result<Vec<CreditHold>> {
        self.list_holds_async(CreditHoldFilter {
            order_id: Some(order_id.into()),
            ..Default::default()
        })
        .await
    }

    pub async fn submit_application_async(
        &self,
        input: SubmitCreditApplication,
    ) -> Result<CreditApplication> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let application_number = generate_credit_application_number();

        sqlx::query(
            "INSERT INTO credit_applications (id, application_number, customer_id, requested_limit,
                approved_limit, status, business_name, tax_id, years_in_business, annual_revenue,
                bank_reference, trade_references, submitted_at, reviewed_by, reviewed_at,
                decision_notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9, $10, $11, $12, NULL, NULL, NULL, $13, $14)",
        )
        .bind(id)
        .bind(&application_number)
        .bind(input.customer_id)
        .bind(input.requested_limit)
        .bind(CreditApplicationStatus::Pending.to_string())
        .bind(input.business_name)
        .bind(input.tax_id)
        .bind(input.years_in_business)
        .bind(input.annual_revenue)
        .bind(input.bank_reference)
        .bind(input.trade_references)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_application_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_application_async(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        let row = sqlx::query_as::<_, CreditApplicationRow>(
            "SELECT id, application_number, customer_id, requested_limit, approved_limit, status,
                    business_name, tax_id, years_in_business, annual_revenue, bank_reference,
                    trade_references, submitted_at, reviewed_by, reviewed_at, decision_notes,
                    created_at, updated_at
             FROM credit_applications WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_credit_application).transpose()
    }

    pub async fn list_applications_async(
        &self,
        filter: CreditApplicationFilter,
    ) -> Result<Vec<CreditApplication>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, application_number, customer_id, requested_limit, approved_limit, status,
                    business_name, tax_id, years_in_business, annual_revenue, bank_reference,
                    trade_references, submitted_at, reviewed_by, reviewed_at, decision_notes,
                    created_at, updated_at
             FROM credit_applications WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND submitted_at >= ").push_bind(from_date);
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND submitted_at <= ").push_bind(to_date_val);
        }

        builder.push(" ORDER BY submitted_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CreditApplicationRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_credit_application).collect::<Result<Vec<_>>>()
    }

    pub async fn review_application_async(
        &self,
        input: ReviewCreditApplication,
    ) -> Result<CreditApplication> {
        // A decision is once-only: `approved`, `denied` and `withdrawn` are
        // terminal. Previously unguarded, re-reviewing an approved application
        // re-ran the account side effects, so a second "approval" at any limit
        // silently rewrote the customer's credit line (and appended a second
        // `limit_change` ledger row) with no application-state trail.
        //
        // The decision and the account it creates or re-limits share ONE
        // transaction, so an approved application always has the matching
        // account.
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id, status FROM credit_applications WHERE id = $1 FOR UPDATE",
        )
        .bind(input.application_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let (customer_id, status) = row.ok_or(CommerceError::NotFound)?;
        let current: CreditApplicationStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid credit_application.status '{status}': {e}"
            ))
        })?;
        if !Self::is_reviewable(current) {
            return Err(CommerceError::Conflict(format!(
                "Credit application {} is '{current}' and has already been decided",
                input.application_id
            )));
        }

        let result = sqlx::query(
            "UPDATE credit_applications SET approved_limit = $1, status = $2, reviewed_by = $3,
                reviewed_at = $4, decision_notes = $5, updated_at = $6
             WHERE id = $7 AND status = $8",
        )
        .bind(input.approved_limit)
        .bind(input.status.to_string())
        .bind(&input.reviewed_by)
        .bind(now)
        .bind(input.decision_notes.clone())
        .bind(now)
        .bind(input.application_id)
        .bind(current.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::Conflict(format!(
                "Credit application {} was decided concurrently",
                input.application_id
            )));
        }

        // If approved, create or re-limit the credit account in the same
        // transaction as the decision.
        if input.status == CreditApplicationStatus::Approved {
            if let Some(limit) = input.approved_limit {
                let existing: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM credit_accounts WHERE customer_id = $1")
                        .bind(customer_id)
                        .fetch_optional(tx.as_mut())
                        .await
                        .map_err(map_db_error)?;

                if existing.is_some() {
                    Self::adjust_credit_limit_tx(
                        &mut tx,
                        customer_id,
                        limit,
                        "Credit application approved",
                        now,
                    )
                    .await?;
                } else {
                    Self::create_credit_account_tx(
                        &mut tx,
                        Uuid::new_v4(),
                        &CreateCreditAccount {
                            customer_id: customer_id.into(),
                            credit_limit: limit,
                            ..Default::default()
                        },
                        now,
                    )
                    .await?;
                }
            }
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_application_async(input.application_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn withdraw_application_async(&self, id: Uuid) -> Result<CreditApplication> {
        // Same terminal-state rule as `review_application_async`: an application
        // that has already been approved, denied or withdrawn cannot be
        // withdrawn (which previously erased the recorded decision's status).
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM credit_applications WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let status = status.ok_or(CommerceError::NotFound)?;
        let current: CreditApplicationStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid credit_application.status '{status}': {e}"
            ))
        })?;
        if !Self::is_reviewable(current) {
            return Err(CommerceError::Conflict(format!(
                "Credit application {id} is '{current}' and cannot be withdrawn"
            )));
        }

        let result = sqlx::query(
            "UPDATE credit_applications SET status = $1, updated_at = $2
             WHERE id = $3 AND status = $4",
        )
        .bind(CreditApplicationStatus::Withdrawn.to_string())
        .bind(now)
        .bind(id)
        .bind(current.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::Conflict(format!(
                "Credit application {id} was decided concurrently"
            )));
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_application_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn record_transaction_async(
        &self,
        input: RecordCreditTransaction,
    ) -> Result<CreditTransaction> {
        // The balance read and the ledger insert are ONE transaction with the
        // account row pinned `FOR UPDATE`, matching the SQLite backend's
        // `with_immediate_transaction`.
        //
        // Previously the `SELECT` ran on the pool (autocommit, no lock) and the
        // `INSERT` on a second pooled connection, with no `begin()` anywhere in
        // the method: a payment or charge committing in between left the row
        // stamped with a balance the account no longer held, so the ledger
        // overstated the receivable with no reconciliation path.
        //
        // A missing account is now `NotFound`. `unwrap_or(Decimal::ZERO)`
        // masked it, deriving the running balance from an invented zero — and
        // the two backends disagreed on the same call, SQLite's `query_row`
        // having always errored.
        let id = Uuid::new_v4();
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let current_balance: Decimal = sqlx::query_scalar(
            "SELECT current_balance FROM credit_accounts WHERE customer_id = $1 FOR UPDATE",
        )
        .bind(input.customer_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let running_balance = match input.transaction_type {
            CreditTransactionType::Payment | CreditTransactionType::CreditMemo => {
                current_balance - input.amount
            }
            _ => current_balance,
        };

        Self::insert_transaction_tx(&mut tx, id, &input, running_balance, now).await?;
        tx.commit().await.map_err(map_db_error)?;

        Ok(CreditTransaction {
            id,
            customer_id: input.customer_id,
            transaction_type: input.transaction_type,
            amount: input.amount,
            running_balance,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            notes: input.notes,
            created_at: now,
        })
    }

    pub async fn list_transactions_async(
        &self,
        filter: CreditTransactionFilter,
    ) -> Result<Vec<CreditTransaction>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, customer_id, transaction_type, amount, running_balance,
                    reference_type, reference_id, notes, created_at
             FROM credit_transactions WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(tx_type) = filter.transaction_type {
            builder.push(" AND transaction_type = ").push_bind(tx_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date_val);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CreditTransactionRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_credit_transaction).collect::<Result<Vec<_>>>()
    }

    pub async fn apply_payment_async(
        &self,
        customer_id: Uuid,
        amount: Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Payment amount must be positive".to_string(),
            ));
        }

        // Balance write, ledger row and available-credit recompute are ONE
        // transaction (see `charge_credit_async`): a crash can never bank a
        // payment the ledger does not show.
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let new_balance: Decimal = sqlx::query_scalar(
            "UPDATE credit_accounts
             SET current_balance = GREATEST(0, current_balance - $1), updated_at = $2
             WHERE customer_id = $3
             RETURNING current_balance",
        )
        .bind(amount)
        .bind(now)
        .bind(customer_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        // The ledger's running balance is the balance this transaction just
        // wrote. Re-deriving it (as the standalone `record_transaction_async`
        // must) would subtract the payment a second time.
        Self::insert_transaction_tx(
            &mut tx,
            Uuid::new_v4(),
            &RecordCreditTransaction {
                customer_id: customer_id.into(),
                transaction_type: CreditTransactionType::Payment,
                amount,
                reference_type: Some("payment".to_string()),
                reference_id,
                notes: None,
            },
            new_balance,
            now,
        )
        .await?;

        Self::recalculate_available_credit_tx(&mut tx, customer_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_credit_account_by_customer_async(customer_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_customer_summary_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerCreditSummary>> {
        let account = self.get_credit_account_by_customer_async(customer_id).await?;

        match account {
            Some(acc) => {
                let holds = self.get_active_holds_async(customer_id).await?;

                Ok(Some(CustomerCreditSummary {
                    customer_id: customer_id.into(),
                    credit_limit: acc.credit_limit,
                    current_balance: acc.current_balance,
                    available_credit: acc.available_credit,
                    oldest_due_date: None,
                    days_past_due: 0,
                    hold_count: holds.len() as i32,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_aging_report_async(&self) -> Result<Vec<(CustomerId, CreditAgingBucket)>> {
        let accounts = self.list_credit_accounts_async(CreditAccountFilter::default()).await?;
        let mut report = Vec::new();

        for acc in accounts {
            report.push((
                acc.customer_id,
                CreditAgingBucket {
                    current: acc.current_balance,
                    days_1_30: Decimal::ZERO,
                    days_31_60: Decimal::ZERO,
                    days_61_90: Decimal::ZERO,
                    days_over_90: Decimal::ZERO,
                    total: acc.current_balance,
                },
            ));
        }

        Ok(report)
    }

    pub async fn get_over_limit_customers_async(&self) -> Result<Vec<CreditAccount>> {
        self.list_credit_accounts_async(CreditAccountFilter {
            over_limit: Some(true),
            ..Default::default()
        })
        .await
    }
}

impl CreditRepository for PgCreditRepository {
    fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount> {
        block_on(self.create_credit_account_async(input))
    }

    fn get_credit_account(&self, id: CreditId) -> Result<Option<CreditAccount>> {
        block_on(self.get_credit_account_async(id.into_uuid()))
    }

    fn get_credit_account_by_customer(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CreditAccount>> {
        block_on(self.get_credit_account_by_customer_async(customer_id.into_uuid()))
    }

    fn update_credit_account(
        &self,
        id: CreditId,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount> {
        block_on(self.update_credit_account_async(id.into_uuid(), input))
    }

    fn list_credit_accounts(&self, filter: CreditAccountFilter) -> Result<Vec<CreditAccount>> {
        block_on(self.list_credit_accounts_async(filter))
    }

    fn adjust_credit_limit(
        &self,
        customer_id: CustomerId,
        new_limit: Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        block_on(self.adjust_credit_limit_async(customer_id.into_uuid(), new_limit, reason))
    }

    fn suspend_credit_account(
        &self,
        customer_id: CustomerId,
        reason: &str,
    ) -> Result<CreditAccount> {
        block_on(self.suspend_credit_account_async(customer_id.into_uuid(), reason))
    }

    fn reactivate_credit_account(&self, customer_id: CustomerId) -> Result<CreditAccount> {
        block_on(self.reactivate_credit_account_async(customer_id.into_uuid()))
    }

    fn check_credit(
        &self,
        customer_id: CustomerId,
        order_amount: Decimal,
    ) -> Result<CreditCheckResult> {
        block_on(self.check_credit_async(customer_id.into_uuid(), order_amount))
    }

    fn reserve_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        block_on(self.reserve_credit_async(customer_id.into_uuid(), order_id.into_uuid(), amount))
    }

    fn release_credit_reservation(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
    ) -> Result<CreditAccount> {
        block_on(
            self.release_credit_reservation_async(customer_id.into_uuid(), order_id.into_uuid()),
        )
    }

    fn charge_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        block_on(self.charge_credit_async(customer_id.into_uuid(), order_id.into_uuid(), amount))
    }

    fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        block_on(self.place_hold_async(input))
    }

    fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>> {
        block_on(self.get_hold_async(id))
    }

    fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        block_on(self.list_holds_async(filter))
    }

    fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        block_on(self.release_hold_async(input))
    }

    fn get_active_holds(&self, customer_id: CustomerId) -> Result<Vec<CreditHold>> {
        block_on(self.get_active_holds_async(customer_id.into_uuid()))
    }

    fn get_holds_for_order(&self, order_id: OrderId) -> Result<Vec<CreditHold>> {
        block_on(self.get_holds_for_order_async(order_id.into_uuid()))
    }

    fn submit_application(&self, input: SubmitCreditApplication) -> Result<CreditApplication> {
        block_on(self.submit_application_async(input))
    }

    fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        block_on(self.get_application_async(id))
    }

    fn list_applications(&self, filter: CreditApplicationFilter) -> Result<Vec<CreditApplication>> {
        block_on(self.list_applications_async(filter))
    }

    fn review_application(&self, input: ReviewCreditApplication) -> Result<CreditApplication> {
        block_on(self.review_application_async(input))
    }

    fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication> {
        block_on(self.withdraw_application_async(id))
    }

    fn record_transaction(&self, input: RecordCreditTransaction) -> Result<CreditTransaction> {
        block_on(self.record_transaction_async(input))
    }

    fn list_transactions(&self, filter: CreditTransactionFilter) -> Result<Vec<CreditTransaction>> {
        block_on(self.list_transactions_async(filter))
    }

    fn apply_payment(
        &self,
        customer_id: CustomerId,
        amount: Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        block_on(self.apply_payment_async(customer_id.into_uuid(), amount, reference_id))
    }

    fn get_customer_summary(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CustomerCreditSummary>> {
        block_on(self.get_customer_summary_async(customer_id.into_uuid()))
    }

    fn get_aging_report(&self) -> Result<Vec<(CustomerId, CreditAgingBucket)>> {
        block_on(self.get_aging_report_async())
    }

    fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>> {
        block_on(self.get_over_limit_customers_async())
    }
}

const fn from_date(date: NaiveDate) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(date.and_time(NaiveTime::MIN), Utc)
}
